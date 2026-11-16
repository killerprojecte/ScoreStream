//! Closed ordered words/metronome/optional-words presentation; numeric tempo is elsewhere.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::{bool_attr, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::{metronome_display::MetronomeDisplay, notation::Placement};
use std::fmt::Write;
pub(super) fn shape(n: &Node) -> bool {
    let types: Vec<_> = n
        .children
        .iter()
        .filter(|d| d.name == "direction-type")
        .collect();
    types.iter().all(|d| d.children.len() == 1)
        && match types.as_slice() {
            [m] => m.child("metronome").is_some(),
            [a, m] => a.child("words").is_some() && m.child("metronome").is_some(),
            [a, m, b] => {
                a.child("words").is_some()
                    && m.child("metronome").is_some()
                    && b.child("words").is_some()
            }
            _ => false,
        }
}
pub(super) fn read(n: &Node, measure: &str) -> Result<Option<MetronomeDisplay>, MappingReport> {
    let types: Vec<_> = n
        .children
        .iter()
        .filter(|d| d.name == "direction-type")
        .collect();
    let Some(m) = types.iter().find_map(|d| d.child("metronome")) else {
        return Ok(None);
    };
    if !shape(n) {
        return Err(unsupported(
            n,
            "metronome_display.group",
            "Only a numeric metronome optionally preceded by one plain words element and optionally followed by one plain words element is represented",
        ));
    }
    if types.len() == 1
        && m.attrs.is_empty()
        && n.attr("placement").is_none()
        && n.attr("system").is_none()
    {
        return Ok(None);
    }
    let placement = match n.attr("placement") {
        None => None,
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => {
            return Err(unsupported(
                n,
                "metronome_display.placement",
                "Expected above or below",
            ));
        }
    };
    let coordinate = |key| {
        m.attr(key)
            .map(|_| exact_decimal(&m.scalar_attribute(key)))
            .transpose()
    };
    Ok(Some(MetronomeDisplay {
        measure: measure.into(),
        placement,
        system: n.attr("system").map(str::to_owned),
        parentheses: m
            .attr("parentheses")
            .map(|_| bool_attr(m, "parentheses"))
            .transpose()?,
        default_x: coordinate("default-x")?,
        default_y: coordinate("default-y")?,
        relative_x: coordinate("relative-x")?,
        relative_y: coordinate("relative-y")?,
        leading_words: if types.len() > 1 {
            types[0]
                .child("words")
                .map(super::words::read_text)
                .transpose()?
        } else {
            None
        },
        trailing_words: if types.len() == 3 {
            types[2]
                .child("words")
                .map(super::words::read_text)
                .transpose()?
        } else {
            None
        },
    }))
}
pub(super) fn start(p: &MetronomeDisplay, out: &mut String) {
    out.push_str("      <direction");
    if let Some(v) = p.placement {
        write!(
            out,
            " placement=\"{}\"",
            match v {
                Placement::Above => "above",
                Placement::Below => "below",
            }
        )
        .unwrap();
    }
    if let Some(v) = &p.system {
        write!(out, " system=\"{}\"", super::xml::escape(v)).unwrap();
    }
    out.push('>');
    if let Some(w) = &p.leading_words {
        super::words::write_text(w, out);
    }
    out.push_str("<direction-type><metronome");
    if let Some(v) = p.parentheses {
        write!(out, " parentheses=\"{}\"", if v { "yes" } else { "no" }).unwrap();
    }
    for (k, v) in [
        ("default-x", &p.default_x),
        ("default-y", &p.default_y),
        ("relative-x", &p.relative_x),
        ("relative-y", &p.relative_y),
    ] {
        if let Some(v) = v {
            write!(
                out,
                " {k}=\"{}\"",
                decimal_text(v).expect("validated metronome geometry")
            )
            .unwrap();
        }
    }
    out.push('>');
}

#[cfg(test)]
mod corpus {
    use super::super::{MusicXml, xml};
    use crate::{MappingReport, ScoreAdapter, sfs::SfsAdapter};
    use scorestream_core::{
        Pitch, Rational,
        notation::{EventId, PartId, VoiceId},
        score::*,
    };
    use std::{collections::BTreeMap, fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned originals"]
    fn original_seven_ordered_metronome_groups_preserve_numeric_clock() {
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
            let mut metronomes = vec![];
            let mut tempos = vec![];
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
                    let mut previous = Rational::zero();
                    let mut extent = Rational::zero();
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
                                let d = &n.value("duration").unwrap().parse::<Rational>().unwrap()
                                    / &divisions[&pid];
                                cursor = if n.name == "backup" {
                                    &cursor - &d
                                } else {
                                    &cursor + &d
                                };
                                extent = extent.max(cursor.clone());
                            }
                            "direction"
                                if n.children.iter().any(|d| d.child("metronome").is_some()) =>
                            {
                                assert!(super::shape(n));
                                assert!(n.child("sound").and_then(|s| s.attr("tempo")).is_some());
                                let mut report = MappingReport::default();
                                xml::schema(n, &mut report);
                                assert!(!report.has_errors(), "{report:?}");
                                super::super::tempo::read(
                                    n,
                                    part,
                                    &mid,
                                    &start,
                                    &cursor,
                                    divisions.get(&pid),
                                    &mut tempos,
                                    &mut metronomes,
                                    &mut report,
                                )?;
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
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                *hash
            );
            assert_eq!(metronomes.len(), [1, 1, 1, 4][i]);
            assert_eq!(tempos.len(), metronomes.len());
            let expected: &[&str] = match i {
                0 => &["216"],
                1 => &["58"],
                2 => &["288"],
                _ => &["168", "288", "168", "224"],
            };
            assert_eq!(
                tempos
                    .iter()
                    .map(|t| t.quarter_bpm.to_string())
                    .collect::<Vec<_>>(),
                expected
            );
            for (j, m) in metronomes.iter().enumerate() {
                let p = m.presentation.as_ref().unwrap();
                assert!(p.leading_words.is_some());
                assert_eq!(p.parentheses, Some(!(i == 3 && j == 3)));
                assert_eq!(p.trailing_words.is_some(), i == 3 && j == 3);
            }
            let wanted: std::collections::BTreeSet<_> =
                metronomes.iter().map(|m| m.part.0.clone()).collect();
            let mut model = ScoreDocument::default();
            model.parts = parts
                .into_values()
                .filter(|p| wanted.contains(&p.id.0))
                .collect();
            for (j, p) in model.parts.iter_mut().enumerate() {
                let voice = VoiceId(format!("v{}_1", j + 1));
                p.voices[0].id = voice.clone();
                for m in &mut p.measures {
                    for e in &mut m.events {
                        e.voice = voice.clone();
                    }
                }
            }
            model.metronomes = metronomes;
            model.tempos = tempos;
            // Explicit family projection retains real positions, complete groups and numeric controls, not original notes.
            let s = SfsAdapter.write(&model).unwrap().value;
            assert_eq!(SfsAdapter.read(&s).unwrap().value, model);
            let out = MusicXml.write(&model).unwrap().value;
            assert_eq!(MusicXml.read(&out).unwrap().value, model);
            let mut plain = model.clone();
            for m in &mut plain.metronomes {
                m.presentation = None;
            }
            let mut a =
                serde_json::to_value(scorestream_app::playback::plan_source(&s).unwrap()).unwrap();
            let mut b = serde_json::to_value(
                scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                    .unwrap(),
            )
            .unwrap();
            for k in ["diagnostics", "warnings"] {
                a.as_object_mut().unwrap().remove(k);
                b.as_object_mut().unwrap().remove(k);
            }
            assert_eq!(a, b);
            println!(
                "movement {}: {} exact ordered metronome groups, explicit quarter BPM {:?}; original hash {}",
                i + 1,
                model.metronomes.len(),
                expected,
                hash
            );
        }
    }
}
