use scorestream_core::{Pitch, Rational, Record, Scalar, score::*};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, GeometryPolicy, MusicXml, MusicXmlOptions},
    sfs::{self, SfsAdapter},
};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn xml(notes: &str, attributes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute 1</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>4</divisions><time><beats>2</beats><beat-type>4</beat-type></time>{attributes}</attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, alter: &str, accidental: &str, grace: bool) -> String {
    format!(
        r#"<note id="{id}">{}<pitch><step>C</step><alter>{alter}</alter><octave>4</octave></pitch>{}<voice>1</voice><type>{}</type>{accidental}<stem>up</stem></note>"#,
        if grace { "<grace/>" } else { "" },
        if grace { "" } else { "<duration>8</duration>" },
        if grace { "16th" } else { "half" }
    )
}
fn glyphs() -> [(&'static str, &'static str); 7] {
    [
        ("natural", "0"),
        ("sharp", "1"),
        ("flat", "-1"),
        ("double-sharp", "2"),
        ("flat-flat", "-2"),
        ("quarter-sharp", "0.5"),
        ("quarter-flat", "-0.5"),
    ]
}
fn flag(key: &str, value: Option<bool>) -> String {
    value
        .map(|v| format!(" {key}=\"{}\"", if v { "yes" } else { "no" }))
        .unwrap_or_default()
}
fn roundtrip(source: &str) -> ScoreDocument {
    let a = MusicXml.read(source).unwrap().value;
    let wire = SfsAdapter.write(&a).unwrap().value;
    let ast = scorestream_codec::parse(&wire).unwrap();
    assert_eq!(sfs::from_document(&ast).unwrap().value, a);
    assert_eq!(
        SfsAdapter
            .read(&scorestream_codec::canonical(&ast))
            .unwrap()
            .value,
        a
    );
    let ast: scorestream_core::Document =
        serde_json::from_str(&serde_json::to_string(&ast).unwrap()).unwrap();
    assert_eq!(sfs::from_document(&ast).unwrap().value, a);
    let output = MusicXml.write(&a).unwrap().value;
    assert_eq!(MusicXml.read(&output).unwrap().value, a);
    a
}
#[test]
fn seven_glyphs_and_all_flag_presences_roundtrip_on_measured_and_grace_notes() {
    for (glyph, alter) in glyphs() {
        for cautionary in [None, Some(false), Some(true)] {
            for parentheses in [None, Some(false), Some(true)] {
                for grace in [false, true] {
                    let accidental = format!(
                        "<accidental{}{}>{glyph}</accidental>",
                        flag("cautionary", cautionary),
                        flag("parentheses", parentheses)
                    );
                    let notes = if grace {
                        format!(
                            "{}{}",
                            note("g", alter, &accidental, true),
                            note("n", "0", "", false)
                        )
                    } else {
                        note("n", alter, &accidental, false)
                    };
                    let a = roundtrip(&xml(&notes, ""));
                    let expected = Some(PrintedAccidental {
                        glyph: AccidentalGlyph::from_name(glyph).unwrap(),
                        cautionary,
                        parentheses,
                    });
                    if grace {
                        assert_eq!(a.graces[0].accidental, expected);
                        assert_eq!(a.graces[0].timing, GraceTiming::default());
                        assert_eq!(a.graces[0].order, -1);
                    } else {
                        assert_eq!(a.parts[0].measures[0].events[0].accidental, expected);
                        assert_eq!(a.parts[0].measures[0].events[0].duration, r("2"));
                    }
                }
            }
        }
    }
}
#[test]
fn glyph_absence_is_not_inferred_from_pitch_or_key_and_old_json_stays_valid() {
    for (_, alter) in glyphs() {
        let source = xml(
            &note("n", alter, "", false),
            "<key><fifths>3</fifths></key>",
        );
        let a = roundtrip(&source);
        assert!(a.parts[0].measures[0].events[0].accidental.is_none());
        let json = serde_json::to_value(&a).unwrap();
        assert!(
            !json["parts"][0]["measures"][0]["events"][0]
                .as_object()
                .unwrap()
                .contains_key("accidental")
        );
        assert_eq!(serde_json::from_value::<ScoreDocument>(json).unwrap(), a);
    }
    let a = roundtrip(&xml(
        &format!("{}{}", note("g", "1", "", true), note("n", "0", "", false)),
        "",
    ));
    assert!(a.graces[0].accidental.is_none());
    assert!(
        !serde_json::to_value(&a.graces[0])
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("accidental")
    );
}
#[test]
fn written_glyph_does_not_change_transposition_duration_or_actual_playback() {
    let source = xml(
        &note(
            "n",
            "1",
            "<accidental cautionary=\"yes\" parentheses=\"no\">sharp</accidental>",
            false,
        ),
        "<transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose>",
    );
    let a = roundtrip(&source);
    let source = SfsAdapter.write(&a).unwrap().value;
    let resolved =
        scorestream_core::notation::lower_document(&scorestream_codec::parse(&source).unwrap())
            .unwrap();
    assert_eq!(resolved.events[0].sounding_pitch, Some(r("47")));
    assert_eq!(
        a.parts[0].measures[0].events[0].pitch,
        Pitch::Pitched {
            step: 'C',
            alteration: r("1"),
            octave: 4.into()
        }
    );
    let with = scorestream_app::playback::plan_source(&source).unwrap();
    assert_eq!(with.notes[0].midi_note, 59.0);
    let mut plain = a.clone();
    plain.parts[0].measures[0].events[0].accidental = None;
    let without =
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap();
    assert_eq!(
        serde_json::to_value(with).unwrap(),
        serde_json::to_value(without).unwrap()
    );
}
#[test]
fn native_valid_extra_attributes_and_duplicate_marks_are_not_dropped() {
    let a = MusicXml
        .read(&xml(
            &note("n", "0", "<accidental>natural</accidental>", false),
            "",
        ))
        .unwrap()
        .value;
    let doc = sfs::to_document(&a).unwrap().value;
    for (key, value) in [
        ("visible", Scalar::Boolean(false)),
        ("editorial", Scalar::Boolean(false)),
        ("id", Scalar::String("glyph1".into())),
    ] {
        let mut changed = doc.clone();
        for record in changed.frames.iter_mut().flat_map(|f| &mut f.records) {
            let Record::Mark(m) = record else { continue };
            if m.kind == "accidental" {
                m.attributes.insert(key.into(), value.clone());
            }
        }
        assert!(
            !scorestream_core::validate(&changed, None)
                .iter()
                .any(|d| d.severity == scorestream_core::Severity::Error)
        );
        assert!(
            sfs::from_document(&changed)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "accidental.attribute")
        );
    }
    let mut duplicated = doc.clone();
    let mark = duplicated.frames[0]
        .records
        .iter()
        .find(|r| matches!(r,Record::Mark(m) if m.kind=="accidental"))
        .unwrap()
        .clone();
    duplicated.frames[0].records.push(mark);
    assert!(
        sfs::from_document(&duplicated)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "accidental.duplicate")
    );
}
#[test]
fn unknown_glyphs_flags_styles_targets_and_pitch_conflicts_fail_closed() {
    let options = MusicXmlOptions {
        geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
        ..Default::default()
    };
    for accidental in [
        "<accidental>other</accidental>",
        "<accidental>sharp-sharp</accidental>",
        "<accidental>\u{85}natural</accidental>",
        "<accidental/>",
        "<accidental cautionary=\"true\">natural</accidental>",
        "<accidental parentheses=\"maybe\">natural</accidental>",
        "<accidental smufl=\"accidentalNatural\">natural</accidental>",
        "<accidental default-x=\"2\">natural</accidental>",
        "<accidental editorial=\"no\">natural</accidental>",
        "<accidental bracket=\"yes\">natural</accidental>",
        "<accidental>natural<unknown/></accidental>",
        "<accidental>natural</accidental><accidental>natural</accidental>",
        "<accidental>flat</accidental>",
    ] {
        let source = xml(&note("n", "0", accidental, false), "");
        assert!(MusicXml.read(&source).is_err(), "{source}");
        assert!(musicxml::read_with_options(&source, &options).is_err());
    }
    let good = MusicXml
        .read(&xml(
            &note("n", "0", "<accidental>natural</accidental>", false),
            "",
        ))
        .unwrap()
        .value;
    for pitch in [
        Pitch::Rest,
        Pitch::Pitched {
            step: 'C',
            alteration: r("1"),
            octave: 4.into(),
        },
    ] {
        let mut bad = good.clone();
        bad.parts[0].measures[0].events[0].pitch = pitch;
        assert!(MusicXml.write(&bad).is_err());
        assert!(SfsAdapter.write(&bad).is_err());
    }
    let hidden = xml(
        &note("n", "0", "<accidental>natural</accidental>", false),
        "",
    )
    .replace("<note id=\"n\">", "<note id=\"n\" print-object=\"no\">");
    assert!(MusicXml.read(&hidden).is_err());
}
#[test]
fn chord_members_ties_labels_and_source_midi_intent_keep_their_own_identity() {
    let first = note("n", "1", "<accidental>sharp</accidental>", false);
    let member = note(
        "c",
        "0",
        "<accidental cautionary=\"yes\" parentheses=\"no\">natural</accidental>",
        false,
    )
    .replace("<note id=\"c\">", "<note id=\"c\"><chord/>");
    let a = roundtrip(&xml(&format!("{first}{member}"), ""));
    assert_eq!(a.parts[0].name.as_deref(), Some("Flute 1"));
    assert_eq!(a.parts[0].measures[0].events.len(), 2);
    assert_eq!(
        a.parts[0].measures[0].events[0]
            .accidental
            .as_ref()
            .unwrap()
            .glyph,
        AccidentalGlyph::Sharp
    );
    assert_eq!(
        a.parts[0].measures[0].events[1]
            .accidental
            .as_ref()
            .unwrap()
            .glyph,
        AccidentalGlyph::Natural
    );
    let tied = xml(
        &note(
            "n",
            "0",
            "<accidental>natural</accidental><notations><tied type=\"start\"/></notations>",
            false,
        )
        .replace("<voice>", "<tie type=\"start\"/><voice>"),
        "",
    )
    .replace(
        "</part></score-partwise>",
        &format!(
            "<measure id=\"m2\" number=\"2\">{}</measure></part></score-partwise>",
            note(
                "next",
                "0",
                "<notations><tied type=\"stop\"/></notations>",
                false
            )
            .replace("<voice>", "<tie type=\"stop\"/><voice>")
        ),
    );
    let a = roundtrip(&tied);
    assert_eq!(a.ties.len(), 1);
    assert!(a.parts[0].measures[1].events[0].accidental.is_none());
    let source=xml(&note("n","0","<accidental>natural</accidental>",false),"").replace("</score-part>","<midi-instrument id=\"i\"><midi-channel>1</midi-channel><midi-program>74</midi-program></midi-instrument></score-part>");
    let a = roundtrip(&source);
    assert!(a.parts[0].instrument_metadata.is_some());
    assert!(scorestream_app::playback::plan_source(&SfsAdapter.write(&a).unwrap().value).is_err());
}

#[test]
fn imported_grace_glyph_survives_the_explicit_schedule_without_extra_attack_or_time() {
    use scorestream_app::playback::{
        GraceSoundPolicy, discover_source_graces, plan_source_with_explicit_graces,
    };
    use scorestream_core::notation::{
        ExplicitGraceGroup, ExplicitGracePolicy, GraceAllocation, GraceQuarterAllocation,
    };
    let a = roundtrip(&xml(
        &format!(
            "{}{}",
            note(
                "g",
                "-1",
                "<accidental parentheses=\"no\">flat</accidental>",
                true
            ),
            note(
                "n",
                "0",
                "<accidental cautionary=\"yes\" parentheses=\"no\">natural</accidental>",
                false
            )
        ),
        "",
    ));
    let source = SfsAdapter.write(&a).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&source).is_err());
    let discovery = discover_source_graces(&source).unwrap();
    let occurrence = &discovery.occurrences[0];
    let policy = ExplicitGracePolicy {
        groups: vec![ExplicitGraceGroup {
            principal: occurrence.principal.clone(),
            allocation: GraceAllocation::ExplicitFollowingQuarters {
                members: vec![GraceQuarterAllocation {
                    event: occurrence.id.clone(),
                    quarters: r("1/4"),
                }],
            },
        }],
    };
    let with = plan_source_with_explicit_graces(
        &source,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
    )
    .unwrap();
    let mut plain = a.clone();
    plain.graces[0].accidental = None;
    plain.parts[0].measures[0].events[0].accidental = None;
    let plain = SfsAdapter.write(&plain).unwrap().value;
    let without = plan_source_with_explicit_graces(
        &plain,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
    )
    .unwrap();
    assert_eq!(with.plan().notes, without.plan().notes);
    assert_eq!(
        with.plan().duration_seconds,
        without.plan().duration_seconds
    );
    assert_eq!(with.plan().notes.len(), 2);
    assert_eq!(SfsAdapter.read(&source).unwrap().value, a);
}

#[test]
fn untouched_cautionary_fragment_maps_but_grace_beams_and_slur_remain_blocking() {
    // Exact original I/P1/m96/note1, lines1939–1949. Only the small surrounding
    // part/measure context is a test fixture, never a substitute original score.
    let cautionary = r#"<note default-x="18.64" default-y="40">
        <pitch>
          <step>G</step>
          <octave>6</octave>
          </pitch>
        <duration>4</duration>
        <voice>1</voice>
        <type>quarter</type>
        <accidental cautionary="yes" parentheses="no">natural</accidental>
        <stem>down</stem>
        </note>"#;
    let options = MusicXmlOptions {
        geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
        ..Default::default()
    };
    let source = xml(cautionary, "<key><fifths>-3</fifths></key>");
    assert!(MusicXml.read(&source).is_err());
    let result = musicxml::read_with_options(&source, &options).unwrap();
    assert_eq!(
        result
            .report
            .diagnostics
            .iter()
            .filter(|d| d.effect == scorestream_formats::MappingEffect::PolicyApplied)
            .count(),
        2
    );
    let score = result.value;
    let e = &score.parts[0].measures[0].events[0];
    assert_eq!(
        e.accidental,
        Some(PrintedAccidental {
            glyph: AccidentalGlyph::Natural,
            cautionary: Some(true),
            parentheses: Some(false)
        })
    );
    assert_eq!(e.duration, r("1"));
    assert_eq!(e.stem, Some(StemDirection::Down));
    assert_eq!(
        e.pitch,
        Pitch::Pitched {
            step: 'G',
            alteration: r("0"),
            octave: 6.into()
        }
    );
    assert_eq!(
        SfsAdapter
            .read(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .value,
        score
    );
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&score).unwrap().value)
            .unwrap()
            .value,
        score
    );
    // Exact original III/P14/m258/note1, lines77849–77865. Its beams and slur
    // stay in this source fragment; no success is obtained by removing them.
    let grace = r#"<note default-x="100.22" default-y="-1503.24">
        <grace/>
        <pitch>
          <step>B</step>
          <octave>3</octave>
          </pitch>
        <voice>1</voice>
        <type>32nd</type>
        <accidental>natural</accidental>
        <stem>up</stem>
        <beam number="1">begin</beam>
        <beam number="2">begin</beam>
        <beam number="3">begin</beam>
        <notations>
          <slur type="start" bezier-x="12.541667" bezier-y="-3.154677" number="1"/>
          </notations>
        </note>"#;
    let report = musicxml::read_with_options(
        &xml(&format!("{grace}{}", note("n", "0", "", false)), ""),
        &options,
    )
    .unwrap_err();
    assert!(!
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "element.beam")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "slur.unmatched")
    );
}

#[test]
fn omitted_type_tuplet_order_and_normalized_event_ids_keep_accidental_ownership() {
    for grace in [false, true] {
        for tuplet in [false, true] {
            let mut first = note(
                if grace { "g" } else { "n" },
                "1",
                "<accidental cautionary=\"no\" parentheses=\"yes\">sharp</accidental>",
                grace,
            );
            for kind in ["half", "16th"] {
                first = first.replace(
                    &format!("<type>{kind}</type>"),
                    if tuplet {
                        "<type>eighth</type><dot/>"
                    } else {
                        ""
                    },
                );
            }
            if tuplet {
                first=first.replace("</accidental>","</accidental><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>").replace("<duration>8</duration>","<duration>2</duration>");
            }
            let notes = if grace {
                format!("{first}{}", note("n", "0", "", false))
            } else {
                first
            };
            let a = roundtrip(&xml(&notes, ""));
            let written = if grace {
                a.graces[0].written.as_ref()
            } else {
                a.parts[0].measures[0].events[0].written.as_ref()
            };
            assert_eq!(written.is_some(), tuplet);
            let output = MusicXml.write(&a).unwrap().value;
            let accidental = output.find("<accidental").unwrap();
            let stem = output.find("<stem>").unwrap();
            assert!(accidental < stem);
            if tuplet {
                let tm = output.find("<time-modification>").unwrap();
                assert!(accidental < tm && tm < stem);
            }
        }
    }
    let a = MusicXml
        .read(&xml(
            &format!(
                "{}{}",
                note(
                    "g.1",
                    "-1",
                    "<accidental parentheses=\"no\">flat</accidental>",
                    true
                ),
                note(
                    "n.1",
                    "1",
                    "<accidental cautionary=\"yes\">sharp</accidental>",
                    false
                )
            ),
            "",
        ))
        .unwrap()
        .value;
    let restored = SfsAdapter
        .read(&SfsAdapter.write(&a).unwrap().value)
        .unwrap()
        .value;
    let event = &restored.parts[0].measures[0].events[0];
    assert_ne!(event.id.0, "n.1");
    assert_ne!(restored.graces[0].id.0, "g.1");
    assert_eq!(restored.graces[0].anchor, event.id);
    assert_eq!(
        event.accidental,
        a.parts[0].measures[0].events[0].accidental
    );
    assert_eq!(restored.graces[0].accidental, a.graces[0].accidental);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&restored).unwrap().value)
            .unwrap()
            .value,
        restored
    );
}
