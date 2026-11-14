use scorestream_codec::{canonical, notation::*, parse, Rational};
fn score(body: &str, end: &str) -> String {
    format!("!score 1.1 unit=q\n!part p instrument=piano staves=2\n!voice v part=p staff=1\n@0\n| p m len={end}\n{body}\n!end at={end}\n")
}
fn resolved(body: &str, end: &str) -> ResolvedScore {
    lower_document(&parse(&score(body, end)).unwrap()).unwrap()
}
fn q(s: &str) -> Rational {
    s.parse().unwrap()
}
#[test]
fn typed_scope_pitch_and_frame_wide_state_are_resolved_once() {
    let s=resolved("v C4:1 id=a\n= v staff=2\n= p instrument=flute transpose_chromatic=2 transpose_diatonic=1 transpose_octaves=0\n+ p.s2 dynamic value=p\n.","1");
    assert_eq!(s.events[0].staff.number, 2);
    assert_eq!(s.events[0].instrument.0, "flute");
    assert_eq!(s.events[0].sounding_pitch, Some(q("50")));
    assert!(matches!(
        s.directions[0].scope,
        Scope::Staff(StaffId { number: 2, .. })
    ));
}
#[test]
fn lowering_does_not_modify_wire_ast_or_canonical_sfs() {
    let source = score(
        "v C4:1 id=a\n+ #a dynamic value=mf id=d\n+ #a words text=\"dolce\"\n.",
        "1",
    );
    let doc = parse(&source).unwrap();
    let before = serde_json::to_string(&doc).unwrap();
    let text = canonical(&doc);
    let model = lower_document(&doc).unwrap();
    assert_eq!(before, serde_json::to_string(&doc).unwrap());
    assert_eq!(text, canonical(&doc));
    assert!(matches!(
        model
            .directions
            .iter()
            .find(|d| matches!(d.kind, DirectionKind::Words { .. }))
            .unwrap()
            .kind,
        DirectionKind::Words { .. }
    ));
}
#[test]
fn all_registered_dynamics_and_articulations_have_typed_lowering() {
    let descriptor: serde_json::Value = serde_json::from_str(include_str!(
        "../../../design/spec/scorestream-language.json"
    ))
    .unwrap();
    for kind in ["dynamic", "articulation"] {
        for token in descriptor["vocabulary"]["mark_fields"][kind]["fields"]["value"]["values"]
            .as_array()
            .unwrap()
        {
            let token = token.as_str().unwrap();
            let s = resolved(&format!("v C4:1 id=a\n+ #a {kind} value={token}\n."), "1");
            match (&s.directions[0].kind, kind) {
                (DirectionKind::Dynamic { .. }, "dynamic")
                | (DirectionKind::Articulation { .. }, "articulation") => (),
                _ => panic!("untyped {token}"),
            }
        }
    }
}
#[test]
fn same_staff_dynamic_replaces_previous_interval_at_exact_boundary() {
    let s = resolved(
        "+ p.s1 dynamic value=p\nv C4:2 id=a\n.\n@2\n+ p.s1 dynamic value=f\nv D4:2 id=b\n.",
        "4",
    );
    let i = resolve_expression(&s).unwrap();
    let scope = Scope::Staff(s.events[0].staff.clone());
    let values = &i.assignments[&scope];
    assert_eq!(values[0].end, q("2"));
    assert_eq!(
        i.assignment_at(&s.events[1], &q("2")).unwrap().level,
        DynamicLevel::F
    );
}
#[test]
fn finite_narrow_scope_returns_to_current_not_cached_ancestor() {
    let s=resolved("+ score dynamic value=mp\nv C4:2 id=a\n+ #a dynamic value=p\n.\n@1\n+ score dynamic value=f\n.\n@2\nv D4:1 id=b\n.","3");
    let i = resolve_expression(&s).unwrap();
    assert_eq!(
        i.assignment_at(&s.events[0], &q("1")).unwrap().level,
        DynamicLevel::P
    );
    assert_eq!(
        i.assignment_at(&s.events[0], &q("2")).unwrap().level,
        DynamicLevel::F
    );
    assert_eq!(
        i.assignment_at(&s.events[1], &q("2")).unwrap().level,
        DynamicLevel::F
    );
}
#[test]
fn transient_and_compound_dynamics_are_distinct_from_level_assignments() {
    let s=resolved("+ v dynamic value=p\nv C4:1 id=a\n.\n@1\n+ v dynamic value=sfz\nv D4:1 id=b\n.\n@2\n+ v dynamic value=fp\nv E4:1 id=c\n.","3");
    let i = resolve_expression(&s).unwrap();
    let scope = Scope::Voice(VoiceId("v".into()));
    assert_eq!(i.assignments[&scope].len(), 2);
    assert_eq!(i.assignments[&scope][0].end, q("2"));
    assert_eq!(i.attacks[&scope].len(), 2);
    assert_eq!(
        i.assignment_at(&s.events[1], &q("1")).unwrap().level,
        DynamicLevel::P
    );
}
#[test]
fn later_same_scope_dynamic_terminates_a_hairpin_not_its_notated_anchor() {
    let s=resolved("+ v dynamic value=p\nv C4:2 id=a\n~ crescendo h start=#a\n.\n@2\n+ v dynamic value=mf\nv D4:2 id=b\n.\n@4\n+ v dynamic value=f\nv E4:1 id=c\n~ crescendo h end=#c\n.","5");
    let i = resolve_expression(&s).unwrap();
    let h = &i.hairpins[&Scope::Voice(VoiceId("v".into()))][0];
    assert_eq!(h.end, q("2"));
    assert_eq!(h.notated_end, q("4"));
    assert!(i.hairpin_at(&s.events[1], &q("2")).is_none());
}
#[test]
fn adjacent_hairpins_do_not_overlap_but_actual_overlap_is_rejected() {
    let s=resolved("v C4:2 id=a\n~ crescendo h start=#a\n.\n@2\nv D4:2 id=b\n~ crescendo h end=#b\n~ diminuendo j start=#b\n.\n@4\nv E4:1 id=c\n~ diminuendo j end=#c\n.","5");
    let i = resolve_expression(&s).unwrap();
    assert_eq!(i.hairpin_at(&s.events[1], &q("2")).unwrap().id.0, "j");
    let s=resolved("v C4:1 id=a\n~ crescendo h start=#a\n.\n@1\nv D4:1 id=b\n~ diminuendo j start=#b\n.\n@2\nv E4:1 id=c\n~ crescendo h end=#c\n.\n@3\nv F4:1 id=d\n~ diminuendo j end=#d\n.","4");
    let error = resolve_expression(&s).unwrap_err();
    assert_eq!(error[0].code, "hairpin_overlap");
    assert_eq!(error[0].object_id.as_deref(), Some("j"));
}
#[test]
fn same_domain_conflicts_are_diagnosed_while_transients_coexist() {
    let conflict = resolved(
        "v C4:1 id=a\n+ v dynamic value=p\n+ v dynamic value=f\n.",
        "1",
    );
    assert_eq!(
        resolve_expression(&conflict).unwrap_err()[0].code,
        "dynamic_conflict"
    );
    let compound = resolved(
        "v C4:1 id=a\n+ v dynamic value=p\n+ v dynamic value=sfz\n.",
        "1",
    );
    assert!(resolve_expression(&compound).is_ok());
}

#[test]
fn shared_model_explicit_finite_staff_scope_uses_latest_parent_after_end() {
    let mut s = resolved(
        "+ p dynamic value=mp\n+ p.s1 dynamic value=p\nv C4:3 id=a\n.\n@1\n+ p dynamic value=f\n.",
        "3",
    );
    s.directions
        .iter_mut()
        .find(|d| matches!(d.scope, Scope::Staff(_)))
        .unwrap()
        .end = Some(q("2"));
    let i = resolve_expression(&s).unwrap();
    assert_eq!(
        i.assignment_at(&s.events[0], &q("1")).unwrap().level,
        DynamicLevel::P
    );
    assert_eq!(
        i.assignment_at(&s.events[0], &q("2")).unwrap().level,
        DynamicLevel::F
    );
}
#[test]
fn explicit_interval_rejects_zero_or_negative_time() {
    let mut s = resolved("+ p dynamic value=p\nv C4:1 id=a\n.", "1");
    s.directions[0].end = Some(q("0"));
    assert_eq!(
        resolve_expression(&s).unwrap_err()[0].code,
        "expression_interval"
    );
}

#[test]
fn end_of_score_dynamic_does_not_reject_valid_legacy_sfs() {
    let s = resolved("v C4:1 id=a\n.\n@1\n+ p dynamic value=p\n.", "1");
    let i = resolve_expression(&s).unwrap();
    assert_eq!(i.diagnostics[0].code, "expression_empty_scope");
}
