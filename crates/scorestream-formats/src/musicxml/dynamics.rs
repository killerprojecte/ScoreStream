//! Numeric MusicXML sound and printed dynamics: no label, text or glyph inference.
use super::{
    read::{decimal, error, unsupported},
    tempo::locations,
    xml::{Node, escape},
};
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Rational,
    notation::{PartId, Placement},
    score::*,
};
use std::{collections::BTreeMap, fmt::Write};
type Result<T> = std::result::Result<T, MappingReport>;
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    node: &Node,
    part: &ScorePart,
    start: &Rational,
    cursor: &Rational,
    divisions: Option<&Rational>,
    marks: &mut Vec<ScoreDynamic>,
    sounds: &mut Vec<ScoreSoundDynamics>,
) -> Result<()> {
    let (visual, sounding) = locations(node, start, cursor, divisions)?;
    let staff = node
        .child("staff")
        .map(|n| {
            n.text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|s| *s > 0 && *s <= part.staves)
                .ok_or_else(|| error(n, "staff.number", "Direction staff is outside its part"))
        })
        .transpose()?;
    let scope = ScorePartScope {
        part: part.id.clone(),
        staff,
    };
    if let Some(d) = node
        .child("direction-type")
        .and_then(|t| t.child("dynamics"))
    {
        let symbol = d
            .children
            .iter()
            .find(|c| crate::dynamics::symbol(&c.name).is_some())
            .ok_or_else(|| {
                unsupported(
                    d,
                    "dynamics.empty",
                    "An explicit dynamic symbol is required",
                )
            })?;
        let sign = crate::dynamics::symbol(&symbol.name).ok_or_else(|| {
            unsupported(
                symbol,
                "dynamics.symbol",
                "Unmapped dynamic symbol; spelling is not substituted",
            )
        })?;
        let placement = match node.attr("placement") {
            None => None,
            Some("above") => Some(Placement::Above),
            Some("below") => Some(Placement::Below),
            _ => return Err(error(node, "dynamics.placement", "Expected above or below")),
        };
        let coordinate = |key| {
            d.attr(key)
                .map(|_| super::geometry::exact_decimal(&d.scalar_attribute(key)))
                .transpose()
        };
        let qualifier = d
            .children
            .iter()
            .position(|c| c.name == "other-dynamics")
            .map(|index| {
                let text = d.children[index].text.clone();
                if text.trim().is_empty() {
                    return Err(error(
                        d,
                        "dynamics.qualifier",
                        "Dynamic qualifier text must not be empty or whitespace-only",
                    ));
                }
                Ok(DynamicQualifier {
                    text,
                    position: if index == 0 {
                        DynamicQualifierPosition::Before
                    } else {
                        DynamicQualifierPosition::After
                    },
                })
            })
            .transpose()?;
        marks.push(ScoreDynamic {
            qualifier,
            default_x: coordinate("default-x")?,
            default_y: coordinate("default-y")?,
            relative_x: coordinate("relative-x")?,
            relative_y: coordinate("relative-y")?,
            at: visual,
            scope: scope.clone(),
            sign,
            placement,
            sound: false,
        });
    }
    let sound = if node.name == "sound" {
        Some(node)
    } else {
        node.child("sound")
    };
    if let Some(sound) = sound
        && let Some(raw) = sound.attr("dynamics")
    {
        let mut number = sound.clone();
        number.text = raw.into();
        let percent = decimal(&number)?;
        if percent.is_negative() {
            return Err(error(
                sound,
                "sound_dynamics.percent",
                "MusicXML numeric dynamics must be nonnegative",
            ));
        }
        sounds.push(ScoreSoundDynamics {
            at: sounding,
            scope,
            percent,
        });
    }
    Ok(())
}
/// Preserve absence rather than inventing an authored assignment. A caller can
/// select a renderer policy separately; this adapter does not change old SFS80.
pub(super) fn report_unassigned_initial(score: &ScoreDocument, r: &mut MappingReport) {
    if score.sound_dynamics.is_empty() && score.dynamics.is_empty() {
        return;
    }
    for part in &score.parts {
        if !(1..=part.staves).all(|staff| {
            score.sound_dynamics.iter().any(|d| {
                d.scope.part == part.id
                    && d.at == Rational::zero()
                    && (d.scope.staff.is_none() || d.scope.staff == Some(staff))
            })
        }) {
            r.push(Format::MusicXml, MappingEffect::Normalized,
                "sound_dynamics.initial_unassigned", &part.id.0,
                "No numeric initial sound is invented for an unassigned staff. The MusicXML MIDI tutorial suggests default velocity90; the existing SFS renderer retains80 until an explicit control applies. Absence is preserved, not playback-default equivalence.");
        }
    }
}
#[derive(Default)]
struct Points<'a> {
    marks: Vec<&'a ScoreDynamic>,
    sounds: Vec<&'a ScoreSoundDynamics>,
}
pub(super) struct Export<'a> {
    by_part: BTreeMap<&'a PartId, BTreeMap<Rational, Points<'a>>>,
}
impl<'a> Export<'a> {
    pub(super) fn new(score: &'a ScoreDocument, r: &mut MappingReport) -> Self {
        report_unassigned_initial(score, r);
        let mut by_part: BTreeMap<_, BTreeMap<Rational, Points<'a>>> = BTreeMap::new();
        for m in &score.dynamics {
            by_part
                .entry(&m.scope.part)
                .or_default()
                .entry(m.at.clone())
                .or_default()
                .marks
                .push(m);
            if m.sound {
                r.push(Format::MusicXml,MappingEffect::Unsupported,"dynamics.symbolic_sound",m.at.to_string(),"A performer-policy symbolic dynamic is not equivalent to explicit MusicXML numeric sound; export requires a display-only mark");
            }
        }
        for d in &score.sound_dynamics {
            by_part
                .entry(&d.scope.part)
                .or_default()
                .entry(d.at.clone())
                .or_default()
                .sounds
                .push(d);
            if super::write::decimal(&d.percent).is_none_or(|v| v.len() > 64) {
                r.push(Format::MusicXml,MappingEffect::Unsupported,"sound_dynamics.decimal",d.at.to_string(),"Exact percentage must have a finite decimal within the supported precision; it is not rounded");
            }
        }
        for points in by_part.values().flat_map(|p| p.values()) {
            for d in &points.sounds {
                if d.scope.staff.is_some() && !points.marks.iter().any(|m| m.scope == d.scope) {
                    r.push(Format::MusicXml,MappingEffect::Unsupported,"sound_dynamics.staff_export",d.at.to_string(),"Staff-scoped numeric export needs a display dynamic at exactly the same position; no invisible or fake direction is invented");
                }
            }
        }
        Self { by_part }
    }
    pub(super) fn offsets(&self, part: &ScorePart, m: &ScoreMeasure) -> Vec<Rational> {
        self.by_part
            .get(&part.id)
            .into_iter()
            .flat_map(|p| {
                p.range(m.start.clone()..&m.start + &m.length)
                    .map(|(at, _)| at - &m.start)
            })
            .collect()
    }
    pub(super) fn write_measure(
        &self,
        part: &ScorePart,
        m: &ScoreMeasure,
        div: u64,
        out: &mut String,
    ) {
        let Some(points) = self.by_part.get(&part.id) else {
            return;
        };
        for (at, values) in points.range(m.start.clone()..&m.start + &m.length) {
            let offset = (&(at - &m.start) * &Rational::from_big_integer(div.into())).to_string();
            let mut staff_written = std::collections::BTreeSet::new();
            for mark in &values.marks {
                let placement = mark
                    .placement
                    .map(|p| {
                        format!(
                            " placement=\"{}\"",
                            match p {
                                Placement::Above => "above",
                                Placement::Below => "below",
                            }
                        )
                    })
                    .unwrap_or_default();
                let symbol = crate::dynamics::symbol_text(mark.sign);
                let mut geometry = String::new();
                for (key, value) in [
                    ("default-x", &mark.default_x),
                    ("default-y", &mark.default_y),
                    ("relative-x", &mark.relative_x),
                    ("relative-y", &mark.relative_y),
                ] {
                    if let Some(value) = value {
                        write!(
                            geometry,
                            " {key}=\"{}\"",
                            super::geometry::decimal_text(value)
                                .expect("validated dynamic decimal")
                        )
                        .unwrap();
                    }
                }
                write!(
                    out,
                    "      <direction{placement}><direction-type><dynamics{geometry}>"
                )
                .unwrap();
                let qualifier = mark
                    .qualifier
                    .as_ref()
                    .map(|q| format!("<other-dynamics>{}</other-dynamics>", escape(&q.text)))
                    .unwrap_or_default();
                if mark
                    .qualifier
                    .as_ref()
                    .is_some_and(|q| q.position == DynamicQualifierPosition::Before)
                {
                    out.push_str(&qualifier);
                }
                write!(out, "<{symbol}/>").unwrap();
                if mark
                    .qualifier
                    .as_ref()
                    .is_some_and(|q| q.position == DynamicQualifierPosition::After)
                {
                    out.push_str(&qualifier);
                }
                write!(
                    out,
                    "</dynamics></direction-type><offset sound=\"no\">{offset}</offset>"
                )
                .unwrap();
                if let Some(staff) = mark.scope.staff {
                    write!(out, "<staff>{staff}</staff>").unwrap();
                    if staff_written.insert(staff)
                        && let Some(d) = values.sounds.iter().find(|d| d.scope == mark.scope)
                    {
                        write!(
                            out,
                            "<sound dynamics=\"{}\"><offset>{offset}</offset></sound>",
                            escape(&super::write::decimal(&d.percent).unwrap())
                        )
                        .unwrap();
                    }
                }
                out.push_str("</direction>\n");
            }
            for sound in values.sounds.iter().filter(|d| d.scope.staff.is_none()) {
                writeln!(
                    out,
                    "      <sound dynamics=\"{}\"><offset>{offset}</offset></sound>",
                    super::write::decimal(&sound.percent).unwrap()
                )
                .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod geometry_corpus {
    use super::*;
    use crate::{
        ScoreAdapter,
        musicxml::{MusicXml, xml},
        sfs::SfsAdapter,
    };
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_dynamics_geometry_and_qualifiers_roundtrip() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        for (i, (total, blocked)) in [(755, 0), (757, 55), (522, 62), (1221, 157)]
            .into_iter()
            .enumerate()
        {
            let path = root.join(format!("Beethoven_Op.67_{}.musicxml", i + 1));
            let mut counts = (0, 0);
            let scan=xml::walk(BufReader::new(File::open(&path).unwrap()), |m,_,phase| {
                if phase != xml::Phase::Complete || m.name != "measure" { return Ok(()) }
                for direction in m.children_named("direction") {
                    for t in direction.children_named("direction-type") {
                        for d in t.children_named("dynamics") {
                            counts.0+=1;
                            let mut report=MappingReport::default(); xml::schema(d,&mut report);
                            if d.child("other-dynamics").is_some() { counts.1 += 1; }
                            assert!(!report.has_errors(),"{report:?}");
                            let mut geometry=String::new();
                            for (key,value) in &d.attrs { write!(geometry," {key}=\"{}\"",escape(value)).unwrap(); }
                            let mut content=String::new();
                            for child in &d.children {
                                if child.name == "other-dynamics" { write!(content,"<other-dynamics>{}</other-dynamics>",escape(&child.text)).unwrap(); }
                                else { assert!(crate::dynamics::symbol(&child.name).is_some()); write!(content,"<{}/>",child.name).unwrap(); }
                            }
                            // Family projection onto a named minimal fixture, not a stripped original.
                            let snippet=format!(r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><direction-type><dynamics{geometry}>{content}</dynamics></direction-type></direction><note id="n"><rest/><duration>4</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#);
                            let model=MusicXml.read(&snippet).unwrap().value;
                            let mark=&model.dynamics[0];
                            if let Some(index)=d.children.iter().position(|c|c.name=="other-dynamics") {
                                let q=mark.qualifier.as_ref().unwrap();
                                assert_eq!(q.text,d.children[index].text);
                                assert_eq!(q.position,if index==0 {DynamicQualifierPosition::Before} else {DynamicQualifierPosition::After});
                            } else { assert!(mark.qualifier.is_none()); }
                            for (key,value) in [("default-x",&mark.default_x),("default-y",&mark.default_y),("relative-x",&mark.relative_x),("relative-y",&mark.relative_y)] {
                                let exact=d.attr(key).map(|_| super::super::geometry::exact_decimal(&d.scalar_attribute(key)).unwrap());
                                assert_eq!(*value,exact);
                            }
                            let sfs=SfsAdapter.write(&model).unwrap().value;
                            if mark.qualifier.is_some() { assert!(scorestream_app::playback::plan_source(&sfs).unwrap().diagnostics.iter().any(|d|d.code=="dynamic_qualifier_not_realized")); }
                            let restored=SfsAdapter.read(&sfs).unwrap().value;
                            assert_eq!(restored,model);
                            assert_eq!(MusicXml.read(&MusicXml.write(&restored).unwrap().value).unwrap().value,model);
                        }
                    }
                }
                Ok(())
            }).unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[i]
            );
            assert_eq!(counts, (total, blocked));
            println!(
                "movement {}: {total} unchanged original dynamic nodes, {} exact typed/SFS/XML roundtrips, {blocked} ordered plain-text qualifiers",
                i + 1,
                total
            );
        }
    }
}
