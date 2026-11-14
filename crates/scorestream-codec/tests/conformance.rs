use scorestream_codec::*;
use serde_json::Value;
use std::{fs, path::PathBuf};
fn design() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../design")
}
fn context() -> FragmentContext {
    FragmentContext {
        bindings: vec![FragmentBinding {
            alias: "mel".into(),
            part: "p".into(),
            staff: 1,
            staves: 1,
        }],
    }
}
fn score(records: &str, end: &str) -> String {
    format!("!score 1.1 unit=q\n!part p instrument=piano staves=2\n!voice v part=p staff=1\n{records}!end at={end}\n")
}
fn has_error(source: &str, code: &str) {
    let errors = parse(source).expect_err("invalid source must fail");
    assert!(
        errors.iter().any(|e| e.code == code),
        "Expected {code}, got {errors:?}"
    );
    assert!(errors.iter().all(|e| e.source.line > 0));
}
#[test]
fn all_supplied_valid_fixtures_match_ast_and_roundtrip() {
    for name in [
        "document-metadata",
        "ensemble",
        "tuplets-and-ties",
        "replacement-fragment",
        "grace-percussion-cross-staff",
    ] {
        let source = fs::read_to_string(design().join(format!("examples/sfs/{name}.sfs"))).unwrap();
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(design().join(format!("examples/ast/{name}.json"))).unwrap(),
        )
        .unwrap();
        let document =
            parse_with_context(&source, &context()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            serde_json::to_value(&document).unwrap(),
            expected,
            "AST fixture {name}"
        );
        let text = canonical(&document);
        let second = parse_with_context(&text, &context()).unwrap();
        assert_eq!(canonical(&second), text, "canonical stability {name}");
        assert!(text.ends_with('\n'));
        assert!(text.lines().all(|line| !line.ends_with(' ')));
    }
}
#[test]
fn all_33_negative_fixtures_report_the_expected_stable_code() {
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(design().join("examples/sfs-invalid/manifest.json")).unwrap(),
    )
    .unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 33);
    for case in cases {
        let path = case["path"].as_str().unwrap();
        let code = case["expected_code"].as_str().unwrap().to_lowercase();
        let source = fs::read_to_string(design().join(path)).unwrap();
        let diagnostics = parse_with_context(&source, &context()).expect_err(path);
        assert!(
            diagnostics.iter().any(|d| d.code == code),
            "{path}: expected {code}, got {diagnostics:?}"
        );
    }
}
#[test]
fn fragment_context_is_required_and_not_inferred() {
    let fragment = "!fragment 1.1 unit=q span=1\n@0\nmel C4:1\n.\n!end at=1\n";
    has_error(fragment, "fragment_context_required");
    assert!(parse_with_context(fragment, &context()).is_ok());
    let empty = FragmentContext::default();
    assert!(parse_with_context(fragment, &empty)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "voice_undeclared"));
}
#[test]
fn no_implicit_rest_and_half_open_active_intervals() {
    let doc = parse(&score(
        "@0\nv C4:4 id=held\n.\n@1\nv E4:1 id=attack\n.\n",
        "4",
    ))
    .unwrap();
    assert_eq!(doc.event_count(), 2);
    assert_eq!(doc.active_events(&"3/2".parse().unwrap()).count(), 2);
    assert_eq!(doc.active_events(&"2".parse().unwrap()).count(), 1);
    assert_eq!(doc.active_events(&"4".parse().unwrap()).count(), 0);
}
#[test]
fn exact_huge_time_and_unbounded_octave_roundtrip() {
    let huge = "1000000000000000000000000000000000000000000000000000000000000000000000000000000";
    let end = "1000000000000000000000000000000000000000000000000000000000000000000000000000001";
    let src = score(
        &format!("@{huge}\nv C(+1/3)99999999999999999999999999999999:1\n.\n"),
        end,
    );
    let doc = parse(&src).unwrap();
    assert_eq!(doc.end.to_string(), end);
    assert_eq!(parse(&canonical(&doc)).unwrap().end, doc.end);
}
#[test]
fn typed_fields_and_json_unicode_strings() {
    let src = score("@0\nv C#4:1 id=a w=4 visible=false\n+ #a lyric verse=1 text=\"two words; #sharp \\uD834\\uDD1E \\\"quoted\\\"\"\n.\n","1");
    let doc = parse(&src).unwrap();
    let ast = serde_json::to_value(&doc).unwrap();
    assert_eq!(ast["frames"][0]["records"][0]["attributes"]["w"], "4");
    assert_eq!(
        ast["frames"][0]["records"][0]["attributes"]["visible"],
        false
    );
    assert_eq!(ast["frames"][0]["records"][1]["attributes"]["verse"], 1);
    assert_eq!(
        canonical(&parse(&canonical(&doc)).unwrap()),
        canonical(&doc)
    );
    let malformed = score("@0\nv C4:1 id=a\n+ #a words text=\"\\uD800\"\n.\n", "1");
    has_error(&malformed, "string_invalid");
}
#[test]
fn ids_are_assigned_deterministically_without_collisions() {
    let doc = parse(&score(
        "@0\nv C4:1 id=n1\nv E4:1\nv G4:1 id=n3\nv B4:1\n.\n",
        "1",
    ))
    .unwrap();
    assert_eq!(
        doc.events()
            .map(|(_, e)| e.id().unwrap())
            .collect::<Vec<_>>(),
        vec!["n1", "n2", "n3", "n4"]
    );
    assert_eq!(
        canonical(&doc),
        canonical(&parse(&canonical(&doc)).unwrap())
    );
    // Generated names must not make an unresolved authored reference valid.
    has_error(
        &score("@0\nv C4:1\n+ #n1 articulation value=staccato\n.\n", "1"),
        "reference_unresolved",
    );
}
#[test]
fn same_frame_forward_reference_and_state_order_are_valid() {
    let src = score(
        "@0\n+ #a articulation value=staccato\nv C4:1 id=a\n= v staff=2\n.\n",
        "1",
    );
    assert!(parse(&src).is_ok());
}
#[test]
fn crossing_relations_do_not_use_stack_nesting() {
    let src = score("@0\nv C4:1 id=a\n~ slur x start=#a\n.\n@1\nv D4:1 id=b\n~ slur y start=#b\n.\n@2\nv E4:1 id=c\n~ slur x end=#c\n.\n@3\nv F4:1 id=d\n~ slur y end=#d\n.\n","4");
    assert!(parse(&src).is_ok());
}
#[test]
fn unsupported_content_is_never_dropped() {
    has_error(
        &score("@0\nv C4:1 x.vendor.data=yes\n.\n", "1"),
        "extension_unsupported",
    );
    has_error("!score 2.0 unit=q\n!end at=0\n", "format_unsupported");
    has_error(
        "!score 1.1 unit=q\n!requires vendor.feature@1\n!end at=0\n",
        "extension_unsupported",
    );
    assert!(parse("!score 1.1 unit=q\n!end at=0\nBOGUS").is_err());
}
#[test]
fn rhythm_disagreement_is_preserved_with_warning() {
    let doc = parse(&score("@0\nv C4:1 id=a w=8\n.\n", "1")).unwrap();
    assert!(doc
        .diagnostics
        .iter()
        .any(|d| d.code == "written_duration_mismatch" && d.severity == Severity::Warning));
    let (_, event) = doc.events().next().unwrap();
    assert_eq!(event.duration.to_string(), "1");
    assert_eq!(
        event.attributes.get("w").and_then(Scalar::as_str),
        Some("8")
    );
}
#[test]
fn incomplete_frames_never_produce_a_document() {
    for src in [
        score("@0\nv C4:1\n", "1"),
        score("@0\nv C4:1\n@1\n.\n", "1"),
        "!score 1.1 unit=q\n@0\n".into(),
    ] {
        has_error(&src, "frame_uncommitted");
    }
}
#[test]
fn unequal_duration_chord_members_keep_order_and_identity() {
    let src = score(
        "@0\nv G4:2 id=high chord=c\nv C4:1 id=low chord=c\n.\n@1\nv E4:1 id=next\n.\n",
        "2",
    );
    let doc = parse(&src).unwrap();
    let text = canonical(&doc);
    assert!(text.find("id=high").unwrap() < text.find("id=low").unwrap());
    assert_eq!(doc.active_events(&"3/2".parse().unwrap()).count(), 2);
}
#[test]
fn transposition_scope_and_same_frame_voice_default_are_applied() {
    let src = score("@0\n= p transpose_chromatic=2 transpose_diatonic=1 transpose_octaves=0\nv C4:1 id=a\n~ tie t start=#a\n.\n@1\nv D4:1 id=b staff=2\n= p.s2 transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=0\n~ tie t end=#b\n.\n","2");
    assert!(parse(&src).is_ok());
    has_error(
        &score("@0\n= p transpose_chromatic=2\nv C4:1\n.\n", "1"),
        "transposition_tuple",
    );
}
#[test]
fn measures_have_separate_duration_and_nonoverlapping_ownership() {
    assert!(parse(&score(
        "@0\n= p.s1 meter=4/4\n| p pickup len=1 implicit=true label=\"0\"\nv C4:1\n.\n",
        "1"
    ))
    .is_ok());
    has_error(
        &score("@0\n| p m1 len=2\nv C4:2\n.\n@1\n| p.s1 m2 len=1\n.\n", "2"),
        "measure_overlap",
    );
}
#[test]
fn grace_chord_order_can_be_shared_but_not_reused_by_another_group() {
    let src = score("@0\nv E4:? id=g1 kind=grace anchor=#a order=-1 chord=g\nv G4:? id=g2 kind=grace anchor=#a order=-1 chord=g\nv C4:1 id=a\n.\n","1");
    assert!(parse(&src).is_ok());
}
#[test]
fn immutable_relation_fields_and_phases_are_checked() {
    has_error(&score("@0\nv C4:1 id=a\n~ slur s start=#a placement=above\n.\n@1\nv D4:1 id=b\n~ slur s end=#b placement=below\n.\n","2"),"relation_attribute_conflict");
    has_error(
        &score(
            "@0\nv C4:1 id=a\n~ tuplet s start=#a\n~ tuplet s end=#a\n.\n",
            "1",
        ),
        "field_required",
    );
}
#[test]
fn accidental_mark_cannot_silently_disagree_with_pitch() {
    has_error(
        &score("@0\nv C#4:1 id=a\n+ #a accidental value=flat\n.\n", "1"),
        "accidental_pitch_mismatch",
    );
}
#[test]
fn crlf_comments_and_blank_lines_normalize() {
    let src = score("; comment # not a pitch\n\n@0\nv F#4:1\n.\n", "1").replace('\n', "\r\n");
    let text = canonical(&parse(&src).unwrap());
    assert!(!text.contains('\r'));
    assert!(!text.contains("comment"));
    assert!(text.contains("F#4:1"));
}
#[test]
fn json_ast_requires_strings_for_exact_times_and_rejects_extra_fields() {
    let doc = parse(&score("@0\nv C4:1\n.\n", "1")).unwrap();
    let mut ast = serde_json::to_value(doc).unwrap();
    ast["end"] = serde_json::json!(1.0);
    assert!(serde_json::from_value::<Document>(ast.clone()).is_err());
    ast["end"] = serde_json::json!("1");
    ast["injected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Document>(ast).is_err());
}
#[test]
fn malformed_unicode_and_partial_inputs_do_not_panic() {
    for text in [
        "",
        "!",
        "@",
        "~",
        "v",
        "λ C4:1",
        "v 😺:1",
        "v C(+1/0)4:1",
        "v C4:1 foo=\"unterminated",
        "v C4:1 text=\"a\"x",
        "v C4:1\u{0}",
    ] {
        assert!(
            std::panic::catch_unwind(|| parse(&score(&format!("@0\n{text}\n.\n"), "1"))).is_ok(),
            "{text:?}"
        );
    }
}
#[test]
fn exact_large_denominators_reduce_without_float_intermediates() {
    let denominator =
        "1000000000000000000000000000000000000000000000000000000000000000000000000000001";
    let first: Rational = format!("1/{denominator}").parse().unwrap();
    let doubled = &first + &first;
    assert_eq!(doubled.to_string(), format!("2/{denominator}"));
    assert_eq!((&doubled - &first), first);
    let src = score(
        &format!("@0\nv C4:{first} id=a\n.\n@{first}\nv D4:{first} id=b\n.\n"),
        &doubled.to_string(),
    );
    let doc = parse(&src).unwrap();
    assert_eq!(
        canonical(&parse(&canonical(&doc)).unwrap()),
        canonical(&doc)
    );
}
#[test]
fn malformed_in_memory_pitch_is_reported_without_panicking() {
    let mut doc = parse(&score("@0\nv C4:1\n.\n", "1")).unwrap();
    if let Record::Event(event) = &mut doc.frames[0].records[0] {
        event.pitch = Pitch::Pitched {
            step: 'X',
            alteration: Rational::zero(),
            octave: 4.into(),
        };
        assert_eq!(event.pitch.semitones(), None);
    }
    assert!(validate(&doc, None)
        .iter()
        .any(|d| d.code == "pitch_invalid"));
}
#[test]
fn measure_validation_does_not_allocate_declared_staff_count() {
    let source = "!score 1.1 unit=q\n!part p instrument=piano staves=4294967295\n!voice v part=p staff=1\n@0\n| p m1 len=1\nv C4:1\n.\n!end at=1\n";
    assert!(parse(source).is_ok());
}
