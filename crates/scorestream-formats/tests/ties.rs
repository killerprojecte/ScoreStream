use scorestream_app::playback::plan_source;
use scorestream_core::{
    Rational,
    notation::{EventId, RelationId},
    score::ScoreTie,
};
use scorestream_formats::{
    MappingEffect, ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter, validate_score,
};
const CHAIN: &str = include_str!("fixtures/ties-transposing-chain.musicxml");
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn error(source: &str, feature: &str) {
    let report = MusicXml.read(source).unwrap_err();
    assert!(
        report.diagnostics.iter().any(|d| d.feature == feature),
        "Expected {feature}, got {report:?}"
    );
    assert!(report.diagnostics.iter().any(|d| d.source.is_some()));
}
#[test]
fn xml_common_sfs_actual_plan_preserves_chain_and_written_notes() {
    let imported = MusicXml.read(CHAIN).unwrap();
    assert!(
        imported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "tie.identity" && d.effect == MappingEffect::Normalized)
    );
    let common = imported.value;
    assert_eq!(common.ties.len(), 2);
    assert_eq!(
        common.ties[0].events,
        [EventId("a".into()), EventId("b".into())]
    );
    assert_eq!(
        common.ties[1].events,
        [EventId("b".into()), EventId("c".into())]
    );
    assert_eq!(
        common.parts[0].measures[0].events[0].pitch.to_string(),
        "A4"
    );
    assert_eq!(
        common.parts[0].measures[2].events[0].pitch.to_string(),
        "G4"
    );
    let sfs = SfsAdapter.write(&common).unwrap().value;
    let recovered = SfsAdapter.read(&sfs).unwrap().value;
    assert_eq!(common.ties, recovered.ties);
    assert_eq!(common.parts[0].measures, recovered.parts[0].measures);
    let plan = plan_source(&sfs).unwrap();
    assert_eq!(plan.notes.len(), 1, "No continuation reattacks");
    assert_eq!(
        plan.event_positions.len(),
        3,
        "All written identities remain addressable"
    );
    assert_eq!(plan.measures.len(), 3);
    close(plan.notes[0].midi_note, 67.);
    close(plan.notes[0].start_seconds, 0.);
    // The existing default gate shortens only the final segment by 8%, not each bar.
    close(plan.notes[0].duration_seconds, 1.46);
    close(plan.duration_seconds, 1.5);
    close(plan.event_positions[2].end_seconds, 1.5);
    // Explicit existing tenuto policy proves the complete 3-quarter sounding span.
    let mut doc = scorestream_codec::parse(&sfs).unwrap();
    doc.frames
        .last_mut()
        .unwrap()
        .records
        .push(scorestream_core::Record::Mark(scorestream_core::Mark {
            target: "#c".into(),
            kind: "articulation".into(),
            attributes: [(
                "value".into(),
                scorestream_core::Scalar::String("tenuto".into()),
            )]
            .into(),
            source: Default::default(),
        }));
    close(
        plan_source(&scorestream_codec::canonical(&doc))
            .unwrap()
            .notes[0]
            .duration_seconds,
        1.5,
    );
}
#[test]
fn xml_export_preserves_endpoint_graph_and_regenerates_numbering() {
    let common = MusicXml.read(CHAIN).unwrap().value;
    let exported = MusicXml.write(&common).unwrap();
    assert!(
        exported
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "tie.identity")
    );
    let again = MusicXml.read(&exported.value).unwrap().value;
    assert_eq!(common.ties, again.ties);
    for (a, b) in common.parts[0]
        .measures
        .iter()
        .zip(&again.parts[0].measures)
    {
        assert_eq!(a.events, b.events);
    }
    let position = exported.value.find("<note id=\"b\">").unwrap();
    let b = &exported.value[position..];
    assert!(b.find("<tie type=\"stop\"").unwrap() < b.find("<tie type=\"start\"").unwrap());
    assert!(b.find("<tied type=\"stop\"").unwrap() < b.find("<tied type=\"start\"").unwrap());
}
#[test]
fn sfs_continuation_keeps_relation_identity_and_three_anchors() {
    let sfs = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m1 len=1 implicit=true\nv C4:1 id=a\n~ tie hold start=#a\n.\n@1\n| p m2 len=1 implicit=true\nv C4:1 id=b\n~ tie hold continue=#b\n.\n@2\n| p m3 len=1 implicit=true\nv C4:1 id=c\n~ tie hold end=#c\n.\n!end at=3\n";
    let common = SfsAdapter.read(sfs).unwrap().value;
    assert_eq!(common.ties[0].id.0, "hold");
    assert_eq!(common.ties[0].events.len(), 3);
    assert_eq!(
        common,
        SfsAdapter
            .read(&SfsAdapter.write(&common).unwrap().value)
            .unwrap()
            .value
    );
    let xml = MusicXml.write(&common).unwrap();
    let imported = MusicXml.read(&xml.value).unwrap().value;
    assert_eq!(
        imported.ties.len(),
        2,
        "XML models separate links, not a chain ID"
    );
    let converted = SfsAdapter.write(&imported).unwrap().value;
    let before = plan_source(sfs).unwrap();
    let mut after = plan_source(&converted).unwrap();
    after.notes[0].voice = before.notes[0].voice.clone(); // Explicitly normalized XML voice identity.
    assert_eq!(before.notes, after.notes);
}
fn note(id: &str, voice: &str, staff: u32, phase: &str, number: u8, chord: bool) -> String {
    format!(
        "<note id=\"{id}\">{}<pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><tie type=\"{phase}\"/><voice>{voice}</voice><staff>{staff}</staff><notations><tied type=\"{phase}\" number=\"{number}\"/></notations></note>",
        if chord { "<chord/>" } else { "" }
    )
}
fn pair(a: &str, b: &str) -> String {
    format!(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>piano</part-name><score-instrument id=\"i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id=\"p\"><measure number=\"1\"><attributes><divisions>1</divisions><staves>2</staves></attributes>{a}</measure><measure number=\"2\">{b}</measure></part></score-partwise>"
    )
}
#[test]
fn cross_staff_tie_is_not_split_into_two_attacks() {
    let xml = pair(
        &note("a", "1", 1, "start", 1, false),
        &note("b", "1", 2, "stop", 1, false),
    );
    let common = MusicXml.read(&xml).unwrap().value;
    assert_eq!(
        common.ties[0].events,
        [EventId("a".into()), EventId("b".into())]
    );
    assert_eq!(
        plan_source(&SfsAdapter.write(&common).unwrap().value)
            .unwrap()
            .notes
            .len(),
        1
    );
    let exported = MusicXml.write(&common).unwrap().value;
    let recovered = MusicXml.read(&exported).unwrap().value;
    assert_eq!(common.ties, recovered.ties);
    assert_eq!(common.parts[0].voices, recovered.parts[0].voices);
}
#[test]
fn simultaneous_unisons_pair_by_number_then_staff_not_note_order() {
    let a = format!(
        "{}{}",
        note("a", "1", 1, "start", 2, false),
        note("b", "1", 2, "start", 4, true)
    );
    let b = format!(
        "{}{}",
        note("c", "1", 1, "stop", 4, false),
        note("d", "1", 2, "stop", 2, true)
    );
    let common = MusicXml.read(&pair(&a, &b)).unwrap().value;
    assert_eq!(
        common.ties[0].events,
        [EventId("b".into()), EventId("c".into())]
    );
    assert_eq!(
        common.ties[1].events,
        [EventId("a".into()), EventId("d".into())]
    );
    assert_eq!(
        plan_source(&SfsAdapter.write(&common).unwrap().value)
            .unwrap()
            .notes
            .len(),
        2
    );
    let a = a.replace("number=\"4\"", "number=\"2\"");
    let b = b.replace("number=\"4\"", "number=\"2\"");
    let common = MusicXml.read(&pair(&a, &b)).unwrap().value;
    assert_eq!(common.ties[0].events[0].0, "a");
    assert_eq!(common.ties[1].events[0].0, "b");
}
#[test]
fn ambiguous_unison_and_cross_voice_stops_fail_instead_of_guessing() {
    let a = format!(
        "{}{}",
        note("a", "1", 1, "start", 1, false),
        note("b", "1", 1, "start", 1, true)
    );
    error(
        &pair(&a, &note("c", "1", 1, "stop", 1, false)),
        "tie.pairing",
    );
    error(
        &pair(
            &note("a", "1", 1, "start", 1, false),
            &note("b", "2", 1, "stop", 1, false),
        ),
        "tie.pairing",
    );
}
#[test]
fn same_written_pitch_different_effective_transposition_is_rejected() {
    error(
        &CHAIN.replace("<step>G</step>", "<step>A</step>"),
        "tie.pairing",
    );
    // Identical effective pitch with enharmonic spelling is allowed.
    let source = pair(
        &note("a", "1", 1, "start", 1, false)
            .replace("<step>C</step>", "<step>C</step><alter>1</alter>"),
        &note("b", "1", 1, "stop", 1, false)
            .replace("<step>C</step>", "<step>D</step><alter>-1</alter>"),
    );
    assert!(MusicXml.read(&source).is_ok());
}
#[test]
fn unsupported_semantics_and_contradictions_are_never_deleted() {
    error(
        &CHAIN.replacen(
            "<tie type=\"start\"/>",
            "<tie type=\"start\" time-only=\"1\"/>",
            1,
        ),
        "tie.repeat_condition",
    );
    error(
        &CHAIN.replacen("<tie type=\"start\"/>", "", 1),
        "tie.sound_notation_mismatch",
    );
    error(
        &CHAIN.replacen("<tied type=\"start\" number=\"2\"/>", "", 1),
        "tie.empty_notations",
    );
    error(
        &CHAIN.replacen("<tied type=\"start\"", "<tied type=\"let-ring\"", 1),
        "tie.notation_type",
    );
    error(
        &CHAIN.replacen("<tied type=\"start\"", "<tied type=\"continue\"", 1),
        "tie.notation_type",
    );
    error(
        &CHAIN.replacen(
            "<tied type=\"stop\" number=\"2\"/><tied type=\"start\" number=\"4\"/>",
            "<tied type=\"start\" number=\"4\"/><tied type=\"stop\" number=\"2\"/>",
            1,
        ),
        "tie.repeat_shape",
    );
    error(
        &CHAIN.replacen("number=\"2\"/>", "number=\"17\"/>", 1),
        "tie.number",
    );
    error(
        &CHAIN.replacen(
            "<tied type=\"start\"",
            "<tied id=\"mark1\" type=\"start\"",
            1,
        ),
        "attribute.id",
    );
    error(
        &CHAIN.replacen(
            "<tied type=\"start\"",
            "<tied placement=\"above\" type=\"start\"",
            1,
        ),
        "attribute.placement",
    );
    error(
        &CHAIN.replacen("<notations>", "<notations><slur type=\"start\"/>", 1),
        "element.slur",
    );
}
#[test]
fn dangling_gapped_duplicate_and_unmatched_endpoints_fail() {
    let a = note("a", "1", 1, "start", 1, false);
    let b = note("b", "1", 1, "stop", 1, false);
    error(
        &pair(&a, &b.replace("type=\"stop\"", "type=\"start\"")),
        "tie.unclosed",
    );
    error(
        &pair(&a, &b.replace("number=\"1\"", "number=\"2\"")),
        "tie.pairing",
    );
    error(
        &pair(&format!("{a}<forward><duration>1</duration></forward>"), &b),
        "tie.pairing",
    );
    error(
        &pair(
            &a.replace(
                "<tie type=\"start\"/>",
                "<tie type=\"start\"/><tie type=\"start\"/>",
            ),
            &b,
        ),
        "tie.duplicate",
    );
    error(
        &pair(
            &a.replace("<pitch><step>C</step><octave>4</octave></pitch>", "<rest/>"),
            &b,
        ),
        "tie.pitch",
    );
}
#[test]
fn public_common_model_validates_relations_before_export() {
    let common = MusicXml.read(CHAIN).unwrap().value;
    for (feature, change) in [
        ("tie.reference", 0),
        ("tie.identity", 1),
        ("tie.branch", 2),
        ("tie.continuity", 3),
        ("tie.pitch", 4),
        ("tie.anchors", 5),
    ] {
        let mut score = common.clone();
        match change {
            0 => score.ties[0].events[0] = EventId("missing".into()),
            1 => score.ties[1].id = score.ties[0].id.clone(),
            2 => score.ties.push(ScoreTie {
                id: RelationId("duplicate".into()),
                events: score.ties[0].events.clone(),
            }),
            3 => score.ties[0].events[1] = EventId("c".into()),
            4 => score.parts[0].measures[1].events[0].pitch = "C4".parse().unwrap(),
            5 => score.ties[0].events.truncate(1),
            _ => unreachable!(),
        }
        let report = validate_score(&score);
        assert!(
            report.diagnostics.iter().any(|d| d.feature == feature),
            "{report:?}"
        );
        assert!(SfsAdapter.write(&score).is_err());
        assert!(MusicXml.write(&score).is_err());
    }
    assert_eq!(
        common.parts[0].measures[0].events[0].duration,
        Rational::one()
    );
}
#[test]
fn sfs_print_only_sound_only_or_extra_engraving_are_explicitly_rejected() {
    let common = MusicXml.read(CHAIN).unwrap().value;
    let source = SfsAdapter.write(&common).unwrap().value;
    for field in ["sound=false", "print=false", "placement=above"] {
        let source = source.replace("start=#a", &format!("start=#a {field}"));
        assert!(SfsAdapter.read(&source).is_err(), "{source}");
    }
}

#[test]
fn musical_order_not_xml_encounter_order_controls_pairing() {
    let b = note("b", "1", 1, "stop", 1, false);
    let a = note("a", "1", 1, "start", 1, false);
    let first = format!(
        "<forward><duration>1</duration></forward>{b}<backup><duration>2</duration></backup>{a}"
    );
    let rest = "<note id=\"r\"><rest/><duration>1</duration></note>";
    let common = MusicXml.read(&pair(&first, rest)).unwrap().value;
    assert_eq!(
        common.ties[0].events,
        [EventId("a".into()), EventId("b".into())]
    );
    assert_eq!(
        plan_source(&SfsAdapter.write(&common).unwrap().value)
            .unwrap()
            .notes
            .len(),
        1
    );
}

#[test]
fn voice_and_part_local_ties_do_not_capture_other_unisons() {
    let first = format!(
        "{}<backup><duration>1</duration></backup>{}",
        note("a", "1", 1, "start", 1, false),
        note("b", "2", 1, "start", 1, false)
    );
    let second = format!(
        "{}<backup><duration>1</duration></backup>{}",
        note("c", "2", 1, "stop", 1, false),
        note("d", "1", 1, "stop", 1, false)
    );
    let source = pair(&first, &second);
    let common = MusicXml.read(&source).unwrap().value;
    assert_eq!(
        common.ties[0].events,
        [EventId("b".into()), EventId("c".into())]
    );
    assert_eq!(
        common.ties[1].events,
        [EventId("a".into()), EventId("d".into())]
    );
    let part = source
        .split("<part id=\"p\">")
        .nth(1)
        .unwrap()
        .split("</part>")
        .next()
        .unwrap();
    let extra = format!("<part id=\"p2\">{part}</part>")
        .replace("id=\"a\"", "id=\"e\"")
        .replace("id=\"b\"", "id=\"f\"")
        .replace("id=\"c\"", "id=\"g\"")
        .replace("id=\"d\"", "id=\"h\"");
    let source = source.replace("</part-list>", "<score-part id=\"p2\"><part-name>piano</part-name><score-instrument id=\"i2\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list>").replace("</score-partwise>", &format!("{extra}</score-partwise>"));
    let common = MusicXml.read(&source).unwrap().value;
    assert_eq!(common.ties.len(), 4);
    assert_eq!(
        plan_source(&SfsAdapter.write(&common).unwrap().value)
            .unwrap()
            .notes
            .len(),
        4
    );
}

#[test]
fn cross_staff_effective_pitch_uses_each_staff_transposition() {
    let a = note("a", "1", 1, "start", 1, false);
    let b = note("b", "1", 2, "stop", 1, false).replace(
        "<step>C</step><octave>4</octave>",
        "<step>B</step><alter>-1</alter><octave>3</octave>",
    );
    let source = pair(&a,&b).replace("<staves>2</staves>", "<staves>2</staves><transpose number=\"1\"><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose><transpose number=\"2\"><diatonic>0</diatonic><chromatic>0</chromatic></transpose>");
    let common = MusicXml.read(&source).unwrap().value;
    let plan = plan_source(&SfsAdapter.write(&common).unwrap().value).unwrap();
    assert_eq!(plan.notes.len(), 1);
    close(plan.notes[0].midi_note, 58.);
    error(
        &source.replace("<chromatic>-2</chromatic>", "<chromatic>-1</chromatic>"),
        "tie.pairing",
    );
}

#[test]
fn many_chained_links_keep_stable_ids_through_sfs_without_numeric_sort_assumptions() {
    let mut source = String::from(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>piano</part-name><score-instrument id=\"i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id=\"p\">",
    );
    for i in 0..24 {
        source.push_str(&format!("<measure number=\"{i}\"><attributes><divisions>1</divisions></attributes><note id=\"n{i}\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>"));
        if i > 0 {
            source.push_str("<tie type=\"stop\"/>");
        }
        if i < 23 {
            source.push_str("<tie type=\"start\"/>");
        }
        source.push_str("<notations>");
        if i > 0 {
            source.push_str("<tied type=\"stop\"/>");
        }
        if i < 23 {
            source.push_str("<tied type=\"start\"/>");
        }
        source.push_str("</notations></note></measure>");
    }
    source.push_str("</part></score-partwise>");
    let common = MusicXml.read(&source).unwrap().value;
    let sfs = SfsAdapter.write(&common).unwrap().value;
    assert_eq!(common.ties, SfsAdapter.read(&sfs).unwrap().value.ties);
    let plan = plan_source(&sfs).unwrap();
    assert_eq!(plan.notes.len(), 1);
    assert_eq!(plan.event_positions.len(), 24);
    close(plan.notes[0].duration_seconds, 11.96);
}

#[test]
fn compressed_mxl_root_uses_the_same_typed_tie_and_playback_path() {
    use std::io::{Cursor, Write};
    let mut bytes = Cursor::new(Vec::new());
    let mut archive = rawzip::ZipArchiveWriter::new(&mut bytes);
    let manifest = "<container><rootfiles><rootfile full-path=\"score/main.musicxml\" media-type=\"application/vnd.recordare.musicxml+xml\"/></rootfiles></container>";
    for (path, content) in [
        ("META-INF/container.xml", manifest),
        ("score/main.musicxml", CHAIN),
    ] {
        let (mut entry, config) = archive
            .new_file(path)
            .compression_method(rawzip::CompressionMethod::DEFLATE)
            .start()
            .unwrap();
        let encoder =
            flate2::write::DeflateEncoder::new(&mut entry, flate2::Compression::default());
        let mut writer = config.wrap(encoder);
        writer.write_all(content.as_bytes()).unwrap();
        let (encoder, descriptor) = writer.finish().unwrap();
        encoder.finish().unwrap();
        entry.finish(descriptor).unwrap();
    }
    archive.finish().unwrap();
    let root = scorestream_formats::mxl::open(Cursor::new(bytes.into_inner())).unwrap();
    assert_eq!(root.root_path(), "score/main.musicxml");
    let common = scorestream_formats::musicxml::read_reader(root.into_reader())
        .unwrap()
        .value;
    let plan = plan_source(&SfsAdapter.write(&common).unwrap().value).unwrap();
    assert_eq!(common.ties.len(), 2);
    assert_eq!(plan.notes.len(), 1);
    assert_eq!(plan.event_positions.len(), 3);
    close(plan.notes[0].midi_note, 67.);
    close(plan.notes[0].duration_seconds, 1.46);
}

#[test]
fn converging_or_diverging_unisons_fail_independently_of_event_id_order() {
    for (source_staffs, target_staffs) in [([1, 2], [1, 1]), ([1, 1], [1, 2])] {
        let first = format!(
            "{}{}",
            note("a", "1", source_staffs[0], "start", 1, false),
            note("b", "1", source_staffs[1], "start", 1, true)
        );
        let second = format!(
            "{}{}",
            note("c", "1", target_staffs[0], "stop", 1, false),
            note("d", "1", target_staffs[1], "stop", 1, true)
        );
        let source = pair(&first, &second);
        error(&source, "tie.pairing");
        error(&source.replace("id=\"c\"", "id=\"z\""), "tie.pairing");
        error(&source.replace("id=\"a\"", "id=\"z\""), "tie.pairing");
    }
}
