# Source-bound rehearsal route evidence, not playback

`trace_document_route` remains strict about an unclosed final ending. The separate
`trace_document_route_with_final_ending` accepts only the existing typed
`UnclosedFinalEndingPolicy` choice. It validates unchanged source and reconciles
ordinary navigation before producing metadata. No source marker is synthesized;
no sound API, hold duration, grace allocation, sample route or render is authorized.

The trace now includes authored grace occurrences with unique occurrence event
and principal IDs, written IDs, owning part, zero-based written measure index,
zero-based measure occurrence, source order, exact principal interval and unchanged
optional timing requests. Legacy untyped graces remain counted, not given invented
authored timing metadata. Words occurrences retain exact text and owning written
measure at their projected coordinate. An Attacca word is not a cross-movement
clock join. Existing fermata occurrence fields are unchanged.

A small metadata-only example requires no audio-device initialization:

```
cargo run -p scorestream-formats --example trace_route -- exact-source.sfs
cargo run -p scorestream-formats --example trace_route -- exact-IV.sfs continue-to-movement-end
```

The output binds the trace to SHA-256 of the entire input. Ordinary API/default
example still rejects IV's unclosed ending. Unknown capabilities remain errors.
Source validation, complete measure coverage and existing route constraints are
not bypassed by the explicit final-ending choice.

## Actual verified II–IV sources

Input hashes match `BEETHOVEN_FIFTH_ROUNDTRIP.md`. These are route coordinates in
quarters before any fermata/grace interpretation, not elapsed seconds.

| Movement | Measure visits | Event occurrences | Grace occurrences / principal groups | Fermata occurrences | End quarter |
| --- | ---: | ---: | ---: | ---: | --- |
| II | 248 | 10,908 | 13 / 10 | 16 | 741/2 |
| III | 396 | 12,165 | 128 / 66 | 80 | 1180 |
| IV, explicit final-ending continuation | 529 | 38,073 | 112 / 80 | 19 | 2062 |

IV has96 written graces;16 acquire a second visit. Their occurrence-qualified
principal IDs differ, so a written ID alone cannot assign timing to both visits.
All actual grace timing fields remain absent. III's exact `Attacca` is retained
for P1/written measure `m1_376`, occurrence395, quarter1177; its movement ends1180.

Fermata intervals by performed coordinate (count in parentheses):

- II: `[367/2,369/2]` (11), `[367/2,184]` (5).
- III: `[22,23]` (7), `[22,24]` (10); `[52,55]` (18);
  `[154,156]` (9); `[790,791]` (11), `[790,792]` (7);
  `[820,823]` (12), `[821,822]` (6).
- IV: `[2058,2062]` (18), `[2060,2062]` (1).

Unequal endpoints are not automatically one common release, and rest anchors
are not automatically extended to the latest endpoint. These traces expose
identities for explicit policy review, including remaining sound-bearing families;
they do not prove chain sustain eligibility or complete performance realization.
Synthetic tests compare grace and fermata occurrence coordinates against existing
qualified repeated-score discovery, retain words without action, and ensure the
open final ending remains a default error and source data remains unchanged.
