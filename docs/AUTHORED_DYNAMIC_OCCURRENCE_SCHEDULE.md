# Explicit authored-dynamic occurrence inspection

This is an **inspectable schedule and qualification report, not gain realization**.
The selected entry point is:

```rust
let schedule = scorestream_core::notation::inspect_authored_dynamic_schedule(
    &document,
    scorestream_core::notation::AuthoredDynamicSchedulePolicy::AdjacentWrittenOwnersV1,
    scorestream_core::notation::PerformanceLimits::default(),
)?;
let report = serde_json::to_string_pretty(&schedule)?;
```

There is no implicit/default policy argument. The result always says
`execution_withheld: true`, does not contain notes or gains, and does not unlock
normal playback. All existing authored-dynamic playback refusals stay intact.

## Why not inject legacy numeric state

The current app NumericDynamics implementation chooses attack velocity **and**
builds gain-step envelopes during already sounding notes, including held/tied
chains. Injecting boundary values into that machinery would therefore violate
the required attack-only behavior. This patch neither feeds that machinery nor
changes it. It does not claim that a qualified schedule is an executable plan.

## Source and occurrence identity

The report retains the complete typed written lane, even if a traversal does
not visit a statement. Every occurrence separately records its source index,
full authored statement, measure occurrence index, part-local visit index,
visit start/length, and exact performed visual and sound times. Fractions stay
exact. Values, scopes, offsets and source owners are never coalesced.

For ordinary playback, written measure starts supply the non-repeated visits.
For repeats/endings, the existing validated perform_document route is reused
on a private discovery copy with only the quarantined authored-dynamic lane
removed. That temporary copy is used solely to obtain measure occurrences.
No resolved score, altered document or audio plan escapes the API. Every other
capability and qualified-performance restriction remains in force: unsupported
combined policies cause an error rather than silently stripping more features.
This is not a general alternate repeat traverser.

## The only assigned two-statement ordering

A same-exact-part/staff/time group qualifies as
`adjacent_terminal_before_opening` only when all of these hold:

1. There are exactly two authored statements at that performed scope/time.
2. One has source cursor at its owning measure end and sound at that visit end.
3. The other has source cursor zero and sound at its visit start.
4. The written owners are consecutive measures in that part.
5. The actual performed visits are consecutive in that part, with the terminal
   visit ending exactly where the opening visit begins.

Only then does the report provide explicit `terminal_then_opening` occurrence
indices. Both percentages remain present. There is no rule based merely on
array order, XML encounter order across jumps, nearby floating-point times,
matching written clock positions, or greater/lower numeric value.

A single statement is `isolated_exact`, not an inferred gain or implicit mute.
Multiple statements without that proof are `unresolved`; no ordering indices
are supplied. Final endpoints remain present. Unpaired terminal/opening visits
and written statements with no visit receive specific disclosures. Different
part/staff groups are not combined into a guessed scope-precedence policy.

## Verified cases

- Ordinary terminal p / opening ff: both values retained, one qualified pair.
- Complete two-bar repeat: distinct qualified pairs at performed times 1 and 3.
- Repeat of the terminal bar alone: at the backward jump, terminal/opening in
  the same written owner remains unresolved; the later true adjacent exit is
  independently qualified.
- First/second ending traversal: written terminal/opening neighbors occur at
  performed times 2 and 3, not the same visit boundary; both remain isolated.
- Three simultaneous authored statements: unresolved, with none discarded.
- Final terminal: exact isolated value retained, no invented next attack.
- Explicit caller occurrence/control budgets reject excess without mutation.
- Original II/P6 and P8: the existing byte-pinned production walker proof also
  checks the occurrence report for each actual two-direction family projection:
  visual 2/3, shared sound boundary 3/2, preserved 44.44 and 106.67, qualified
  neighboring owners, and execution still withheld.

Tests verify that the source Document is unchanged and normal plan_source still
rejects the authored lane after inspection. Existing authored storage and legacy
dynamics tests are unchanged. No full workspace or whole-original playback
acceptance is claimed.

## Remaining execution seam

A later explicit attack-only realization must consume this occurrence evidence,
settle any unresolved groups, determine applicable assignments for *new attacks*,
and leave existing note/tie envelopes unchanged. It must also expose the chosen
policy and preserve the full source lane. Qualifying an adjacent pair here is
necessary evidence, not permission to run generic last-wins in NumericDynamics.

A small explicitly selected practice-only attack-percentage consumer is documented
in PRACTICE_BOUNDARY_ATTACK_RESOLVER.md. It verifies exact attack identity/time,
never updates held notes, and does not unlock normal playback or imply renderer
integration.
