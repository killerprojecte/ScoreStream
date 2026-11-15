# Explicit following-principal grace playback

The separate, source-qualified Rust API now turns the [authored grace bridge](GRACE_IMPORT.md)
into actual `PlaybackPlan` attacks. It requires a complete caller-selected timing
policy and sound policy. It does not alter the source, infer missing timing,
change the ordinary Play command, produce an audio file, or establish full
original Beethoven import.

## Source, discovery and authorization of timing

`scorestream_app::playback::discover_source_graces(source)` validates the original
SFS, its `scorestream.grace@1` capability and any supported navigation route.
It returns every authored grace, including unvisited ending material, plus each
visited occurrence's unique grace and principal IDs, part, measure occurrence,
source order, optional slash, authored timing fields, written anchor and nominal
principal interval. Repeat visits have distinct IDs; a written ID is not a
policy assignment to every visit.

`plan_source_with_explicit_graces(source, policy,
GraceSoundPolicy::FullGraceThenGatedRemainder)` returns an opaque
`ExplicitGracePlayback` with read-only `plan()` and `schedule()`. The corresponding
core function, `perform_document_with_explicit_graces`, owns validation,
navigation projection and complete allocation. Neither entry point accepts a
caller-assembled performed snapshot. No capability is stripped to enter legacy
lowering. Projected grace events remain unmeasured `Duration::Grace`, anchored to
their principal in that same occurrence. Actual sound windows are a separate
typed result, not editable measured notation.

Every visited principal group must occur exactly once in `ExplicitGracePolicy`:

- `AuthoredFollowingPercent` explicitly selects the source's following-note
  percentage. This first profile accepts only one grace, with only a positive
  `steal_following` value strictly below 100. Effective tempo must be constant
  across the nominal principal interval. It does not choose between quarter-time
  and elapsed-time percentages across a tempo change.
- `ExplicitFollowingQuarters` requires an exact positive allocation for every
  member of a group whose three authored timing fields are all absent. Explicit
  zero is not absence. Caller member order does not override consecutive source
  order. The sum must be strictly less than the nominal principal duration.

The [MusicXML grace definition](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/grace/)
supplies following/previous percentages and make-time in divisions, but not this
engine's group allocation or gate order. [Grace notes lack measured duration](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/).
The selected sound policy is an explicit interpretation, not imported authorship.
Slash, written note type, dots and graphic cue size never choose timing.

## Exact sound windows and controls

For a principal beginning at quarter `q`, nominal duration `D`, and allocations
`d1..dn`, the grace attacks fill `[q, q + sum(d)]` consecutively. The principal
attacks at `q + sum(d)` and releases at
`q + sum(d) + 23/25 * (D - sum(d))`. Graces use their full allocations. The gate
operates in quarters before exact tempo integration. Other voices, source
measures, tempo/control coordinates and total score duration are unchanged.

Numeric sound dynamics are sampled at each **actual** attack; changes within an
allocated window remain ordered gain-envelope steps at their original absolute
times. No early principal dynamic level is copied over a later control. Pitch,
staff and instrument route retain their authored-onset resolution; a later
state change does not retarget an already anchored grace/principal. Subsequent
ordinary events use their own state. Supported repeat
checkpoints restore state before entry controls and remap every grace's principal
anchor. Each repeat visit can receive a different explicit allocation.

`schedule().windows()` exposes exact quarter/second sound starts and ends.
`schedule().seconds()` rejects out-of-range compiler queries. `seek()` uses the
same exact tempo clock and clamps to the score boundaries. The plan keeps written
grace markers zero-length at their authored onset and preserves the principal's
nominal written interval. Seeking a written principal marker therefore does not
mean seeking its delayed sound attack. Consumers that need actual sound selection
must use note windows. Existing notation preview is not made grace-aware here.

Every selected allocation and sound window produces structured policy diagnostics.
There is no default policy, fixed grace length, equal split, hidden pause or tempo
edit. An absent tempo still uses the existing quarter=120 playback fallback.

## Deliberate errors and budgets

The first profile rejects previous-note stealing, make-time, mixed timing hints,
source-hinted multi-note groups, variable-tempo source percentages, missing or
duplicate policies, unknown/written/wrong-group IDs, nonpositive allocations and
allocations exhausting the principal. Affected grace/principal articulations and
relation anchors, including principal ties, require a future interpretation and
reject. Invalid/silent/out-of-MIDI-range affected pitches and zero sound dynamics at
an affected actual attack reject, never disappear from a successful qualified
plan. Later explicit zero/restart dynamics remain envelope changes. Mixed silent-cue/ordinary sounding tie chains
also reject; graphical cue size alone is not a silent event role.

The qualified route also retains the bounded performed projector’s restrictions:
explicit contiguous measure coverage, closed ordinary navigation, numeric sound
dynamics rather than sounding symbolic dynamics, and no unsupported expression
spans or cross-jump ties. Unrelated supported ordinary tie chains remain intact.

Combined grace and fermata capabilities reject in **both** qualified APIs. There
is no defined ordering yet between donated grace time and fermata holds. Ordinary
imported-grace playback still rejects. Legacy SFS graces without the fidelity
capability retain their previous warning/omission behavior, not this new policy.

Default limits include 4 MiB source text, 100,000 events/groups/members, bounded
source records/controls, 4096-bit exact rationals and at most six hours. Source
budget checks precede semantic validation in the qualified parser, but lexical
and AST parsing still precede that callback. Allocation counts, cumulative
projection positions and exact clock intermediates are checked. Positive windows
that collapse at the final floating output boundary or fall below the existing
one-microsecond audio minimum reject. This is not a general parser sandbox or
an unbounded score renderer.

## Reproducible evidence, not an audio demonstration

`qualified_grace_playback` imports small unchanged MusicXML fixtures through the
common model and SFS before calling the actual new API. One fixture has an authored
25% singleton at quarter=120: grace duration 1/8 second, principal attack 1/8 second
and gated duration 69/200 second, with unchanged 1/2-second score end.

The checked-in synthetic `grace-playback.musicxml` has two genuine unmeasured
graces with no timing hints before a principal at quarters 1/2–2. Explicit
allocations 1/8 and 1/4 cross a tempo change at quarter 3/4. The actual attacks
start at 1/4, 5/16 and 1/2 seconds, with durations 1/16, 3/16 and 207/200 seconds.
The second grace preserves a numeric dynamics step; the principal starts at the
later level. Score duration remains 13/8 seconds. Reverse-lexical IDs test source
order rather than accidental ID sorting.

Rust generates and equality-checks `desktop/tests/fixtures/grace-playback.json`.
Frontend tests consume that real plan and check interrupted grace ages, control
steps, delayed-principal attacks, written-marker seeking and clamping with mocked
Web Audio. These are scheduling assertions, not rendered waveform, hardware,
sample-library quality or listening acceptance.

The four untouched originals contain 237 graces in 140 groups, none with authored
timing directives. Their inventory is documented and hash-pinned in
[GRACE_IMPORT.md](GRACE_IMPORT.md). This API does not assign policies to them, strip
their unsupported notation, or claim they now render. A UI policy-entry workflow,
combined fermata realization and full-score sampled audio remain separate work.
