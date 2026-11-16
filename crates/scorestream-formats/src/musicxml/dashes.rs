//! Authored dashes endpoint fidelity, independent of sounding hairpin policy.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::error,
    xml::{Node, escape},
};
use crate::MappingReport;
use scorestream_core::{
    Rational,
    dashes::{DashesEndpoint, DashesType},
    notation::Placement,
};
use std::{collections::BTreeMap, fmt::Write};
pub(super) fn read(
    n: &Node,
    part: &str,
    measure: &str,
    cursor: &Rational,
    divisions: Option<&Rational>,
) -> Result<Option<DashesEndpoint>, MappingReport> {
    let Some(w) = n
        .children
        .iter()
        .filter(|d| d.name == "direction-type")
        .find_map(|d| d.child("dashes"))
    else {
        return Ok(None);
    };
    let kind = w
        .attr("type")
        .and_then(DashesType::from_name)
        .ok_or_else(|| {
            error(
                w,
                "dashes.type",
                "Only start and stop endpoints are supported",
            )
        })?;
    let number = w
        .attr("number")
        .map(|s| {
            s.parse::<u8>()
                .ok()
                .filter(|v| (1..=16).contains(v))
                .ok_or_else(|| error(w, "dashes.number", "Expected MusicXML number-level 1..16"))
        })
        .transpose()?;
    let staff = n
        .child("staff")
        .map(|s| {
            s.text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|s| *s > 0)
                .ok_or_else(|| error(n, "dashes.staff", "Expected positive staff"))
        })
        .transpose()?;
    let placement = match n.attr("placement") {
        None => None,
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => return Err(error(n, "dashes.placement", "Expected above or below")),
    };
    let coordinate = |key| {
        w.attr(key)
            .map(|_| exact_decimal(&w.scalar_attribute(key)))
            .transpose()
    };
    let (offset, _) = super::tempo::locations(n, &Rational::zero(), cursor, divisions)?;
    Ok(Some(DashesEndpoint {
        words: n
            .child("direction-type")
            .and_then(|d| d.child("words"))
            .map(|w| {
                let number = |key| {
                    w.attr(key)
                        .map(|_| exact_decimal(&w.scalar_attribute(key)))
                        .transpose()
                };
                Ok::<_, MappingReport>(scorestream_core::dashes::DashesWords {
                    text: w.text.clone(),
                    font_family: w.attr("font-family").map(str::to_owned),
                    font_style: w.attr("font-style").map(str::to_owned),
                    font_size: number("font-size")?,
                    default_y: number("default-y")?,
                    relative_x: number("relative-x")?,
                })
            })
            .transpose()?,
        system: n.attr("system").map(str::to_owned),
        part: part.into(),
        measure: measure.into(),
        offset,
        staff,
        placement,
        kind,
        number,
        default_y: coordinate("default-y")?,
        relative_x: coordinate("relative-x")?,
    }))
}
pub(super) struct Export<'a> {
    by_measure: BTreeMap<(&'a str, &'a str), Vec<&'a DashesEndpoint>>,
}
impl<'a> Export<'a> {
    pub fn new(values: &'a [DashesEndpoint]) -> Self {
        let mut by_measure: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for e in values {
            by_measure
                .entry((e.part.as_str(), e.measure.as_str()))
                .or_default()
                .push(e);
        }
        Self { by_measure }
    }
    pub fn offsets(&self, part: &str, measure: &str) -> Vec<Rational> {
        self.by_measure
            .get(&(part, measure))
            .into_iter()
            .flatten()
            .map(|e| e.offset.clone())
            .collect()
    }
    pub fn write(&self, part: &str, measure: &str, div: u64, out: &mut String) {
        for e in self.by_measure.get(&(part, measure)).into_iter().flatten() {
            let placement = e
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
            write!(out, "      <direction{placement}").unwrap();
            if let Some(system) = &e.system {
                write!(out, " system=\"{}\"", escape(system)).unwrap();
            }
            out.push('>');
            if let Some(w) = &e.words {
                out.push_str("<direction-type><words");
                for (key, value) in [
                    ("font-family", w.font_family.as_deref()),
                    ("font-style", w.font_style.as_deref()),
                ] {
                    if let Some(value) = value {
                        write!(out, " {key}=\"{}\"", escape(value)).unwrap();
                    }
                }
                for (key, value) in [
                    ("font-size", &w.font_size),
                    ("default-y", &w.default_y),
                    ("relative-x", &w.relative_x),
                ] {
                    if let Some(value) = value {
                        write!(
                            out,
                            " {key}=\"{}\"",
                            decimal_text(value).expect("validated words decimal")
                        )
                        .unwrap();
                    }
                }
                write!(out, ">{}</words></direction-type>", escape(&w.text)).unwrap();
            }
            write!(out, "<direction-type><dashes type=\"{}\"", e.kind.name()).unwrap();
            if let Some(n) = e.number {
                write!(out, " number=\"{n}\"").unwrap();
            }
            for (key, v) in [("default-y", &e.default_y), ("relative-x", &e.relative_x)] {
                if let Some(v) = v {
                    write!(
                        out,
                        " {key}=\"{}\"",
                        escape(&decimal_text(v).expect("validated dashes decimal"))
                    )
                    .unwrap();
                }
            }
            write!(
                out,
                "/></direction-type><offset sound=\"no\">{}</offset>",
                super::write::units(&e.offset, div)
            )
            .unwrap();
            if let Some(s) = e.staff {
                write!(out, "<staff>{s}</staff>").unwrap();
            }
            out.push_str("</direction>\n");
        }
    }
}

#[cfg(test)]
mod corpus {
    use super::super::{MusicXml, xml};
    use super::*;
    use crate::{ScoreAdapter, sfs::SfsAdapter};
    use scorestream_core::{
        Pitch,
        notation::{EventId, PartId, VoiceId},
        score::*,
    };
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned original XML/MXL directory"]
    fn original_470_endpoints_preserve_positions_and_unresolved_source_chains() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let template=MusicXml.read(r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions></attributes><note id="e"><rest/><duration>4</duration><voice>1</voice></note></measure></part></score-partwise>"#).unwrap().value;
        for (i, hash) in hashes.iter().enumerate() {
            let mut parts = BTreeMap::<String, ScorePart>::new();
            let mut divisions = BTreeMap::<String, Rational>::new();
            let mut endpoints = vec![];
            let scan = xml::walk(
                BufReader::new(
                    File::open(root.join(format!("Beethoven_Op.67_{}.musicxml", i + 1))).unwrap(),
                ),
                |m, parent, phase| {
                    if phase != xml::Phase::Complete || m.name != "measure" {
                        return Ok(());
                    }
                    let pid = parent.unwrap().attr("id").unwrap().to_owned();
                    let part = parts.entry(pid.clone()).or_insert_with(|| {
                        let mut p = template.parts[0].clone();
                        p.id = PartId(pid.clone());
                        p.voices[0].id = VoiceId(format!("v{pid}"));
                        p.instrument_metadata.as_mut().unwrap().source_id = format!("i{pid}");
                        p.measures.clear();
                        p
                    });
                    let ordinal = part.measures.len() + 1;
                    let mid = format!("m{pid}_{ordinal}");
                    let start = part
                        .measures
                        .last()
                        .map(|m| &m.start + &m.length)
                        .unwrap_or_default();
                    let mut cursor = Rational::zero();
                    let mut extent = Rational::zero();
                    let mut previous = Rational::zero();
                    for n in &m.children {
                        match n.name.as_str() {
                            "attributes" => {
                                if let Some(d) = n.value("divisions") {
                                    divisions.insert(pid.clone(), d.parse().unwrap());
                                }
                            }
                            "note" if n.child("grace").is_none() => {
                                let duration =
                                    &n.value("duration").unwrap().parse::<Rational>().unwrap()
                                        / &divisions[&pid];
                                if n.child("chord").is_none() {
                                    previous = cursor.clone();
                                    cursor = &cursor + &duration;
                                }
                                extent = extent.max(&previous + &duration).max(cursor.clone());
                            }
                            "backup" | "forward" => {
                                let duration =
                                    &n.value("duration").unwrap().parse::<Rational>().unwrap()
                                        / &divisions[&pid];
                                cursor = if n.name == "backup" {
                                    &cursor - &duration
                                } else {
                                    &cursor + &duration
                                };
                                extent = extent.max(cursor.clone());
                            }
                            "direction"
                                if n.children
                                    .iter()
                                    .filter(|d| d.name == "direction-type")
                                    .find_map(|d| d.child("dashes"))
                                    .is_some() =>
                            {
                                let mut report = MappingReport::default();
                                xml::schema(n, &mut report);
                                assert!(!report.has_errors(), "{report:?}");
                                assert!(
                                    n.child("offset").is_none()
                                        && n.child("staff").is_none()
                                        && n.child("sound").is_none()
                                );
                                endpoints.push(
                                    read(n, &pid, &mid, &cursor, divisions.get(&pid))?.unwrap(),
                                );
                            }
                            _ => {}
                        }
                    }
                    let mut measure = template.parts[0].measures[0].clone();
                    measure.id = mid;
                    measure.label = m.attr("number").unwrap().into();
                    measure.start = start;
                    measure.length = extent.clone();
                    measure.states.clear();
                    measure.events[0].id = EventId(format!("e{pid}_{ordinal}"));
                    measure.events[0].voice = part.voices[0].id.clone();
                    measure.events[0].duration = extent;
                    measure.events[0].pitch = Pitch::Rest;
                    part.measures.push(measure);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                &scan
                    .fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hash
            );
            assert_eq!(endpoints.len(), [226, 10, 92, 142][i]);
            if endpoints.is_empty() {
                println!("movement {}: 0 endpoints; original hash {}", i + 1, hash);
                continue;
            }
            let wanted: std::collections::BTreeSet<_> =
                endpoints.iter().map(|e| e.part.clone()).collect();
            let mut model = ScoreDocument::default();
            model.parts = parts
                .into_values()
                .filter(|p| wanted.contains(&p.id.0))
                .collect();
            for (index, part) in model.parts.iter_mut().enumerate() {
                let voice = VoiceId(format!("v{}_1", index + 1));
                part.voices[0].id = voice.clone();
                for measure in &mut part.measures {
                    for event in &mut measure.events {
                        event.voice = voice.clone();
                    }
                }
            }
            let positions: BTreeMap<_, _> = model
                .parts
                .iter()
                .flat_map(|p| {
                    p.measures
                        .iter()
                        .map(move |m| ((p.id.0.as_str(), m.id.as_str()), &m.start))
                })
                .collect();
            endpoints.sort_by(|a, b| {
                a.part.cmp(&b.part).then(
                    positions[&(a.part.as_str(), a.measure.as_str())]
                        .cmp(positions[&(b.part.as_str(), b.measure.as_str())]),
                )
            });
            model.dashes = endpoints;
            assert_eq!(
                scorestream_core::dashes::unresolved_pairs(&model.dashes),
                [66, 0, 24, 18][i]
            );
            // Named family projection retains real measure extents/endpoint offsets, not original notes or import claims.
            let sfs = SfsAdapter.write(&model).unwrap().value;
            let restored = SfsAdapter.read(&sfs).unwrap().value;
            assert_eq!(restored, model);
            let exported = MusicXml.write(&restored).unwrap().value;
            assert_eq!(MusicXml.read(&exported).unwrap().value, model);
            let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
            assert!(
                plan.diagnostics
                    .iter()
                    .any(|d| d.code == "dashes.authored_only")
            );
            if i != 1 {
                assert!(
                    plan.diagnostics
                        .iter()
                        .any(|d| d.code == "dashes.pairing_unresolved")
                );
            }
            println!(
                "movement {}: {} exact endpoints; {} unresolved source-chain incidences retained; original hash {}",
                i + 1,
                model.dashes.len(),
                scorestream_core::dashes::unresolved_pairs(&model.dashes),
                hash
            );
        }
    }
}
