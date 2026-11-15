# Authored accidental glyphs

This bounded bridge preserves explicit MusicXML accidental signs on measured and
true grace notes. It is not full-original import, complete engraving support, or
whole-score sampled audio acceptance.

## Typed authoring boundary

`ScoreEvent.accidental` and `ScoreGraceNote.accidental` are optional
`PrintedAccidental` values: a typed glyph plus independent optional `cautionary`
and `parentheses` booleans. Old common-model JSON omitting the field stays valid.
Absence, explicit no, and yes remain distinct through XML, common JSON, native
SFS marks, canonical SFS text and export. No foreign markup is retained.

The finite glyph set is the existing SFS registry: natural, sharp, flat,
double-sharp, flat-flat, quarter-sharp and quarter-flat. These MusicXML names map
exactly; they are not interchangeable approximations. Other values, including
sharp-sharp, other/SMuFL variants, and extended microtonal symbols remain blocked.
The four original Beethoven files use only the first three glyphs.

SFS uses its existing event mark, for example:

```text
+ #n accidental value=natural cautionary=true parentheses=false
```

The glyph does not change pitch, key, transposition, duration, type, stem, note
role or routing. Absence is not inferred from a natural pitch, and presence is
not inferred from an altered pitch. The existing SFS written-pitch consistency
rule is unchanged: a conflicting glyph/pitch pair fails explicitly rather than
rewriting either field. Sounding pitch is not used for this check.

Only direct note accidental leaves with optional cautionary/parentheses yes/no
attributes are newly accepted. Duplicate leaves/marks, unknown children,
editorial/bracket, print style/positions/fonts/size, and SMuFL overrides remain
unsupported by this bridge. Coordinate regeneration does not widen that scope.
Native SFS visible/editorial/mark-ID attributes remain valid native data, but
conversion rejects them because this common/XML slice has no exact mapping;
they are never silently discarded. Native playback validation is unchanged.

## Preview and playback

The preview consumes each supported measured natural/sharp/flat mark exactly
once as a child MEI `accid`. It replaces the previous pitch-derived accidental
for that note instead of duplicating it, and retains the original event-selection
mapping. Absent marks leave the existing automatic preview behavior unchanged.
Written pitch is authoritative even on transposing parts.

Cautionary and enclosure are independent: `func="caution"` does not imply
parentheses, explicit false emits `enclose="none"`, and true emits
`enclose="paren"`. Unspecified enclosure stays unspecified. Real pinned Verovio
5.6 SVG tests verify repeated glyphs, key signatures, all nine optional-flag
combinations, chord stems, cue size, full-measure rests and written C-sharp under
B-flat transposition. Unsupported values, styling, duplicate/unmapped targets or
unconsumed signs withhold preview rather than falling back to a derived glyph.
Grace layout remains explicitly unsupported.

Ordinary playback and explicitly allocated grace playback tests confirm unchanged
attacks, pitches and timing with/without the printed signs. Existing grace,
fermata and authored MIDI-intent guards/policies are not bypassed.

## Original-source receipts

The production-decoder corpus test verifies all four original SHA-256 pins and
all 68,559 notes without removing source content. It checks 3,550 glyphs: 1,649
naturals, 1,012 flats and 889 sharps. Their movement counts are 1,134 / 700 /
534 / 1,182. All 323 cautionaries have explicit parentheses=no; the remaining
3,227 omit both flags. There are 15 genuine grace glyphs, 125 chord-member glyphs
and 229 tie-start glyphs. All match written pitch; no rest/cue/unpitched/hidden
accidentals occur. Another 6,554 notes have pitch alteration but no authored
glyph, and that absence is retained. Untouched full originals still fail on
other unimplemented metadata and notation.

The exact I/P1/m96 cautionary note is tested through conversion with the explicit
coordinate policy. The exact III/P14/m258 grace note retains its beams and slur
and still fails on its slur, rather than becoming an accepted cleaned excerpt.
The separate [beam bridge](BEAM_IMPORT.md) does not admit the slur. Small
synthetic roundtrip/performance fixtures are identified separately from these
source receipts.

```sh
cargo test --locked -p scorestream-formats --test accidentals
SCORESTREAM_MXL_FIXTURES=/path/to/original-inputs \
  cargo test --locked -p scorestream-formats original_beethoven_accidental -- --ignored
cd desktop && npm test && npm run build
```

Primary definitions: [MusicXML accidental](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/accidental/)
and [accidental-value](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/accidental-value/).
