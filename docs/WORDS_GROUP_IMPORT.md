# Ordered words siblings in one direction-type

MusicXML 4.0 permits multiple text components for multiple fonts:
[direction-type](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction-type/),
[words](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/words/).

`WordsDirection.words` remains the first row; `continuations: Vec<WordsText>`
holds subsequent siblings in authored order. Each row has independent literal
text, font/style and exact rational geometry. The direction retains its own
placement, staff, system and logical offset. The XML writer emits one wrapper,
not several directions or concatenated text. No dynamic, volume or sound role
is inferred from text or private-use code points.

The existing `scorestream.words@1` wire now permits positive `row` on a words
mark. Absent `row` is the first row; explicit zero rejects. `index` identifies the
direction, and rows must be contiguous from one, with exactly one first row.
Every row must agree on direction owner, onset, staff, placement and system.
Physical record order is not semantic row order. Duplicates, gaps, orphan rows,
unknown fields and incompatible types reject. Existing single-row wire remains
unchanged. No JSON or raw XML payload is stored.

Only one all-words direction-type is added. Multi-row groups with sound,
symbols, dynamics, unknown attributes, or additional direction-type wrappers
still reject atomically. Existing supported standalone sound and dashes cases
are unchanged.

## Pinned original proof

`Beethoven_Op.67_2.musicxml` SHA-256
`c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a`,
part P14, measure ordinal 33 / authored label 32, source lines 72928–72930:
- first words: exact trailing-space text `sempre `, default-y -40,
  relative-x -3.92, relative-y -34.16;
- second words: exactly U+E522 U+E522, font-family `Leland Text`;
- owning direction placement below; no sound sibling.

The ignored production walker test hashes the entire original, selects that
actual direction, validates its schema and typed fields, and roundtrips its
family projection through SFS and MusicXML. It does not claim whole-original
acceptance or preserve unrelated original music in the projection.

Current preview does not engrave authored words. It explicitly warns that
both rows are preserved but not drawn; it does not substitute a fallback font,
flatten the group, or claim glyph fidelity. Playback equivalence is tested
against the same score with the words removed. No additional sound policy.

Targeted checks: formats `--test words`; ignored lib test
`original_movement_two_p14_ordered_words_group` with
`SCORESTREAM_MXL_FIXTURES`; pure desktop `tests/words-group.test.ts`.
