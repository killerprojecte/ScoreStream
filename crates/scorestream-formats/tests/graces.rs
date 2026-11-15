use scorestream_core::{Rational, notation::EventId, score::*};
use scorestream_formats::{MappingEffect, ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn note(id: &str, grace: Option<&str>, pitch: &str) -> String {
    format!(
        "<note id=\"{id}\">{}<pitch><step>{pitch}</step><octave>5</octave></pitch>{}<voice>1</voice><type>{}</type></note>",
        grace.map(|g| format!("<grace{g}/>")).unwrap_or_default(),
        if grace.is_some() {
            ""
        } else {
            "<duration>4</duration>"
        },
        if grace.is_some() { "16th" } else { "quarter" }
    )
}
fn xml(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>4</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"#
    )
}
fn fixture() -> String {
    xml(&format!(
        "{}{}{}{}{}",
        note("a", Some(""), "D"),
        note("b", Some(" slash=\"yes\""), "E"),
        note("main", None, "F"),
        note("c", Some(" slash=\"no\""), "G"),
        note("next", None, "A")
    ))
}
#[test]
fn typed_graces_have_no_measured_duration_and_preserve_order_slash_and_boundaries() {
    let result = MusicXml.read(&fixture()).unwrap();
    assert!(
        result
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "grace.authored_only" && d.effect == MappingEffect::Preserved)
    );
    let a = result.value;
    assert_eq!(a.graces.len(), 3);
    assert_eq!(a.parts[0].measures[0].events.len(), 2);
    assert_eq!(a.parts[0].measures[0].length, r("2"));
    assert_eq!(
        a.graces
            .iter()
            .map(|g| (&g.id.0, &g.anchor.0, g.order, g.slash))
            .collect::<Vec<_>>(),
        [
            (&"a".into(), &"main".into(), -2, None),
            (&"b".into(), &"main".into(), -1, Some(true)),
            (&"c".into(), &"next".into(), -1, Some(false))
        ]
    );
    assert!(a.graces.iter().all(|g| g.timing == GraceTiming::default()));
    let wire = SfsAdapter.write(&a).unwrap().value;
    assert!(wire.contains("!requires scorestream.grace@1"));
    assert_eq!(wire.matches(":?").count(), 3);
    assert_eq!(wire.matches("kind=grace").count(), 3);
    assert!(wire.contains("slash=false"));
    let b = SfsAdapter.read(&wire).unwrap().value;
    assert_eq!(a, b);
    let out = MusicXml.write(&b).unwrap().value;
    assert_eq!(out.matches("<duration>").count(), 2);
    assert_eq!(out.matches("<grace").count(), 3);
    assert!(out.contains("<grace/>"));
    assert!(out.contains("<grace slash=\"no\"/>"));
    assert_eq!(MusicXml.read(&out).unwrap().value, a);
    assert!(
        !serde_json::to_value(&a.graces[0])
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("duration")
    );
}
#[test]
fn exact_authored_timing_is_retained_without_realizing_it() {
    for attrs in [
        " steal-time-previous=\"12.5\"",
        " steal-time-following=\"0\"",
        " steal-time-following=\"100\"",
        " make-time=\"1.5\"",
        " steal-time-previous=\"25\" steal-time-following=\"50\" make-time=\"0\"",
    ] {
        let source = xml(&format!(
            "{}{}{}",
            note("g", Some(attrs), "D"),
            note("main", None, "E"),
            note("tail", None, "F")
        ));
        let a = MusicXml.read(&source).unwrap().value;
        let wire = SfsAdapter.write(&a).unwrap().value;
        let b = SfsAdapter.read(&wire).unwrap().value;
        assert_eq!(a, b);
        let out = MusicXml.write(&b).unwrap().value;
        assert_eq!(MusicXml.read(&out).unwrap().value, a);
        assert!(
            scorestream_app::playback::plan_source(&wire)
                .unwrap_err()
                .contains("grace.performance_unsupported")
        );
    }
    let a = MusicXml
        .read(&xml(&format!(
            "{}{}",
            note("g", Some(" make-time=\"1.5\""), "D"),
            note("main", None, "E")
        )))
        .unwrap()
        .value;
    assert_eq!(a.graces[0].timing.make_time_quarters, Some(r("3/8")));
}
#[test]
fn new_fidelity_rejects_performance_while_legacy_sfs_grace_is_unchanged() {
    let score = MusicXml.read(&fixture()).unwrap().value;
    let source = SfsAdapter.write(&score).unwrap().value;
    assert!(
        scorestream_app::playback::plan_source(&source)
            .unwrap_err()
            .contains("grace.performance_unsupported")
    );
    let legacy = source
        .lines()
        .filter(|line| !line.contains("scorestream.grace"))
        .collect::<Vec<_>>()
        .join("\n");
    let plan = scorestream_app::playback::plan_source(&legacy).unwrap();
    assert_eq!(plan.notes.len(), 2);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "grace_unrealized")
    );
    // Conversion is stricter than legacy playback; it must not change the policy silently.
    assert!(
        SfsAdapter
            .read(&legacy)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "grace.required_fidelity")
    );
    for damaged in [
        source.replace("!requires scorestream.grace@1\n", ""),
        source.replace("+ #a scorestream.grace\n", ""),
        source.replace("+ #a scorestream.grace", "+ #main scorestream.grace"),
        source.replace("order=-2", "order=-4"),
    ] {
        assert!(scorestream_codec::parse(&damaged).is_err(), "{damaged}");
    }
}
#[test]
fn invalid_or_unsupported_grace_never_enters_the_common_model() {
    let valid = fixture();
    for source in [
        valid.replacen("<grace/>", "<grace/><duration>1</duration>", 1),
        valid.replacen("<grace/>", "<grace/><chord/>", 1),
        valid.replacen("<grace/>", "<grace/><cue/>", 1),
        valid.replacen("<grace/>", "<grace/><tie type=\"start\"/>", 1),
        valid.replacen("<grace/>", "<grace/><notations><fermata/></notations>", 1),
        valid.replacen("<grace/>", "<grace slash=\"maybe\"/>", 1),
        valid.replacen("<grace/>", "<grace steal-time-following=\"101\"/>", 1),
        valid.replacen("<grace/>", "<grace steal-time-previous=\"-1\"/>", 1),
        valid.replacen("<grace/>", "<grace make-time=\"-1\"/>", 1),
        valid.replacen("<grace/>", "<grace unknown=\"yes\"/>", 1),
        valid.replacen("<grace/>", "<grace><duration>1</duration></grace>", 1),
        valid.replacen("<voice>1</voice>", "<voice>2</voice>", 1),
        valid.replacen(
            "<note id=\"main\">",
            "<forward><duration>1</duration></forward><note id=\"main\">",
            1,
        ),
        xml(&note("unanchored", Some(""), "D")),
    ] {
        assert!(MusicXml.read(&source).is_err(), "{source}");
    }
    let missing = valid.replacen("<duration>4</duration>", "", 1);
    assert!(MusicXml.read(&missing).is_err());
}
#[test]
fn typed_input_cannot_forge_anchor_identity_order_or_timing() {
    let good = MusicXml.read(&fixture()).unwrap().value;
    let mut cases = vec![];
    let mut s = good.clone();
    s.graces[0].anchor = EventId("missing".into());
    cases.push(s);
    let mut s = good.clone();
    s.graces[0].id = EventId("main".into());
    cases.push(s);
    let mut s = good.clone();
    s.graces[0].order = 1;
    cases.push(s);
    let mut s = good.clone();
    s.graces[0].timing.steal_previous = Some(r("101"));
    cases.push(s);
    let mut s = good.clone();
    s.graces[0].pitch = scorestream_core::Pitch::Rest;
    cases.push(s);
    for s in cases {
        assert!(MusicXml.write(&s).is_err());
        assert!(SfsAdapter.write(&s).is_err());
    }
}
#[test]
fn unsupported_source_tags_are_not_removed_when_grace_is_mapped() {
    for tag in [
        "<stem default-y=\"1\">up</stem>",
        "<beam number=\"1\">begin</beam>",
        "<accidental>natural</accidental>",
        "<notations><slur type=\"start\"/></notations>",
    ] {
        assert!(
            MusicXml
                .read(&fixture().replacen("</type>", &format!("</type>{tag}"), 1))
                .is_err()
        );
    }
    assert!(
        MusicXml
            .read(&fixture().replacen("<note id=\"a\">", "<note id=\"a\" default-x=\"12\">", 1))
            .is_err()
    );
}

#[test]
fn xml_numeric_boundaries_reject_unreadable_grace_output_without_rounding() {
    let good = MusicXml.read(&fixture()).unwrap().value;
    for digits in [64, 65] {
        let huge = r(&format!("1{}", "0".repeat(digits - 1)));
        for timing in [false, true] {
            let mut score = good.clone();
            if timing {
                score.graces[0].timing.make_time_quarters = Some(huge.clone());
            } else if let scorestream_core::Pitch::Pitched { alteration, .. } =
                &mut score.graces[0].pitch
            {
                *alteration = huge.clone();
            }
            let result = MusicXml.write(&score);
            if digits == 64 {
                assert_eq!(MusicXml.read(&result.unwrap().value).unwrap().value, score);
            } else {
                assert!(result.is_err());
            }
        }
    }
    // Accepted source can still need a larger exact output divisions value.
    let source = fixture()
        .replacen(
            "<grace/>",
            &format!("<grace make-time=\"{}\"/>", "9".repeat(64)),
            1,
        )
        .replacen("<duration>4</duration>", "<duration>0.1</duration>", 1)
        .replacen("<duration>4</duration>", "<duration>7.9</duration>", 1);
    let source_model = MusicXml.read(&source).unwrap().value;
    assert!(
        MusicXml
            .write(&source_model)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "grace.make_time_precision")
    );
    // The exact emitted value, not just the unscaled quarters, must be bounded.
    let mut score = good;
    score.graces[0].timing.make_time_quarters = Some(r(&"9".repeat(64)));
    score.graces[1].timing.make_time_quarters = Some(r("1/2"));
    assert!(
        MusicXml
            .write(&score)
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "grace.make_time_precision")
    );
    assert_eq!(
        SfsAdapter
            .read(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .value,
        score
    );
}

#[test]
fn typed_workspace_keeps_unmeasured_graces_through_pitch_edit_undo_and_redo() {
    let score = MusicXml.read(&fixture()).unwrap().value;
    let source = SfsAdapter.write(&score).unwrap().value;
    let mut workspace = scorestream_app::Workspace::from_source(&source).unwrap();
    let view = workspace.current();
    assert!(view.valid);
    assert_eq!(
        view.events.iter().filter(|e| e["duration"] == "?").count(),
        3
    );
    assert_eq!(
        view.document["required_extensions"],
        serde_json::json!(["scorestream.instrument@1", "scorestream.grace@1"])
    );
    let changed = workspace
        .set_event_pitch("a", "D#5", &view.revision_id)
        .unwrap();
    let edited = SfsAdapter.read(&changed.source).unwrap().value;
    assert_eq!(edited.parts, score.parts);
    assert_eq!(edited.graces[0].anchor, score.graces[0].anchor);
    assert_eq!(edited.graces[0].written, score.graces[0].written);
    assert_eq!(edited.graces[0].slash, score.graces[0].slash);
    assert_eq!(edited.graces[0].timing, score.graces[0].timing);
    assert_ne!(edited.graces[0].pitch, score.graces[0].pitch);
    assert_eq!(
        SfsAdapter
            .read(&workspace.undo().unwrap().source)
            .unwrap()
            .value,
        score
    );
    assert_eq!(
        SfsAdapter
            .read(&workspace.redo().unwrap().source)
            .unwrap()
            .value,
        edited
    );
}
#[test]
fn preserved_reports_never_hide_blocking_errors_at_the_diagnostic_limit() {
    use scorestream_formats::{Format, MappingReport};
    for position in [None, Some(0), Some(1500), Some(2500)] {
        let mut report = MappingReport::default();
        for i in 0..3000 {
            report.push(
                Format::MusicXml,
                if position == Some(i) {
                    MappingEffect::Unsupported
                } else {
                    MappingEffect::Preserved
                },
                "test.grace",
                "/",
                "bounded report",
            );
        }
        assert_eq!(report.has_errors(), position.is_some());
        assert_eq!(report.diagnostics.len(), 1024);
        assert_eq!(report.finish(()).is_err(), position.is_some());
    }
}
#[test]
fn written_grace_rhythm_is_not_a_measured_duration_or_a_sounding_cue() {
    let source = fixture().replacen("<type>16th</type>",
        "<type size=\"grace-cue\">32nd</type><dot/><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes><normal-type>eighth</normal-type><normal-dot/></time-modification>", 1);
    let model = MusicXml.read(&source).unwrap().value;
    assert_eq!(
        model.graces[0].written.as_ref().unwrap().size,
        Some(NoteSize::GraceCue)
    );
    assert_eq!(model.parts[0].measures[0].events[0].offset, r("0"));
    let wire = SfsAdapter.write(&model).unwrap().value;
    assert!(wire.contains("note_size=grace-cue"));
    assert!(wire.contains("kind=grace"));
    assert!(!wire.contains("kind=cue"));
    let back = SfsAdapter.read(&wire).unwrap().value;
    assert_eq!(model, back);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&back).unwrap().value)
            .unwrap()
            .value,
        model
    );
    assert!(scorestream_app::playback::plan_source(&wire).is_err());
}
