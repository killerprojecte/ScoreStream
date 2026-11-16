# Explicit practice-only boundary attack percentage resolver

This patch supplies a small reusable resolver, **not an app-wide render switch**.
It is additive to the occurrence schedule and does not change strict playback,
legacy routing, NumericDynamics, note intervals, tie chains or gain envelopes.

```rust
let result = scorestream_core::notation::resolve_practice_boundary_attacks(
    &document,
    &qualified_attack_facts,
    scorestream_core::notation::PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,
    scorestream_core::notation::PerformanceLimits::default(),
)?;
```

The policy must be explicitly selected. It has no default and is not an alias
for normal playback. The function regenerates the validated occurrence schedule
rather than trusting caller-supplied ordering indices. Its output contains that
complete retained source/occurrence evidence and one decision per attack.

## Narrow selection rule

A percentage is returned only for a genuinely new measured pitched attack:
- the schedule proves terminal-before-opening for exactly matching part/staff;
- the candidate belongs to that exact opening measure and performed occurrence;
- onset is at or after the proven opening and strictly before that visit ends;
- the written event ID, exact mapped onset, part and staff match the source;
- the qualified continuity facts identify a new sounding attack, not an older
  chain; an incoming sounding tie in the source independently prevents selection;
- no existing explicit per-note attack percentage is replaced;
- no further overlapping authored assignment has occurred by that onset;
- no matching legacy numeric/sounding dynamic control in the opening owner is
  being overwritten. Event/voice/part/staff targets are checked as applicable.

The returned value is the separately retained **opening statement's exact
percentage**, with both terminal/opening schedule indices. For the proven II
family this is 106.67 = 10667/100; the terminal 44.44 = 1111/25 remains preserved
in the schedule. Neither is rounded, converted to gain, silently coalesced, nor
written into a persistent state.

The window ends at the opening visit's end, even if no new dynamic appears.
It never carries a guessed value across a later measure, repeat jump or ending.
A competing statement stops suggestions; the resolver does not implement a
more general state machine or last-wins. Multiple matching scope pairs remain
ambiguous rather than assuming staff/part priority.

## Caller contract and unchanged sounding material

QualifiedPracticeAttack is a small bridge from the existing qualified route
and continuity layer. It supplies the actual occurrence ID, written event ID,
owner/visit, staff, onset and original sounding chain-attack time. The resolver
checks source identity/time/scope for selected candidates and conservatively
rejects sounding tie continuations. It does not discover or rebuild note routes,
continuity, instrument routing, releases or articulation policies.

The output is a per-occurrence percentage suggestion only. It has no release,
existing-note gain, envelope or tie-edit fields. A practice adapter may consume
Selected decisions only at creation of the named new attack. It must not feed
them into NumericDynamics' persistent held-note envelope machinery. All other
decisions mean leave that attack's existing policy untouched. No application
adapter, WAV export, MIDI velocity conversion or end-to-end renderer wiring is
included here. Normal plan_source still reports the authored-lane performance
requirement after resolution; the returned schedule still says execution is
withheld for that ordinary path.

## Focused evidence

Synthetic source-backed tests cover:
- opening and later new attacks inside the qualified visit;
- an earlier attack and a visit-end attack receiving no suggestion;
- real tied source notes held across the boundary, including a caller falsely
  claiming the tied continuation is a new attack;
- subsequent competing authored sound, existing note percentage and legacy
  numeric control remaining untouched;
- a wrong source ID/onset receiving no suggestion;
- each repeat visit resolving independently, a backward jump not qualifying,
  and first/second ending visits not being paired by written time alone.

The original II P6/P8 byte-pinned production test additionally invokes the
resolver with no attack candidates and confirms it retains the exact verified
boundary schedule unchanged. That is actual-source **schedule evidence**, not a
fabricated original-note transcript or whole-movement audio claim. Full original
attack transcript/route receipts and any practice-render integration are separate.

Targeted verification: authored_dynamic_schedule, authored_dynamics, dynamics,
and the ignored original_two_boundary_pairs_preserve_linked_authored_statements
production test. No full workspace suite or application integration is claimed.
