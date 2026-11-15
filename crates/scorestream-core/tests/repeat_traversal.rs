use scorestream_core::repeat_traversal::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

fn movement(measures: usize, repeats: Vec<RepeatRegion>) -> Movement {
    Movement {
        id: "test".into(),
        measure_labels: (1..=measures).map(|n| n.to_string()).collect(),
        repeats,
    }
}
fn plain(start: usize, end: usize, times: Option<u32>) -> RepeatRegion {
    RepeatRegion {
        id: "r".into(),
        start: RepeatStart::Explicit(Boundary(start)),
        backward: Boundary(end),
        times,
        endings: vec![],
    }
}
fn volta(start: usize, first: usize, backward: usize, last_end: EndingEnd) -> RepeatRegion {
    RepeatRegion {
        id: "r".into(),
        start: RepeatStart::Explicit(Boundary(start)),
        backward: Boundary(backward),
        times: None,
        endings: vec![
            Ending {
                passes: vec![1],
                start: Boundary(first),
                end: EndingEnd::Closed {
                    at: Boundary(backward),
                    closure: EndingClosure::Stop,
                },
            },
            Ending {
                passes: vec![2],
                start: Boundary(backward),
                end: last_end,
            },
        ],
    }
}
fn visits(plan: &MovementPlan) -> Vec<usize> {
    plan.steps
        .iter()
        .filter_map(|s| match s {
            Step::ReplayMeasure { measure_index, .. } => Some(*measure_index),
            _ => None,
        })
        .collect()
}
fn closed(at: usize) -> EndingEnd {
    EndingEnd::Closed {
        at: Boundary(at),
        closure: EndingClosure::Discontinue,
    }
}

#[test]
fn repeatless_and_explicit_counts_preserve_labels_and_indices() {
    let mut m = movement(3, vec![]);
    m.measure_labels = vec!["0".into(), "1a".into(), "1a".into()];
    assert_eq!(
        visits(&traverse(&m, Policy::default()).unwrap()),
        vec![0, 1, 2]
    );
    assert_eq!(m.measure_labels, ["0", "1a", "1a"]);
    for count in [1, 2, 3] {
        let m = movement(4, vec![plain(1, 3, Some(count))]);
        let p = traverse(&m, Policy::default()).unwrap();
        let expected: Vec<_> = std::iter::once(0)
            .chain((0..count).flat_map(|_| [1, 2]))
            .chain([3])
            .collect();
        assert_eq!(visits(&p), expected);
        assert!(p.diagnostics.is_empty());
        assert_eq!(
            p.steps
                .iter()
                .filter(|s| matches!(s, Step::RestoreCheckpoint { .. }))
                .count(),
            count as usize - 1
        );
    }
}

#[test]
fn implicit_start_and_conventional_count_are_visible_policies() {
    let mut r = plain(0, 2, None);
    r.start = RepeatStart::ImplicitMovementStart;
    let m = movement(3, vec![r]);
    let p = traverse(&m, Policy::default()).unwrap();
    assert_eq!(visits(&p), [0, 1, 0, 1, 2]);
    assert_eq!(p.diagnostics.len(), 2);
    assert_eq!(
        traverse(
            &m,
            Policy {
                allow_implicit_movement_start: false,
                ..Policy::default()
            }
        ),
        Err(Error::ImplicitStartNotAllowed)
    );
}

#[test]
fn closed_first_and_final_endings_keep_pass_identity() {
    let m = movement(6, vec![volta(1, 3, 4, closed(5))]);
    let plan = traverse(&m, Policy::default()).unwrap();
    assert_eq!(visits(&plan), [0, 1, 2, 3, 1, 2, 4, 5]);
    assert!(plan.steps.contains(&Step::SkipFirstEnding {
        repeat_index: 0,
        from: Boundary(3),
        to: Boundary(4),
        pass: 2
    }));
    let ordinals: Vec<_> = plan
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::ReplayMeasure {
                occurrence_index, ..
            } => Some(*occurrence_index),
            _ => None,
        })
        .collect();
    assert_eq!(ordinals, (0..8).collect::<Vec<_>>());
    let mut multi = m;
    multi.repeats[0].endings[0].passes = vec![2, 1];
    multi.repeats[0].endings[1].passes = vec![3];
    assert_eq!(
        visits(&traverse(&multi, Policy::default()).unwrap()),
        [0, 1, 2, 3, 1, 2, 3, 1, 2, 4, 5]
    );
}

#[test]
fn checkpoint_restore_and_skip_do_not_leak_first_ending_controls() {
    let m = movement(4, vec![volta(1, 2, 3, closed(4))]);
    let p = traverse(&m, Policy::default()).unwrap();
    // Prefix establishes A. First ending changes to B. The repeated body and
    // final ending inherit A, NOT B from the skipped first ending's written path.
    let authored_controls = [Some("A"), None, Some("B"), None];
    let mut state = None;
    let mut checkpoints = BTreeMap::new();
    let mut observed = Vec::new();
    for step in p.steps {
        match step {
            Step::CaptureCheckpoint { repeat_index, .. } => {
                checkpoints.insert(repeat_index, state);
            }
            Step::RestoreCheckpoint { repeat_index, .. } => {
                state = checkpoints[&repeat_index];
            }
            Step::ReplayMeasure { measure_index, .. } => {
                if let Some(value) = authored_controls[measure_index] {
                    state = Some(value);
                }
                observed.push(state);
            }
            Step::SkipFirstEnding { .. } => {} // Crucially executes no controls.
            _ => {}
        }
    }
    assert_eq!(
        observed,
        [Some("A"), Some("A"), Some("B"), Some("A"), Some("A")]
    );
}

#[test]
fn unclosed_final_ending_needs_policy_and_is_never_repaired() {
    let m = movement(7, vec![volta(1, 3, 5, EndingEnd::Unclosed)]);
    let original = m.clone();
    assert_eq!(
        traverse(&m, Policy::default()),
        Err(Error::UnclosedFinalEnding)
    );
    let policy = Policy {
        unclosed_final_ending: UnclosedFinalEndingPolicy::ContinueToMovementEnd,
        ..Policy::default()
    };
    let plan = traverse(&m, policy).unwrap();
    assert_eq!(visits(&plan), [0, 1, 2, 3, 4, 1, 2, 5, 6]);
    assert_eq!(m, original);
    assert!(plan.diagnostics.iter().any(|d| matches!(
        d,
        Diagnostic::UnclosedFinalEndingContinues {
            start: Boundary(5),
            ..
        }
    )));
    let mut later = m;
    let mut r = plain(5, 7, None);
    r.id = "later".into();
    later.repeats.push(r);
    assert_eq!(
        traverse(&later, policy),
        Err(Error::OverlappingOrNestedRepeats)
    );
}

#[test]
fn malformed_unsupported_and_over_budget_routes_fail_without_a_plan() {
    for r in [plain(2, 2, None), plain(2, 1, None), plain(0, 5, None)] {
        assert_eq!(
            traverse(&movement(4, vec![r]), Policy::default()),
            Err(Error::InvalidBoundary)
        );
    }
    assert_eq!(
        traverse(&movement(4, vec![plain(0, 2, Some(0))]), Policy::default()),
        Err(Error::InvalidRepeatCount)
    );
    assert_eq!(
        traverse(
            &movement(4, vec![plain(0, 2, Some(u32::MAX))]),
            Policy::default()
        ),
        Err(Error::OccurrenceBudget)
    );
    let mut r = volta(0, 1, 2, closed(3));
    r.endings[0].end = EndingEnd::Unclosed;
    assert_eq!(
        traverse(&movement(4, vec![r]), Policy::default()),
        Err(Error::UnclosedFirstEnding)
    );
    let mut r = volta(0, 1, 2, closed(3));
    r.endings[0].passes = vec![2];
    assert_eq!(
        traverse(&movement(4, vec![r]), Policy::default()),
        Err(Error::UnsupportedEndingShape)
    );
    let mut r = volta(0, 1, 2, closed(3));
    r.endings[1].passes = vec![3];
    assert_eq!(
        traverse(&movement(4, vec![r]), Policy::default()),
        Err(Error::UnsupportedEndingShape)
    );
    let mut r = volta(0, 1, 2, closed(3));
    r.times = Some(2);
    assert_eq!(
        traverse(&movement(4, vec![r]), Policy::default()),
        Err(Error::ConflictingRepeatCountAndEndings)
    );
    let mut inner = plain(1, 2, None);
    inner.id = "inner".into();
    assert_eq!(
        traverse(
            &movement(4, vec![plain(0, 3, None), inner]),
            Policy::default()
        ),
        Err(Error::OverlappingOrNestedRepeats)
    );
    assert_eq!(
        traverse(
            &movement(4, vec![plain(0, 2, None), plain(2, 4, None)]),
            Policy::default()
        ),
        Err(Error::InvalidRepeatId)
    );
}

#[test]
fn sequential_regions_are_ordered_and_movement_joins_do_not_merge_identities() {
    let mut second = plain(2, 4, Some(2));
    second.id = "second".into();
    let a = movement(4, vec![second, plain(0, 2, Some(2))]);
    assert_eq!(
        visits(&traverse(&a, Policy::default()).unwrap()),
        [0, 1, 0, 1, 2, 3, 2, 3]
    );
    let mut b = movement(1, vec![]);
    b.id = "next".into();
    let s = sequence(
        &[a.clone(), b.clone()],
        &[MovementJoin::Attacca],
        Policy::default(),
    )
    .unwrap();
    assert_eq!(s.joins, [MovementJoin::Attacca]);
    assert_eq!(s.movements[0].movement_id, "test");
    assert_eq!(s.movements[1].movement_id, "next");
    assert!(matches!(s.movements[1].steps[0], Step::BeginMovement));
    assert_eq!(
        sequence(&[a.clone(), b.clone()], &[], Policy::default()),
        Err(Error::MovementJoinCount)
    );
    assert_eq!(
        sequence(
            &[a.clone(), a],
            &[MovementJoin::Separate],
            Policy::default()
        ),
        Err(Error::DuplicateMovementId)
    );
    assert_eq!(
        sequence(
            &[
                b.clone(),
                Movement {
                    id: "third".into(),
                    ..b
                }
            ],
            &[MovementJoin::Separate],
            Policy {
                maximum_occurrences: 1,
                ..Policy::default()
            }
        ),
        Err(Error::OccurrenceBudget)
    );
}

#[derive(Deserialize)]
struct Audit {
    movements: Vec<AuditedMovement>,
}
#[derive(Deserialize)]
struct AuditedMovement {
    id: String,
    measure_labels: Vec<String>,
    part_count: usize,
    repeat_marks: Vec<RepeatMark>,
    ending_marks: Vec<EndingMark>,
    sound_navigation: Vec<serde_json::Value>,
    attacca: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct RepeatMark {
    boundary: usize,
    attributes: BTreeMap<String, String>,
    parts: Vec<String>,
}
#[derive(Deserialize)]
struct EndingMark {
    boundary: usize,
    number: String,
    phase: String,
    parts: Vec<String>,
}

fn corpus() -> Vec<(Movement, usize)> {
    let audit: Audit =
        serde_json::from_str(include_str!("fixtures/beethoven-navigation.json")).unwrap();
    audit
        .movements
        .into_iter()
        .map(|m| {
            assert!(m.sound_navigation.is_empty());
            if m.id.ends_with('3') {
                assert_eq!(m.attacca.len(), 1);
                assert_eq!(m.attacca[0]["measure_index"], 375);
                assert_eq!(m.attacca[0]["text"], "Attacca");
            } else {
                assert!(m.attacca.is_empty());
            }
            let mut start = RepeatStart::ImplicitMovementStart;
            let mut repeats = Vec::new();
            for mark in m.repeat_marks {
                assert_eq!(mark.parts.len(), m.part_count);
                assert_eq!(
                    mark.parts.iter().collect::<BTreeSet<_>>().len(),
                    m.part_count
                );
                assert_eq!(mark.attributes.len(), 1); // corpus has no ignored repeat options
                if mark.attributes["direction"] == "forward" {
                    start = RepeatStart::Explicit(Boundary(mark.boundary));
                } else {
                    assert_eq!(mark.attributes["direction"], "backward");
                    repeats.push(RepeatRegion {
                        id: "r".into(),
                        start,
                        backward: Boundary(mark.boundary),
                        times: None,
                        endings: vec![],
                    });
                }
            }
            assert!(repeats.len() <= 1);
            let mut open = BTreeMap::<String, usize>::new();
            if let Some(repeat) = repeats.first_mut() {
                for mark in m.ending_marks {
                    assert_eq!(mark.parts, ["P1"]);
                    if mark.phase == "start" {
                        let index = repeat.endings.len();
                        assert!(open.insert(mark.number.clone(), index).is_none());
                        repeat.endings.push(Ending {
                            passes: vec![mark.number.parse().unwrap()],
                            start: Boundary(mark.boundary),
                            end: EndingEnd::Unclosed,
                        });
                    } else {
                        let index = open.remove(&mark.number).unwrap();
                        let closure = match mark.phase.as_str() {
                            "stop" => EndingClosure::Stop,
                            "discontinue" => EndingClosure::Discontinue,
                            _ => panic!("unknown phase"),
                        };
                        repeat.endings[index].end = EndingEnd::Closed {
                            at: Boundary(mark.boundary),
                            closure,
                        };
                    }
                }
            } else {
                assert!(m.ending_marks.is_empty());
            }
            (
                Movement {
                    id: m.id,
                    measure_labels: m.measure_labels,
                    repeats,
                },
                m.part_count,
            )
        })
        .collect()
}

#[test]
fn pinned_original_navigation_has_exact_corpus_routes_and_attacca_boundary() {
    let all = corpus();
    let movements: Vec<_> = all.iter().map(|(m, _)| m.clone()).collect();
    let policy = Policy {
        unclosed_final_ending: UnclosedFinalEndingPolicy::ContinueToMovementEnd,
        ..Policy::default()
    };
    let sequence = sequence(
        &movements,
        &[
            MovementJoin::Separate,
            MovementJoin::Separate,
            MovementJoin::Attacca,
        ],
        policy,
    )
    .unwrap();
    let expected = [
        (0..124).chain(0..502).collect::<Vec<_>>(),
        (0..248).collect(),
        (0..162).chain(141..161).chain(162..376).collect(),
        (0..86).chain(1..84).chain(86..446).collect(),
    ];
    for (plan, expected) in sequence.movements.iter().zip(expected) {
        assert_eq!(visits(plan), expected);
    }
    assert_eq!(
        sequence
            .movements
            .iter()
            .map(|p| p.occurrences)
            .collect::<Vec<_>>(),
        [626, 248, 396, 529]
    );
    assert_eq!(
        sequence
            .movements
            .iter()
            .map(|p| p.occurrences)
            .sum::<usize>(),
        1799
    );
    assert_eq!(movements[1].measure_labels[0], "0");
    assert_eq!(movements[3].repeats[0].endings[1].end, EndingEnd::Unclosed);
    assert_eq!(
        traverse(&movements[3], Policy::default()),
        Err(Error::UnclosedFinalEnding)
    );
}

#[test]
fn exhaustive_small_closed_routes_preserve_every_expected_occurrence() {
    for n in 3..=8 {
        for start in 0..n - 1 {
            for first in start..n - 1 {
                for backward in first + 1..n {
                    for exit in backward + 1..=n {
                        let m = movement(n, vec![volta(start, first, backward, closed(exit))]);
                        let p = traverse(&m, Policy::default()).unwrap();
                        let expected: Vec<_> = (0..backward)
                            .chain(start..first)
                            .chain(backward..n)
                            .collect();
                        assert_eq!(visits(&p), expected);
                        assert_eq!(p.occurrences, expected.len());
                        assert!(visits(&p).iter().all(|i| *i < n));
                    }
                }
            }
        }
    }
}
