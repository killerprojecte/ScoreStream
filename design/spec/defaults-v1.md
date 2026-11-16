# Authored score-wide defaults v1

`scorestream.defaults@1` retains typed engraving defaults; it does not configure
the renderer, page exporter, fonts installed on the computer, or playback.
`ScoreDocument.defaults: Option<Defaults>` distinguishes absent defaults from an
explicit empty `<defaults/>`. It is not a foreign XML/JSON container.

## Closed MusicXML profile

The supported sequence is optional scaling, optional page-layout, optional
appearance, optional music-font, optional word-font, then ordered unadorned
lyric-font entries. Defaults must precede credits, part-list and musical parts.
Unknown children/attributes and reordered or duplicate singleton sections reject.

- Scaling contains both millimeters and tenths, exact positive finite decimals.
- Page-layout has an optional paired height/width, followed by zero to two
  ordered margin groups. Each group has all four exact signed finite-decimal
  margins and optional odd/even/both type. Absence differs from explicit both.
  Dimensions and margins preserve zero and negative decimal values rather than
  guessing geometry. Two margins is the W3C structural bound, not a file cap.
- Appearance preserves ordered line-width entries followed by note-size entries.
  The implemented typed line-width set is the 19 actual original values: light
  barline, heavy barline, beam, bracket, dashes, enclosure, ending, extend, leger,
  pedal, octave shift, slur middle, slur tip, staff, stem, tie middle, tie tip,
  tuplet bracket, wedge. Values are exact signed decimal tenths. Note-size types
  cue/grace/grace-cue use nonnegative exact decimal percentages. Duplicate entries
  are not collapsed or sorted. Empty appearance is distinct from absence.
- Font objects preserve optional exact family text and optional positive numeric
  decimal size. Empty font objects and absent fields remain distinct. Lyric fonts
  retain sequence, but verse selectors, named size keywords, font-style/weight,
  language, and other font fields remain outside this profile.

Concert-score, system/staff layout in defaults, distance/glyph/other appearance,
custom line types, lyric-language and other shapes are not silently discarded.
The line-type enum and positive scaling/font domains are this implementation's
closed profile, not claims to accept every value allowed by MusicXML.

Official references: [defaults](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/defaults/),
[page-layout](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/page-layout/),
[line-width](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/line-width/).
MusicXML's line-width type is extensible; unsupported strings fail rather than
being smuggled into an untyped payload.

## Native form

A complete score declares the capability and places `scorestream.defaults` marks
at time zero, targeted to `score`. Globally contiguous zero-based `index` retains
order. `kind=root` is required once, followed by closed typed rows for scaling,
page_layout/page_margins, appearance/line_width/note_size, music_font, word_font,
and lyric_font. Explicit container markers retain empty page-layout/appearance.
No floating-point numbers, serialized foreign subtree, arbitrary maps, or
renderer instructions enter the model. Native rational strings use the existing
strict canonical rational grammar. Unknown fields, orphan rows, duplicate
sections, bad indices/hosts/times/capability or impossible structure reject.

## Consumer contract and evidence

Import reports exact preservation. Desktop shows a warning that its independent
engraving does not apply these defaults; otherwise supported notation is still
previewed. Sounding projection ignores validated defaults and does not modify
pitch, rhythm, velocity, articulation, instruments or timing.

The ignored `original_four_defaults_exact_tree_projection_and_passive_audio`
receipt verifies all four pinned hashes and literal XML/MXL equality. It compares
every original defaults child, attribute, value and order against exported XML
through typed Score/SFS/XML, ignoring only indentation whitespace. Each has
19 line-width entries, 3 note-size entries, even then odd margins, and Leland,
Edwin, FreeSerif fonts. Scaling differs by movement and is retained exactly.
Audio equals the same neutral-note score without defaults. This is a full
**defaults-family** receipt, not acceptance of all other original score content.
Untouched original import reports are measured separately.
