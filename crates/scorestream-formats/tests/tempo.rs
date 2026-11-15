use scorestream_app::playback::plan_source;
use scorestream_core::{
    Rational,
    notation::{BeatUnit, lower_document},
    score::ScoreTempo,
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter, validate_score};

fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
fn note(id: &str, duration: &str) -> String {
    format!(
        "<note id=\"{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>{duration}</duration><voice>1</voice></note>"
    )
}
fn mark(unit: &str, dots: &str, rate: &str, offset: &str, sound: &str) -> String {
    format!(
        "<direction><direction-type><metronome><beat-unit>{unit}</beat-unit>{dots}<per-minute>{rate}</per-minute></metronome></direction-type>{offset}{sound}</direction>"
    )
}
fn score(body: &str) -> String {
    format!(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"p\"><part-name>piano</part-name><score-instrument id=\"i\"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id=\"p\"><measure id=\"m\" number=\"1\"><attributes><divisions>4</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"
    )
}
fn full(body: &str) -> String {
    score(&format!("{body}{}", note("n", "16")))
}
fn common(source: &str) -> scorestream_core::score::ScoreDocument {
    MusicXml.read(source).unwrap().value
}
fn error(source: &str, feature: &str) {
    let report = MusicXml.read(source).unwrap_err();
    assert!(
        report.diagnostics.iter().any(|d| d.feature == feature),
        "expected {feature}, got {report:?}"
    );
}

#[test]
fn source_derived_beethoven_rates_preserve_written_beat_and_explicit_sound() {
    // Numeric fields taken from originals: I opening, III opening, IV opening.
    // This is not an extracted/cleaned score or a claim the originals import.
    for (unit, dots, rate, sounding) in [
        ("half", "", "108", "216"),
        ("half", "<beat-unit-dot/>", "96", "288"),
        ("half", "", "84", "168"),
    ] {
        let doc = common(&full(&mark(
            unit,
            dots,
            rate,
            "",
            &format!("<sound tempo=\"{sounding}\"/>"),
        )));
        assert_eq!(doc.tempos[0].quarter_bpm, r(sounding));
        assert_eq!(doc.metronomes[0].value.beat.unit, BeatUnit::Half);
        assert_eq!(doc.metronomes[0].value.beat.dotted, !dots.is_empty());
        assert_eq!(doc.metronomes[0].value.bpm, r(rate));
        let text = SfsAdapter.write(&doc).unwrap().value;
        let lowered = lower_document(&scorestream_codec::parse(&text).unwrap()).unwrap();
        assert_eq!(lowered.tempos[0].value.bpm, r(sounding));
        close(
            plan_source(&text).unwrap().duration_seconds,
            240.0 / sounding.parse::<f64>().unwrap(),
        );
    }
}
#[test]
fn explicit_sound_wins_without_overwriting_disagreeing_printed_mark() {
    let doc = common(&full(&mark(
        "half",
        "<beat-unit-dot/>",
        "96",
        "",
        "<sound tempo=\"60.25\"/>",
    )));
    assert_eq!(doc.tempos[0].quarter_bpm, r("241/4"));
    assert_eq!(doc.metronomes[0].value.bpm, r("96"));
    let text = SfsAdapter.write(&doc).unwrap().value;
    assert!(text.contains("tempo=q:241/4"));
    assert!(text.contains("metronome beat=h. bpm=96"));
    close(plan_source(&text).unwrap().duration_seconds, 960.0 / 241.0);
}
#[test]
fn printed_mark_alone_is_visual_and_uses_existing_default_playback_tempo() {
    let doc = common(&full(&mark("eighth", "", "116", "", "")));
    assert!(doc.tempos.is_empty());
    let text = SfsAdapter.write(&doc).unwrap().value;
    let plan = plan_source(&text).unwrap();
    close(plan.duration_seconds, 2.0);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "metronome_visual_only")
    );
}
#[test]
fn direction_offsets_default_to_visual_only_and_yes_shifts_sound() {
    for (attribute, sounding) in [("", "0"), (" sound=\"no\"", "0"), (" sound=\"yes\"", "1/2")] {
        let doc = common(&full(&mark(
            "quarter",
            "",
            "80",
            &format!("<offset{attribute}>2</offset>"),
            "<sound tempo=\"60\"/>",
        )));
        assert_eq!(doc.metronomes[0].at, r("1/2"));
        assert_eq!(doc.tempos[0].at, r(sounding));
        close(
            plan_source(&SfsAdapter.write(&doc).unwrap().value)
                .unwrap()
                .duration_seconds,
            if sounding == "0" { 4.0 } else { 3.75 },
        );
    }
}
#[test]
fn sound_child_offset_overrides_direction_and_uses_current_cursor() {
    let direction = mark(
        "quarter",
        "",
        "80",
        "<offset sound=\"yes\">-2</offset>",
        "<sound tempo=\"60\"><offset>-1</offset></sound>",
    );
    let doc = common(&score(&format!(
        "{}{direction}{}",
        note("a", "4"),
        note("b", "12")
    )));
    assert_eq!(doc.metronomes[0].at, r("1/2"));
    assert_eq!(doc.tempos[0].at, r("3/4"));
}
#[test]
fn standalone_sound_offsets_follow_backup_cursor_and_do_not_move_notes() {
    let doc = common(&score(&format!(
        "{}<backup><duration>16</duration></backup><sound tempo=\"90\"><offset>2</offset></sound>",
        note("n", "16")
    )));
    assert_eq!(doc.tempos[0].at, r("1/2"));
    assert_eq!(doc.parts[0].measures[0].events[0].offset, r("0"));
    assert_eq!(doc.parts[0].measures[0].length, r("4"));
}
#[test]
fn pickup_and_tempo_changes_inside_held_note_and_rest_reach_real_plan() {
    let xml = include_str!("fixtures/tempo-pickup.musicxml");
    let doc = common(xml);
    assert_eq!(
        doc.tempos.iter().map(|t| t.at.clone()).collect::<Vec<_>>(),
        [r("0"), r("1"), r("5/2")]
    );
    let text = SfsAdapter.write(&doc).unwrap().value;
    let p = plan_source(&text).unwrap();
    close(p.duration_seconds, 2.0625);
    close(p.measures[0].end_seconds, 0.5);
    close(p.measures[1].start_seconds, 0.5);
    close(p.measures[1].end_seconds, 2.0625);
    for (event, (start, end)) in
        p.event_positions
            .iter()
            .zip([(0.0, 0.5), (0.5, 1.0625), (1.0625, 2.0625)])
    {
        close(event.start_seconds, start);
        close(event.end_seconds, end);
    }
    close(p.notes[1].start_seconds, 0.5);
    // Existing ordinary gate is 0.92 of written duration, not a tempo-map omission.
    close(p.notes[1].duration_seconds, 0.4825);
}
#[test]
fn simultaneous_global_tempos_merge_only_when_identical_in_all_parts() {
    let first = full("<sound tempo=\"60\"/>");
    for (second_rate, valid) in [("60.0", true), ("90", false)] {
        let second_decl = "<score-part id=\"p2\"><part-name>piano</part-name></score-part>";
        let part_start = first.find("<part id=").unwrap();
        let part_end = first.find("</part>").unwrap() + 7;
        let second = first[part_start..part_end]
            .replace("id=\"p\"", "id=\"p2\"")
            .replace("id=\"m\"", "id=\"m2\"")
            .replace("id=\"n\"", "id=\"n2\"")
            .replace("tempo=\"60\"", &format!("tempo=\"{second_rate}\""));
        let source = first
            .replace("</part-list>", &format!("{second_decl}</part-list>"))
            .replace("</score-partwise>", &format!("{second}</score-partwise>"));
        if valid {
            assert_eq!(common(&source).tempos.len(), 1);
        } else {
            error(&source, "tempo.conflict");
        }
    }
    error(
        &full("<sound tempo=\"60\"/><sound tempo=\"90\"/>"),
        "tempo.conflict",
    );
}
#[test]
fn tempo_and_metronome_roundtrip_musicxml_sfs_common_without_merging_lanes() {
    let xml = full(&mark(
        "eighth",
        "<beat-unit-dot/>",
        "116.5",
        "<offset sound=\"yes\">1</offset>",
        "<sound tempo=\"60.125\"/>",
    ));
    let first = common(&xml);
    let sfs = SfsAdapter.write(&first).unwrap().value;
    let mut from_sfs = SfsAdapter.read(&sfs).unwrap().value;
    from_sfs.parts[0].name = first.parts[0].name.clone();
    from_sfs.metadata.source.clear();
    assert_eq!(first, from_sfs);
    let output = MusicXml.write(&first).unwrap().value;
    let mut second = common(&output);
    second.metadata.source.clear();
    assert_eq!(first, second);
    assert_eq!(
        plan_source(&sfs).unwrap(),
        plan_source(&SfsAdapter.write(&second).unwrap().value).unwrap()
    );
}
#[test]
fn sfs_dotted_beat_playback_normalizes_to_exact_quarter_bpm() {
    let source = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=2\n= score tempo=h.:96\n+ p.s1 metronome beat=e. bpm=233/2\nv C4:2 id=n\n.\n!end at=2\n";
    let doc = SfsAdapter.read(source).unwrap().value;
    assert_eq!(doc.tempos[0].quarter_bpm, r("288"));
    assert_eq!(doc.metronomes[0].staff, Some(1));
    assert_eq!(doc.metronomes[0].value.bpm, r("233/2"));
    let output = MusicXml.write(&doc).unwrap().value;
    let again = common(&output);
    assert_eq!(doc.tempos, again.tempos);
    assert_eq!(doc.metronomes, again.metronomes);
    close(
        plan_source(&SfsAdapter.write(&again).unwrap().value)
            .unwrap()
            .duration_seconds,
        5.0 / 12.0,
    );
}
#[test]
fn unsupported_tempo_semantics_fail_closed() {
    for (extra, feature) in [
        ("<words>Allegro</words>", "element.words"),
        (
            "<metronome-relation>equals</metronome-relation>",
            "element.metronome-relation",
        ),
    ] {
        error(
            &full(
                &mark("quarter", "", "100", "", "<sound tempo=\"100\"/>")
                    .replace("</metronome>", &format!("{extra}</metronome>")),
            ),
            feature,
        );
    }
    error(
        &full(&mark(
            "quarter",
            "",
            "100",
            "",
            "<sound tempo=\"100\" pan=\"50\"/>",
        )),
        "attribute.pan",
    );
    error(
        &full(&mark(
            "quarter",
            "",
            "100",
            "",
            "<sound tempo=\"100\" time-only=\"2\"/>",
        )),
        "attribute.time-only",
    );
    error(
        &full(&mark(
            "quarter",
            "<beat-unit-dot/><beat-unit-dot/>",
            "100",
            "",
            "",
        )),
        "metronome.dots",
    );
    error(&full(&mark("32nd", "", "100", "", "")), "metronome.beat");
    error(
        &full(
            &mark("quarter", "", "100", "", "")
                .replace("<metronome>", "<metronome parentheses=\"yes\">"),
        ),
        "attribute.parentheses",
    );
    error(
        &full(
            &mark("quarter", "", "100", "", "")
                .replace("<direction>", "<direction placement=\"above\">"),
        ),
        "attribute.placement",
    );
    error(
        &full(
            &mark("quarter", "", "100", "", "")
                .replace("<direction>", "<direction system=\"only-top\">"),
        ),
        "attribute.system",
    );
    error(
        &full(&mark(
            "quarter",
            "",
            "100",
            "<offset sound=\"maybe\">1</offset>",
            "",
        )),
        "boolean",
    );
}
#[test]
fn invalid_or_ambiguous_rates_offsets_and_scopes_are_rejected() {
    for rate in ["0", "-1", "1e2", "NaN", "120-132", "c. 108"] {
        assert!(
            MusicXml
                .read(&full(&mark("quarter", "", rate, "", "")))
                .is_err()
        );
    }
    error(&full("<sound tempo=\"0\"/>"), "tempo.rate");
    error(&full("<sound/>"), "sound.empty");
    for offset in ["-1", "16", "17"] {
        error(
            &full(&mark(
                "quarter",
                "",
                "100",
                &format!("<offset>{offset}</offset>"),
                "",
            )),
            "tempo.measure_boundary",
        );
    }
    error(
        &full(&mark("quarter", "", "100", "<staff>2</staff>", "")),
        "staff.number",
    );
    error(
        &full("<sound tempo=\"100\"><offset sound=\"no\">1</offset></sound>"),
        "sound.offset_attribute",
    );
    let mut model = common(&full("<sound tempo=\"60\"/>"));
    model.tempos.push(ScoreTempo {
        at: r("0"),
        quarter_bpm: r("120"),
    });
    assert!(validate_score(&model).has_errors());
    assert!(SfsAdapter.write(&model).is_err());
    assert!(MusicXml.write(&model).is_err());
}
#[test]
fn xml_export_never_rounds_non_decimal_tempos() {
    let mut doc = common(&full("<sound tempo=\"60\"/>"));
    doc.tempos[0].quarter_bpm = r("100/3");
    assert!(SfsAdapter.write(&doc).is_ok());
    let error = MusicXml.write(&doc).unwrap_err();
    assert!(
        error
            .diagnostics
            .iter()
            .any(|d| d.feature == "tempo.decimal")
    );
}
#[test]
fn legacy_score_json_defaults_new_lanes_without_losing_strict_unknown_fields() {
    let model = common(&full(""));
    let mut json = serde_json::to_value(&model).unwrap();
    assert!(json.get("tempos").is_none());
    assert!(json.get("metronomes").is_none());
    assert_eq!(
        serde_json::from_value::<scorestream_core::score::ScoreDocument>(json.clone()).unwrap(),
        model
    );
    json["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).is_err());
}

#[test]
fn fractional_pickup_start_is_included_when_scaling_exported_offsets() {
    let source = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p pickup len=1/3 implicit=true\nv C4:1/3 id=a\n.\n@1/3\n| p m len=1\nv D4:1 id=b\n.\n@1\n= score tempo=q:60\n+ p metronome beat=q bpm=60\n.\n!end at=4/3\n";
    let doc = SfsAdapter.read(source).unwrap().value;
    let xml = MusicXml.write(&doc).unwrap().value;
    assert!(xml.contains("<offset>2</offset>"));
    let again = common(&xml);
    assert_eq!(doc.tempos, again.tempos);
    assert_eq!(doc.metronomes, again.metronomes);
    close(
        plan_source(&SfsAdapter.write(&again).unwrap().value)
            .unwrap()
            .duration_seconds,
        5.0 / 6.0,
    );
}
#[test]
fn semantic_tempo_mapping_is_identical_with_tiny_streaming_buffers() {
    use std::io::{BufReader, Cursor};
    let xml = full(&mark(
        "quarter",
        "<beat-unit-dot/>",
        "90.5",
        "<offset sound=\"yes\">2</offset>",
        "<sound tempo=\"135.75\"/>",
    ));
    let streamed = scorestream_formats::musicxml::read_reader(BufReader::with_capacity(
        2,
        Cursor::new(xml.as_bytes()),
    ))
    .unwrap()
    .value;
    assert_eq!(common(&xml), streamed);
}
#[test]
fn unsupported_sfs_display_fields_and_scopes_are_not_silently_discarded() {
    let base = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=n\n+ TARGET metronome beat=q bpm=120 EXTRA\n.\n!end at=1\n";
    for (target, extra) in [
        ("score", ""),
        ("v", ""),
        ("#n", ""),
        ("p", "text=\"Allegro\""),
        ("p", "id=tempo_mark"),
    ] {
        assert!(
            SfsAdapter
                .read(&base.replace("TARGET", target).replace("EXTRA", extra))
                .is_err()
        );
    }
}
#[test]
fn offsets_without_divisions_and_malformed_cardinalities_fail_closed() {
    let source = full(&mark("quarter", "", "100", "<offset>1</offset>", ""));
    error(
        &source.replace("<divisions>4</divisions>", ""),
        "divisions.missing",
    );
    error(
        &full(&mark(
            "quarter",
            "",
            "100",
            "<offset>1</offset><offset>2</offset>",
            "",
        )),
        "xml.cardinality",
    );
    error(&full(&mark("quarter","","100","","").replace("</direction-type>","</direction-type><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>100</per-minute></metronome></direction-type>")),"xml.cardinality");
}

#[test]
fn nested_metronome_json_unknown_fields_cannot_silently_erase_display_semantics() {
    let model = common(&full(&mark("quarter", "", "100", "", "")));
    for field in ["text", "parentheses", "print_object"] {
        let mut json = serde_json::to_value(&model).unwrap();
        json["metronomes"][0]["value"][field] = serde_json::json!("Adagio");
        assert!(
            serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).is_err(),
            "accepted {field}"
        );
    }
    let mut json = serde_json::to_value(&model).unwrap();
    json["metronomes"][0]["value"]["beat"]["dot_count"] = serde_json::json!(2);
    assert!(serde_json::from_value::<scorestream_core::score::ScoreDocument>(json).is_err());
}
#[test]
fn all_supported_display_units_and_single_dots_roundtrip_without_changing_playback() {
    for unit in ["whole", "half", "quarter", "eighth", "16th"] {
        for dots in ["", "<beat-unit-dot/>"] {
            let model = common(&full(&mark(
                unit,
                dots,
                "75.5",
                "",
                "<sound tempo=\"111.125\"/>",
            )));
            let sfs = SfsAdapter.write(&model).unwrap().value;
            let parsed = SfsAdapter.read(&sfs).unwrap().value;
            assert_eq!(model.metronomes, parsed.metronomes);
            assert_eq!(model.tempos, parsed.tempos);
            let xml = MusicXml.write(&parsed).unwrap().value;
            assert_eq!(model.metronomes, common(&xml).metronomes);
            close(plan_source(&sfs).unwrap().duration_seconds, 240.0 / 111.125);
        }
    }
}
