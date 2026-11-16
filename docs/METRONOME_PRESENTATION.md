# Ordered authored metronome presentation

ScoreMetronome retains its existing single numeric Tempo value and optional
typed presentation metadata. Supported directions contain one numeric metronome,
or exactly words → metronome → optional words in distinct ordered direction-type
children. Shared WordsText preserves text, whitespace and supported style.
Groups do not create duplicate standalone words entries.

Metadata retains owning measure, placement, system relation, explicit parentheses
yes/no versus absence, and all four exact signed default/relative coordinate axes.
The existing part/staff scope and absolute logical onset remain on ScoreMetronome.
Parentheses absence has MusicXML's default no behavior but remains distinct from
an explicit no for interchange. Drawing coordinates are not musical time.

Canonical SFS retains one existing numeric metronome mark. A collision-free native
attachment ID links one closed scorestream.metronome_display@1 metadata mark to
that numeric host at the same onset. Metadata cannot replace beat/BPM or sound
tempo. Attachment IDs are regenerated implementation identities, not preserved
MusicXML source IDs. Invalid hosts, duplicate associations, wrong measures,
wrong times, aliasing existing object identities and incomplete word fields reject.

Current preview retains its existing numeric metronome behavior, but does not
realize the added group text, parentheses or geometry. The explicit
metronome_display.authored_only warning states this boundary. Playback uses only
its existing explicit sound tempo lane; neither prose nor printed BPM assigns
a clock. Divergent printed and sounding values remain distinct.

XML export normalizes groups before notes with exact logical offsets and separate
supported sound controls. Source cursor spelling, omitted sound flags and
interleaving with unrelated direction families are not byte-fidelity claims.
Metric modulation, textual/ranged rates, extra dots, arbitrary composites,
unrepresented font attributes on metronome itself, and unrelated rich markup
remain unsupported. Existing numeric rate/export limits are unchanged; this
extension adds no score-wide byte or event cap.

## Untouched originals

The ignored original_seven_ordered_metronome_groups_preserve_numeric_clock test
verifies all four pinned original SHA-256 values and projects the actual seven
complete direction groups with original cursor positions and measure extents.
Source notes/instrument routing are replaced by explicit rest/piano fixtures,
so this is not a complete original-score import/performance claim.

Counts I1/II1/III1/IV4. Printed values use half, eighth and dotted half beats.
Explicit quarter BPM remain I216, II58, III288, IV168/288/168/224.
The final IV group preserves "Presto [" then numeric half=112 then "]",
with parentheses=no. The other six explicitly use parentheses=yes.
Typed/SFS/XML equality and unchanged plans apart from diagnostics are checked.

Run:
SCORESTREAM_MXL_FIXTURES=/path/to/originals cargo test -p scorestream-formats original_seven_ordered --lib -- --ignored --nocapture

## Primary references

- [Metronome](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/metronome/)
- [Direction](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/)
- [Sound](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/)
- [MusicXML schema](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/)

MusicXML separates numeric printed beat/per-minute marks from sound tempo in
quarter notes per minute and defines parentheses as a visual yes/no attribute.
