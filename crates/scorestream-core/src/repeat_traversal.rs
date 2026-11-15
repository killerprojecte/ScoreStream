//! Typed ordinary-repeat traversal, separate from authored notation and audio.
//!
//! Input is already reconciled score-wide navigation, not arbitrary MusicXML or
//! per-part marks. This module neither changes the authoring score nor expands
//! notes. Consumers replay authored controls for each `ReplayMeasure`, restore
//! the captured entry state at backward edges, and execute no skipped controls.
//! Ties/other relations must be rebound and validated across performed edges.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Boundary(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatStart {
    Explicit(Boundary),
    /// Preserve the absence of an authored forward-repeat sign.
    ImplicitMovementStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndingClosure {
    Stop,
    Discontinue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndingEnd {
    Closed {
        at: Boundary,
        closure: EndingClosure,
    },
    /// Not interchangeable with an authored stop/discontinue at movement end.
    Unclosed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ending {
    /// Explicit positive pass numbers, not display text such as "1.".
    pub passes: Vec<u32>,
    pub start: Boundary,
    pub end: EndingEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepeatRegion {
    pub id: String,
    pub start: RepeatStart,
    /// Boundary after the first ending, or after the body if there are no endings.
    pub backward: Boundary,
    /// MusicXML `times` is supported only without endings. None selects the
    /// documented two-pass convention, with a returned policy diagnostic.
    pub times: Option<u32>,
    /// Minimum profile: none, or a closed first ending followed by a final ending.
    /// The first may apply to passes 1..N-1; the final applies only to pass N.
    pub endings: Vec<Ending>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Movement {
    pub id: String,
    /// Labels are display strings. Positions, not parsed labels, drive traversal.
    pub measure_labels: Vec<String>,
    pub repeats: Vec<RepeatRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnclosedFinalEndingPolicy {
    Reject,
    /// Only when the final ending is the last navigation structure. This does
    /// not add or locate a missing engraving endpoint in the authored model.
    ContinueToMovementEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub allow_implicit_movement_start: bool,
    pub unclosed_final_ending: UnclosedFinalEndingPolicy,
    /// Caller-selected output allocation budget, not an XML/source-size limit.
    pub maximum_occurrences: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            allow_implicit_movement_start: true,
            unclosed_final_ending: UnclosedFinalEndingPolicy::Reject,
            maximum_occurrences: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    ConventionalTwoPasses {
        repeat_id: String,
    },
    ImplicitStartAtMovementStart {
        repeat_id: String,
    },
    /// The source remains unclosed; this is a performance policy, not a repair.
    UnclosedFinalEndingContinues {
        repeat_id: String,
        start: Boundary,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepeatPass {
    pub repeat_index: usize,
    pub pass: u32,
    pub total_passes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Begin with this movement's authored initial state, not prior-movement IDs.
    BeginMovement,
    /// Capture state BEFORE executing any controls in the entry measure.
    CaptureCheckpoint {
        repeat_index: usize,
        before: Boundary,
    },
    /// A non-sequential performed edge. Restore the saved state, then replay the
    /// target measure. Relations across this edge require separate validation.
    RestoreCheckpoint {
        repeat_index: usize,
        from: Boundary,
        to: Boundary,
        pass: u32,
    },
    ReplayMeasure {
        measure_index: usize,
        occurrence_index: usize,
        repeat_pass: Option<RepeatPass>,
    },
    /// Controls and notes in [from, to) are NOT executed; current performed state
    /// continues. Do not load a linear written-state snapshot at the destination.
    SkipFirstEnding {
        repeat_index: usize,
        from: Boundary,
        to: Boundary,
        pass: u32,
    },
    ReleaseCheckpoint {
        repeat_index: usize,
    },
    EndMovement,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovementPlan {
    pub movement_id: String,
    pub steps: Vec<Step>,
    pub occurrences: usize,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyMovementId,
    DuplicateMovementId,
    EmptyMovement,
    InvalidRepeatId,
    InvalidBoundary,
    ImplicitStartNotAllowed,
    InvalidRepeatCount,
    UnsupportedEndingShape,
    UnclosedFirstEnding,
    UnclosedFinalEnding,
    ConflictingRepeatCountAndEndings,
    OverlappingOrNestedRepeats,
    OccurrenceBudget,
    MovementJoinCount,
}

struct Region {
    source_index: usize,
    start: usize,
    backward: usize,
    body_end: usize,
    exit: usize,
    plays: u32,
    has_endings: bool,
}

fn region(
    input: &RepeatRegion,
    source_index: usize,
    measures: usize,
    policy: Policy,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Region, Error> {
    let start = match input.start {
        RepeatStart::Explicit(b) => b.0,
        RepeatStart::ImplicitMovementStart => {
            if !policy.allow_implicit_movement_start {
                return Err(Error::ImplicitStartNotAllowed);
            }
            diagnostics.push(Diagnostic::ImplicitStartAtMovementStart {
                repeat_id: input.id.clone(),
            });
            0
        }
    };
    if start >= input.backward.0 || input.backward.0 > measures {
        return Err(Error::InvalidBoundary);
    }
    let mut result = Region {
        source_index,
        start,
        backward: input.backward.0,
        body_end: input.backward.0,
        exit: input.backward.0,
        plays: 2,
        has_endings: !input.endings.is_empty(),
    };
    if input.endings.is_empty() {
        result.plays = match input.times {
            Some(0) => return Err(Error::InvalidRepeatCount),
            Some(n) => n,
            None => {
                diagnostics.push(Diagnostic::ConventionalTwoPasses {
                    repeat_id: input.id.clone(),
                });
                2
            }
        };
        return Ok(result);
    }
    if input.times.is_some() {
        return Err(Error::ConflictingRepeatCountAndEndings);
    }
    let [first, final_ending] = input.endings.as_slice() else {
        return Err(Error::UnsupportedEndingShape);
    };
    let EndingEnd::Closed { at: first_end, .. } = first.end else {
        return Err(Error::UnclosedFirstEnding);
    };
    let [last_pass] = final_ending.passes.as_slice() else {
        return Err(Error::UnsupportedEndingShape);
    };
    if *last_pass < 2 || first.passes.len() != (*last_pass as usize).saturating_sub(1) {
        return Err(Error::UnsupportedEndingShape);
    }
    let mut first_passes = first.passes.clone();
    first_passes.sort_unstable();
    if first_passes.iter().copied().ne(1..*last_pass) {
        return Err(Error::UnsupportedEndingShape);
    }
    if first.start.0 < start
        || first.start.0 >= first_end.0
        || first_end != input.backward
        || final_ending.start != first_end
    {
        return Err(Error::InvalidBoundary);
    }
    result.body_end = first.start.0;
    result.plays = *last_pass;
    result.exit = match final_ending.end {
        EndingEnd::Closed { at, .. } => at.0,
        EndingEnd::Unclosed => {
            if policy.unclosed_final_ending == UnclosedFinalEndingPolicy::Reject {
                return Err(Error::UnclosedFinalEnding);
            }
            diagnostics.push(Diagnostic::UnclosedFinalEndingContinues {
                repeat_id: input.id.clone(),
                start: final_ending.start,
            });
            measures
        }
    };
    if result.exit <= final_ending.start.0 || result.exit > measures {
        return Err(Error::InvalidBoundary);
    }
    Ok(result)
}

/// Compile ordinary non-nested repeats to typed state/replay steps. Unknown
/// navigation must be rejected by the adapter, never omitted before calling this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validation {
    pub occurrences: usize,
    pub diagnostics: Vec<Diagnostic>,
}
/// Validate and count a route without expanding any performed occurrences.
pub fn validate(movement: &Movement, policy: Policy) -> Result<Validation, Error> {
    let (_, validation) = prepare(movement, policy)?;
    Ok(validation)
}
fn prepare(movement: &Movement, policy: Policy) -> Result<(Vec<Region>, Validation), Error> {
    if movement.id.is_empty() {
        return Err(Error::EmptyMovementId);
    }
    let n = movement.measure_labels.len();
    if n == 0 {
        return Err(Error::EmptyMovement);
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut diagnostics = Vec::new();
    let mut regions = Vec::new();
    for (index, repeat) in movement.repeats.iter().enumerate() {
        if repeat.id.is_empty() || !ids.insert(&repeat.id) {
            return Err(Error::InvalidRepeatId);
        }
        regions.push(region(repeat, index, n, policy, &mut diagnostics)?);
    }
    regions.sort_by_key(|r| r.start);
    if regions.windows(2).any(|r| r[1].start < r[0].exit) {
        return Err(Error::OverlappingOrNestedRepeats);
    }
    let mut occurrences = n;
    for r in &regions {
        let extra = (r.backward - r.start)
            .checked_mul((r.plays - 1) as usize)
            .ok_or(Error::OccurrenceBudget)?;
        occurrences = occurrences
            .checked_add(extra)
            .ok_or(Error::OccurrenceBudget)?;
        if r.has_endings {
            occurrences -= r.backward - r.body_end;
        }
    }
    if occurrences > policy.maximum_occurrences {
        return Err(Error::OccurrenceBudget);
    }
    Ok((
        regions,
        Validation {
            occurrences,
            diagnostics,
        },
    ))
}
pub fn traverse(movement: &Movement, policy: Policy) -> Result<MovementPlan, Error> {
    let (
        regions,
        Validation {
            occurrences,
            diagnostics,
        },
    ) = prepare(movement, policy)?;
    let n = movement.measure_labels.len();
    let mut plan = MovementPlan {
        movement_id: movement.id.clone(),
        steps: vec![Step::BeginMovement],
        occurrences,
        diagnostics,
    };
    let mut ordinal = 0;
    let mut cursor = 0;
    for r in regions {
        replay(&mut plan.steps, &mut ordinal, cursor, r.start, None);
        plan.steps.push(Step::CaptureCheckpoint {
            repeat_index: r.source_index,
            before: Boundary(r.start),
        });
        for pass in 1..=r.plays {
            if pass > 1 {
                plan.steps.push(Step::RestoreCheckpoint {
                    repeat_index: r.source_index,
                    from: Boundary(r.backward),
                    to: Boundary(r.start),
                    pass,
                });
            }
            let context = Some(RepeatPass {
                repeat_index: r.source_index,
                pass,
                total_passes: r.plays,
            });
            let final_ending = r.has_endings && pass == r.plays;
            replay(
                &mut plan.steps,
                &mut ordinal,
                r.start,
                if final_ending { r.body_end } else { r.backward },
                context,
            );
            if final_ending {
                plan.steps.push(Step::SkipFirstEnding {
                    repeat_index: r.source_index,
                    from: Boundary(r.body_end),
                    to: Boundary(r.backward),
                    pass,
                });
                replay(&mut plan.steps, &mut ordinal, r.backward, r.exit, context);
            }
        }
        plan.steps.push(Step::ReleaseCheckpoint {
            repeat_index: r.source_index,
        });
        cursor = r.exit;
    }
    replay(&mut plan.steps, &mut ordinal, cursor, n, None);
    plan.steps.push(Step::EndMovement);
    debug_assert_eq!(ordinal, plan.occurrences);
    Ok(plan)
}

fn replay(
    steps: &mut Vec<Step>,
    ordinal: &mut usize,
    start: usize,
    end: usize,
    pass: Option<RepeatPass>,
) {
    for index in start..end {
        steps.push(Step::ReplayMeasure {
            measure_index: index,
            occurrence_index: *ordinal,
            repeat_pass: pass,
        });
        *ordinal += 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementJoin {
    /// Explicit sequence intent: no inserted inter-movement pause. This does not
    /// tie notes or reuse part IDs between movements, nor truncate release tails.
    Attacca,
    /// A separate movement; no arbitrary pause duration is invented by this core.
    Separate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequencePlan {
    pub movements: Vec<MovementPlan>,
    pub joins: Vec<MovementJoin>,
}

pub fn sequence(
    movements: &[Movement],
    joins: &[MovementJoin],
    policy: Policy,
) -> Result<SequencePlan, Error> {
    if movements.is_empty() {
        return Err(Error::EmptyMovement);
    }
    if joins.len() != movements.len() - 1 {
        return Err(Error::MovementJoinCount);
    }
    let mut ids = std::collections::BTreeSet::new();
    if movements.iter().any(|m| !ids.insert(&m.id)) {
        return Err(Error::DuplicateMovementId);
    }
    let mut total = 0usize;
    let mut plans = Vec::new();
    for movement in movements {
        let plan = traverse(movement, policy)?;
        total = total
            .checked_add(plan.occurrences)
            .ok_or(Error::OccurrenceBudget)?;
        if total > policy.maximum_occurrences {
            return Err(Error::OccurrenceBudget);
        }
        plans.push(plan);
    }
    Ok(SequencePlan {
        movements: plans,
        joins: joins.to_vec(),
    })
}
