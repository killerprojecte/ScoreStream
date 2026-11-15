# Explicit fermata timing prototype

The pure exact performed-time transform now also has a separate, opt-in **source-qualified PlaybackPlan integration** under an explicit strict sound policy. Ordinary source lowering, repeat projection and `plan_source` still reject the fermata capability. This work does not create an audio demonstration, change the original score, remove marks to bypass validation, or claim complete Beethoven import.

## Separate authored data from interpretation

MusicXML's fermata sign has an event anchor, shape, orientation and optional placement. It does not supply a number of hold seconds. Those authored fields stay unchanged.

The `scorestream_core::fermata_timing::interpret` API takes the validated source `Document`, a supplied `PerformedScore` snapshot with written-event and measure-occurrence receipts, an `ExplicitFermataPolicy`, and `TimingLimits`. It returns a `TimingProjection` or structured `TimingError`, never a partial success. The performed snapshot must come from a correct projection. Route legality and source-to-performed controls/tempo, pitches, voice/staff and relations are caller preconditions, not independently proved by this transform. The checked identity/part/time receipts do not constitute a complete performed-projection validator. Ordinary public source projection deliberately still rejects fermatas. The separate `notation::perform_document_with_explicit_fermatas` constructor now privately projects unchanged validated source and invokes this clock before exposing its read-only qualified result. The original pure-clock tests still construct typed snapshots explicitly and label them as fixtures, not successful complete source imports.

The source is validated and authored signs are decoded directly. Stable event IDs and explicit per-part measures are required. A complete audit retains each written sign and its visited occurrences, including unvisited signs in skipped material. A policy addresses unique performed event IDs, not just written IDs, so repeated visits can have different deliberately selected holds. Every visited sign must be assigned exactly once. Unknown policy members, omitted visited measured events, duplicate memberships and inconsistent identity/part/time receipts reject without a partial result. Source and performed grace events reject, including a source grace event omitted from the supplied snapshot.

## One explicit common-release group, one global hold

Each group has a unique identity, an explicit member list, and a positive exact rational number of added seconds. No duration is selected by default, and shape does not choose a multiplier.

This first profile requires all members to have the same exact performed quarter-note end. Different starts are allowed. A group adds its seconds once regardless of how many parts repeat the sign. Separate groups at the same boundary reject rather than adding duplicate pauses. Separate groups whose anchor-support intervals overlap also reject; this conservative restriction is an implementation boundary, not a universal musical interpretation.

The known original patterns motivate these distinct tests:

- I m21: one 0–2-quarter note plus sixteen 1–2-quarter rest anchors can form one common-release group.
- IV m446: eighteen 0–4-quarter notes plus the 2–4-quarter timpani anchor can form one group.
- II m123: 0–1-quarter notes and 0–½-quarter rests cannot form one common-release group. Splitting them into overlapping groups also rejects. A different synchronization policy is needed.
- I m268: the separate 0–2, 5–6 and 9–10-quarter anchors can form three explicitly specified groups.
- III m257: nested 0–3 and 1–2-quarter rests likewise need a later policy.

The opening's eighteen copies are preserved as eighteen authored occurrences, not eighteen added pauses. Nothing here assigns a hold length to the original recordings or scores.

## One exact global clock

Let B(q) be seconds obtained by integrating the performed tempo map at exact quarter position q.

- before(q) = B(q) + added seconds from holds strictly before q.
- after(q) = B(q) + added seconds from holds at or before q.

A hold at q occupies the half-open interval [before(q), after(q)). Event and measure markers retain separate written and performed quarter coordinates, original labels and occurrence identities; both seconds endpoints map using after. Thus intervals ending at the boundary include the hold, while new events beginning there start afterward. This is a global marker-time policy, not a claim that the current audio gate sustains every sound.

A tempo change at the boundary affects subsequent musical time. It cannot shorten the preceding explicit hold. A final-boundary hold extends total performed duration. A tied stop keeps its own written and occurrence marker; the transform does not move its sign to the audible chain's root or change tie relations.

Exact inverse seeking returns either a musical quarter position or a hold identity, fixed boundary and elapsed added seconds. Seeking inside a hold never invents an intermediate beat. At the exact hold end, musical time resumes. Finite seeks before zero and after total duration clamp to the endpoints. All calculations remain rational; this prototype does not claim a device's floating-point seek accuracy.

If the supplied tempo map has no initial tempo, the existing quarter=120 convention is reported with its exact affected interval. Every hold produces a diagnostic stating that its added seconds come from explicit interpretation policy, not the authored sign. Resource limits bound source records, occurrences, controls, policy membership and timing-rational bit size. The total timeline has a configurable limit and an absolute six-hour maximum.

## Pure-clock boundary

The existing note gate commonly releases at 23/25 of written duration. Merely inserting seconds at the written end would therefore leave silence instead of producing a sustained fermata. Actual playback integration must reconcile selected note gates, tie-chain releases, explicit rests, numeric/symbolic envelopes, boundary controls and seek scheduling against the same global clock. It must consume every authored sign and report the chosen policy.

Ordinary source capability guards remain unchanged. The clock is not serialized as SFS and cannot become an alternate editable score. Bare-snapshot callers must preserve written identities and supply a correct annotated performed snapshot. Audio callers use the source-qualified path below instead. Unsupported synchronization, route or sound realization remains an explicit error rather than falling back to linear or hold-free audio.

Primary reference: [MusicXML fermata](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/fermata/). Source facts and exact positions are recorded in [authored fermata import](FERMATA_IMPORT.md).

## Checks

```sh
cargo test --locked -p scorestream-core --test fermata_timing
cargo test --locked -p scorestream-formats --test fermata_timing
cargo test --workspace --locked
```

The formats test retains the synthetic MusicXML note/tie-stop/rest signs through the shared model and SFS, then joins an explicitly constructed performed snapshot to the pure clock. It confirms that ordinary lowering, repeat projection and app playback still reject the same source. It is neither an automatic annotated projection nor an audio-render test.


## Source-qualified strict audio API

`scorestream_app::playback::discover_source_fermatas(source)` returns only authored signs and occurrence metadata, including written IDs, unique visit IDs, part, measure occurrence and exact written/performed anchor intervals. It never returns a playable score. Discovery requires `scorestream.fermata@1`, optionally `scorestream.navigation@1`, aligned contiguous explicit measures, and the existing bounded ordinary route profile. No marks or extensions are stripped. Skipped repeat visits execute no controls, and source instrument, staff, transpose, tempo and numeric dynamics state still uses the route's checkpoints.

The caller explicitly constructs every group and its positive exact added seconds, then calls:

```rust
plan_source_with_explicit_fermatas(
    source,
    &policy,
    FermataSoundPolicy::SustainSelectedAnchors,
)
```

There is no default grouping, duration or sound policy. The returned `ExplicitFermataPlayback` contains the real immutable-by-convention `PlaybackPlan`, the exact `TimingProjection`, and the selected sound policy. `seek(&Rational)` delegates to the exact music-or-hold result. The core qualified constructor cannot accept an external performed snapshot; its private lower/project path validates the unchanged source, preserves typed authored signs, projects actual occurrence IDs and controls, then requires complete explicit policy consumption before returning its read-only result.

This first sound profile deliberately:

- Overrides the ordinary 23/25 gate only for a selected terminal note or selected tie tail. That sound reaches the exact written endpoint and ends at `after(endpoint)`, including the hold.
- Keeps a selected interior tie segment in one audible chain, with the ordinary final tail gate unless the tail is also selected. The sign stays on its own written/occurrence marker and there is no retrigger.
- Adds rest-anchor holds globally without inventing notes.
- Rejects every audible chain whose notated interval crosses or ends at a hold unless that specific hold contains an anchor in that chain. Selection at a different hold is insufficient. A chain starting exactly at the boundary attacks after the hold.
- Rejects mixed cue/note sounding tie chains, selected cues, unpitched/unmapped or out-of-range notes, all audible articulations (including tenuto in this deliberately conservative first profile), grace timing, sounding symbolic dynamics, non-tie spans, and malformed or cross-jump ties.

The mixed-role tie check applies to cue roots, interiors and tails before deciding whether the root sounds. It prevents both a normal root sounding through a silent cue and a cue root suppressing a later normal attack. Graphic `note_size=cue` is independent notation and remains allowed on normal sounding notes. Ordinary legacy playback is unchanged.

A single `PlaybackClock` serves compiler markers, gates, symbolic-expression plumbing and numeric dynamics. Its explicit variant uses exact `TimingProjection::after` before converting to f64 at existing plan fields. It does not warp a finished floating-point plan. Numeric steps retain paired old/new points at the same `after(boundary)`, keeping old gain throughout the hold; terminal controls remain excluded. Positive exact holds whose f64 endpoints collapse are rejected. Audio clock queries/releases outside the exact score domain reject instead of using the metadata clock's intentional clamping. Timing metadata reports explicit-policy hold seconds and default-quarter=120 intervals emitted or restored by route checkpoints.

This is a Rust API and scheduler-contract integration, **not** a UI/CLI policy editor or a File Open playback option. Existing seconds-based transport accepts the corrected plan; current interpolated playhead geometry is not hold-aware and must not be presented as fixed. Callers needing a frozen cursor use the separate hold metadata and exact seek result. The native sampled adapter still rejects non-unity per-note/step envelopes, an existing backend limitation, so a plan accepted by the WebAudio scheduler is not automatically accepted by every sampler. No audition, waveform correctness or complete original Beethoven performance is claimed.

`crates/scorestream-formats/tests/qualified_fermata_playback.rs` exercises synthetic MusicXML through the shared model and unchanged SFS into the real qualified compiler, including the named synchronization shapes, policies, ties, controls, repeats, errors and exact seek. `fermata-playback.musicxml` plus its deliberately assigned 1/4, 1/2 and 3/4 second holds regenerates `desktop/tests/fixtures/fermata-playback.json`; the desktop test consumes that real Rust plan for scheduler/transport regression. These are test fixtures, not user-facing mock audio.

## Separate joint grace policy

The [combined source-qualified API](COMBINED_PLAYBACK.md) supports explicitly
allocated grace groups whose nominal principal intervals do not contain a global
fermata hold boundary. It uses the same route and retains both source capabilities.
The public prototype and standalone fermata API described above continue to
reject grace input; their contracts are not relaxed.
