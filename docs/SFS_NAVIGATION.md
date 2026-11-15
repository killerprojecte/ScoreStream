# Authored SFS navigation capability

The required capability `scorestream.navigation@1` enables typed barline and ending interchange in SFS 1.1. This slice supports XML → common score → SFS → common score → XML roundtrip for the supported ordinary closed-repeat profile. It preserves authored notes and controls once, not an expanded performance. Old readers reject the unknown mandatory capability; current performed-notation lowering explicitly returns `navigation.replay_unsupported`. Repeat engraving is not implemented and codec inspection reports that limitation. CLI validate, canonical and inspect print structured warnings on stderr even when authored syntax is valid.

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

## Rigorous performed replay work still required

The next performance slice must consume one original common/notation model plus the exact occurrence plan. It must not use pre-expanded copies as a second source of truth.

1. Build a scoped written-control index. Store checkpoint state before entry controls: tempo, instrument/transposition, numeric dynamics, pedal and other supported persistent controls. Restore the whole supported scope state at backward edges, then replay authored entry controls in order.
2. Project controls only from visited measure intervals onto exact performed quarter positions. Skip edges apply no notes or controls from excluded endings. Explicit final-boundary controls need a documented half-open ownership rule rather than being attached twice.
3. Create performed event identities `(movement, occurrence, written event)` while retaining written event references for selection. Recompute sounding pitch from the state active on that occurrence exactly once.
4. Rebind and validate ties and other spans across actual performed adjacency. Never stretch a written tie across a nonmatching jump or silently reattack a continuation. Unsupported per-pass controls and conditional relations must remain diagnostics.
5. Build the tempo clock from projected performed controls, then realize durations and policies such as fermatas. Test tempo changes inside held notes and skipped endings, all restored scopes, repeated source positions, budget limits and deterministic rendering.
6. Keep movement identity separate from local part IDs. III→IV attacca adds no pause but does not reuse instrument/tempo state or manufacture cross-file ties. Tail overlap/truncation requires a deliberate audio policy.

Acceptance requires actual XML/MXL import through the common model, matching closed source traversal and state traces, no once-through fallback, then identical scheduler/offline timing. Full Beethoven import still has other unsupported semantics and the irregular 10-quarter cadenza measure; this capability does not bypass those blockers or produce the requested complete audio.
