# Authored standalone score words

The standalone words profile supports exactly one direction-type containing
one plain words element. It does not absorb the separate ordered words/dashes
group model. Shared typed text/style and XML text helpers keep the two consistent;
the existing dashes wire profile remains unchanged and rejects extra styling
outside its declared subset.

Standalone words retain exact decoded text (including whitespace and private-use
font glyphs), optional font family, numeric positive font size, normal/italic
style and normal/bold weight. All four default-x/default-y/relative-x/relative-y
axes retain exact finite decimal values and absence versus zero. Direction
part/measure ownership, exact rational logical offset, optional staff, placement
and system relation are retained independently.

Canonical SFS uses scorestream.words@1 with closed scorestream.words marks.
Indices retain within-measure authored order even when offsets are not increasing;
cross-measure grouping is canonical by part identifier and measure start. XML
export emits groups before notes with explicit logical offsets. Lexical decimal
spelling, source cursor/offset spelling, omitted sound flags and interleaving with
unrelated direction families are not byte-preservation promises.

## Coverage and strict boundaries

words.authored_only warns that current preview omits these instructions and
playback does not interpret them. They remain authored score content, not
analytical annotations. “pizz.”, “arco”, “Attacca”, tempo prose, cresc./dim. and
font-specific private-use glyphs do not invent timbre, navigation, tempo or gain.
Private-use text is preserved with its font name, not converted to dynamic glyphs.

Explicit supported adjacent sound tempo/dynamics remain separate and retain
their independent sounding positions, including offset sound=yes/no. Existing
staff-scoped numeric export restrictions remain unchanged; no invisible or fake
dynamic direction is invented to bypass them.
Explicit yes/no sound pizzicato on this owner is separately typed and authorized
by `scorestream.pizzicato@1`; see [sound state](PIZZICATO_SOUND.md). It is not
inferred from text and no pizzicato timbre rendition is claimed.
Ordered words/metronome/optional-words now use the separate metronome presentation
profile. Multiple plain words in one wrapper use the [ordered group profile](WORDS_GROUP_IMPORT.md). Rich markup, unrepresented styles, and group-level sound on a multiple-words direction remain unsupported. Default-x is drawing geometry, not onset time.

## Untouched source gate

The ignored original_286 test verifies all four original SHA-256 values and
extracts 286 standalone words directions (11/163/38/74). It scans actual measure
cursors and extents and round-trips an explicitly named words-family projection
through typed model, canonical SFS and XML, with rests replacing original notes.

The source additionally has seven mixed words/metronome directions (one contains
two words nodes), handled by the separate metronome presentation profile, plus
one direction-type with two words children handled by the ordered group profile. These are audited separately from the standalone words. Among the 286 standalone directions, 24 contain explicit sound
pizzicato siblings (8/0/6/10). The audit retains these sound states and exact
sounding positions along with the words. It does not claim full original
scores import or perform. The other 262 standalone directions fit this profile with
currently supported adjacent sound where present.

Run:
SCORESTREAM_MXL_FIXTURES=/path/to/originals cargo test -p scorestream-formats original_286 --lib -- --ignored --nocapture

## Primary semantics

- [MusicXML words](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/words/)
- [MusicXML direction](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/)
- [MusicXML sound](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/)

Words encode textual directions; their display axes are tenths coordinates.
An explicit sound pizzicato command affects following notes and is not equivalent
to merely preserving the literal text “pizz.”.
