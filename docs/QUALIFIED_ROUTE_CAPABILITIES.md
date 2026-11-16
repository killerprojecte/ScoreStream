# Qualified zero-grace clock and non-playable route trace

## Zero authored grace is not a missing musical choice

The existing explicit joint articulation + synchronized fermata path now admits
a complete validated score with **no grace capability, no grace events and no
grace marks**. The caller supplies an explicitly empty grace-group policy. The
same source-qualified constructor, articulation checks, hold membership checks,
exact rational timing and final global clock run unchanged. No grace extension
or dummy event is added. A fabricated nonempty allocation, an unregistered grace
record or an unknown required extension still fails. Actual authored graces still
require their registered capability and complete explicit allocation.

## Closed passive receipts, individually checked

These exact v1 capabilities may coexist with the qualified route. Each is decoded
by its existing typed decoder, not merely accepted by string prefix. The returned
performance retains a `performance.passive_receipts` disclosure. Source records
remain untouched.

| Family | Why no additional sound interpretation is selected |
| --- | --- |
| provenance | Producer/software/encoding claims do not determine audio capability or musical behavior. |
| metronome_display | Printed parentheses/words/presentation are separate from explicit sounding tempo; a printed rate never replaces the sound clock. |
| part_labels | Abbreviations and printed name components do not select patches, transposition or routes. |
| defaults | Scaling, page/system/staff layout and print defaults are geometric/style data, not duration or velocity. |
| credits | Page credit text/layout is not a musical direction. |
| lyrics | Preserved syllable/text/layout is printed notation; no speech, singing or instrument substitution is synthesized. |
| print | Page/system/staff layout does not move sound events. |
| geometry | Exact event/measure/dot drawing coordinates do not replace logical pitch, onset or duration. |

These decisions do **not** apply to unknown versions or arbitrary marks. Existing
beam, words, instrument and selected articulation/fermata/grace handling remains
separate. Explicit pizzicato words siblings still require their own sound policy.

Wedge, dashes, authored slur and ornaments families are **not** newly admitted.
They can carry crescendo/phrasing/ornament intent; no velocity curve, duration,
gate or substituted tone is inferred. The qualified sound route names them in
`performance.unresolved_families` and stops. Other existing musical/native gates
remain in effect after family qualification.

## Read-only route discovery is a different type

`notation::trace_document_route` validates the complete source, reconciles the
same aligned partwise measures and closed repeat/ending structures, and uses
`repeat_traversal` for actual visits. It traces original note/rest/grace records
and fermata anchors in exact quarter coordinates. It returns neither a
`ResolvedScore` nor a `PlaybackPlan` and cannot be supplied to a timing constructor
as a qualified sound proof. No method produces sound from this trace.

The trace reports all event occurrences, **not audible attacks**; ties, cues and
rests are not collapsed into a fake sound count. Capability inventory, original
anchor identities and navigation conventions remain visible. It does not resolve
unselected sound families or claim conditional ties across jumps work. Unknown or
malformed required semantics still fail the original typed validation. No source
capability or mark is stripped to obtain the trace.

The isolated audition example exposes `trace-route` separately from
`discover-joint`, `preflight` and `render`. The latter paths continue to require
qualified sound interpretation and a verified native palette.

## Untouched original movement I observation

Input: 3,791,385-byte native SFS produced by the separately audited unchanged
original XML import. Exact SHA-256:
`ffac34c86e31df50b028ca7afe0e91f822c2b122af28b55c5e0c1ba10769f77b`.
The trace observed:

- 15,261 written events, 0 written graces, 177 authored fermata signs.
- 626 measure visits, 18,971 event occurrences, 248 fermata occurrences.
- Performed route endpoint: 1260 quarters, **not a chosen number of seconds**.
- Existing route conventions disclosed: `repeat1` uses implicit movement start
  and conventional two passes because those details were absent in the source.

The stricter unchanged-source joint discovery now stops specifically at
`wedge@1`, `dashes@1`, `ornaments@1`, `slur@1`, rather than an artificial need for
a grace capability or unclassified passive layouts. No hold durations were chosen
and no full-movement WAV was generated. These counts are route evidence, not
full-score listening/fidelity certification.

Focused regressions prove zero-grace compilation, no dummy-source mutation,
nonempty-policy rejection, all eight valid passive families with unchanged notes
and routes, invalid passive data rejection, sound-family trace versus playback
separation, unknown capability rejection, and exact trace/qualified fermata
coordinates across a repeated route.
