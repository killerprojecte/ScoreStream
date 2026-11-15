# Ordinary repeat traversal prototype

This document records the original independently tested pure-core prototype and corpus evidence. The subsequent [barline import slice](BARLINE_IMPORT.md) exports `crates/scorestream-core/src/repeat_traversal.rs` and connects actual typed MusicXML barlines to the common-model replay plan. SFS grammar, desktop, preview and audio realization remain unsupported; the original full scores still do not import successfully. The traversal tests now exercise the exported core directly.

## Exact original evidence

The checked-in `crates/scorestream-core/tests/fixtures/beethoven-navigation.json` records navigation-only evidence extracted read-only from the four pinned Hauptstimme MusicXML payloads. `scripts/audit_beethoven_navigation.py INPUT_DIRECTORY OUTPUT_JSON` verifies all four SHA-256 hashes before parsing, requires matching measure-label sequences across every part, records each repeat/ending and which parts actually contain it, and preserves the absence of a closing ending marker. It is a corpus audit, not an alternative production importer. No source notes or directions are removed or rewritten.

The source repository is [Hauptstimme](https://github.com/MarkGotham/Hauptstimme), tree `8f677853c45b74ca0afcd79677e5379c06c82354`, path `data/Beethoven,_Ludwig_van/Symphony_No.5,_Op.67/{1,2,3,4}/Beethoven_Op.67_{1,2,3,4}.mxl`. Embedded XML rights identify scores as CC0 and annotations as CC-By-SA. This fixture contains score navigation facts, not the annotation layer.

| Movement | Source labels | Repeat evidence | Performed labels under stated policy | Occurrences |
| --- | --- | --- | --- | ---: |
| I | 1–502, 18 parts | Backward after 124, no authored forward mark | 1–124, 1–502 | 626 |
| II | 0–247, 18 parts | None | 0–247 | 248 |
| III | 1–376, 18 parts | Forward at 142; first ending 162; backward after 162; second ending 163, explicitly discontinued after 163 | 1–162, 142–161, 163–376 | 396 |
| IV | 1–446, 23 parts | Forward at 2; first ending 85–86; backward after 86; second ending starts 87 without a closing marker | 1–86, 2–84, 87–446 | 529 |

All repeat signs agree across every part. Ending marks appear only in P1. There are no D.C./D.S./coda/fine/time-only sound-navigation attributes in these originals. A future adapter must preserve the part provenance while reconciling these conductor-score navigation marks into a single score-wide graph, rejecting conflicts rather than multiplying repeats by part count.

The route counts total 1,799 performed measure occurrences from 1,572 written measures across the four movements. They are not note-attack counts or duration estimates. Movement I uses an explicit reported two-pass convention and inferred start at the movement boundary; no forward-repeat glyph is invented.

### Unclosed second ending in IV

The default policy **rejects** an unclosed final ending. The explicit `ContinueToMovementEnd` policy is available only when it is the final navigation structure, emits `UnclosedFinalEndingContinues`, and leaves the original `EndingEnd::Unclosed` unchanged. It does not synthesize a stop/discontinue marker or claim that the printed bracket extends to the end.

The conditional corpus traversal is a documented performance inference: after playing the closed first ending once, the final pass skips 85–86, enters the explicitly numbered second ending at 87, and continues linearly because no further navigation is encoded. A later or overlapping repeat causes rejection rather than guessing a missing boundary. Source fidelity and routing feasibility remain separate: ordinary MusicXML `stop` and `discontinue` have distinct engraving meaning, and neither is equivalent to a missing endpoint.

## Typed boundary and state contract

- Positions are boundaries between indexed measures, not arithmetic on display labels. Labels such as 0, 1a and duplicate display labels remain intact.
- `RepeatStart` distinguishes explicit starts from an absent forward mark. `RepeatRegion` holds the backward boundary, optional authored play count, and typed ending pass numbers/closure status.
- Supported traversal shapes are non-nested ordinary repeats without endings, or a closed first ending followed by a final ending. The first ending may apply to passes 1..N−1; the final applies only to N. More complicated multiple endings, nested/overlapping regions, middle-barline navigation and jumps are not silently approximated.
- `CaptureCheckpoint` precedes the first execution of the entry measure. On backward traversal, `RestoreCheckpoint` restores that entry state and is followed by `ReplayMeasure` instructions.
- `ReplayMeasure` replays the original measure's authored events and controls at a unique performed occurrence. It does not copy precomputed MIDI/audio or edit the source timeline.
- `SkipFirstEnding` executes no notes or controls in the skipped range. It carries the current performed state forward. Loading a linear written-state snapshot at the destination would incorrectly apply controls inside the skipped first ending.
- Ties, slurs, hairpins, pedal and other relationships must be rebound to performed occurrences and validated across jump/skip edges. The prototype exposes these edges but does not claim that relation realization is implemented.
- A caller-selected occurrence budget bounds allocation. Validation precedes output, so errors return no partial plan. Validation sorts R repeat regions and E ending-pass entries, costing O(R log R + E log E); plan emission is linear in performed measure occurrences plus navigation steps. Memory is proportional to the emitted plan plus input navigation. Nested traversal is rejected by this profile.

An independent source-state audit found different repeat-entry vs first-pass-end dynamic values in 14 parts of movement III. For example, Flute 1 has 44.44 at entry and 87.78 by the repeat end. This proves why carrying the last dynamic blindly over the backward jump is incorrect. The focused simulator test also demonstrates a first-ending-only control change that must not leak into the second ending.

## Movement continuity

Movement III P1, measure 376, explicitly contains `Attacca`, and its final barline is light-light. The sequence model keeps III→IV as `MovementJoin::Attacca`: no inserted pause, while preserving movement boundaries. Other joins remain `Separate`, with no invented silence duration.

Part IDs must be namespaced by movement. P1 is Flute 1 in III and Piccolo in IV, so equal local part IDs cannot imply a continuing instrument identity. Each movement begins with its own initial state. Attacca does not create cross-movement ties, merge routes, truncate instrument release tails or carry a previous movement's tempo/transposition blindly. The source's final III tie endpoints are stops, not starts of cross-file ties.

## Additional full-import blocker discovered

Movement I measure 268, the oboe Adagio cadenza, encodes **10 quarter notes of time in all 18 parts** while the inherited meter remains 2/4 and the measure is not marked implicit. Hidden rests align the other parts. A faithful authoring model needs actual irregular measure duration distinct from nominal meter; reducing it to two quarters would truncate the source. The oboe's `<type size="cue">` means reduced printed note size, not a `<cue/>` event that should be silent. This prototype does not alter the measure importer to handle that separately scoped issue.

## Checks and remaining integration

Run:

```sh
cargo test --locked -p scorestream-core --test repeat_traversal
python3 scripts/audit_beethoven_navigation.py /path/to/pinned/inputs /tmp/beethoven-navigation.json
```

Tests cover explicit/default counts, implicit starts, multiple passes, closed endings, checkpoint restoration, skipped controls, exact original routes, explicit open-ending policy, preserved unclosed source data, sequential repeats, rejected nested/conflicting/malformed/budget-overflow cases, unique performed occurrences and attacca identity separation. Exhaustive small closed-route tests validate every supported boundary combination through eight measures.

The barline slice now supplies typed authoring fields, per-part reconciliation and MusicXML roundtrip. Before complete-score performance: map SFS without loss; connect state replay and relation rebinding; support irregular measure lengths; route the original imported model to preview/playback; then verify complete-score audio. A successful traversal fixture is not full-score import or listening acceptance.

Primary references: [MusicXML repeat](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/repeat/), [ending](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/ending/), [ending-number](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/ending-number/), [barline](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/barline/), [sound navigation](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/).
