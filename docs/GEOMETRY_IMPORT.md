# Explicit coordinate regeneration during MusicXML import

The default MusicXML adapter remains strict. One finite, input-only policy can
now regenerate three known geometric attributes while retaining the supported
musical authoring model:

```rust
use scorestream_formats::musicxml::{self, GeometryPolicy, MusicXmlOptions};
let options = MusicXmlOptions {
    geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
    ..Default::default()
};
// read_with_options or read_reader_with_options; no CLI/desktop default changes.
```

This variant is permanently scoped to:

- `/score-partwise/part/measure/@width`
- `/score-partwise/part/measure/note/@default-x`
- `/score-partwise/part/measure/note/@default-y`

The generated XML ancestry and attribute name must both match. All three fields
use signed exact decimal tenths, including negative values and zero. The existing
64-character numeric bound applies; no floating point, NaN, infinity, exponents
or rational-slash lexical values are accepted. Only XML whitespace is permitted,
not NBSP or other Unicode spacing characters. Both incremental passes validate
these fields. The source fingerprint still includes every coordinate byte, so a
change even to discarded geometry fails the two-pass identity check.

## Loss and reporting contract

Opt-in means these original coordinate values are **not retained** in
`ScoreDocument`, SFS or regenerated MusicXML. A downstream engraver must compute
new positions. This is deliberate layout loss, not lossless roundtrip or equivalent
normalization. Pitch, time, duration, visibility, staff ownership and authored
symbols are not authorized to change. The caller must surface this limitation
before selecting the policy; the default cannot select it automatically.

A constant-size first-pass audit counts each accepted attribute across the whole
input. Up to three `policy_applied / geometry.regenerate_coordinates` diagnostics
include the exact counts, first encountered source path/position and explicit
loss wording. These summaries precede ordinary diagnostics, survive semantic
mapping failures and cannot turn truncated errors into success. A fatal parse may
only have visited part of the input; summaries say “encountered in the first pass,”
not that import succeeded. Failed import never returns a partial score.

The geometry choice does not persist as permission for later imports. Its serde
default is `Strict`; old options JSON still decodes strictly and omits the strict
field when serialized. XML export has no geometry to restore and does not infer
positions from this input-only option. Exact irregular-measure contracts remain
independent. The integrity-checked MXL root enters the same options-aware reader.

## What remains blocked

This is not an allowlist of arbitrary “layout” nodes. Unknown descendants and
attributes remain errors, including when nested inside known notes. In particular:

- `print-object`, print-dot/leger/lyric, visibility, attack/release, dynamics,
  pizzicato and repeat-conditioned timing are outside this geometry choice.
- Beams, slurs, accidental display, lyric text, words and other meaningful
  notation must receive their own typed authoring semantics. Attribute-free stems
  and full-rest identity now have a separate [lossless typed bridge](NOTE_APPEARANCE_IMPORT.md);
  the geometry policy never discards or interprets them.
- Positioning on other elements, `relative-x`/`relative-y`, color, placement,
  fonts, unpitched display information and time-signature symbols are not covered.
- `defaults` remains blocked. It can contain concert-score and lyric-language,
  not just sizes and margins.
- `print` remains blocked. Besides geometry, it can change displayed part names,
  abbreviations, measure numbering and page information connected to credits.
- Credit text, rights and instrument/device information are never discarded by
  calling their containing subtree “engraving-only.”

The next possible geometry slice should validate an explicit complete grammar
for scaling/page/system/staff dimensions, one shape at a time. It must not accept
an entire `defaults` or `print` subtree when an unknown or semantic child exists.
Fonts, numbering, labels and credits need separate preservation decisions.

## Untouched original evidence

The corpus regression hashes the original XMLs and compares options-aware XML
and integrity-checked MXL failure reports. It recognizes the following geometric
attributes without modifying the sources:

| Movement | Measure width | Note default-x | Note default-y |
| --- | ---: | ---: | ---: |
| I | 9,036 | 15,261 | 15,261 |
| II | 4,464 | 10,908 | 10,908 |
| III | 6,768 | 11,488 | 11,488 |
| IV | 10,258 | 30,902 | 30,902 |
| Total | 30,526 | 68,559 | 68,559 |

This resolves the classification of 167,644 coordinate occurrences under opt-in;
it does **not** make any original import succeed. The bounded reports still
contain unsupported header, print and musical fields. Separately identified small
fixtures verify unchanged notes/rests, exact model/SFS roundtrips, playback,
numeric rejection, error truncation and changed-source detection. No stripped
original, substituted whole score, new audio acceptance or native UI is claimed.

## Official definitions

- [MusicXML note: default-x and default-y positioning, distinct from pitch and performance attributes](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/)
- [Partwise measure: width in tenths](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-partwise/)
- [Signed decimal tenths](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/tenths/)
- [Defaults includes concert-score and lyric-language](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/defaults/)
- [Print includes changing part-name and abbreviation display](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/print/)
