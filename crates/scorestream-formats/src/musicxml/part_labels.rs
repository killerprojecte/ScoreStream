//! Narrow authored label details on score-part declarations, not instrument routing.
use super::{
    read::unsupported,
    xml::{Node, escape},
};
use crate::MappingReport;
use scorestream_core::part_labels::{NameAccidental, PartLabels, PartNameDisplay};
use std::fmt::Write;
pub(super) fn read(n: &Node) -> Result<Option<PartLabels>, MappingReport> {
    let abbreviation = n.child("part-abbreviation").map(|n| n.text.clone());
    let name_display = n
        .child("part-name-display")
        .map(|d| {
            if d.children.len() != 3
                || d.children[0].name != "display-text"
                || d.children[1].name != "accidental-text"
                || d.children[2].name != "display-text"
                || d.children[1].text != "flat"
            {
                return Err(unsupported(
                    d,
                    "part_labels.display_shape",
                    "Only unstyled display-text, flat accidental-text, display-text is represented",
                ));
            }
            Ok(PartNameDisplay {
                leading_text: d.children[0].text.clone(),
                accidental: NameAccidental::Flat,
                trailing_text: d.children[2].text.clone(),
            })
        })
        .transpose()?;
    Ok(if abbreviation.is_some() || name_display.is_some() {
        Some(PartLabels {
            abbreviation,
            name_display,
        })
    } else {
        None
    })
}
pub(super) fn write(v: &PartLabels, out: &mut String) {
    if let Some(d) = &v.name_display {
        write!(out,"<part-name-display><display-text>{}</display-text><accidental-text>{}</accidental-text><display-text>{}</display-text></part-name-display>",escape(&d.leading_text),match d.accidental {NameAccidental::Flat=>"flat"},escape(&d.trailing_text)).unwrap();
    }
    if let Some(a) = &v.abbreviation {
        write!(out, "<part-abbreviation>{}</part-abbreviation>", escape(a)).unwrap();
    }
}

#[cfg(test)]
mod corpus {
    use super::super::{MusicXml, xml};
    use super::*;
    use crate::{ScoreAdapter, sfs::SfsAdapter};
    use scorestream_core::notation::{EventId, PartId, VoiceId};
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned originals"]
    fn original_77_abbreviations_and_four_ordered_name_displays_roundtrip() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let template=MusicXml.read(r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions></attributes><note id="e"><rest/><duration>4</duration><voice>1</voice></note></measure></part></score-partwise>"#).unwrap().value;
        for (i, hash) in hashes.iter().enumerate() {
            let mut model = template.clone();
            model.parts.clear();
            let mut displays = 0;
            let scan = xml::walk(
                BufReader::new(
                    File::open(root.join(format!("Beethoven_Op.67_{}.musicxml", i + 1))).unwrap(),
                ),
                |node, _, phase| {
                    if phase != xml::Phase::Complete || node.name != "part-list" {
                        return Ok(());
                    }
                    for original in node.children_named("score-part") {
                        for name in ["part-abbreviation", "part-name-display"] {
                            if let Some(n) = original.child(name) {
                                let mut report = MappingReport::default();
                                xml::schema(n, &mut report);
                                assert!(!report.has_errors(), "{report:?}");
                            }
                        }
                        let labels = read(original)?.unwrap();
                        assert!(labels.abbreviation.is_some());
                        displays += usize::from(labels.name_display.is_some());
                        if original.attr("id") == Some("P10") && i == 0 {
                            assert_eq!(
                                labels.name_display.as_ref().unwrap().trailing_text,
                                " Horn 2 "
                            );
                        }
                        let index = model.parts.len() + 1;
                        let mut p = template.parts[0].clone();
                        p.id = PartId(original.attr("id").unwrap().into());
                        p.name = Some(original.child("part-name").unwrap().text.clone());
                        p.labels = Some(labels);
                        p.instrument_metadata.as_mut().unwrap().source_id = format!("i{index}");
                        p.voices[0].id = VoiceId(format!("v{index}_1"));
                        p.measures[0].id = format!("m{index}");
                        p.measures[0].events[0].id = EventId(format!("e{index}"));
                        p.measures[0].events[0].voice = p.voices[0].id.clone();
                        model.parts.push(p);
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
                *hash
            );
            assert_eq!(model.parts.len(), [18, 18, 18, 23][i]);
            assert_eq!(displays, if i == 0 { 4 } else { 0 });
            // Named labels-only projection keeps original declared ownership/text, not original notes or routes.
            let s = SfsAdapter.write(&model).unwrap().value;
            assert_eq!(SfsAdapter.read(&s).unwrap().value, model);
            let xml = MusicXml.write(&model).unwrap().value;
            assert_eq!(MusicXml.read(&xml).unwrap().value, model);
            let mut plain = model.clone();
            for p in &mut plain.parts {
                p.labels = None;
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
                "movement {}: {} exact abbreviations, {} ordered displays; original hash {}",
                i + 1,
                model.parts.len(),
                displays,
                hash
            );
        }
    }
}
