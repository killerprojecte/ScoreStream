# Authored words and dashes groups

The closed MusicXML profile preserves a direction containing exactly two
ordered direction-type children, plain words then start dashes, or one
direction-type containing stop dashes alone. Standalone words now use the separate
authored words profile (see AUTHORED_WORDS.md); reversed groups,
markup, arbitrary composites and continuation are still rejected.

Typed endpoints retain part, owning measure, exact logical offset, optional
staff, placement and system relation (only-top/also-top/none), start/stop and
optional number. Words retain exact text including whitespace, optional font
family, positive decimal font size, normal/italic style, default-y and relative-x.
Dashes retain their independent default-y and relative-x. All coordinates are
exact finite decimals, with absence distinct from zero; lexical decimal spelling
is normalized. No foreign markup or open attribute bag is stored.

Canonical SFS uses the declared scorestream.dashes@1 capability and closed
scorestream.dashes marks. Indices preserve authored endpoint order within a
measure; groups are canonically ordered by part identifier and measure start.
XML export positions groups before notes with explicit logical offsets.
This preserves semantic position, not original cursor-relative offset spelling,
omitted sound flags or interleaving with unrelated directions. Existing explicit
numeric sound/tempo records stay independent.

## Realization boundary

dashes.authored_only warns that current preview omits these groups and playback
does not interpret their expressions. “cresc.”, “dim.” and “poco ritard.” are
preserved as text, never turned into guessed gain or tempo ramps. Normal
preview/playback remains available. The words are not analytical annotations.

dashes.pairing_unresolved counts orphan stops, duplicate starts and dangling
starts in document order using part/staff/number scope; absent number defaults
to 1 for this audit only. It never repairs or creates relationships. Zero
incidences do not prove a temporally valid expressive span.

## Original gate

The ignored original_470_endpoints test verifies all four pinned original
SHA-256 values, scans actual note/backup/forward cursors and measure extents,
and round-trips a named family projection through typed model, SFS and XML.
It substitutes rests for source notes and is not an entire-score import or
original-performance claim.

Expected endpoint counts: I 226, II 10, III 92, IV 142. Document-order unresolved
incidences: I 66, II 0, III 24, IV 18. Equal total starts and stops do not imply
valid pairing. Two movement-III starts say “poco ritard.” without font styling;
these and their two stops retain system=only-top, not a guessed tempo policy.

Run:
SCORESTREAM_MXL_FIXTURES=/path/to/originals cargo test -p scorestream-formats original_470 --lib -- --ignored --nocapture

## Primary semantics references

- [Dashes](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dashes/)
- [Direction](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/)
- [Words](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/words/)
- [System relation](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/system-relation/)

MusicXML explicitly permits words followed by a dashed line as ordered
direction-type siblings. Number distinguishes overlaps in document order;
drawing coordinates are not timing, volume or tempo values.
