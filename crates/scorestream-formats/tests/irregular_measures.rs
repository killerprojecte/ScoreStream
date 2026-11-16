use scorestream_core::{
    Rational,
    notation::{EventRole, lower_document},
    score::{Meter, NoteSize},
};
use scorestream_formats::{
    MappingEffect, ScoreAdapter,
    musicxml::{self, IrregularMeasure, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        geometry: Default::default(),
        articulations: Default::default(),
        irregular_measures: vec![IrregularMeasure {
            ordinal: 1,
            label: "268".into(),
            written_meter: Meter {
                symbol: None,
                beats: vec![2],
                beat_type: 4,
            },
            actual_length: r("10"),
        }],
    }
}
/// A deliberately small semantic fixture, not a substitute for the original score.
fn fixture() -> String {
    let declarations=(1..=2).map(|i| format!(r#"<score-part id="p{i}"><part-name>Oboe {i}</part-name><score-instrument id="i{i}"><instrument-name>oboe</instrument-name></score-instrument></score-part>"#)).collect::<String>();
    let notes=(0..10).map(|_| r#"<note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type size="cue">quarter</type></note>"#.to_owned()).collect::<String>();
    let parts=(1..=2).map(|i| format!(r#"<part id="p{i}"><measure number="268"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>{notes}</measure><measure number="269"><note><rest/><duration>2</duration><voice>1</voice><type>half</type></note></measure></part>"#)).collect::<String>();
    format!(
        r#"<score-partwise version="4.0"><part-list>{declarations}</part-list>{parts}</score-partwise>"#
    )
}
#[test]
fn explicit_extent_preserves_meter_numbering_all_events_and_next_boundary() {
    let err = MusicXml.read(&fixture()).unwrap_err();
    assert!(
        err.diagnostics
            .iter()
            .any(|d| d.feature == "measure.overfull"
                && d.message.contains("10")
                && d.message.contains("2"))
    );
    let mapped = musicxml::read_with_options(&fixture(), &options()).unwrap();
    assert_eq!(
        mapped
            .report
            .diagnostics
            .iter()
            .filter(|d| d.effect == MappingEffect::PolicyApplied)
            .count(),
        2
    );
    assert!(!mapped.report.has_errors());
    for p in &mapped.value.parts {
        assert_eq!(p.measures[0].length, r("10"));
        assert_eq!(p.measures[1].start, r("10"));
        assert_eq!(p.measures[1].length, r("2"));
        assert_eq!(
            p.measures[0].states[0].meter.as_ref().unwrap(),
            &options().irregular_measures[0].written_meter
        );
        assert!(!p.measures[0].implicit);
        assert_eq!(p.measures[0].label, "268");
        assert_eq!(p.measures[0].events.len(), 10);
        assert!(
            p.measures[0]
                .events
                .iter()
                .all(|e| e.written.as_ref().unwrap().size == Some(NoteSize::Cue))
        );
    }
    let text = SfsAdapter.write(&mapped.value).unwrap().value;
    assert!(text.contains("len=10"));
    assert!(text.contains("meter=2/4"));
    assert_eq!(text.matches("note_size=cue").count(), 20);
    assert!(!text.contains("kind=cue"));
    let restored = SfsAdapter.read(&text).unwrap().value;
    assert_eq!(mapped.value, restored);
    assert!(
        MusicXml
            .write(&restored)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.length")
    );
    let xml = musicxml::write_with_options(&restored, &options())
        .unwrap()
        .value;
    assert!(!xml.contains("implicit=\"yes\""));
    assert_eq!(xml.matches("<type size=\"cue\">quarter</type>").count(), 20);
    assert!(MusicXml.read(&xml).is_err());
    assert_eq!(
        musicxml::read_with_options(&xml, &options()).unwrap().value,
        restored
    );
}
#[test]
fn cue_size_reaches_real_playback_as_normal_notes_with_exact_boundaries() {
    let score = musicxml::read_with_options(&fixture(), &options())
        .unwrap()
        .value;
    let text = SfsAdapter.write(&score).unwrap().value;
    let doc = scorestream_codec::parse(&text).unwrap();
    let notation = lower_document(&doc).unwrap();
    assert!(notation.events.iter().all(|e| e.role == EventRole::Note));
    let plan = scorestream_app::playback::plan_source(&text).unwrap();
    assert_eq!(plan.notes.len(), 20);
    assert_eq!(plan.duration_seconds, 6.0);
    assert_eq!(plan.measures.len(), 4);
    assert_eq!(plan.measures[0].end_seconds, 5.0);
    assert_eq!(plan.measures[2].start_seconds, 5.0);
    let ordinary =
        scorestream_app::playback::plan_source(&text.replace(" note_size=cue", "")).unwrap();
    assert_eq!(
        serde_json::to_value(plan).unwrap(),
        serde_json::to_value(ordinary).unwrap()
    );
}
#[test]
fn exact_policy_rejects_changed_multipart_extent_label_meter_and_implicit_status() {
    for source in [
        fixture().replacen("<duration>1</duration>", "<duration>2</duration>", 1),
        fixture().replacen("number=\"268\"", "number=\"other\"", 1),
        fixture().replacen("<beats>2</beats>", "<beats>3</beats>", 1),
        fixture().replacen("number=\"268\"", "number=\"268\" implicit=\"yes\"", 1),
        fixture().replacen(
            "<time><beats>2</beats><beat-type>4</beat-type></time>",
            "",
            1,
        ),
    ] {
        let err = musicxml::read_with_options(&source, &options()).unwrap_err();
        assert!(
            err.diagnostics
                .iter()
                .any(|d| d.feature == "measure.extent_policy_mismatch"),
            "{err:?}"
        );
    }
    let mut opts = options();
    opts.irregular_measures[0].ordinal = 3;
    // Give the first bar nominal-compatible content so an unused policy is reached.
    let source = fixture().replace("<beats>2</beats>", "<beats>10</beats>");
    assert!(
        musicxml::read_with_options(&source, &opts)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.extent_policy_unused")
    );
}
#[test]
fn no_blanket_length_switch_or_unmatched_shortening() {
    for (ordinal, beats, unit, length) in [
        (0, 2, 4, "10"),
        (1, 0, 4, "10"),
        (1, 2, 0, "10"),
        (1, 2, 4, "2"),
        (1, 2, 4, "0"),
    ] {
        let mut opts = options();
        let e = &mut opts.irregular_measures[0];
        e.ordinal = ordinal;
        e.written_meter.beats = vec![beats];
        e.written_meter.beat_type = unit;
        e.actual_length = r(length);
        assert!(
            musicxml::read_with_options(&fixture(), &opts)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "measure.extent_policy")
        );
    }
    let mut short = options();
    short.irregular_measures[0].actual_length = r("1");
    assert!(
        musicxml::read_with_options(&fixture(), &short)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.extent_policy_mismatch")
    );
    let mut opts = options();
    opts.irregular_measures
        .push(opts.irregular_measures[0].clone());
    assert!(musicxml::read_with_options(&fixture(), &opts).is_err());
    let mut score = musicxml::read_with_options(&fixture(), &options())
        .unwrap()
        .value;
    score.parts[1].measures[0].label = "stale".into();
    assert!(
        musicxml::write_with_options(&score, &options())
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.extent_policy_mismatch")
    );
}
#[test]
fn all_graphic_size_values_roundtrip_but_silent_cues_and_graces_still_reject() {
    for (spelling, size) in [
        ("full", NoteSize::Full),
        ("cue", NoteSize::Cue),
        ("grace-cue", NoteSize::GraceCue),
        ("large", NoteSize::Large),
    ] {
        let source = fixture().replace("size=\"cue\"", &format!("size=\"{spelling}\""));
        let a = musicxml::read_with_options(&source, &options())
            .unwrap()
            .value;
        assert_eq!(
            a.parts[0].measures[0].events[0]
                .written
                .as_ref()
                .unwrap()
                .size,
            Some(size)
        );
        let text = SfsAdapter.write(&a).unwrap().value;
        assert_eq!(SfsAdapter.read(&text).unwrap().value, a);
        let xml = musicxml::write_with_options(&a, &options()).unwrap().value;
        assert_eq!(
            musicxml::read_with_options(&xml, &options()).unwrap().value,
            a
        );
    }
    for extra in ["<cue/>", "<grace/>", "<unknown/>"] {
        assert!(
            musicxml::read_with_options(
                &fixture().replace("<note>", &format!("<note>{extra}")),
                &options()
            )
            .is_err()
        );
    }
    assert!(
        musicxml::read_with_options(
            &fixture().replace("size=\"cue\"", "size=\"tiny\""),
            &options()
        )
        .is_err()
    );
    let absent = musicxml::read_with_options(&fixture().replace(" size=\"cue\"", ""), &options())
        .unwrap()
        .value;
    assert!(
        absent.parts[0].measures[0].events[0]
            .written
            .as_ref()
            .unwrap()
            .size
            .is_none()
    );
    let text = SfsAdapter.write(&absent).unwrap().value;
    assert!(!text.contains("note_size="));
    assert!(scorestream_codec::parse(&text.replace(" w=4", " note_size=cue")).is_err());
}

#[test]
fn policy_reports_do_not_hide_errors_after_diagnostic_truncation() {
    use scorestream_formats::{Format, MappingReport};
    for error_first in [false, true] {
        let mut report = MappingReport::default();
        if error_first {
            report.push(
                Format::MusicXml,
                MappingEffect::Invalid,
                "test.invalid",
                "/",
                "invalid",
            );
        }
        for _ in 0..2000 {
            report.push(
                Format::MusicXml,
                MappingEffect::PolicyApplied,
                "measure.irregular_extent",
                "/",
                "exact extent retained",
            );
        }
        if !error_first {
            report.push(
                Format::MusicXml,
                MappingEffect::Unsupported,
                "test.unsupported",
                "/",
                "unsupported",
            );
        }
        for _ in 0..2000 {
            report.push(
                Format::MusicXml,
                MappingEffect::PolicyApplied,
                "measure.irregular_extent",
                "/",
                "exact extent retained",
            );
        }
        assert!(report.has_errors());
        assert_eq!(report.diagnostics.len(), 1024);
        assert!(report.finish(()).is_err());
    }
}
#[test]
fn irregular_length_is_not_inherited_and_export_contract_cannot_be_stale() {
    let source = fixture().replace(
        "number=\"269\"><note><rest/><duration>2",
        "number=\"269\"><note><rest/><duration>3",
    );
    assert!(
        musicxml::read_with_options(&source, &options())
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.overfull")
    );
    let score = musicxml::read_with_options(&fixture(), &options())
        .unwrap()
        .value;
    let mut opts = options();
    opts.irregular_measures.push(IrregularMeasure {
        ordinal: 3,
        ..opts.irregular_measures[0].clone()
    });
    assert!(
        musicxml::write_with_options(&score, &opts)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.extent_policy_unused")
    );
    let mut changed = score.clone();
    changed.parts[1].measures[0].states[0].meter = Some(Meter {
        symbol: None,
        beats: vec![1],
        beat_type: 2,
    });
    assert!(
        musicxml::write_with_options(&changed, &options())
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.extent_policy_mismatch")
    );
    let mut changed = score.clone();
    changed.parts[1].staves = 2;
    assert!(
        musicxml::write_with_options(&changed, &options())
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "measure.extent_policy_mismatch")
    );
}
#[test]
fn graphic_size_survives_workspace_view_pitch_edit_and_undo() {
    let score = musicxml::read_with_options(&fixture(), &options())
        .unwrap()
        .value;
    let text = SfsAdapter.write(&score).unwrap().value;
    let mut workspace = scorestream_app::Workspace::from_source(&text).unwrap();
    let view = workspace.current();
    let id = view.events[0]["id"].as_str().unwrap();
    assert_eq!(view.events[0]["attributes"]["note_size"], "cue");
    let changed = workspace
        .set_event_pitch(id, "E5", &view.revision_id)
        .unwrap();
    assert_eq!(changed.events[0]["attributes"]["note_size"], "cue");
    assert_eq!(workspace.undo().unwrap().source, text);
}

#[test]
fn repeated_irregular_measure_uses_actual_extent_and_keeps_sounding_cue_size() {
    let source=fixture().replace("</measure><measure number=\"269\">","<barline location=\"right\"><repeat direction=\"backward\" times=\"2\"/></barline></measure><measure number=\"269\">");
    let score = musicxml::read_with_options(&source, &options())
        .unwrap()
        .value;
    let text = SfsAdapter.write(&score).unwrap().value;
    let plan = scorestream_app::playback::plan_source(&text).unwrap();
    assert_eq!(plan.notes.len(), 40);
    assert_eq!(plan.duration_seconds, 11.0);
    assert_eq!(plan.measures.len(), 6);
    let starts: Vec<_> = plan
        .measures
        .iter()
        .filter(|m| m.part == "p1")
        .map(|m| m.start_seconds)
        .collect();
    assert_eq!(starts, [0.0, 5.0, 10.0]);
    assert_eq!(SfsAdapter.read(&text).unwrap().value, score);
}

#[test]
fn exact_short_contract_preserves_authored_forward_meter_and_next_boundary() {
    let declarations = (1..=2).map(|i|format!(r#"<score-part id="p{i}"><part-name>Piano</part-name><score-instrument id="i{i}"><instrument-name>piano</instrument-name></score-instrument></score-part>"#)).collect::<String>();
    let parts = (1..=2).map(|i|format!(r#"<part id="p{i}"><measure number="141"><attributes><divisions>2</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><note><rest/><duration>2</duration><voice>1</voice><type>quarter</type></note><forward><duration>2</duration></forward></measure><measure number="142"><note><rest/><duration>6</duration><voice>1</voice><type>half</type><dot/></note></measure></part>"#)).collect::<String>();
    let source = format!(
        r#"<score-partwise version="4.0"><part-list>{declarations}</part-list>{parts}</score-partwise>"#
    );
    let mut opts = options();
    let e = &mut opts.irregular_measures[0];
    e.label = "141".into();
    e.written_meter.beats = vec![3];
    e.actual_length = r("2");
    let ordinary = MusicXml.read(&source).unwrap().value;
    assert_eq!(ordinary.parts[0].measures[0].length, r("3"));
    let short = musicxml::read_with_options(&source, &opts).unwrap().value;
    for part in &short.parts {
        assert_eq!(part.measures[0].length, r("2"));
        assert_eq!(part.measures[1].start, r("2"));
        assert!(!part.measures[0].implicit);
        assert_eq!(
            part.measures[0].states[0].meter.as_ref().unwrap().beats,
            vec![3]
        );
    }
    let native = SfsAdapter.write(&short).unwrap().value;
    let restored = SfsAdapter.read(&native).unwrap().value;
    assert_eq!(short, restored);
    assert!(MusicXml.write(&restored).is_err());
    let xml = musicxml::write_with_options(&restored, &opts)
        .unwrap()
        .value;
    assert!(xml.contains("<forward><duration>1</duration>"));
    assert!(!xml.contains("implicit=\"yes\""));
    assert_eq!(
        musicxml::read_with_options(&xml, &opts).unwrap().value,
        short
    );
    // The contract measures authored forward, not just the last note's end.
    for bad in [
        source.replacen("<forward><duration>2</duration></forward>", "", 1),
        source.replacen(
            "<forward><duration>2</duration></forward>",
            "<forward><duration>4</duration></forward>",
            1,
        ),
        source.replacen("<beats>3</beats>", "<beats>4</beats>", 1),
    ] {
        assert!(
            musicxml::read_with_options(&bad, &opts)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "measure.extent_policy_mismatch")
        );
    }
}
