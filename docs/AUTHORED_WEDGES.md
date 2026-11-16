# Authored wedge endpoints

The MusicXML adapter preserves ordered crescendo, diminuendo and stop endpoints
as typed authored engraving, separate from the existing sounding hairpin model.
The canonical SFS capability is `scorestream.wedge@1`, with closed
`scorestream.wedge` marks targeting their owning measure.

Each endpoint retains its owning part and measure, exact logical rational offset,
optional staff and placement, start/stop type, optional number, and exact decimal
default-y and spread. Absence differs from explicit zero or explicit number 1.
Decimal spelling (for example 11.50 versus 11.5) is normalized, not retained.
Canonical grouping is by part identifier and measure start; endpoint order within
a measure is preserved even when logical offsets differ from document order.
Global contiguous indices preserve this order through time-ordered SFS frames.
XML export places endpoints before notes with explicit logical offsets; original
cursor-relative offset spelling and interleaving with unrelated directions are
not byte-preservation promises.

The supported endpoint profile intentionally rejects continuation, niente,
default-x and other unrepresented attributes/children rather than discarding them.
Signed direction offsets are supported when the resulting position is inside
the owning measure, including its end. Numbered overlapping endpoints are
retained, not resolved into performance curves.

## Honest preview and performance boundary

`wedge.authored_only` warns that preview does not render these wedges and
performance does not interpret their expression. Import, normal preview and
playback remain available. Numeric sound dynamics and note attack dynamics
remain independent and unchanged; spread is not converted into gain, and no
missing target velocity is invented.

`wedge.pairing_unresolved` reports orphan stops, duplicate open starts and
dangling starts using authored document order and part/staff/number scope
(absent number defaults to 1 for this audit only). It does not repair endpoints.
A zero count is not proof of a musically valid or temporally ordered curve.
The authored-only model never supplies implicit sounding pairs.

## Original-source gate

The ignored `original_206_endpoints_preserve_positions_and_unresolved_source_chains`
test reads all four pinned untouched Beethoven Op.67 original XML files,
verifies SHA-256, scans actual note/backup/forward cursors and measure extents,
and round-trips a named family projection through typed model, canonical SFS
and XML. The projection replaces notes with rests; it is not a claim that the
entire original score is supported or that its performance was tested.

Expected endpoints: I 14, II 188, III 4, IV 0. Original document-order unresolved
incidences: I 4, II 12, III 0, IV 0. In particular, movement I includes stops in
measure 305 followed by dangling starts in measure 306 in P9 and P10; these are
preserved rather than silently reordered or repaired.

Run with `SCORESTREAM_MXL_FIXTURES` pointing to the original directory:
`cargo test -p scorestream-formats original_206_endpoints --lib -- --ignored --nocapture`.

## Semantics sources

- [MusicXML 4.0 wedge](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/wedge/)
- [Wedge types](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/wedge-type/)
- [MusicXML schema](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/)

MusicXML describes number as a document-order overlap discriminator and spread
in tenths of engraving distance. Authored spread is retained even on endpoints
where the specification says it is ignored for drawing; it is never treated as
a target loudness.
