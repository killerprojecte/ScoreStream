//! Boundary statements retain their authored wrapper; this is not an execution policy.
use super::{
    read::error,
    xml::{escape, Node},
};
use crate::MappingReport;
use scorestream_core::{authored_dynamics::AuthoredDynamic, score::*, Rational};
use std::fmt::Write;
pub(super) fn capture(
    n: &Node,
    part: &ScorePart,
    measure: &str,
    order: u32,
    cursor: &Rational,
    divisions: Option<&Rational>,
    marks: &[ScoreDynamic],
    sounds: &[ScoreSoundDynamics],
) -> Result<Option<AuthoredDynamic>, MappingReport> {
    if n.name != "direction"
        || marks.len() != 1
        || sounds.len() != 1
        || marks[0].qualifier.is_some()
    {
        return Ok(None);
    }
    let sound = n.child("sound").unwrap();
    if sound.attrs.keys().any(|k| k != "dynamics") {
        return Ok(None);
    }
    let offset = |o: &Node| -> Result<Rational, MappingReport> {
        let q = super::geometry::exact_decimal(o)?;
        if q == Rational::zero() {
            Ok(q)
        } else {
            Ok(&q
                / divisions
                    .ok_or_else(|| error(o, "divisions.missing", "Exact offset needs divisions"))?)
        }
    };
    let o = n.child("offset");
    let m = &marks[0];
    let s = &sounds[0];
    Ok(Some(AuthoredDynamic {
        part: part.id.0.clone(),
        measure: measure.into(),
        order,
        cursor: cursor.clone(),
        visual_at: m.at.clone(),
        sounding_at: s.at.clone(),
        sign: crate::dynamics::symbol_text(m.sign).into(),
        percent: s.percent.clone(),
        staff: m.scope.staff,
        placement: m.placement.map(|p| {
            match p {
                scorestream_core::notation::Placement::Above => "above",
                scorestream_core::notation::Placement::Below => "below",
            }
            .into()
        }),
        offset: o.map(offset).transpose()?,
        offset_sound: o.and_then(|o| o.attr("sound")).map(|s| s == "yes"),
        sound_offset: sound.child("offset").map(offset).transpose()?,
        default_x: m.default_x.clone(),
        default_y: m.default_y.clone(),
        relative_x: m.relative_x.clone(),
        relative_y: m.relative_y.clone(),
    }))
}
pub(super) fn select(
    part: &ScorePart,
    candidates: &[AuthoredDynamic],
    marks: &mut Vec<ScoreDynamic>,
    sounds: &mut Vec<ScoreSoundDynamics>,
) -> Vec<AuthoredDynamic> {
    let endpoints: Vec<_> = candidates
        .iter()
        .filter(|e| {
            part.measures
                .iter()
                .any(|m| m.id == e.measure && e.sounding_at == &m.start + &m.length)
        })
        .collect();
    let owners: std::collections::BTreeSet<_> = candidates
        .iter()
        .filter(|e| {
            endpoints
                .iter()
                .any(|end| end.sounding_at == e.sounding_at && end.staff == e.staff)
        })
        .map(|e| e.measure.as_str())
        .collect();
    let mut out = Vec::new();
    for e in candidates {
        if !owners.contains(e.measure.as_str()) {
            continue;
        }
        if let Some(i) = marks.iter().position(|m| {
            m.at == e.visual_at
                && m.scope.staff == e.staff
                && crate::dynamics::symbol_text(m.sign) == e.sign
                && m.default_x == e.default_x
                && m.default_y == e.default_y
                && m.relative_x == e.relative_x
                && m.relative_y == e.relative_y
        }) {
            marks.remove(i);
        } else {
            continue;
        }
        let i = sounds
            .iter()
            .position(|s| {
                s.at == e.sounding_at && s.scope.staff == e.staff && s.percent == e.percent
            })
            .expect("captured numeric statement");
        sounds.remove(i);
        out.push(e.clone());
    }
    out
}
pub(super) fn offsets(values: &[AuthoredDynamic], part: &str, measure: &str) -> Vec<Rational> {
    values
        .iter()
        .filter(|e| e.part == part && e.measure == measure)
        .flat_map(|e| {
            std::iter::once(e.cursor.clone())
                .chain(e.offset.clone())
                .chain(e.sound_offset.clone())
        })
        .collect()
}
pub(super) fn write(
    values: &[AuthoredDynamic],
    part: &str,
    measure: &str,
    div: u64,
    out: &mut String,
) {
    for e in values
        .iter()
        .filter(|e| e.part == part && e.measure == measure)
    {
        let units = |q: &Rational| super::write::units(q, div);
        if e.cursor != Rational::zero() {
            write!(
                out,
                "<forward><duration>{}</duration></forward>",
                units(&e.cursor)
            )
            .unwrap();
        }
        out.push_str("<direction");
        if let Some(p) = &e.placement {
            write!(out, " placement=\"{}\"", escape(p)).unwrap();
        }
        out.push_str("><direction-type><dynamics");
        for (name, q) in [
            ("default-x", &e.default_x),
            ("default-y", &e.default_y),
            ("relative-x", &e.relative_x),
            ("relative-y", &e.relative_y),
        ] {
            if let Some(q) = q {
                write!(
                    out,
                    " {name}=\"{}\"",
                    super::geometry::decimal_text(q).unwrap()
                )
                .unwrap();
            }
        }
        write!(out, "><{}/></dynamics></direction-type>", e.sign).unwrap();
        if let Some(q) = &e.offset {
            out.push_str("<offset");
            if let Some(b) = e.offset_sound {
                write!(out, " sound=\"{}\"", if b { "yes" } else { "no" }).unwrap();
            }
            write!(out, ">{}</offset>", units(q)).unwrap();
        }
        if let Some(staff) = e.staff {
            write!(out, "<staff>{staff}</staff>").unwrap();
        }
        write!(
            out,
            "<sound dynamics=\"{}\"",
            super::geometry::decimal_text(&e.percent).unwrap()
        )
        .unwrap();
        if let Some(q) = &e.sound_offset {
            write!(out, "><offset>{}</offset></sound>", units(q)).unwrap();
        } else {
            out.push_str("/>");
        }
        out.push_str("</direction>");
        if e.cursor != Rational::zero() {
            write!(
                out,
                "<backup><duration>{}</duration></backup>",
                units(&e.cursor)
            )
            .unwrap();
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod corpus {
    use super::*;
    use crate::{
        musicxml::{xml, MusicXml},
        sfs::SfsAdapter,
        ScoreAdapter,
    };
    use std::{collections::BTreeMap, fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned originals"]
    fn original_two_boundary_pairs_preserve_linked_authored_statements() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let source = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m1" number="217"><attributes><divisions>2</divisions><time><beats>3</beats><beat-type>8</beat-type></time></attributes><note id="a"><rest/><duration>3</duration><voice>1</voice></note></measure><measure id="m2" number="218"><note id="b"><rest/><duration>3</duration><voice>1</voice></note></measure></part></score-partwise>"#;
        let template = MusicXml.read(source).unwrap().value;
        let mut divisions = BTreeMap::<String, Rational>::new();
        let mut captured = BTreeMap::<String, Vec<AuthoredDynamic>>::new();
        let scan = xml::walk(
            BufReader::new(File::open(root.join("Beethoven_Op.67_2.musicxml")).unwrap()),
            |m, parent, phase| {
                if phase != xml::Phase::Complete || m.name != "measure" {
                    return Ok(());
                }
                let pid = parent.unwrap().attr("id").unwrap();
                if !matches!(pid, "P6" | "P8") {
                    return Ok(());
                }
                let mut cursor = Rational::zero();
                for n in &m.children {
                    match n.name.as_str() {
                        "attributes" => {
                            if let Some(d) = n.value("divisions") {
                                divisions.insert(pid.into(), d.parse().unwrap());
                            }
                        }
                        "note" if n.child("grace").is_none() && n.child("chord").is_none() => {
                            cursor = &cursor
                                + &(&n.value("duration").unwrap().parse::<Rational>().unwrap()
                                    / &divisions[pid])
                        }
                        "backup" | "forward" => {
                            let d = &n.value("duration").unwrap().parse::<Rational>().unwrap()
                                / &divisions[pid];
                            cursor = if n.name == "backup" {
                                &cursor - &d
                            } else {
                                &cursor + &d
                            };
                        }
                        "direction"
                            if matches!(m.attr("number"), Some("217" | "218"))
                                && n.child("direction-type")
                                    .and_then(|d| d.child("dynamics"))
                                    .is_some() =>
                        {
                            assert_eq!(divisions[pid].to_string(), "24");
                            let (start, mid) = if m.attr("number") == Some("217") {
                                assert_eq!(cursor.to_string(), "3/2");
                                (Rational::zero(), "m1")
                            } else {
                                assert_eq!(cursor, Rational::zero());
                                ("3/2".parse().unwrap(), "m2")
                            };
                            let mut marks = vec![];
                            let mut sounds = vec![];
                            super::super::dynamics::read(
                                n,
                                &template.parts[0],
                                &start,
                                &cursor,
                                divisions.get(pid),
                                &mut marks,
                                &mut sounds,
                            )?;
                            captured.entry(pid.into()).or_default().push(
                                capture(
                                    n,
                                    &template.parts[0],
                                    mid,
                                    0,
                                    &cursor,
                                    divisions.get(pid),
                                    &marks,
                                    &sounds,
                                )?
                                .unwrap(),
                            );
                        }
                        _ => {}
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            scan.fingerprint
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a"
        );
        assert_eq!(captured.len(), 2);
        for (pid, entries) in captured {
            assert_eq!(entries.len(), 2, "{pid}");
            assert_eq!(entries[0].visual_at.to_string(), "2/3");
            assert_eq!(entries[0].sounding_at.to_string(), "3/2");
            assert_eq!(entries[1].sounding_at, entries[0].sounding_at);
            assert_eq!(entries[0].percent.to_string(), "1111/25");
            assert_eq!(entries[1].percent.to_string(), "10667/100");
            let mut model = template.clone();
            model.authored_dynamics = entries;
            let sfs = SfsAdapter.write(&model).unwrap().value;
            assert_eq!(SfsAdapter.read(&sfs).unwrap().value, model);
            let xml = MusicXml.write(&model).unwrap().value;
            assert_eq!(MusicXml.read(&xml).unwrap().value, model);
            assert!(scorestream_app::playback::plan_source(&sfs)
                .unwrap_err()
                .contains("authored_dynamics.performance_required"));
        }
    }
}
