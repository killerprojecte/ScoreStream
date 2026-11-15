use scorestream_core::{
    Record, Scalar, Severity, navigation_extension as ext, repeat_traversal as route, score::*,
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, navigation, sfs::SfsAdapter};
const XML: &str = include_str!("fixtures/navigation-closed.musicxml");
fn source() -> String {
    SfsAdapter
        .write(&MusicXml.read(XML).unwrap().value)
        .unwrap()
        .value
}
fn has(report: &scorestream_formats::MappingReport, feature: &str) {
    assert!(
        report.diagnostics.iter().any(|d| d.feature == feature),
        "{report:?}"
    );
}
#[test]
fn actual_xml_common_sfs_common_xml_preserves_closed_authored_repeat() {
    let common = MusicXml.read(XML).unwrap().value;
    let written = SfsAdapter.write(&common).unwrap();
    assert!(written.value.contains("!requires scorestream.navigation@1"));
    assert!(written.value.contains("source_id=\"repeat.head\""));
    assert!(written.value.contains("ending_y=777/10"));
    let recovered = SfsAdapter.read(&written.value).unwrap().value;
    assert_eq!(common, recovered);
    assert_eq!(
        navigation::plan(&common, "same", route::Policy::default()).unwrap(),
        navigation::plan(&recovered, "same", route::Policy::default()).unwrap()
    );
    assert_eq!(
        scorestream_codec::parse(&written.value)
            .unwrap()
            .event_count(),
        4,
        "No performed note duplication in authoring format"
    );
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&recovered).unwrap().value)
            .unwrap()
            .value,
        common
    );
    has(&written.report, "navigation.authored_only");
    let doc = scorestream_codec::parse(&written.value).unwrap();
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.code == "navigation.realization_unavailable"
                && d.severity == Severity::Warning)
    );
    assert!(
        scorestream_core::notation::lower_document(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "navigation.replay_unsupported")
    );
    assert!(
        scorestream_app::playback::plan_source(&written.value)
            .unwrap_err()
            .contains("navigation.replay_unsupported")
    );
}
#[test]
fn missing_unknown_duplicate_or_empty_capability_cannot_be_accepted() {
    let s = source();
    for text in [
        s.replace("!requires scorestream.navigation@1\n", ""),
        s.replace("navigation@1", "navigation@2"),
        s.replace(
            "!requires scorestream.navigation@1",
            "!requires scorestream.navigation@1\n!requires scorestream.navigation@1",
        ),
    ] {
        assert!(scorestream_codec::parse(&text).is_err());
    }
    let empty = "!score 1.1 unit=q\n!requires scorestream.navigation@1\n!end at=0\n";
    assert!(scorestream_codec::parse(empty).is_err());
    let mut doc = scorestream_codec::parse(&s).unwrap();
    doc.required_extensions.clear();
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.code == "navigation.requirement")
    );
}
#[test]
fn unknown_fields_bad_pairing_and_wrong_boundary_ownership_reject() {
    let s = source();
    for text in [
        s.replace("side=left", "side=middle"),
        s.replace("measure=m1", "measure=missing"),
        s.replace("+ P1 scorestream.barline", "+ score scorestream.barline"),
        s.replace("measure=m1", "measure=m2"),
        s.replace("side=left", "side=right"),
        s.replace("passes=\"1\"", "passes=\"1,1\""),
        s.replace("passes=\"1\"", "passes=\"0\""),
        s.replace("passes=\"1\"", "passes=\"1, 2\""),
        s.replace("repeat=forward", "repeat=forward times=2"),
        s.replace("repeat=backward", "times=2"),
        s.replace("ending=start", ""),
        s.replace("ending_y=777/10", "ending_y=not-a-rational"),
        s.replace("style=heavy-light", "style=heavy-light x.opaque=true"),
        s.replace("style=heavy-light", "style=heavy-light id=bar"),
        s.replace("source_id=\"repeat.head\"", "source_id=\"m1\""),
    ] {
        assert!(scorestream_codec::parse(&text).is_err(), "{text}");
    }
}
#[test]
fn unclosed_final_ending_is_inspectable_but_strict_conversion_and_playback_reject() {
    let xml = XML.replace(
        "<barline location=\"right\"><ending number=\"2\" type=\"discontinue\"/></barline>",
        "",
    );
    let common = MusicXml.read(&xml).unwrap().value;
    let report = SfsAdapter.write(&common).unwrap_err();
    has(&report, "navigation.route_policy");
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("UnclosedFinalEnding"))
    );
    let s = source()
        .lines()
        .filter(|line| !line.contains("ending=discontinue"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let inspected = scorestream_codec::parse(&s).unwrap();
    assert!(
        inspected
            .diagnostics
            .iter()
            .any(|d| d.code == "navigation.unclosed_ending")
    );
    has(&SfsAdapter.read(&s).unwrap_err(), "navigation.route_policy");
    assert!(scorestream_app::playback::plan_source(&s).is_err());
}
#[test]
fn duplicate_positions_and_conflicting_parts_never_silently_normalize() {
    let mut doc = scorestream_codec::parse(&source()).unwrap();
    let bar = doc.frames[0]
        .records
        .iter()
        .find(|r| matches!(r,Record::Mark(m)if m.kind==ext::KIND))
        .unwrap()
        .clone();
    doc.frames[0].records.push(bar);
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.code == "navigation.duplicate")
    );
    assert!(scorestream_formats::sfs::from_document(&doc).is_err());
    let mut common = MusicXml.read(XML).unwrap().value;
    common
        .barlines
        .iter_mut()
        .find_map(|b| {
            b.repeat
                .as_mut()
                .filter(|r| r.direction == RepeatDirection::Backward)
        })
        .unwrap()
        .times = Some(3);
    has(
        &SfsAdapter.write(&common).unwrap_err(),
        "navigation.route_policy",
    );
}
#[test]
fn exact_display_text_identity_and_alias_mapping_survive() {
    let mut common = MusicXml.read(XML).unwrap().value;
    common
        .barlines
        .iter_mut()
        .find_map(|b| b.ending.as_mut())
        .unwrap()
        .text = "first\r\n\t\u{7f}\u{85} \"quoted\" 中文".into();
    let s = SfsAdapter.write(&common).unwrap().value;
    assert!(s.contains("\\u007f") && s.contains("\\u0085"));
    assert_eq!(common, SfsAdapter.read(&s).unwrap().value);
    common.parts[0].measures[0].id = "_source.m1".into();
    common.barlines[0].measure = "_source.m1".into();
    common.barlines[0].id = Some("measure1".into());
    let s = SfsAdapter.write(&common).unwrap().value;
    let new = SfsAdapter.read(&s).unwrap().value;
    assert_eq!(new.barlines[0].id, Some("measure1".into()));
    assert_eq!(new.barlines[0].measure, new.parts[0].measures[0].id);
    assert_ne!(new.parts[0].measures[0].id, "measure1");
}
#[test]
fn json_typed_scalars_and_nonempty_source_identity_fail_closed() {
    let original = scorestream_codec::parse(&source()).unwrap();
    for (key, value) in [
        ("side", Scalar::Integer(1)),
        ("source_id", Scalar::String("".into())),
        ("times", Scalar::Integer(-1)),
        ("passes", Scalar::Boolean(true)),
    ] {
        let mut doc = original.clone();
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                Record::Mark(m) if m.kind == ext::KIND => Some(m),
                _ => None,
            })
            .unwrap();
        mark.attributes.insert(key.into(), value);
        assert!(
            scorestream_core::validate(&doc, None)
                .iter()
                .any(|d| d.severity == Severity::Error)
        );
        assert!(scorestream_formats::sfs::from_document(&doc).is_err());
    }
}
#[test]
fn validation_counts_without_emitting_occurrences() {
    let movement = route::Movement {
        id: "count".into(),
        measure_labels: vec!["1".into()],
        repeats: vec![route::RepeatRegion {
            id: "r".into(),
            start: route::RepeatStart::Explicit(route::Boundary(0)),
            backward: route::Boundary(1),
            times: Some(1_000_000),
            endings: vec![],
        }],
    };
    let v = route::validate(&movement, route::Policy::default()).unwrap();
    assert_eq!(v.occurrences, 1_000_000);
    assert!(v.diagnostics.is_empty());
    assert_eq!(
        route::validate(
            &movement,
            route::Policy {
                maximum_occurrences: 999_999,
                ..route::Policy::default()
            }
        ),
        Err(route::Error::OccurrenceBudget)
    );
}

#[test]
fn generated_event_ids_reserve_authored_ids_in_both_parse_and_ast_bridge() {
    let s = source()
        .replace("source_id=\"repeat.head\"", "source_id=\"n1\"")
        .replace(" id=a ", " ");
    let parsed = scorestream_codec::parse(&s).unwrap();
    assert!(
        !scorestream_core::validate(&parsed, None)
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    assert_ne!(parsed.events().next().unwrap().1.id(), Some("n1"));
    let canonical = scorestream_codec::canonical(&parsed);
    assert!(scorestream_codec::parse(&canonical).is_ok());
    let mut ast = scorestream_codec::parse(&source()).unwrap();
    for r in ast.frames.iter_mut().flat_map(|f| &mut f.records) {
        match r {
            Record::Event(e) if e.id() == Some("a") => {
                e.attributes.remove("id");
            }
            Record::Mark(m) if m.kind == ext::KIND && m.attributes.contains_key("source_id") => {
                m.attributes
                    .insert("source_id".into(), Scalar::String("event1".into()));
            }
            _ => {}
        }
    }
    assert!(
        !scorestream_core::validate(&ast, None)
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    let common = scorestream_formats::sfs::from_document(&ast).unwrap().value;
    assert_ne!(common.parts[0].measures[0].events[0].id.0, "event1");
    assert_eq!(common.barlines[0].id.as_deref(), Some("event1"));
}
