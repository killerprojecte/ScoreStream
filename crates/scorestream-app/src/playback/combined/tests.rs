use super::*;
use scorestream_codec::{
    Duration, Record, Scalar,
    fermata_timing::{self, ExplicitHoldGroup},
};
fn source() -> String {
    "!score 1.1 unit=q\n!requires scorestream.grace@1\n!requires scorestream.fermata@1\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=3\n= score tempo=q:120\n= p sound_dynamics=40\nv C4:1 id=held\n+ #held scorestream.fermata shape=normal\n.\n@1\n= p sound_dynamics=80\nv D4:? id=g kind=grace anchor=#principal order=-1\n+ #g scorestream.grace\nv F4:1 id=principal\n.\n@2\nv G4:1 id=tail\n.\n!end at=3\n".into()
}
fn policies(s: &str) -> (ExplicitGracePolicy, ExplicitFermataPolicy) {
    let discovery = discover_source_graces_and_fermatas(s).unwrap();
    let mut by_principal =
        std::collections::BTreeMap::<EventId, Vec<GraceQuarterAllocation>>::new();
    for o in discovery.graces.occurrences {
        by_principal
            .entry(o.principal)
            .or_default()
            .push(GraceQuarterAllocation {
                event: o.id,
                quarters: "1/4".parse().unwrap(),
            });
    }
    (
        ExplicitGracePolicy {
            groups: by_principal
                .into_iter()
                .map(|(principal, members)| ExplicitGraceGroup {
                    principal,
                    allocation: GraceAllocation::ExplicitFollowingQuarters { members },
                })
                .collect(),
        },
        ExplicitFermataPolicy {
            groups: discovery
                .fermatas
                .occurrences
                .into_iter()
                .enumerate()
                .map(|(i, o)| ExplicitHoldGroup {
                    id: format!("hold{i}"),
                    members: vec![o.id],
                    added_seconds: Rational::one(),
                })
                .collect(),
        },
    )
}
fn plan(s: &str) -> Result<ExplicitCombinedPlayback, String> {
    let (g, f) = policies(s);
    plan_source_with_explicit_graces_and_fermatas(
        s,
        &g,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &f,
        FermataSoundPolicy::SustainSelectedAnchors,
    )
}
fn qualified(s: &str) -> QualifiedCombinedPerformance {
    let (g, f) = policies(s);
    perform_document_with_explicit_graces_and_fermatas(
        &scorestream_codec::parse(s).unwrap(),
        &g,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &f,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
        TimingLimits::default(),
    )
    .unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-12, "{a} != {b}");
}
#[test]
fn preceding_hold_allows_graces_after_its_end_with_source_markers_unchanged() {
    let s = source();
    let doc = scorestream_codec::parse(&s).unwrap();
    let original = doc.clone();
    let (g, f) = policies(&s);
    let q = perform_document_with_explicit_graces_and_fermatas(
        &doc,
        &g,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &f,
        PerformanceLimits::default(),
        GraceTimingLimits::default(),
        TimingLimits::default(),
    )
    .unwrap();
    assert_eq!(doc, original);
    let event = q
        .performed()
        .score
        .events
        .iter()
        .find(|e| e.role == EventRole::Grace)
        .unwrap();
    assert_eq!(event.duration, Duration::Grace);
    assert_eq!(event.start, event.end());
    assert_eq!(q.timing().events.len(), q.performed().events.len());
    let marker = q
        .timing()
        .events
        .iter()
        .find(|e| e.written_id.0 == "g")
        .unwrap();
    assert_eq!(marker.written_quarter_start, marker.written_quarter_end);
    assert_eq!(marker.seconds_start, "3/2".parse().unwrap());
    assert_eq!(marker.seconds_start, marker.seconds_end);
    let p = plan(&s).unwrap();
    assert_eq!(p.plan().notes.len(), 4);
    close(p.plan().duration_seconds, 2.5);
    let held = p
        .plan()
        .notes
        .iter()
        .find(|n| n.written_id.as_deref() == Some("held"))
        .unwrap();
    close(held.duration_seconds, 1.5);
    let grace = &p.grace_windows()[0];
    assert_eq!(grace.seconds_start, "3/2".parse().unwrap());
    assert_eq!(grace.seconds_end, "13/8".parse().unwrap());
    let principal = &p.grace_windows()[1];
    assert_eq!(principal.seconds_start, "13/8".parse().unwrap());
    assert_eq!(principal.seconds_end, "197/100".parse().unwrap());
    assert!(
        p.plan()
            .diagnostics
            .iter()
            .any(|d| d.code == "combined.explicit_order_boundary")
    );
    assert!(
        p.plan()
            .diagnostics
            .iter()
            .any(|d| d.code == "grace.explicit_following_quarters")
    );
    assert!(
        p.plan()
            .diagnostics
            .iter()
            .any(|d| d.code == "fermata.explicit_policy_hold")
    );
    assert!(matches!(
        p.seek(&"1".parse().unwrap()),
        SeekPosition::Hold { .. }
    ));
    assert_eq!(
        p.seek(&"3/2".parse().unwrap()),
        SeekPosition::Music {
            quarter: Rational::one()
        }
    );
    assert_eq!(
        p.seek(&"10".parse().unwrap()),
        SeekPosition::Music {
            quarter: Rational::from_integer(3)
        }
    );
}
#[test]
fn ordinary_and_single_feature_and_raw_clock_guards_still_reject() {
    let s = source();
    let doc = scorestream_codec::parse(&s).unwrap();
    let (g, f) = policies(&s);
    let q = qualified(&s);
    assert!(crate::playback::plan_source(&s).is_err());
    assert!(lower_document(&doc).is_err());
    assert!(perform_document(&doc, PerformanceLimits::default()).is_err());
    assert!(
        crate::playback::plan_source_with_explicit_graces(
            &s,
            &g,
            GraceSoundPolicy::FullGraceThenGatedRemainder
        )
        .is_err()
    );
    assert!(
        crate::playback::plan_source_with_explicit_fermatas(
            &s,
            &f,
            FermataSoundPolicy::SustainSelectedAnchors
        )
        .is_err()
    );
    assert_eq!(
        fermata_timing::interpret(&doc, q.performed(), &f, TimingLimits::default())
            .unwrap_err()
            .code,
        "fermata.unsupported_grace"
    );
}
#[test]
fn hold_onset_controls_take_effect_after_hold_and_actual_attack_dynamics_apply() {
    let s = source().replace("@2\n", "@9/8\n= p sound_dynamics=20\n.\n@2\n");
    let p = plan(&s).unwrap();
    let by_id = |id: &str| {
        p.plan()
            .notes
            .iter()
            .find(|n| n.written_id.as_deref() == Some(id))
            .unwrap()
    };
    assert_eq!(by_id("held").velocity, 36);
    assert_eq!(by_id("g").velocity, 72);
    assert_eq!(by_id("principal").velocity, 18);
    assert!(by_id("held").gain_envelope.iter().all(|p| p.gain == 1.));
    let envelope = &by_id("g").gain_envelope;
    assert!(
        envelope
            .iter()
            .any(|p| (p.offset_seconds - 0.0625).abs() < 1e-12)
    );
}
#[test]
fn holds_inside_or_at_nominal_principal_end_reject_even_on_another_staff() {
    for duration in ["1/2", "1"] {
        let s=source().replace("staves=1","staves=2").replace("@0\n","!voice other part=p staff=2\n@0\n")
            .replace("v F4:1 id=principal",&format!("v F4:1 id=principal\nother r:{duration} id=otherhold\n+ #otherhold scorestream.fermata shape=normal"));
        let e = plan(&s).unwrap_err();
        assert!(e.contains("combined.grace_hold_overlap"), "{e}");
        assert!(
            e.contains("principal") && e.contains("hold1") && e.contains("boundary"),
            "{e}"
        );
    }
    let s = source().replace(
        "v F4:1 id=principal",
        "v F4:1 id=principal\n+ #principal scorestream.fermata shape=normal",
    );
    assert!(
        plan(&s)
            .unwrap_err()
            .contains("combined.affected_fermata_anchor")
    );
}
#[test]
fn every_grace_and_fermata_policy_member_is_required_exactly_once() {
    let s = source();
    let (g, f) = policies(&s);
    for (g, f, code) in [
        (
            ExplicitGracePolicy { groups: vec![] },
            f.clone(),
            "grace.policy_missing",
        ),
        (
            g.clone(),
            ExplicitFermataPolicy { groups: vec![] },
            "fermata.incomplete_policy",
        ),
        (
            {
                let mut p = g.clone();
                p.groups.push(p.groups[0].clone());
                p
            },
            f.clone(),
            "grace.policy_duplicate",
        ),
        (
            g,
            {
                let mut p = f.clone();
                p.groups.push(p.groups[0].clone());
                p
            },
            "fermata.",
        ),
    ] {
        let e = plan_source_with_explicit_graces_and_fermatas(
            &s,
            &g,
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &f,
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .unwrap_err();
        assert!(e.contains(code), "{e}");
    }
}
#[test]
fn strict_sound_checks_cover_unrelated_chains_and_affected_graces() {
    let s = source()
        .replace("staves=1", "staves=2")
        .replace("@0\n", "!voice other part=p staff=2\n@0\n")
        .replace("v C4:1 id=held", "v C4:1 id=held\nother A4:1 id=unselected");
    assert!(plan(&s).unwrap_err().contains("fermata.unselected_overlap"));
    let s = source().replace(
        "v G4:1 id=tail",
        "v G4:1 id=tail\n+ #tail articulation value=tenuto",
    );
    assert!(
        plan(&s)
            .unwrap_err()
            .contains("fermata.articulation_unsupported")
    );
    let s = source().replace(
        "v F4:1 id=principal",
        "v F4:1 id=principal\n+ #principal articulation value=tenuto",
    );
    assert!(
        plan(&s)
            .unwrap_err()
            .contains("grace.articulation_unsupported")
    );
    let s = source().replace("sound_dynamics=80", "sound_dynamics=0");
    assert!(plan(&s).unwrap_err().contains("grace.inaudible_anchor"));
}
#[test]
fn source_and_both_policy_preflights_precede_route_allocation() {
    let s = source();
    let (g, mut f) = policies(&s);
    let mut doc = scorestream_codec::parse(&s).unwrap();
    let run =
        |doc: &scorestream_codec::Document, g: &ExplicitGracePolicy, f: &ExplicitFermataPolicy| {
            perform_document_with_explicit_graces_and_fermatas(
                doc,
                g,
                GraceSoundPolicy::FullGraceThenGatedRemainder,
                f,
                PerformanceLimits {
                    maximum_events: 0,
                    ..Default::default()
                },
                GraceTimingLimits {
                    maximum_rational_bits: 8,
                    ..Default::default()
                },
                TimingLimits::default(),
            )
            .unwrap_err()
        };
    f.groups[0].added_seconds = "1000000".parse().unwrap();
    assert_eq!(run(&doc, &g, &f)[0].code, "fermata.rational_budget");
    let (_, f) = policies(&s);
    let mut big_g = g.clone();
    if let GraceAllocation::ExplicitFollowingQuarters { members } = &mut big_g.groups[0].allocation
    {
        members[0].quarters = "1/1000000".parse().unwrap();
    }
    assert_eq!(run(&doc, &big_g, &f)[0].code, "grace.rational_budget");
    for frame in &mut doc.frames {
        for record in &mut frame.records {
            match record {
                Record::State(st) if st.attributes.contains_key("tempo") => {
                    st.attributes
                        .insert("tempo".into(), Scalar::String("q:1000000".into()));
                }
                _ => (),
            }
        }
    }
    assert_eq!(run(&doc, &g, &f)[0].code, "grace.rational_budget");
}
#[test]
fn final_mapped_window_precision_budget_is_checked_after_adding_holds() {
    let s = source();
    let (g, mut f) = policies(&s);
    let doc = scorestream_codec::parse(&s).unwrap();
    f.groups[0].added_seconds = "1/128".parse().unwrap();
    // Base windows fit 8 bits (197/200), but the preceding hold makes their
    // final endpoints exceed the denominator budget. No base-only result leaks.
    let e = perform_document_with_explicit_graces_and_fermatas(
        &doc,
        &g,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &f,
        PerformanceLimits::default(),
        GraceTimingLimits {
            maximum_rational_bits: 8,
            ..Default::default()
        },
        TimingLimits::default(),
    )
    .unwrap_err();
    assert_eq!(e[0].code, "combined.rational_budget", "{e:?}");
}
