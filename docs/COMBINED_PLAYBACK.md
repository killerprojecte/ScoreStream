# Explicit grace and fermata composition

The new source-qualified Rust API combines the existing explicit grace and
fermata interpretations for a bounded, nonoverlapping timing case. Both source
capabilities stay present. It requires complete policies for both, projects one
route, and returns actual playback attacks with one exact hold-aware clock.
It does not infer any durations or make complete original-score audio available.

## Supported boundary and proof

For each visited grace principal, let `s` and `e` be its nominal performed-route
quarter start and end. Both conditions are required:

1. No visited fermata sign is anchored to a grace member or its principal.
2. No global hold boundary `h` satisfies `s < h <= e`, even if its anchor is on
   another staff or part. The error identifies the principal, interval, hold,
   boundary and members.

A hold at `h = s` is allowed: the preceding music finishes its hold before the
grace group begins. A hold at `h = e` is rejected, even though the principal's
ordinary gate might finish earlier. This retains the existing strict fermata
rule against unselected notated notes crossing or ending at a hold.

The order is explicit. First allocate exact grace quarters using the existing
policy. For allocations `d1..dn`, `A = sum(d)` and `D = e-s`, grace windows occupy
consecutive intervals from `s` to `s+A`. The principal sounds from `s+A` through
`s+A+(23/25)*(D-A)`. Then apply the global hold clock:

- `B(q)` is exact tempo integration on the single performed route.
- `before(q) = B(q) + sum(H for h < q)`.
- `after(q) = B(q) + sum(H for h <= q)`.
- Every sound endpoint uses `after`; a hold occupies
  `[before(h), after(h))` seconds.

The boundary condition proves that both ends of every allocated grace/principal
window have the same added-hold prefix. Its duration is therefore exactly its
base-tempo duration. A hold cannot lengthen a grace, consume its allocation or
change its gate. A tempo change inside an explicitly allocated group still uses
exact piecewise integration. The narrower source-percentage mode still requires
constant effective tempo across its nominal principal interval.

Simply allowing both capabilities would not prove this. A hold at the end of a
grace would lengthen that grace under `after` conversion; switching its release to
`before` would instead invent a silent pause. A fermata on the delayed principal
would also conflict with its shortened gate. These interactions reject rather
than choosing one interpretation silently.

## Source-qualified API and immutable results

`discover_source_graces_and_fermatas(source)` returns both authored audits and
their visited occurrence IDs from the same route. Marks skipped on a repeat pass
remain in the authored audit; occurrence lists follow the actual route. Written
IDs alone cannot assign every repeated visit.

`plan_source_with_explicit_graces_and_fermatas` takes the source, explicit grace
policy, `GraceSoundPolicy::FullGraceThenGatedRemainder`, explicit fermata policy
and `FermataSoundPolicy::SustainSelectedAnchors`. Its opaque
`ExplicitCombinedPlayback` exposes `plan()`, `timing()`, `grace_windows()`,
`grace_discovery()`, `grace_groups()`, selected sound policies and `seek()`.
The corresponding core constructor is
`perform_document_with_explicit_graces_and_fermatas`.

The source is never rewritten or stripped of a capability, and graces never
become measured source events. The private joint profile admits exactly grace
and fermata capabilities, optionally with supported navigation. Unknown required
semantics reject. The internal qualified clock retains every zero-length grace
marker and checks complete source-to-occurrence receipts, including graces.
It cannot accept a caller-manufactured performed snapshot or allocation schedule.

Both old single-feature APIs still reject combined input. The public raw
`fermata_timing::interpret` also retains its no-grace contract. Only the opaque
joint source qualification reaches the combined compiler path.

The final grace windows contain hold-aware seconds, not the standalone grace
clock's base seconds. Written markers remain distinct: grace markers are
zero-length at `after(original onset)` and the principal marker retains its
nominal interval. `seek()` returns the shared exact musical-position or hold
result, including elapsed time within a hold, and clamps outside the score.
Existing interpolated notation cursor geometry is not made hold/grace-aware.

## Controls, ties and strict limits

All previous grace and fermata restrictions still apply. Each visited sign and
grace group/member must be assigned exactly once. Unknown, duplicated, omitted
or wrong-visit policy IDs reject. Previous-note stealing, make-time, conflicting
hints, unsupported expressions, affected principal ties/articulations and
unsupported grace percentages are not newly interpreted.

Selected terminal fermata notes/tie tails sustain to their exact nominal endpoint
and through its hold. Interior tied signs retain one attack. Unselected audible
chains crossing or ending at a hold, mixed silent-cue/ordinary ties and other
unsupported sounding anchors still reject. Grace sound validation still rejects
zero dynamics at an affected actual attack rather than dropping that attack.

Tempo or dynamics changes at a hold boundary apply after the hold. A sustained
terminal note keeps its old numeric gain throughout that hold; the following
grace attacks use the new level. Later steps within a grace window retain their
absolute control times. Pitch, staff and instrument route remain resolved at the
authored onset, just as in the standalone grace policy. Repeat checkpoints restore
state before entry controls, and both policy families address the same remapped
occurrence IDs. Holds are global and are not repeated per staff copy.

Joint preflight uses the stricter source, event, measure/control and rational
budgets before validation or route expansion. It also selects and validates the
stricter timeline limit, which is enforced on the projected base clock and again
on the final clock including holds. The parser still performs lexical/AST work
before its read-only preflight callback. Final shifted windows are checked again
for exact rational growth and floating audio-clock precision. The six-hour limit
includes added holds. Short or collapsed output windows reject rather than
disappear.

## Verification and remaining scope

The synthetic `joint-playback.musicxml` passes through MusicXML, the common model
and unchanged SFS before the actual joint API. A one-second hold follows a C4
quarter at tempo 120. Two graces follow that hold, with explicit allocations of
1/8 and 1/4 quarter; tempo and numeric dynamics change inside the second grace.
The real plan has four attacks:

- Held C4: seconds 0–3/2, initial velocity 45, no premature boundary jump.
- D5 grace: starts at 3/2, duration 1/8, velocity 90.
- E5 grace: starts at 13/8, duration 3/16, velocity 90, with gain halved after 1/8.
- G4 principal: starts at 29/16, duration 299/400, velocity 45.

Score end stays at 21/8 seconds. Rust generates and equality-checks the frontend
JSON fixture. Frontend tests consume that plan and test note ages, step boundaries,
hold/grace transitions, written-marker seeking and clamping with mocked audio.
These checks are not a waveform, listening or sampled-orchestra acceptance test.

Read-only inventory of the four pinned originals found 140 grace groups and no
fermata-bearing grace principals or fermata ends inside `(s,e]`. Movement I has
no graces. One real onset adjacency is movement III, measure 258, part P14 Violin 1:
its principal starts at quarter 765, where twelve preceding measure 257 fermata
anchors end. That preceding measure still has independently unsupported unequal
fermata alignment. This evidence supports the narrow boundary; it does not
establish complete original import or authorize guessed timings.

No policies have been assigned to the originals. Other import and interpretation
gaps, UI policy entry and actual full-score sampled audio remain separate work.
See [grace interpretation](GRACE_PLAYBACK.md) and
[fermata interpretation](FERMATA_TIMING_PROTOTYPE.md) for their source semantics
and existing backend restrictions.

## Explicit synchronization of earlier rest fermatas

`plan_source_with_explicit_graces_and_synchronized_fermatas` and the corresponding
core `perform_document_with_explicit_graces_and_synchronized_fermatas` constructor
accept `ExplicitSynchronizedFermataPolicy` alongside the existing explicit grace
allocation and both sound policies. The earlier common-release API is unchanged.

Both APIs use the same source projection, exact grace allocation, source-qualified
fermata clock and final hold-aware windows. The synchronized variant only supplies
the caller's explicit latest-release boundaries to that clock. Earlier anchors
must still be rests; their original markers are not extended or moved. Holds
inside a grace principal's nominal interval, or fermatas on an affected grace or
principal, still require a different musical interpretation and reject. No
fallback or automatic grouping/duration is introduced. Caller-selected arithmetic
and timeline budgets remain effective; no fixed score-size or duration cap is added.

The committed `synchronized-joint-playback.musicxml` fixture has a rest fermata
ending at quarter 1/2, a selected note ending at quarter 1 and a grace/principal
pair beginning at quarter 1. Explicitly choosing boundary 1, 3/2 added seconds and
1/4 quarter for the grace produces one hold `[1/2, 2)` and one grace window
`[2, 17/8)` at the default tempo. The rest marker still ends at second 1/4 and the
principal starts sounding at second 17/8. The unchanged source passes the
MusicXML → common model → SFS roundtrip before scheduling.

This is a synthetic executable fixture, not a cleaned Beethoven excerpt or a
complete original-score import. Authored articulation playback is still a separate
open interpretation requirement; this new entry point does not bypass its existing
checks. It adds no UI policy editor or automatic score-wide performance choice.

## Explicit articulation, grace and synchronized-hold composition

`discover_source_joint_performance(source, articulation_policy)` supplies policy
entry metadata without exposing a playable score. The new
`plan_source_with_explicit_joint_policies` accepts all three explicit choices:
`SimpleStaccatoAccentV1`, exact grace allocations plus grace sound policy, and
synchronized fermata groups/seconds plus fermata sound policy. The core constructor
has the corresponding name and accepts caller-selected resource budgets.

This uses one projection and one exact hold-aware clock, not a warped second plan.
Its read-only articulation audit retains the unchanged complete source, all authored
articulation records and selected policy. Existing strict/common-release and
synchronized grace/fermata APIs are unchanged: they do not silently select an
articulation interpretation.

The selected simple articulation policy applies to note/tie chains away from holds.
Any articulated sounding chain whose notated interval crosses or ends at a selected
hold is rejected, even if an ordinary staccato gate would have ended earlier; this
avoids inventing precedence between articulation and sustained hold. Grace/principal
articulations still need a separate window interpretation. A final tie-segment
staccato away from holds remains a single attack with the selected shortened tail;
interior staccato and continuation accent remain unsupported interpretations.
Direct numeric dynamics still combine with staccato but not attack-changing accents.

`joint-articulation-playback.musicxml` is an executable synthetic two-measure
contract: articulation in measure 1, a short rest fermata and later synchronized hold
in measure 2, then an authored grace/principal pair. With boundary 3 and one added
second, the rest marker ends at second 5/4, the one hold is `[3/2, 5/2)`, and the grace
starts at 5/2. A disjoint tied staccato also works. This is not full original-score
import and does not provide a UI editor, default interpretation, support for other
articulation families or unsupported native source semantics.
