# Typed tempo import: written display and sounding time

This is a **limited MusicXML 4.0 conversion profile**, not full Beethoven score import or a sampled-orchestra acceptance result. The unmodified Beethoven originals still contain unsupported words, layout, expressions, navigation, and other notation. Do not delete those constructs to obtain a successful conversion.

## Shared model and supported mapping

`ScoreDocument` now has two distinct, typed lanes, both with exact quarter-note positions:

- `ScoreTempo { at, quarter_bpm }`: explicit, score-wide playback tempo. `sound/@tempo` is an exact decimal quarter-note BPM value, whether `sound` is inside a direction or directly in a measure. It is never multiplied by a printed beat unit.
- `ScoreMetronome { at, part, staff, value }`: a printed numeric metronome mark, with part or explicit staff scope, whole/half/quarter/eighth/sixteenth beat, zero or one dot, and exact positive displayed BPM. It does not assign playback tempo.

SFS already has both lanes: `= score tempo=q:...` and `+ part[.sN] metronome beat=h. bpm=...`. No wire vocabulary or playback engine change is needed. SFS sounding tempo with another beat unit is normalized to equivalent quarter-note BPM; the normalization is reported. Printed metronome beat and dot remain unchanged. A disagreement between a printed mark and explicit `sound` is retained in both lanes; **explicit sound controls playback**. A metronome without sound stays visual-only, with the existing playback default and diagnostic. Prose is never interpreted as a tempo.

For example, the original Beethoven I opening's numeric values are half-note 108 and explicit quarter-note 216; movement III uses dotted-half 96 and explicit quarter-note 288. The tests exercise these numeric values in small synthetic fixtures. They do not import cleaned excerpts or establish support for the full original directions, which also have words, parentheses, placement, and layout.

## Offsets and scope

Positions come from the active measure cursor, including backup/forward, and use the divisions active at that point:

1. A direction offset always moves the printed metronome position. Its absent/default `sound="no"` leaves playback at the current cursor.
2. Direction `offset sound="yes"` also moves the associated sound position.
3. An offset inside `sound` overrides the direction's offset for playback, even when the direction offset is visual-only. It is relative to the current cursor, not relative to the direction offset.
4. Directions and sound do not advance the note cursor or enlarge pickup measures. This profile requires both resulting positions to remain strictly inside the containing measure. Cross-measure positions, including the next measure boundary, are explicitly rejected.

Sound tempo is reconciled into one score-wide lane across parts. Equal simultaneous values are coalesced with a normalization diagnostic; different values fail with `tempo.conflict`, independent of part/document order. There is no guessed “conductor part wins” rule. Global tempo export uses the first part, which must cover every tempo position. Metronome display remains on its part/staff.

MusicXML export writes separate sound controls and display directions. It indexes controls by time, includes their exact relative offsets in the divisions LCM, and never truncates fractional offsets after a pickup. Rational BPM with no finite decimal MusicXML representation fails instead of rounding. SFS continues to represent arbitrary positive rational BPM exactly.

## Explicit boundaries

The new MusicXML direction subset is exactly one direction-type containing one numeric metronome, optional offset/staff, and optional sound tempo. Standalone sound tempo is also supported. All other attributes and children remain fail-closed, including:

- Words, tempo ranges or approximate text, metric modulation, beat-unit ties, multiple dots, smaller metronome units, multiple direction-types, voice-scoped directions.
- `system`, placement, parentheses, print-object, graphical positioning, font/style, IDs and other unmapped display metadata.
- Dynamics, MIDI configuration, swing, pedals, navigation, repeat-conditioned `time-only`, and any sound property other than tempo and its child offset.
- The sound flag on an offset nested inside `sound`, which is outside this profile; the unambiguous nested offset without that flag is supported.
- `sound tempo="0"`, which calls for interactive tempo selection, rather than silently becoming a default.
- SFS score/voice/event metronome display scopes and metronome text/ID fields, which this shared display model cannot yet preserve.

These failures produce no partial score or converted CLI stdout. Old shared-model JSON without the two new lanes remains readable, while unknown fields still fail.

## Verification

Run:

```sh
cargo test --locked -p scorestream-formats --test tempo
cargo test --workspace --locked
cargo run --locked -p scorestream-cli -- convert musicxml sfs crates/scorestream-formats/tests/fixtures/tempo-pickup.musicxml
```

The focused tests cover original-source numeric values, exact decimal sounding tempo, dotted display, printed/sounding disagreement, visual-only marks, all supported offset precedence cases, negative offsets after a note, backup cursor behavior, partwise agreement/conflicts, streaming with two-byte buffers, SFS/common/MusicXML roundtrips, unsupported payloads, malformed cardinalities, and old-model JSON defaults.

Playback assertions call the actual `scorestream_app::playback::plan_source` after MusicXML → shared model → SFS. They check note attacks/gates, written note/rest intervals, pickup and measure boundaries, and total seconds with tempo changes inside a held note and a rest. This remains written-time playback; repeat traversal is a separate integration gate. No synthesized or sampled WAV quality, device, MIDI/VST, desktop rendering, or platform UI claim follows from these tests.

## Primary semantic references

- [MusicXML 4.0 sound](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/): quarter BPM, zero's interactive meaning, nested offset precedence.
- [MusicXML 4.0 offset](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/offset/): divisions-relative current location and default visual-only direction offset.
- [MusicXML 4.0 direction](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/): visual sequence, voice/staff and system display semantics.
- [MusicXML 4.0 metronome](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/metronome/): numeric marks versus metric relationships and display attributes.
