use scorestream_formats::{MappingEffect, ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
const DECL: &str = r#"<score-instrument id="P1-I1"><instrument-name>B♭ Clarinet</instrument-name><instrument-abbreviation>Cl. ♭</instrument-abbreviation><instrument-sound>wind.reed.clarinet.bflat</instrument-sound></score-instrument>"#;
const MIDI: &str = r#"<midi-device id="P1-I1"></midi-device><midi-instrument id="P1-I1"><midi-channel>12</midi-channel><midi-program>72</midi-program><volume>78.7402</volume><pan>0</pan></midi-instrument>"#;
fn xml(header: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Clarinet I / 分部</part-name>{header}</score-part></part-list><part id="P1"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#
    )
}
fn read(header: &str) -> scorestream_core::score::ScoreDocument {
    MusicXml.read(&xml(header)).unwrap().value
}
fn without_provenance(
    mut score: scorestream_core::score::ScoreDocument,
) -> scorestream_core::score::ScoreDocument {
    score.metadata.source.clear();
    score
}
#[test]
fn exact_identity_label_sound_and_midi_roundtrip_xml_sfs_and_json() {
    let imported = MusicXml.read(&xml(&format!("{DECL}{MIDI}"))).unwrap();
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "instrument.catalog_projection" && d.message.contains("broader"))
    );
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "instrument.midi_authored_only")
    );
    let score = imported.value;
    let part = &score.parts[0];
    assert_eq!(part.name.as_deref(), Some("Clarinet I / 分部"));
    assert_eq!(part.instrument.as_ref().unwrap().0, "clarinet");
    let value = part.instrument_metadata.as_ref().unwrap();
    assert_eq!(value.source_id, "P1-I1");
    assert_eq!(value.name, "B♭ Clarinet");
    assert_eq!(value.midi_device.as_ref().unwrap().port, None);
    assert_eq!(value.midi_device.as_ref().unwrap().name, "");
    assert_eq!(
        value.midi_instrument.as_ref().unwrap().volume,
        Some("393701/5000".parse().unwrap())
    );
    let sfs = SfsAdapter.write(&score).unwrap().value;
    assert!(sfs.contains("!requires scorestream.instrument@1"));
    assert!(sfs.contains("volume=393701/5000"));
    assert!(!sfs.contains("port="));
    let from_sfs = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(
        without_provenance(from_sfs.clone()),
        without_provenance(score.clone())
    );
    let json = serde_json::to_string(&from_sfs).unwrap();
    assert_eq!(
        serde_json::from_str::<scorestream_core::score::ScoreDocument>(&json).unwrap(),
        from_sfs
    );
    let out = MusicXml.write(&from_sfs).unwrap().value;
    assert!(out.contains("<volume>78.7402</volume>"));
    assert!(!out.contains("port="));
    let again = MusicXml.read(&out).unwrap().value;
    assert_eq!(without_provenance(again), without_provenance(score));
    assert!(
        scorestream_app::playback::plan_source(&sfs)
            .unwrap_err()
            .contains("instrument.performance_unsupported")
    );
}
#[test]
fn absent_empty_and_explicit_zero_are_distinct() {
    for midi in [
        "".to_string(),
        "<midi-device/>".into(),
        "<midi-instrument id=\"P1-I1\"/>".into(),
        MIDI.into(),
        MIDI.replace("id=\"P1-I1\"></midi-device>", "port=\"1\"></midi-device>")
            .replace("<volume>78.7402</volume>", "<volume>0</volume>"),
    ] {
        let score = read(&format!("{DECL}{midi}"));
        let sfs = SfsAdapter.write(&score).unwrap().value;
        let restored = SfsAdapter.read(&sfs).unwrap().value;
        assert_eq!(
            restored.parts[0].instrument_metadata,
            score.parts[0].instrument_metadata
        );
        let out = MusicXml.write(&restored).unwrap().value;
        assert_eq!(
            MusicXml.read(&out).unwrap().value.parts[0].instrument_metadata,
            score.parts[0].instrument_metadata
        );
        assert_eq!(
            scorestream_app::playback::plan_source(&sfs).is_err(),
            !midi.is_empty()
        );
    }
}
#[test]
fn all_reviewed_beethoven_sound_identities_are_canonical_without_program_inference() {
    let cases = [
        ("wind.flutes.flute", "flute"),
        ("wind.flutes.flute.piccolo", "piccolo"),
        ("wind.reed.oboe", "oboe"),
        ("wind.reed.clarinet", "clarinet"),
        ("wind.reed.clarinet.bflat", "clarinet"),
        ("wind.reed.bassoon", "bassoon"),
        ("wind.reed.contrabassoon", "contrabassoon"),
        ("brass.natural-horn", "horn"),
        ("brass.trumpet.c", "trumpet"),
        ("brass.trombone.alto", "alto_trombone"),
        ("brass.trombone.tenor", "trombone"),
        ("brass.trombone.bass", "bass_trombone"),
        ("drum.timpani", "timpani"),
        ("strings.violin", "violin"),
        ("strings.viola", "viola"),
        ("strings.cello", "cello"),
        ("strings.contrabass", "double_bass"),
    ];
    for (sound, id) in cases {
        let score = read(
            &DECL
                .replace("wind.reed.clarinet.bflat", sound)
                .replace("B♭ Clarinet", "Unrelated Unicode label / 音源"),
        );
        assert_eq!(score.parts[0].instrument.as_ref().unwrap().0, id);
        let source = SfsAdapter.write(&score).unwrap().value;
        let plan = scorestream_app::playback::plan_source(&source).unwrap();
        assert_eq!(plan.notes[0].instrument, id);
        assert_eq!(plan.notes[0].midi_note, 58.0); // only authored transpose; no octave by name
    }
}
#[test]
fn explicit_unknown_sound_never_falls_back_to_name_or_midi() {
    for sound in [
        "wind.reed.clarinet.unknown",
        "",
        " wind.reed.clarinet.bflat ",
    ] {
        let input = format!(
            "{}{}",
            DECL.replace("wind.reed.clarinet.bflat", sound)
                .replace("B♭ Clarinet", "flute"),
            MIDI
        );
        let report = MusicXml.read(&xml(&input)).unwrap_err();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.feature == "instrument.catalog_unresolved"
                    && d.effect == MappingEffect::Unsupported)
        );
    }
}
#[test]
fn unknown_shapes_and_dynamic_or_multiple_instrument_payloads_remain_blocked() {
    for header in [format!("{DECL}{DECL}"), format!("{DECL}{MIDI}{MIDI}"), format!("{DECL}{MIDI}").replace("<midi-program>", "<midi-bank>1</midi-bank><midi-program>"), DECL.replace("</score-instrument>","<virtual-instrument><virtual-name>Library</virtual-name></virtual-instrument></score-instrument>"), MIDI.into()] {
        assert!(MusicXml.read(&xml(&header)).is_err(), "{header}");
    }
    let valid = xml(&format!("{DECL}{MIDI}"));
    for input in [valid.replace("<duration>","<instrument id=\"P1-I1\"/><duration>"),valid.replace("</measure>","<sound><midi-instrument id=\"P1-I1\"><midi-program>1</midi-program></midi-instrument></sound></measure>")] {
        assert!(MusicXml.read(&input).is_err());
    }
}
#[test]
fn exact_idrefs_and_one_based_boundaries_are_checked_without_defaults() {
    for invalid in [
        MIDI.replace("id=\"P1-I1\"", "id=\"missing\""),
        MIDI.replace("<midi-channel>12", "<midi-channel>0"),
        MIDI.replace("<midi-channel>12", "<midi-channel>17"),
        MIDI.replace("<midi-program>72", "<midi-program>129"),
        MIDI.replace("<volume>78.7402", "<volume>-0.0001"),
        MIDI.replace("<volume>78.7402", "<volume>100.0001"),
        MIDI.replace("<pan>0", "<pan>180.001"),
        MIDI.replace("<midi-device", "<midi-device port=\"0\""),
    ] {
        assert!(
            MusicXml.read(&xml(&format!("{DECL}{invalid}"))).is_err(),
            "{invalid}"
        );
    }
    for (channel, program, volume, pan, port) in
        [(1, 1, "0", "-180", 1), (16, 128, "100", "180", 16)]
    {
        let midi = MIDI
            .replace("<midi-device", &format!("<midi-device port=\"{port}\""))
            .replace("<midi-channel>12", &format!("<midi-channel>{channel}"))
            .replace("<midi-program>72", &format!("<midi-program>{program}"))
            .replace("<volume>78.7402", &format!("<volume>{volume}"))
            .replace("<pan>0", &format!("<pan>{pan}"));
        let value = read(&format!("{DECL}{midi}"));
        assert!(MusicXml.write(&value).is_ok());
    }
}
#[test]
fn xml_legal_unicode_labels_survive_sfs_escaping_and_forbidden_text_rejects() {
    for text in [
        "名字 &amp; &lt;flute&gt;",
        "line&#10;two&#13;three&#9;tab",
        "a&#127;&#133;&#8232;b",
        "",
    ] {
        let score = read(&DECL.replace("B♭ Clarinet", text));
        let sfs = SfsAdapter.write(&score).unwrap().value;
        let again = SfsAdapter.read(&sfs).unwrap().value;
        assert_eq!(
            again.parts[0].instrument_metadata,
            score.parts[0].instrument_metadata
        );
        let out = MusicXml.write(&again).unwrap().value;
        assert_eq!(
            MusicXml.read(&out).unwrap().value.parts[0].instrument_metadata,
            score.parts[0].instrument_metadata
        );
    }
    let mut score = read(DECL);
    score.parts[0].instrument_metadata.as_mut().unwrap().name = "bad\0name".into();
    assert!(SfsAdapter.write(&score).is_err());
    assert!(MusicXml.write(&score).is_err());
}
#[test]
fn public_model_rejects_projection_changes_id_collisions_and_unknown_json_fields() {
    let mut score = read(DECL);
    score.parts[0].instrument.as_mut().unwrap().0 = "flute".into();
    assert!(SfsAdapter.write(&score).is_err());
    assert!(MusicXml.write(&score).is_err());
    let mut score = read(DECL);
    score.parts[0]
        .instrument_metadata
        .as_mut()
        .unwrap()
        .source_id = "m".into();
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.id")
    );
    let mut json = serde_json::to_value(read(DECL)).unwrap();
    json["parts"][0]["instrument_metadata"]["sample_library"] = "not inferred".into();
    assert!(serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).is_err());
}
#[test]
fn rational_intent_is_exact_in_sfs_but_non_decimal_xml_is_not_rounded() {
    let mut score = read(&format!("{DECL}{MIDI}"));
    score.parts[0]
        .instrument_metadata
        .as_mut()
        .unwrap()
        .midi_instrument
        .as_mut()
        .unwrap()
        .volume = Some("1/3".parse().unwrap());
    let sfs = SfsAdapter.write(&score).unwrap().value;
    assert_eq!(
        SfsAdapter.read(&sfs).unwrap().value.parts[0].instrument_metadata,
        score.parts[0].instrument_metadata
    );
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "instrument.decimal")
    );
}
#[test]
fn sfs_extension_is_required_scoped_typed_and_not_empty() {
    let source = SfsAdapter
        .write(&read(&format!("{DECL}{MIDI}")))
        .unwrap()
        .value;
    for invalid in [
        source.replace("!requires scorestream.instrument@1\n", ""),
        source.replace("scorestream.instrument@1", "scorestream.instrument@2"),
        source.replace(
            "+ P1 scorestream.instrument",
            "+ P1.s1 scorestream.instrument",
        ),
        source.replace("midi_ref=\"P1-I1\"", "midi_ref=\"other\""),
        source.replace("name=\"B♭ Clarinet\"", "name=10"),
        source
            .replace(" port=1", " port=0")
            .replace("channel=12", "channel=0"),
        source.replace("volume=393701/5000", "volume=101"),
        source.replace("pan=0", "pan=-181"),
        source.replace("device=\"\" ", ""),
        source.replace("program=72", "program=72 extra=true"),
    ] {
        assert!(SfsAdapter.read(&invalid).is_err(), "{invalid}");
    }
    let mark = source
        .lines()
        .find(|l| l.starts_with("+ P1 scorestream.instrument"))
        .unwrap();
    assert!(SfsAdapter.read(&source.replace(mark, "")).is_err());
    assert!(
        SfsAdapter
            .read(&source.replace(mark, &format!("{mark}\n{mark}")))
            .is_err()
    );
}
#[test]
fn plain_identity_uses_existing_routes_and_later_explicit_sfs_state() {
    let source = SfsAdapter.write(&read(DECL)).unwrap().value;
    let plan = scorestream_app::playback::plan_source(&source).unwrap();
    assert_eq!(plan.notes[0].midi_note, 58.0);
    let changed = source.replace("= P1.s1", "= P1 instrument=flute\n= P1.s1");
    assert_eq!(
        scorestream_app::playback::plan_source(&changed)
            .unwrap()
            .notes[0]
            .instrument,
        "flute"
    );
    assert!(SfsAdapter.read(&changed).is_err()); // MusicXML/shared model cannot silently discard that state.
}

#[test]
fn same_channel_and_program_on_separate_ports_do_not_collapse_instrument_identity() {
    let mut score = read(&format!("{DECL}{MIDI}"));
    let mut second = score.parts[0].clone();
    second.id.0 = "P2".into();
    second.voices[0].id.0 = "v2_1".into();
    second.measures[0].id = "m2".into();
    second.measures[0].events[0].id.0 = "n2".into();
    second.measures[0].events[0].voice = second.voices[0].id.clone();
    for (part, source, sound, id, port) in [
        (
            &mut score.parts[0],
            "P1-I1",
            "brass.trombone.alto",
            "alto_trombone",
            1,
        ),
        (
            &mut second,
            "P2-I1",
            "brass.trombone.bass",
            "bass_trombone",
            3,
        ),
    ] {
        part.instrument.as_mut().unwrap().0 = id.into();
        let value = part.instrument_metadata.as_mut().unwrap();
        value.source_id = source.into();
        value.sound = Some(sound.into());
        value.midi_device.as_mut().unwrap().instrument_ref = Some(source.into());
        value.midi_device.as_mut().unwrap().port = Some(port);
        value.midi_instrument.as_mut().unwrap().instrument_ref = source.into();
        value.midi_instrument.as_mut().unwrap().program = Some(58);
    }
    score.parts.push(second);
    let sfs = SfsAdapter.write(&score).unwrap().value;
    let again = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(again.parts, score.parts);
    assert!(scorestream_app::playback::plan_source(&sfs).is_err());
    let out = MusicXml.write(&again).unwrap().value;
    assert_eq!(MusicXml.read(&out).unwrap().value.parts, score.parts);
}
#[test]
fn normalized_sfs_object_ids_do_not_collide_with_preserved_source_identity() {
    let mut score = read(DECL);
    score.parts[0].measures[0].id = "m.with.dot".into();
    score.parts[0]
        .instrument_metadata
        .as_mut()
        .unwrap()
        .source_id = "measure1".into();
    let sfs = SfsAdapter.write(&score).unwrap().value;
    assert!(sfs.contains("| P1 measure2 "));
    let again = SfsAdapter.read(&sfs).unwrap().value;
    assert!(MusicXml.write(&again).is_ok());
}
#[test]
fn legacy_unknown_sfs_remains_open_but_musicxml_export_rejects_unresolvable_identity() {
    let sfs = "!score 1.1 unit=q\n!part p instrument=custom_synth staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=n\n.\n!end at=1\n";
    let score = SfsAdapter.read(sfs).unwrap().value;
    assert_eq!(
        score.parts[0].instrument.as_ref().unwrap().0,
        "custom_synth"
    );
    assert!(score.parts[0].instrument_metadata.is_none());
    assert!(
        SfsAdapter
            .write(&score)
            .unwrap()
            .value
            .contains("instrument=custom_synth")
    );
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "instrument.catalog_unresolved")
    );
}
