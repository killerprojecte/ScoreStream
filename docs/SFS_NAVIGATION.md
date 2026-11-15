# Authored SFS navigation capability

The required capability `scorestream.navigation@1` enables typed barline and ending interchange in SFS 1.1. This slice supports XML → common score → SFS → common score → XML roundtrip for the supported ordinary closed-repeat profile. It preserves authored notes and controls once, not an expanded performance. Old readers reject the unknown mandatory capability. Ordinary `lower_document` refuses once-through lowering; `perform_document` and application playback use the supported occurrence projection described below. Repeat engraving is not implemented and codec inspection reports that limitation. CLI validate, canonical and inspect print structured warnings on stderr even when authored syntax is valid.

```text
!requires scorestream.navigation@1
@0
| P1 m1 len=4 label="1"
+ P1 scorestream.barline measure=m1 side=left style=heavy-light repeat=forward source_id="repeat.head"
.
@4
+ P1 scorestream.barline measure=m1 side=right style=light-heavy repeat=backward times=2
.
```

This is a declaration/frame excerpt, not a complete score. The referenced measure belongs to the declared part target. A left boundary's frame is the exact measure start; a right boundary's frame is its exact end. Labels do not resolve references. One authored record is allowed at each part/measure/side. Mark order within a frame does not change boundary identity.

Registered fields:

- Required `measure` identifier and `side=left|right`.
- Optional typed `style`, `repeat=forward|backward` and authored unsigned 32-bit `times`. A count requires a backward repeat and cannot coexist with ending-conditioned playback in this conversion profile.
- Optional `ending=start|stop|discontinue`, requiring quoted comma-separated `passes` with 1–1,024 distinct positive 32-bit integers. Stop, discontinue and an absent endpoint are different.
- Optional quoted `ending_text` and exact rational `ending_y` in tenths require an ending. Printed text never selects passes.
- Optional quoted nonempty `source_id` preserves the authored barline identity independently of wire-safe aliases. The generic mark `id` is not accepted by this capability. Source identities cannot collide with part, event, measure or other barline identities after mapping.

The mandatory capability must occur exactly once and requires at least one authored boundary. Unknown fields/versions, malformed scalar types, unresolved/cross-part measure references, wrong boundary times, duplicate positions and fragment navigation fail validation. JSON and text paths use the same typed checks. Exact control characters in quoted display strings are escaped rather than discarded. Generated alias allocation reserves preserved barline identities to avoid introducing collisions.

## Closed conversion policy

The SFS common-model bridge reconciles per-part navigation and validates the ordinary route without allocating performed occurrences. Unsupported complex/nested navigation and malformed pairings reject with no converted output. An unclosed final ending, including the actual Beethoven IV second ending beginning at 87, rejects under `navigation.route_policy`; this slice exposes no implicit continuation override. A raw SFS document may be inspected with its authored open sign intact, but reports `navigation.unclosed_ending` and cannot pass strict common-model conversion or playback.

Absent repeat count and absent forward sign remain absent in the authored data. When the supported route uses a two-pass or movement-entry convention, the conversion report names that inference. No new glyph, count or ending endpoint is inserted.

## Bounded performed projection

`notation::perform_document` consumes the original validated SFS document and the shared score-wide route reconciler. It returns a derived `PerformedScore`; it does not serialize expanded SFS or replace the authoring model. Application playback automatically uses this path for navigation-capable scores. The format adapter and performer now share the same reconciliation code in core; app dependencies and all lockfiles are unchanged.

Supported behavior:

- Closed ordinary repeats, explicit counts and the existing closed first/final ending profile. Absent-count and absent-forward conventions remain explicit returned policy diagnostics. An unclosed final ending still rejects with no continuation override.
- Exact rational performed quarter positions. Measure intervals are half-open: controls at a boundary belong to the following measure, and skipped intervals execute no controls. A meaningful control at final EOF is explicitly rejected because there is no following measure to own it.
- Checkpoints capture before entry controls. Tempo, part instruments, voice default staffs and part/staff transposition maps restore together. Authored entry controls then execute again. Sounding pitch is recomputed from the written pitch exactly once per occurrence using the shared state reducer.
- Explicit numeric dynamics restore by scope, including absence of an assignment. Projected assignments have finite performed-measure intervals, so a first-ending-only staff override cannot leak into the final pass. Printed `sound=false` dynamics remain display-only. Tempo changes inside a held note are projected before the existing exact tempo clock converts to seconds.
- Ordinary ties rebind within contiguous performed runs, including ties across sequential measures inside a repeat. Any tie partially present across a backward/skip edge rejects. Other spans, sounding symbolic dynamics, grace timing and measured events extending beyond their authored measure remain explicitly unsupported in this navigation profile. Additional required capabilities, including unresolved fermata timing, reject before projection.
- Performed event IDs are occurrence-qualified; `Origin.object_id` and `EventOccurrence.written_id` retain the written identity. Playback notes expose `written_id`, event positions retain written `id` plus `occurrence_id`, and measure positions retain written ID plus `occurrence_index`. Playback selection/playhead code keeps every visit instead of overwriting repeated written IDs. Clicking a written note seeks its first performed visit; the measure picker distinguishes later visits by their time.

Default budgets are 100,000 performed measures, 100,000 events, and 1,000,000 projected controls/relation anchors or part-measure references. Checks precede each bounded allocation; failure returns no partial playback plan. Written input is indexed once, navigation uses the shared bounded traversal, and ties are considered only in contiguous runs visiting their anchors rather than a full routes×relations scan.

The synthetic `navigation-performed.musicxml` fixture is an actual complete XML → common model → SFS → PlaybackPlan test, not a replacement for Beethoven. Its five authored notes yield six performed occurrences and exactly 5.5 seconds with restored tempo, numeric dynamics and transposition. Further tests cover restored defaults, staff/instrument changes, held-note tempo changes, tied visits, cross-jump rejection, policy diagnostics and budgets. The original four scores still have independent import blockers; no full-score audio or native listening acceptance is claimed.

## Remaining fidelity work

Fermata durations, symbolic dynamics/hairpin/pedal checkpoint policy, conditional cross-jump relations, grace timing, engraving and irregular-measure import remain separate explicit work. Movement IDs and local part IDs must stay distinct; III→IV attacca adds no pause but cannot carry instrument/tempo state or manufacture cross-file ties. Tail overlap/truncation needs a deliberate audio policy. Full original MusicXML/MXL import, native preview and complete-score listening remain required before claiming the requested Beethoven audio is finished.
