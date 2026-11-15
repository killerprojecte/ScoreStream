# Exact authored slur endpoints

This bounded bridge reuses SFS `slur` relations. It adds an optional `slurs`
collection to the common authoring model; it does not add a replacement graph,
change note durations, choose a legato interpretation or establish full original
Beethoven import.

## Identity and endpoint data

Each `ScoreSlur` has a unique relation ID, a start and an end. Each endpoint owns
its exact event ID and independently optional rational `bezier_x`, `bezier_y`,
`orientation` (`over` / `under`) and `placement` (`above` / `below`). Absent values
are not zero or inferred from the opposite endpoint. Orientation is not placement.
Coordinates are retained exactly, not silently regenerated. XML output requires
exact finite decimal coordinates within the supported numeric budget; it rejects
nonterminating rationals instead of rounding them.

Two slurs may have identical start/end events and identical geometry but distinct
IDs. Both remain. Grace notes remain separate unmeasured events: a grace-to-main
slur can have equal logical timestamps and still has two distinct ordered anchors.
Pitch, stems, written rhythm, chord membership and other supported note metadata
are never copied or changed by a slur.

MusicXML `number` disambiguates overlap in document order inside a part, not a
persistent global relation or voice/staff identifier. The reader resolves source
encounters before assigning stable relation IDs. Either endpoint may occur first
in XML traversal. Reuse is allowed after a pair is complete; duplicate phases,
unclosed links and ambiguous encounters reject. Export assigns finite local
numbers against its actual emitted note/voice/grace order and reports the identity
normalization. It never pairs by pitch, deduplicates endpoint pairs or assumes
nested spans. More than sixteen simultaneous document-order spans reject.

## Required authored fidelity

SFS uses the existing `~ slur ID start=#EVENT` / `end=#EVENT` records plus the
required capability `scorestream.slur@1`. The capability makes supported notation
fields per-endpoint, rather than immutable fields shared from the start. Every
imported slur receives the capability, including slurs with no geometric fields.
Unknown fields, incomplete pairs, unsupported continuations and missing capability
for newly introduced endpoint fields reject, including direct native JSON/AST.

Legacy SFS slurs without this capability keep their existing behavior. Conversion
from that legacy profile remains explicitly unsupported; it is not silently
upgraded to the imported profile. Existing `line_type` vocabulary remains valid
for legacy SFS but is outside this initial authored MusicXML endpoint profile.

Supported imported anchors are distinct pitched measured or authored grace events
in one part. Cross-voice and cross-staff source traversal does not redefine their
identities. Equal logical timestamps require provable order within an authored
grace/principal group; arbitrary simultaneous measured notes are not guessed.
Rest, unpitched and silent-cue anchors remain outside this bounded profile.

The only MusicXML slur attributes mapped here are `type` (start/stop), `number`,
`bezier-x`, `bezier-y`, `orientation` and `placement`. No descendants are allowed.
Continuation geometry, `line-type`, source endpoint IDs, color, dash lengths,
secondary/deprecated Bézier fields and other layout attributes remain explicit
errors. Other note/grace notations are not admitted merely because slurs are known.

## Playback and preview remain separate

The new capability is deliberately rejected by ordinary lowering/playback and
remains outside all qualified navigation, grace, fermata and combined profiles.
It cannot silently activate legacy ordinary slur gate behavior or disappear at a
repeat jump. Legacy same-voice native slurs still use their existing legato gate
policy. A future explicit realization must resolve occurrence identities and
interactions with grace allocation, holds and other articulations first.

Preview withholds the authored profile rather than drawing a score without its
slurs or substituting automatic curves. This change does not implement engraving.

## Original receipts and limitations

The pinned four XML files contain 2,533 pairs (489 / 582 / 462 / 1,000), with
5,066 endpoints. There are 325 groups of parallel identical-endpoint pairs,
representing 650 separate slur IDs, and 130 grace-to-principal slurs sharing a
logical timestamp. Every endpoint contains both Bézier coordinates. Three finale
starts have `orientation="under" placement="below"`; their stops omit both.

All actual source endpoints are in voice 1 with no explicit staff. Cross-voice
number reuse, cross-staff traversal and matched stop-before-start order therefore
need explicit synthetic regressions; they must not be advertised as corpus cases.
All eight untouched XML/MXL inputs still require other mappings and performance
policies. Field-decoder receipts and contextual raw fragments are not a complete
source import or a sampled whole-score result.

Primary definitions: [MusicXML slur](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/slur/),
[number-level](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/),
and [start-stop-continue](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/start-stop-continue/).
