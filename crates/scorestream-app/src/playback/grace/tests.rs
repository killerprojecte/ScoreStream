use super::*;
use scorestream_codec::{Document, Duration, Record, Scalar};
fn source() -> String {
    "!score 1.1 unit=q\n!requires scorestream.grace@1\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv D4:? id=g kind=grace anchor=#p order=-1\n+ #g scorestream.grace\nv C4:1 id=p\n.\n!end at=1\n".into()
}
fn policy(source: &str, quarters: &str) -> ExplicitGracePolicy {
    let occurrence = discover_source_graces(source)
        .unwrap()
        .occurrences
        .remove(0);
    ExplicitGracePolicy {
        groups: vec![ExplicitGraceGroup {
            principal: occurrence.principal,
            allocation: GraceAllocation::ExplicitFollowingQuarters {
                members: vec![GraceQuarterAllocation {
                    event: occurrence.id,
                    quarters: quarters.parse().unwrap(),
                }],
            },
        }],
    }
}
fn limited(doc: &Document, limits: GraceTimingLimits) -> Vec<scorestream_codec::Diagnostic> {
    perform_document_with_explicit_graces(
        doc,
        &ExplicitGracePolicy { groups: vec![] },
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits::default(),
        limits,
    )
    .unwrap_err()
}
#[test]
fn scheduled_source_stays_unmeasured_and_ordinary_guards_remain() {
    let s = source();
    let doc = scorestream_codec::parse(&s).unwrap();
    let original = doc.clone();
    let p = policy(&s, "1/4");
    let result = perform_document_with_explicit_graces(
        &doc,
        &p,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
    )
    .unwrap();
    assert_eq!(doc, original);
    let grace = result
        .performed()
        .score
        .events
        .iter()
        .find(|e| e.role == EventRole::Grace)
        .unwrap();
    assert_eq!(grace.duration, Duration::Grace);
    assert_eq!(grace.start, grace.end());
    assert_eq!(grace.grace.as_ref().unwrap().anchor, p.groups[0].principal);
    assert!(
        super::super::plan_source(&s)
            .unwrap_err()
            .contains("grace.performance_unsupported")
    );
    assert!(lower_document(&doc).is_err());
    assert!(perform_document(&doc, PerformanceLimits::default()).is_err());
    assert!(
        result
            .schedule()
            .seconds(&"-1".parse().unwrap())
            .unwrap_err()
            .contains("grace.clock_range")
    );
    assert!(result.schedule().seconds(&"2".parse().unwrap()).is_err());
    assert_eq!(
        result.schedule().seek(&"2".parse().unwrap()),
        Rational::one()
    );
}
#[test]
fn almost_all_allocated_principal_is_never_dropped() {
    let s = source();
    let result = plan_source_with_explicit_graces(
        &s,
        &policy(&s, "19/20"),
        GraceSoundPolicy::FullGraceThenGatedRemainder,
    )
    .unwrap();
    assert_eq!(result.plan().notes.len(), 2);
    assert!((result.plan().notes[1].start_seconds - 0.475).abs() < 1e-12);
    assert!((result.plan().notes[1].duration_seconds - 0.023).abs() < 1e-12);
}
#[test]
fn inaudible_actual_attack_and_sub_microsecond_windows_reject() {
    let s = source().replace("| p m len=1", "| p m len=1\n= p sound_dynamics=0");
    assert!(
        plan_source_with_explicit_graces(
            &s,
            &policy(&s, "1/4"),
            GraceSoundPolicy::FullGraceThenGatedRemainder
        )
        .unwrap_err()
        .contains("grace.inaudible_anchor")
    );
    let s = source();
    for amount in ["1/100000000", "99999999/100000000"] {
        assert!(
            plan_source_with_explicit_graces(
                &s,
                &policy(&s, amount),
                GraceSoundPolicy::FullGraceThenGatedRemainder
            )
            .unwrap_err()
            .contains("grace.clock_precision")
        );
    }
}
#[test]
fn source_scalar_and_policy_bits_are_bounded_before_validation_or_route_work() {
    let mut doc = scorestream_codec::parse(&source()).unwrap();
    for frame in &mut doc.frames {
        for record in &mut frame.records {
            if let Record::Mark(m) = record {
                m.attributes
                    .insert("steal_following".into(), Scalar::String("1".repeat(100)));
            }
        }
    }
    let errors = limited(
        &doc,
        GraceTimingLimits {
            maximum_rational_bits: 8,
            ..Default::default()
        },
    );
    assert_eq!(errors[0].code, "grace.rational_budget");
    assert!(errors[0].message.contains("before validation"));
    let doc = scorestream_codec::parse(&source()).unwrap();
    let mut p = policy(&source(), "1/100000000");
    if let GraceAllocation::ExplicitFollowingQuarters { members } = &mut p.groups[0].allocation {
        members[0].quarters = "1/10000000000000000000000000000".parse().unwrap();
    }
    let errors = perform_document_with_explicit_graces(
        &doc,
        &p,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits {
            maximum_events: 0,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_rational_bits: 8,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "grace.rational_budget");
}
#[test]
fn source_control_bits_precede_route_event_budget() {
    let s = source().replace("| p m len=1", "| p m len=1\n= score tempo=q:1000000000");
    let doc = scorestream_codec::parse(&s).unwrap();
    let errors = perform_document_with_explicit_graces(
        &doc,
        &ExplicitGracePolicy { groups: vec![] },
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits {
            maximum_events: 0,
            ..Default::default()
        },
        GraceTimingLimits {
            maximum_rational_bits: 8,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "grace.rational_budget");
}
fn repeated(first: &str, controls: &str) -> String {
    let q: Rational = first.parse().unwrap();
    let tail = &Rational::one() - &q;
    source().replace("!requires scorestream.grace@1", "!requires scorestream.grace@1\n!requires scorestream.navigation@1")
        .replace("| p m len=1", &format!("| p m len={first}\n+ p scorestream.barline measure=m side=left repeat=forward"))
        .replace("v C4:1 id=p",&format!("v C4:{first} id=p"))
        .replace("!end at=1",&format!("{controls}@{first}\n+ p scorestream.barline measure=m side=right repeat=backward\n| p tail len={tail}\nv r:{tail} id=r\n.\n!end at=1"))
}
#[test]
fn cumulative_occurrences_and_shifted_controls_bound_before_policy() {
    for s in [
        repeated("250/251", ""),
        repeated("1/2", "@1/251\n= score tempo=q:120\n.\n"),
    ] {
        let errors = limited(
            &scorestream_codec::parse(&s).unwrap(),
            GraceTimingLimits {
                maximum_rational_bits: 8,
                ..Default::default()
            },
        );
        assert_eq!(errors[0].code, "grace.rational_budget");
        assert!(
            errors[0].message.contains("before occurrence allocation"),
            "{errors:?}"
        );
    }
}
#[test]
fn separate_capability_profiles_never_compose_by_accident() {
    let s = source()
        .replace(
            "!requires scorestream.grace@1",
            "!requires scorestream.grace@1\n!requires scorestream.fermata@1",
        )
        .replace(
            "v C4:1 id=p",
            "v C4:1 id=p\n+ #p scorestream.fermata shape=normal",
        );
    assert!(
        discover_source_graces(&s)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
    assert!(
        super::super::discover_source_fermatas(&s)
            .unwrap_err()
            .contains("fermata.unsupported_grace")
    );
}
#[test]
fn direct_documents_missing_ids_are_structured_errors() {
    let mut doc = scorestream_codec::parse(&source()).unwrap();
    // Make an unrelated ordinary event lack its codec-assigned ID.
    let mut event = doc
        .events()
        .find(|(_, e)| e.id() == Some("p"))
        .unwrap()
        .1
        .clone();
    event.attributes.remove("id");
    event.pitch = scorestream_codec::Pitch::Rest;
    doc.frames[0].records.push(Record::Event(event));
    assert!(
        discover_document_graces(&doc)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "grace.source_identity")
    );
}
#[test]
fn legacy_native_graces_still_warn_and_omit() {
    let s = source()
        .replace("!requires scorestream.grace@1\n", "")
        .replace("+ #g scorestream.grace\n", "");
    let plan = super::super::plan_source(&s).unwrap();
    assert_eq!(plan.notes.len(), 1);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "grace_unrealized")
    );
}

#[test]
fn effective_staff_checkpoint_cannot_create_cross_staff_grace_group() {
    let s = source()
        .replace("staves=1", "staves=2")
        .replace("| p m len=1", "| p m len=1\n= v staff=2")
        .replace("order=-1", "order=-1 staff=1");
    // Static authored validation cannot resolve this persistent staff override.
    assert!(scorestream_codec::parse(&s).is_ok());
    assert!(
        discover_source_graces(&s)
            .unwrap_err()
            .contains("grace.anchor_scope")
    );
}

#[test]
fn matching_effective_staff_checkpoint_keeps_source_pair_and_schedule() {
    let s = source()
        .replace("staves=1", "staves=2")
        .replace("| p m len=1", "| p m len=1\n= v staff=2");
    let doc = scorestream_codec::parse(&s).unwrap();
    let result = perform_document_with_explicit_graces(
        &doc,
        &policy(&s, "1/4"),
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
    )
    .unwrap();
    assert!(
        result
            .performed()
            .score
            .events
            .iter()
            .all(|e| e.staff.number == 2)
    );
    assert_eq!(result.schedule().windows().len(), 2);
    assert_eq!(result.schedule().default_tempo_intervals().len(), 1);
    assert_eq!(
        result.schedule().default_tempo_intervals()[0].quarter_bpm,
        Rational::from_integer(120)
    );
}
