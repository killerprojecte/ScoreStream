use super::*;
use crate::{
    Attributes, DocumentKind, Duration, Event, Frame, Mark, Measure, Part, Scalar, SourceRange,
    Voice,
};
fn source() -> Document {
    let mut doc = Document::new(DocumentKind::Score);
    doc.end = Rational::from_integer(2);
    doc.required_extensions = vec![
        crate::grace_extension::EXTENSION.into(),
        crate::fermata_extension::EXTENSION.into(),
    ];
    doc.parts.push(Part {
        id: "p".into(),
        instrument: "piano".into(),
        staves: 1,
        display_name: None,
        source: SourceRange::default(),
    });
    doc.voices.push(Voice {
        id: "v".into(),
        part: "p".into(),
        staff: 1,
        source: SourceRange::default(),
    });
    let note = |id: &str, grace: bool| {
        let mut attributes: Attributes = [("id".into(), Scalar::String(id.into()))].into();
        if grace {
            attributes.extend([
                ("kind".into(), Scalar::String("grace".into())),
                ("anchor".into(), Scalar::String("#principal".into())),
                ("order".into(), Scalar::Integer(-1)),
            ]);
        }
        Record::Event(Event {
            voice: "v".into(),
            pitch: "C4".parse().unwrap(),
            duration: if grace {
                Duration::Grace
            } else {
                Duration::Measured(Rational::one())
            },
            attributes,
            source: SourceRange::default(),
        })
    };
    let mark = |id: &str, kind: &str| {
        Record::Mark(Mark {
            target: format!("#{id}"),
            kind: kind.into(),
            attributes: Attributes::new(),
            source: SourceRange::default(),
        })
    };
    doc.frames = vec![
        Frame {
            time: Rational::zero(),
            source: SourceRange::default(),
            records: vec![
                Record::Measure(Measure {
                    target: "p".into(),
                    id: "m".into(),
                    length: doc.end.clone(),
                    attributes: Attributes::new(),
                    source: SourceRange::default(),
                }),
                note("held", false),
                mark("held", crate::fermata_extension::KIND),
            ],
        },
        Frame {
            time: Rational::one(),
            source: SourceRange::default(),
            records: vec![
                note("g", true),
                mark("g", crate::grace_extension::KIND),
                note("principal", false),
            ],
        },
    ];
    doc
}
fn fixture() -> (
    Document,
    PerformedScore,
    GraceSchedule,
    ExplicitFermataPolicy,
) {
    let doc = source();
    let (g, t) = limits(
        &doc,
        &GraceTimingLimits::default(),
        &TimingLimits::default(),
    )
    .unwrap();
    let performed = prepare(&doc, PerformanceLimits::default(), &g, &t, None).unwrap();
    let discovered = grace::discover(&doc, &performed).unwrap();
    let member = &discovered.occurrences[0];
    let policy = ExplicitGracePolicy {
        groups: vec![ExplicitGraceGroup {
            principal: member.principal.clone(),
            allocation: GraceAllocation::ExplicitFollowingQuarters {
                members: vec![GraceQuarterAllocation {
                    event: member.id.clone(),
                    quarters: "1/4".parse().unwrap(),
                }],
            },
        }],
    };
    let schedule = grace::allocate(
        &doc,
        &performed,
        &policy,
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &g,
    )
    .unwrap();
    let fermata = fermata::discover(&doc, &performed, &t).unwrap();
    let policy = ExplicitFermataPolicy {
        groups: vec![fermata_timing::ExplicitHoldGroup {
            id: "hold".into(),
            members: vec![fermata.occurrences[0].id.clone()],
            added_seconds: Rational::one(),
        }],
    };
    (doc, performed, schedule, policy)
}
#[test]
fn internal_marker_interpreter_requires_every_scheduled_grace_receipt() {
    let (doc, performed, schedule, policy) = fixture();
    let projection = fermata_timing::interpret_qualified_graces(
        &doc,
        &performed,
        &policy,
        TimingLimits::default(),
        &schedule,
    )
    .unwrap();
    assert_eq!(projection.events.len(), 3);
    let g = &schedule.discovery().occurrences[0].id;
    let marker = projection.events.iter().find(|e| &e.id == g).unwrap();
    assert_eq!(marker.quarter_start, marker.quarter_end);
    assert_eq!(marker.written_quarter_start, marker.written_quarter_end);
    assert_eq!(marker.seconds_start, marker.seconds_end);
    for remove_event in [false, true] {
        let mut damaged = performed.clone();
        damaged.events.retain(|e| &e.id != g);
        if remove_event {
            damaged.score.events.retain(|e| &e.id != g);
        }
        let error = fermata_timing::interpret_qualified_graces(
            &doc,
            &damaged,
            &policy,
            TimingLimits::default(),
            &schedule,
        )
        .unwrap_err();
        assert_eq!(error.code, "fermata.event_receipt");
    }
    let mut damaged = performed.clone();
    damaged
        .events
        .iter_mut()
        .find(|e| &e.id == g)
        .unwrap()
        .written_id = EventId("held".into());
    assert_eq!(
        fermata_timing::interpret_qualified_graces(
            &doc,
            &damaged,
            &policy,
            TimingLimits::default(),
            &schedule
        )
        .unwrap_err()
        .code,
        "fermata.event_receipt"
    );
}
#[test]
fn complete_source_index_includes_graces_not_in_a_detached_schedule() {
    let (mut doc, performed, schedule, policy) = fixture();
    let mut extra = doc.frames[1].records[0].clone();
    if let Record::Event(e) = &mut extra {
        e.attributes
            .insert("id".into(), Scalar::String("other_grace".into()));
        e.attributes.insert("order".into(), Scalar::Integer(-2));
    }
    doc.frames[1].records.insert(0, extra);
    doc.frames[1].records.push(Record::Mark(Mark {
        target: "#other_grace".into(),
        kind: crate::grace_extension::KIND.into(),
        attributes: Attributes::new(),
        source: SourceRange::default(),
    }));
    // Deliberately invalid detached pairing: the route and schedule both omit a
    // valid newly authored grace. The completeness index must still find it.
    let e = fermata_timing::interpret_qualified_graces(
        &doc,
        &performed,
        &policy,
        TimingLimits::default(),
        &schedule,
    )
    .unwrap_err();
    assert_eq!(e.code, "fermata.event_receipt");
    assert!(e.message.contains("omitted event other_grace"), "{e}");
}
#[test]
fn unknown_required_capability_cannot_be_stripped_in_combined_profile() {
    let mut doc = source();
    doc.required_extensions.push("example.future@1".into());
    let e = discover_document_graces_and_fermatas(&doc).unwrap_err();
    assert!(e.iter().any(|d| d.code == "extension_unsupported"), "{e:?}");
}
#[test]
fn restrictive_budget_applies_to_both_capabilities_before_validation() {
    let mut doc = source();
    // Invalid sign syntax would fail semantic validation, but the source record
    // budget is checked first and independently of either capability's decoder.
    if let Record::Mark(m) = &mut doc.frames[0].records[2] {
        m.attributes.insert("unknown".into(), Scalar::Boolean(true));
    }
    let e = preflight_combined_source(
        &doc,
        &GraceTimingLimits::default(),
        &TimingLimits {
            maximum_source_records: 2,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(e[0].code, "grace.timing_budget");
    let e = preflight_combined_source(
        &source(),
        &GraceTimingLimits::default(),
        &TimingLimits {
            maximum_authored_fermatas: 0,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(e[0].code, "combined.timing_budget");
}
