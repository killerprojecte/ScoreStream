//! Synthetic, independently constructed performed snapshots exercise only the
//! opt-in pure clock. No test bypasses the ordinary source projector's guard.
use scorestream_core::{
    Attributes, Document, DocumentKind, Duration, Event, Frame, Mark, Measure, Part, Pitch,
    Rational, Record, Scalar, SourceRange, Voice,
    fermata_extension::{EXTENSION, KIND},
    fermata_timing::*,
    notation::*,
};
use std::collections::BTreeMap;

fn q(s: &str) -> Rational {
    s.parse().unwrap()
}
fn id(s: &str) -> EventId {
    EventId(s.into())
}
fn part(n: usize) -> PartId {
    PartId(format!("p{n}"))
}
fn origin(s: &str) -> Origin {
    Origin {
        object_id: s.into(),
        source: SourceRange::default(),
    }
}
#[derive(Clone)]
struct Written {
    id: String,
    part: usize,
    measure: usize,
    offset: Rational,
    duration: Rational,
    fermata: bool,
}
fn note(
    id: &str,
    part: usize,
    measure: usize,
    offset: &str,
    duration: &str,
    fermata: bool,
) -> Written {
    Written {
        id: id.into(),
        part,
        measure,
        offset: q(offset),
        duration: q(duration),
        fermata,
    }
}
fn group(id: &str, members: &[&str], seconds: &str) -> ExplicitHoldGroup {
    ExplicitHoldGroup {
        id: id.into(),
        members: members.iter().map(|m| EventId((*m).into())).collect(),
        added_seconds: q(seconds),
    }
}
fn policy(groups: Vec<ExplicitHoldGroup>) -> ExplicitFermataPolicy {
    ExplicitFermataPolicy { groups }
}
fn tempo(at: &str, bpm: &str) -> TempoChange {
    TempoChange {
        at: q(at),
        value: Tempo {
            beat: Beat {
                unit: BeatUnit::Quarter,
                dotted: false,
            },
            bpm: q(bpm),
        },
        origin: origin("tempo"),
    }
}
struct Fixture {
    source: Document,
    performed: PerformedScore,
}
impl Fixture {
    /// All measures have the supplied length. Route contains source measure
    /// indices; their legality is outside the clock's independent contract.
    fn new(
        parts: usize,
        measures: usize,
        length: &str,
        events: Vec<Written>,
        route: &[usize],
    ) -> Self {
        let length = q(length);
        let mut source = Document::new(DocumentKind::Score);
        source.end = &length * &Rational::from_integer(measures as i64);
        if events.iter().any(|e| e.fermata) {
            source.required_extensions.push(EXTENSION.into());
        }
        let mut frames: BTreeMap<Rational, Vec<Record>> = BTreeMap::new();
        for p in 0..parts {
            source.parts.push(Part {
                id: part(p).0,
                instrument: "piano".into(),
                staves: 1,
                display_name: None,
                source: SourceRange::default(),
            });
            source.voices.push(Voice {
                id: format!("v{p}"),
                part: part(p).0,
                staff: 1,
                source: SourceRange::default(),
            });
            for m in 0..measures {
                frames
                    .entry(&length * &Rational::from_integer(m as i64))
                    .or_default()
                    .push(Record::Measure(Measure {
                        target: part(p).0,
                        id: format!("m{p}_{m}"),
                        length: length.clone(),
                        attributes: [("label".into(), Scalar::String(format!("{}", m + 1)))].into(),
                        source: SourceRange::default(),
                    }));
            }
        }
        for e in &events {
            let start = &(&length * &Rational::from_integer(e.measure as i64)) + &e.offset;
            let records = frames.entry(start).or_default();
            records.push(Record::Event(Event {
                voice: format!("v{}", e.part),
                pitch: "C4".parse().unwrap(),
                duration: Duration::Measured(e.duration.clone()),
                attributes: [("id".into(), Scalar::String(e.id.clone()))].into(),
                source: SourceRange::default(),
            }));
            if e.fermata {
                records.push(Record::Mark(Mark {
                    target: format!("#{}", e.id),
                    kind: KIND.into(),
                    attributes: Attributes::new(),
                    source: SourceRange::default(),
                }));
            }
        }
        source.frames = frames
            .into_iter()
            .map(|(time, records)| Frame {
                time,
                records,
                source: SourceRange::default(),
            })
            .collect();
        let mut performed = PerformedScore {
            score: ResolvedScore {
                end: &length * &Rational::from_integer(route.len() as i64),
                events: vec![],
                directions: vec![],
                relations: vec![],
                tempos: vec![],
            },
            events: vec![],
            measures: vec![],
            diagnostics: vec![],
        };
        for (occurrence, &written_measure) in route.iter().enumerate() {
            let start = &length * &Rational::from_integer(occurrence as i64);
            for p in 0..parts {
                performed.measures.push(MeasureOccurrence {
                    written_id: format!("m{p}_{written_measure}"),
                    label: format!("{}", written_measure + 1),
                    part: part(p),
                    occurrence_index: occurrence,
                    start: start.clone(),
                    length: length.clone(),
                });
            }
            for e in events.iter().filter(|e| e.measure == written_measure) {
                let occurrence_id = id(&format!("occ{occurrence}:{}", e.id));
                performed.events.push(EventOccurrence {
                    id: occurrence_id.clone(),
                    written_id: id(&e.id),
                    measure_occurrence: occurrence,
                });
                performed.score.events.push(ResolvedEvent {
                    id: occurrence_id,
                    voice: VoiceId(format!("v{}", e.part)),
                    part: part(e.part),
                    staff: StaffId {
                        part: part(e.part),
                        number: 1,
                    },
                    instrument: InstrumentId("piano".into()),
                    start: &start + &e.offset,
                    duration: Duration::Measured(e.duration.clone()),
                    written_pitch: "C4".parse().unwrap(),
                    sounding_pitch: Some(q("60")),
                    role: EventRole::Note,
                    grace: None,
                    origin: origin(&e.id),
                });
            }
        }
        let errors: Vec<_> = scorestream_core::validate(&source, None)
            .into_iter()
            .filter(|d| d.severity == scorestream_core::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "Invalid fixture: {errors:?}");
        Self { source, performed }
    }
    fn interpret(&self, policy: ExplicitFermataPolicy) -> TimingProjection {
        interpret(
            &self.source,
            &self.performed,
            &policy,
            TimingLimits::default(),
        )
        .unwrap()
    }
    fn rejects(&self, policy: ExplicitFermataPolicy, code: &str) {
        let error = interpret(
            &self.source,
            &self.performed,
            &policy,
            TimingLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, code, "{error}");
    }
}

#[test]
fn eighteen_written_copies_make_one_global_hold_and_full_audit() {
    let events = (0..18)
        .map(|p| note(&format!("n{p}"), p, 0, "0", "4", true))
        .collect();
    let mut f = Fixture::new(18, 1, "4", events, &[0]);
    // Preserve every authored detail without interpreting shape as seconds.
    for frame in &mut f.source.frames {
        for record in &mut frame.records {
            if let Record::Mark(m) = record {
                m.attributes = [
                    ("shape".into(), Scalar::String("double-square".into())),
                    ("orientation".into(), Scalar::String("inverted".into())),
                    ("default_y".into(), Scalar::String("-7/2".into())),
                    ("relative_y".into(), Scalar::String("1/3".into())),
                ]
                .into();
            }
        }
    }
    let before_source = f.source.clone();
    let before_performed = f.performed.clone();
    let members = f.performed.events.iter().map(|e| e.id.clone()).collect();
    let result = f.interpret(policy(vec![ExplicitHoldGroup {
        id: "orchestra".into(),
        members,
        added_seconds: q("3/2"),
    }]));
    assert_eq!(result.authored.len(), 18);
    assert_eq!(result.fermatas.len(), 18);
    assert_eq!(result.holds().len(), 1);
    assert_eq!(result.holds()[0].added_seconds, q("3/2"));
    assert_eq!(result.duration_seconds(), &q("7/2"));
    assert!(result.authored.iter().all(|a| a.occurrence_ids.len() == 1));
    for occurrence in &result.fermatas {
        assert_eq!(occurrence.sign.default_y, Some(q("-7/2")));
        assert_eq!(occurrence.sign.relative_y, Some(q("1/3")));
        assert_eq!(occurrence.quarter_start, q("0"));
        assert_eq!(occurrence.quarter_end, q("4"));
    }
    assert_eq!(f.source, before_source);
    assert_eq!(f.performed, before_performed);
    assert!(
        matches!(&result.diagnostics[1], TimingDiagnostic::ExplicitPolicyHold {
        group_id, members, added_seconds, duration_source: HoldDurationSource::ExplicitPolicyNotAuthoredSign,
    } if group_id == "orchestra" && members.len() == 18 && added_seconds == &q("3/2"))
    );
}

#[test]
fn i21_unequal_starts_share_one_exact_boundary() {
    let f = Fixture::new(
        2,
        1,
        "4",
        vec![
            note("long", 0, 0, "0", "4", true),
            note("short", 1, 0, "2", "2", true),
        ],
        &[0],
    );
    let r = f.interpret(policy(vec![group(
        "I.21",
        &["occ0:long", "occ0:short"],
        "2",
    )]));
    assert_eq!(r.holds()[0].support_start, q("0"));
    assert_eq!(r.holds()[0].boundary, q("4"));
    assert_eq!(r.fermatas[1].quarter_start, q("2"));
    assert_eq!(r.events[1].seconds_start, q("1"));
    assert_eq!(r.events[1].seconds_end, q("4"));
}

#[test]
fn iv446_unequal_entrances_notes_and_rest_preserve_identity() {
    let mut f = Fixture::new(
        3,
        1,
        "4",
        vec![
            note("a", 0, 0, "0", "4", true),
            note("b", 1, 0, "1", "3", true),
            note("rest", 2, 0, "3", "1", true),
        ],
        &[0],
    );
    for frame in &mut f.source.frames {
        for record in &mut frame.records {
            if let Record::Event(e) = record {
                if e.id() == Some("rest") {
                    e.pitch = Pitch::Rest;
                }
            }
        }
    }
    f.performed.score.events[2].written_pitch = Pitch::Rest;
    f.performed.score.events[2].sounding_pitch = None;
    let r = f.interpret(policy(vec![group(
        "IV.446",
        &["occ0:a", "occ0:b", "occ0:rest"],
        "5/3",
    )]));
    assert_eq!(r.holds().len(), 1);
    assert_eq!(r.events[2].quarter_start, q("3"));
    assert_eq!(r.events[2].quarter_end, q("4"));
    assert_eq!(r.duration_seconds(), &q("11/3"));
}

#[test]
fn ii123_unequal_group_and_overlapping_split_are_rejected() {
    let f = Fixture::new(
        2,
        1,
        "4",
        vec![
            note("upper", 0, 0, "0", "4", true),
            note("lower", 1, 0, "1", "2", true),
        ],
        &[0],
    );
    f.rejects(
        policy(vec![group("II.123", &["occ0:upper", "occ0:lower"], "1")]),
        "fermata.unequal_boundaries",
    );
    f.rejects(
        policy(vec![
            group("upper", &["occ0:upper"], "1"),
            group("lower", &["occ0:lower"], "1"),
        ]),
        "fermata.overlapping_support",
    );
}

#[test]
fn i268_three_disjoint_groups_retain_separate_holds() {
    let f = Fixture::new(
        1,
        1,
        "6",
        vec![
            note("a", 0, 0, "0", "2", true),
            note("b", 0, 0, "2", "2", true),
            note("c", 0, 0, "4", "2", true),
        ],
        &[0],
    );
    let r = f.interpret(policy(vec![
        group("I.268-c", &["occ0:c"], "1"),
        group("I.268-a", &["occ0:a"], "1/3"),
        group("I.268-b", &["occ0:b"], "2/3"),
    ]));
    assert_eq!(
        r.holds()
            .iter()
            .map(|h| h.boundary.clone())
            .collect::<Vec<_>>(),
        vec![q("2"), q("4"), q("6")]
    );
    assert_eq!(r.duration_seconds(), &q("5"));
}

#[test]
fn exact_tempo_integration_and_hold_edges_without_float_rounding() {
    let mut f = Fixture::new(
        1,
        1,
        "4",
        vec![
            note("a", 0, 0, "0", "2", true),
            note("b", 0, 0, "2", "1", true),
            note("c", 0, 0, "3", "1", false),
        ],
        &[0],
    );
    f.performed.score.tempos = vec![tempo("0", "120"), tempo("2", "60")];
    let r = f.interpret(policy(vec![
        group("a", &["occ0:a"], "1/3"),
        group("b", &["occ0:b"], "2/3"),
    ]));
    assert_eq!(r.before(&q("2")), q("1"));
    assert_eq!(r.after(&q("2")), q("4/3"));
    assert_eq!(r.before(&q("3")), q("7/3"));
    assert_eq!(r.after(&q("3")), q("3"));
    assert_eq!(r.after(&q("4")), q("4"));
    assert_eq!(r.duration_seconds(), &q("4"));
    assert_eq!(
        r.seek(&q("1")),
        SeekPosition::Hold {
            group_id: "a".into(),
            boundary: q("2"),
            elapsed_seconds: q("0")
        }
    );
    assert_eq!(
        r.seek(&q("7/6")),
        SeekPosition::Hold {
            group_id: "a".into(),
            boundary: q("2"),
            elapsed_seconds: q("1/6")
        }
    );
    assert_eq!(r.seek(&q("4/3")), SeekPosition::Music { quarter: q("2") });
    assert_eq!(
        r.seek(&q("11/6")),
        SeekPosition::Music { quarter: q("5/2") }
    );
    assert_eq!(r.seek(&q("3")), SeekPosition::Music { quarter: q("3") });
    assert_eq!(r.events[1].seconds_start, q("4/3"));
    assert_eq!(r.events[1].seconds_end, q("3"));
    assert_eq!(r.events[2].seconds_start, q("3"));
    assert_eq!(r.measures[0].seconds_end, q("4"));
    assert!(
        !r.diagnostics
            .iter()
            .any(|d| matches!(d, TimingDiagnostic::DefaultQuarterTempo { .. }))
    );
    for quarter in ["0", "1/3", "1", "2", "5/2", "3", "11/3", "4"] {
        assert_eq!(
            r.seek(&r.after(&q(quarter))),
            SeekPosition::Music {
                quarter: q(quarter)
            }
        );
    }
}

#[test]
fn repeats_use_distinct_occurrence_ids_and_skipped_signs_stay_audited() {
    let f = Fixture::new(
        1,
        3,
        "2",
        vec![
            note("a", 0, 0, "0", "2", true),
            note("skipped", 0, 1, "0", "2", true),
            note("c", 0, 2, "0", "2", false),
        ],
        &[0, 0, 2],
    );
    let r = f.interpret(policy(vec![
        group("first", &["occ0:a"], "1"),
        group("repeat", &["occ1:a"], "2"),
    ]));
    assert_eq!(r.authored.len(), 2);
    assert_eq!(
        r.authored[0].occurrence_ids,
        vec![id("occ0:a"), id("occ1:a")]
    );
    assert!(r.authored[1].occurrence_ids.is_empty());
    assert_eq!(r.fermatas[0].written_id, r.fermatas[1].written_id);
    assert_ne!(r.fermatas[0].id, r.fermatas[1].id);
    assert_eq!(r.fermatas[1].measure_occurrence, 1);
    assert_eq!(r.fermatas[1].quarter_start, q("2"));
    assert_eq!(r.fermatas[1].written_quarter_start, q("0"));
    assert_eq!(r.fermatas[1].written_quarter_end, q("2"));
    assert_eq!(r.events[1].written_quarter_start, q("0"));
    assert_eq!(r.events[1].quarter_start, q("2"));
    assert_eq!(r.measures[1].written_quarter_start, q("0"));
    assert_eq!(r.measures[1].written_quarter_end, q("2"));
    assert_eq!(r.measures[1].quarter_start, q("2"));
    assert_eq!(r.duration_seconds(), &q("6"));
    f.rejects(
        policy(vec![group("written-id-is-not-an-occurrence", &["a"], "1")]),
        "fermata.unknown_member",
    );
}

#[test]
fn tie_stop_and_crossing_note_markers_do_not_relocate_the_hold() {
    let mut f = Fixture::new(
        2,
        1,
        "4",
        vec![
            note("tie-start", 0, 0, "0", "1", false),
            note("tie-stop", 0, 0, "1", "1", true),
            note("crossing", 1, 0, "0", "4", false),
        ],
        &[0],
    );
    f.performed.score.relations.push(ResolvedRelation {
        id: RelationId("tie".into()),
        scope: Scope::Voice(VoiceId("v0".into())),
        kind: RelationKind::Tie {
            sound: true,
            print: true,
        },
        anchors: vec![
            RelationAnchor {
                event: id("occ0:tie-start"),
                at: q("0"),
                pedal_value: None,
                origin: origin("start"),
            },
            RelationAnchor {
                event: id("occ0:tie-stop"),
                at: q("1"),
                pedal_value: None,
                origin: origin("stop"),
            },
        ],
        origin: origin("tie"),
    });
    let before = f.performed.clone();
    let r = f.interpret(policy(vec![group("stop", &["occ0:tie-stop"], "1")]));
    assert_eq!(r.holds()[0].boundary, q("2"));
    assert_eq!(r.fermatas[0].quarter_start, q("1"));
    assert_eq!(r.events[0].quarter_end, q("1"));
    assert_eq!(r.events[0].seconds_end, q("1/2"));
    assert_eq!(r.events[1].quarter_start, q("1"));
    assert_eq!(r.events[1].quarter_end, q("2"));
    assert_eq!(r.events[2].quarter_start, q("0"));
    assert_eq!(r.events[2].quarter_end, q("4"));
    assert_eq!(r.events[2].seconds_end, q("3"));
    assert_eq!(f.performed, before);
}

#[test]
fn final_hold_seek_and_out_of_range_clamps_are_exact() {
    let f = Fixture::new(1, 1, "4", vec![note("final", 0, 0, "0", "4", true)], &[0]);
    let r = f.interpret(policy(vec![group("final", &["occ0:final"], "1/3")]));
    for s in ["-100", "0"] {
        assert_eq!(r.seek(&q(s)), SeekPosition::Music { quarter: q("0") });
    }
    assert_eq!(r.seek(&q("3/2")), SeekPosition::Music { quarter: q("3") });
    assert_eq!(
        r.seek(&q("2")),
        SeekPosition::Hold {
            group_id: "final".into(),
            boundary: q("4"),
            elapsed_seconds: q("0")
        }
    );
    assert_eq!(
        r.seek(&q("13/6")),
        SeekPosition::Hold {
            group_id: "final".into(),
            boundary: q("4"),
            elapsed_seconds: q("1/6")
        }
    );
    for s in ["7/3", "100"] {
        assert_eq!(r.seek(&q(s)), SeekPosition::Music { quarter: q("4") });
    }
    assert_eq!(r.before(&q("100")), q("2"));
    assert_eq!(r.after(&q("100")), q("7/3"));
    assert_eq!(r.after(&q("-1")), q("0"));
}

fn one() -> Fixture {
    Fixture::new(
        1,
        1,
        "4",
        vec![
            note("a", 0, 0, "0", "2", true),
            note("ordinary", 0, 0, "2", "2", false),
        ],
        &[0],
    )
}
fn one_policy() -> ExplicitFermataPolicy {
    policy(vec![group("a", &["occ0:a"], "1")])
}

#[test]
fn invalid_policy_members_groups_and_durations_are_explicit_errors() {
    let f = one();
    f.rejects(policy(vec![]), "fermata.incomplete_policy");
    f.rejects(policy(vec![group("a", &[], "1")]), "fermata.empty_group");
    f.rejects(
        policy(vec![group(" ", &["occ0:a"], "1")]),
        "fermata.group_identity",
    );
    for seconds in ["0", "-1"] {
        f.rejects(
            policy(vec![group("a", &["occ0:a"], seconds)]),
            "fermata.hold_duration",
        );
    }
    for member in ["missing", "occ0:ordinary"] {
        f.rejects(
            policy(vec![group("a", &[member], "1")]),
            "fermata.unknown_member",
        );
    }
    f.rejects(
        policy(vec![group("a", &["occ0:a", "occ0:a"], "1")]),
        "fermata.duplicate_member",
    );
    f.rejects(
        policy(vec![
            group("a", &["occ0:a"], "1"),
            group("b", &["occ0:a"], "1"),
        ]),
        "fermata.duplicate_member",
    );
    f.rejects(
        policy(vec![
            group("a", &["occ0:a"], "1"),
            group("a", &["occ0:a"], "1"),
        ]),
        "fermata.group_identity",
    );
    let f = Fixture::new(
        2,
        1,
        "4",
        vec![
            note("a", 0, 0, "0", "4", true),
            note("b", 1, 0, "2", "2", true),
        ],
        &[0],
    );
    f.rejects(
        policy(vec![
            group("a", &["occ0:a"], "1"),
            group("b", &["occ0:b"], "1"),
        ]),
        "fermata.duplicate_boundary",
    );
}

#[test]
fn receipts_reject_unknown_duplicates_missing_and_source_mismatches() {
    type Mutation = Box<dyn Fn(&mut PerformedScore)>;
    let mutations: Vec<Mutation> = vec![
        Box::new(|p| {
            p.events.push(p.events[0].clone());
        }),
        Box::new(|p| {
            p.events.pop();
        }),
        Box::new(|p| {
            p.events[0].id = id("unknown");
        }),
        Box::new(|p| {
            p.events[0].written_id = id("unknown");
        }),
        Box::new(|p| {
            p.events[0].measure_occurrence = 77;
        }),
        Box::new(|p| {
            p.events[1].written_id = id("a");
        }),
        Box::new(|p| {
            p.score.events.push(p.score.events[0].clone());
        }),
        Box::new(|p| {
            p.score.events[0].duration = Duration::Measured(q("1"));
        }),
        Box::new(|p| {
            p.score.events[0].part = part(1);
        }),
        Box::new(|p| {
            p.score.events[0].staff.part = part(1);
        }),
        Box::new(|p| {
            p.score.events[0].start = q("1");
        }),
        // Omitting both halves still fails the visited-source completeness check.
        Box::new(|p| {
            p.events.remove(0);
            p.score.events.remove(0);
        }),
        Box::new(|p| {
            p.events.pop();
            p.score.events.pop();
        }),
    ];
    for mutate in mutations {
        let mut f = one();
        mutate(&mut f.performed);
        f.rejects(one_policy(), "fermata.event_receipt");
    }
}

#[test]
fn malformed_source_and_measure_receipts_are_rejected() {
    let mut f = one();
    for frame in &mut f.source.frames {
        for record in &mut frame.records {
            if let Record::Mark(m) = record {
                m.target = "#missing".into();
            }
        }
    }
    f.rejects(one_policy(), "fermata.invalid_source");
    let mut f = one();
    let mark = f.source.frames[0]
        .records
        .iter()
        .find(|r| matches!(r, Record::Mark(_)))
        .unwrap()
        .clone();
    f.source.frames[0].records.push(mark);
    f.rejects(one_policy(), "fermata.invalid_source");
    for mutate in [
        |p: &mut PerformedScore| {
            p.measures.push(p.measures[0].clone());
        },
        |p: &mut PerformedScore| {
            p.measures[0].written_id = "missing".into();
        },
        |p: &mut PerformedScore| {
            p.measures[0].length = q("3");
        },
        |p: &mut PerformedScore| {
            p.measures[0].label = "changed".into();
        },
        |p: &mut PerformedScore| {
            p.measures.clear();
        },
    ] {
        let mut f = one();
        mutate(&mut f.performed);
        f.rejects(one_policy(), "fermata.measure_receipt");
    }
}

#[test]
fn invalid_times_grace_and_tempo_are_rejected() {
    let mut f = one();
    f.performed.score.events[0].start = q("-1");
    f.rejects(one_policy(), "fermata.invalid_time");
    let mut f = one();
    f.performed.score.events[0].duration = Duration::Measured(q("0"));
    f.rejects(one_policy(), "fermata.invalid_duration");
    let mut f = one();
    f.performed.score.events[0].duration = Duration::Grace;
    f.rejects(one_policy(), "fermata.unsupported_grace");
    for bpm in ["0", "-60"] {
        let mut f = one();
        f.performed.score.tempos = vec![tempo("0", bpm)];
        f.rejects(one_policy(), "fermata.invalid_tempo");
    }
    let mut f = one();
    f.performed.score.tempos = vec![tempo("2", "120"), tempo("1", "60")];
    f.rejects(one_policy(), "fermata.tempo_order");
    let mut f = one();
    f.performed.score.tempos = vec![tempo("0", "120"), tempo("0", "60")];
    f.rejects(one_policy(), "fermata.tempo_conflict");
    for at in ["-1", "5"] {
        let mut f = one();
        f.performed.score.tempos = vec![tempo(at, "120")];
        f.rejects(one_policy(), "fermata.invalid_time");
    }
}

#[test]
fn dotted_beats_equivalent_duplicates_and_missing_initial_tempo() {
    let mut f = Fixture::new(1, 1, "4", vec![note("plain", 0, 0, "0", "4", false)], &[0]);
    let mut dotted = tempo("2", "40");
    dotted.value.beat.dotted = true;
    f.performed.score.tempos = vec![dotted, tempo("2", "60")];
    let r = f.interpret(policy(vec![]));
    assert_eq!(r.duration_seconds(), &q("3"));
    assert_eq!(
        r.diagnostics,
        vec![TimingDiagnostic::DefaultQuarterTempo {
            quarter_bpm: q("120"),
            until_quarter: q("2")
        }]
    );
    assert_eq!(r.seek(&q("2")), SeekPosition::Music { quarter: q("3") });
}

#[test]
fn caller_selected_count_rational_and_timeline_budgets_are_enforced() {
    let f = one();
    let limits = TimingLimits {
        maximum_events: 1,
        ..Default::default()
    };
    assert_eq!(
        interpret(&f.source, &f.performed, &one_policy(), limits)
            .unwrap_err()
            .code,
        "fermata.timing_budget"
    );
    for limits in [
        TimingLimits {
            maximum_measures: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_authored_fermatas: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_hold_groups: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_group_members: 0,
            ..Default::default()
        },
        TimingLimits {
            maximum_source_records: 0,
            ..Default::default()
        },
    ] {
        assert_eq!(
            interpret(&f.source, &f.performed, &one_policy(), limits)
                .unwrap_err()
                .code,
            "fermata.timing_budget"
        );
    }
    let mut tempo_fixture = one();
    tempo_fixture.performed.score.tempos.push(tempo("0", "120"));
    assert_eq!(
        interpret(
            &tempo_fixture.source,
            &tempo_fixture.performed,
            &one_policy(),
            TimingLimits {
                maximum_tempos: 0,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        "fermata.timing_budget"
    );
    for cap in ["0", "-1"] {
        assert_eq!(
            interpret(
                &f.source,
                &f.performed,
                &one_policy(),
                TimingLimits {
                    maximum_timeline_seconds: Some(q(cap)),
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
            "fermata.invalid_limits"
        );
    }
    assert_eq!(
        interpret(
            &f.source,
            &f.performed,
            &one_policy(),
            TimingLimits {
                maximum_rational_bits: 2,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        "fermata.rational_budget"
    );
    assert_eq!(
        interpret(
            &f.source,
            &f.performed,
            &one_policy(),
            TimingLimits {
                maximum_timeline_seconds: Some(q("2")),
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        "fermata.timeline_budget"
    );
    let long = f.interpret(policy(vec![group("long-hold", &["occ0:a"], "21600")]));
    assert_eq!(long.duration_seconds(), &q("21602"));
    let exact = f.interpret(policy(vec![group("exact-limit", &["occ0:a"], "21598")]));
    assert_eq!(exact.duration_seconds(), &q("21600"));
    let mut f = one();
    f.performed.score.tempos = vec![tempo("0", "1/100")];
    assert!(f.interpret(one_policy()).duration_seconds() > &q("21600"));
}

#[test]
fn ordinary_source_projectors_still_refuse_fermatas() {
    let f = one();
    assert!(
        lower_document(&f.source)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "fermata.performance_unsupported")
    );
    assert!(
        perform_document(&f.source, PerformanceLimits::default())
            .unwrap_err()
            .iter()
            .any(|d| d.code == "fermata.performance_unsupported")
    );
    assert!(
        interpret(
            &f.source,
            &f.performed,
            &one_policy(),
            TimingLimits::default()
        )
        .is_ok()
    );
    assert!(f.source.required_extensions.iter().any(|e| e == EXTENSION));
}

#[test]
fn empty_route_and_cross_part_occurrence_mismatch_fail() {
    let mut f = one();
    f.performed.score.end = q("0");
    f.performed.score.events.clear();
    f.performed.events.clear();
    f.performed.measures.clear();
    f.rejects(policy(vec![]), "fermata.invalid_projection");
    let mut f = Fixture::new(2, 1, "4", vec![note("a", 0, 0, "0", "4", true)], &[0]);
    f.performed.measures[1].occurrence_index = 1;
    f.rejects(one_policy(), "fermata.measure_receipt");
}

#[test]
fn absent_source_measure_label_uses_written_measure_id() {
    let mut f = one();
    for frame in &mut f.source.frames {
        for record in &mut frame.records {
            if let Record::Measure(m) = record {
                m.attributes.remove("label");
            }
        }
    }
    f.performed.measures[0].label = "m0_0".into();
    let r = f.interpret(one_policy());
    assert_eq!(r.measures[0].label, "m0_0");
}

#[test]
fn omitted_source_grace_attack_is_rejected_even_with_complete_measured_receipts() {
    let mut f = one();
    f.source.frames[0].records.push(Record::Event(Event {
        voice: "v0".into(),
        pitch: "D4".parse().unwrap(),
        duration: Duration::Grace,
        attributes: [
            ("id".into(), Scalar::String("grace".into())),
            ("kind".into(), Scalar::String("grace".into())),
            ("anchor".into(), Scalar::String("#a".into())),
            ("order".into(), Scalar::Integer(-1)),
        ]
        .into(),
        source: SourceRange::default(),
    }));
    let errors: Vec<_> = scorestream_core::validate(&f.source, None)
        .into_iter()
        .filter(|d| d.severity == scorestream_core::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Valid authored grace should pass core validation: {errors:?}"
    );
    assert!(
        !f.performed
            .events
            .iter()
            .any(|e| e.written_id == id("grace"))
    );
    f.rejects(one_policy(), "fermata.unsupported_grace");
}

#[test]
fn oversized_source_time_is_rejected_before_tempo_integration() {
    let oversized = format!("1{}", "0".repeat(1234));
    let f = Fixture::new(
        1,
        1,
        &oversized,
        vec![note("a", 0, 0, "0", "1", true)],
        &[0],
    );
    f.rejects(one_policy(), "fermata.rational_budget");
}

#[test]
fn default_timeline_is_unbounded_and_long_caller_budget_is_valid() {
    let f = one();
    let mut policy = one_policy();
    policy.groups[0].added_seconds = q("21601");
    let unlimited = interpret(&f.source, &f.performed, &policy, TimingLimits::default()).unwrap();
    assert!(unlimited.duration_seconds() > &q("21600"));
    interpret(
        &f.source,
        &f.performed,
        &policy,
        TimingLimits {
            maximum_timeline_seconds: Some(q("30000")),
            ..Default::default()
        },
    )
    .unwrap();
}
