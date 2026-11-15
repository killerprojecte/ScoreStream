//! The authored v1 bridge preserves scalar types, hierarchy and event order.
use scorestream_core::{notation::EventId, score::*};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};

#[test]
fn authored_scalar_sfs_json_and_codec_roundtrip() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note id="a"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><note id="b"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure></part></score-partwise>"#;
    let mut score = MusicXml.read(xml).unwrap().value;
    let event = |id: &str| ArticulationEvent {
        event: EventId(id.into()),
        notations: vec![
            ArticulationNotations::default(),
            ArticulationNotations {
                source_id: Some(format!("wrapper_{id}")),
                print_object: Some(false),
                groups: vec![
                    ArticulationGroup::default(),
                    ArticulationGroup {
                        source_id: Some(format!("group_{id}")),
                        items: vec![AuthoredArticulation {
                            sign: ArticulationSign::OtherArticulation {
                                text: "passive <literal> & text\nsecond line".into(),
                                smufl: Some("articAccentAbove".into()),
                            },
                            placement: None,
                            print_style: ArticulationPrintStyle {
                                default_x: Some("-7/4".parse().unwrap()),
                                font_size: Some(ArticulationFontSize::Css(
                                    ArticulationCssFontSize::Small,
                                )),
                                ..Default::default()
                            },
                        }],
                    },
                ],
            },
        ],
    };
    score.authored_articulations = Some(AuthoredArticulations {
        version: ArticulationVersion::V1,
        events: vec![event("b"), event("a")],
    });
    let sfs = SfsAdapter.write(&score).unwrap().value;
    assert!(sfs.contains("scorestream.articulation@1"));
    let parsed = scorestream_codec::parse(&sfs).unwrap();
    let json = serde_json::to_string(&parsed).unwrap();
    let decoded: scorestream_core::Document = serde_json::from_str(&json).unwrap();
    let canonical = scorestream_codec::canonical(&decoded);
    assert_eq!(SfsAdapter.read(&canonical).unwrap().value, score);
    assert_reserved_sources_survive_normalization(score);
}

fn assert_reserved_sources_survive_normalization(mut score: ScoreDocument) {
    use scorestream_core::notation::{PartId, VoiceId};
    let reserved = [
        "part1", "voice1", "measure1", "event1", "chord1", "tie1", "slur1",
    ];
    let part = &mut score.parts[0];
    part.id = PartId("foreign part".into());
    part.voices[0].id = VoiceId("foreign voice".into());
    part.measures[0].id = "foreign measure".into();
    for event in &mut part.measures[0].events {
        event.id.0 = format!("foreign {}", event.id.0);
        event.voice = VoiceId("foreign voice".into());
    }
    let authored = score.authored_articulations.as_mut().unwrap();
    for event in &mut authored.events {
        event.event.0 = format!("foreign {}", event.event.0);
        for wrapper in &mut event.notations {
            wrapper.source_id = None;
            for group in &mut wrapper.groups {
                group.source_id = None;
            }
        }
    }
    authored.events[0].notations[0].source_id = Some(reserved[0].into());
    authored.events[0].notations[1]
        .groups
        .extend(reserved[1..].iter().map(|id| ArticulationGroup {
            source_id: Some((*id).into()),
            items: vec![],
        }));
    let written = SfsAdapter.write(&score).unwrap();
    let mapped = SfsAdapter.read(&written.value).unwrap().value;
    assert_eq!(mapped.parts[0].id.0, "part2");
    assert_eq!(mapped.parts[0].voices[0].id.0, "voice2");
    assert_eq!(mapped.parts[0].measures[0].id, "measure2");
    assert_eq!(mapped.parts[0].measures[0].events[0].id.0, "event2");
    assert_eq!(mapped.parts[0].measures[0].events[1].id.0, "event3");
    let mapped_authored = mapped.authored_articulations.unwrap();
    assert_eq!(mapped_authored.events[0].event.0, "event3");
    assert_eq!(mapped_authored.events[1].event.0, "event2");
    let preserved: std::collections::BTreeSet<_> = mapped_authored
        .events
        .iter()
        .flat_map(|e| &e.notations)
        .flat_map(|w| {
            w.source_id
                .iter()
                .chain(w.groups.iter().filter_map(|g| g.source_id.as_ref()))
        })
        .map(String::as_str)
        .collect();
    assert_eq!(preserved, reserved.into_iter().collect());
}
