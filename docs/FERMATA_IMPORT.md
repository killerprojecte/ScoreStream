# Authored fermatas without invented performance timing

This bounded slice retains source fermata signs through MusicXML → shared model → SFS → MusicXML. It **does not realize holds, create audio, or complete full original Beethoven import**. A fermata is not an authored number of seconds or a tempo multiplier. Playback explicitly rejects the required capability instead of playing the written timeline without it.

## Typed profile

`ScoreDocument.fermatas` contains strict `ScoreFermata` values:

- `event`: the exact measured note or rest event, not a merged tie chain or a guessed part-wide time.
- `shape`: optional normal, angled, square, double-angled, double-square, double-dot, half-curve or curlew. Absent preserves empty MusicXML content, whose meaning is normal; explicit normal remains explicit.
- `orientation`: optional upright or inverted. Absent preserves the implicit upright default. Orientation is not staff placement or hold length.
- `default_y`, `relative_y`: optional exact signed rational tenths. MusicXML decimal coordinates never pass through binary floating point. XML export rejects nonterminating/out-of-profile decimals rather than rounding.

One sign per measured event is supported. The direct core SFS annotation can target an unpitched measured note, but both shared-model adapter profiles still explicitly reject unpitched conversion. Distinct signs in different parts or events are all retained, even when their onsets or ends coincide. Notes, rests, ties, measured durations and written rhythm remain unchanged. Fermatas on tied stops remain attached to the written stop event.

Barline/grace/spacer anchors, multiple signs on one event, source IDs, x positioning, fonts/colors, editorial/hidden notation containers and other unmapped attributes are explicit errors. Unknown siblings such as ornaments or slurs still reject; recognizing a fermata does not authorize dropping those siblings. XML 1.0 decoding and existing strict character/ID rules are unchanged.

## Required SFS capability

```sfs
!requires scorestream.fermata@1
@1
+ #stop scorestream.fermata orientation=upright default_y=1/25 relative_y=5
.
```

The mark frame must be the referenced event's exact onset. All mark fields are optional, preserving authored absence; arbitrary timing fields are not accepted. Capability declaration is mandatory, unique and requires at least one sign. Unknown versions, undeclared/wrong-time anchors, fragments and duplicate signs reject in both text and AST validation. Older readers reject the required capability rather than overlooking the sign.

Strict adapters report `fermata.authored_only`; core inspection warns `fermata.realization_unavailable`; generic notation/playback lowering returns `fermata.performance_unsupported`. No preview engraving or sound realization is claimed. A new performed-replay entry point must reject this capability too until it implements an explicit global timing policy. Ordinary scores without fermatas retain existing behavior.

## Untouched Beethoven source evidence

The optional pinned-corpus test uses the production incremental reader and fermata mapper on original note fragments, with all four input SHA-256 values checked. It does not remove unsupported note fields or construct a cleaned score. Whole-file conversion still rejects other unsupported constructs.

Source counts by movement are 177 / 16 / 80 / 19, totaling 292. Every sign has empty normal shape, explicit upright orientation and relative-y; 78 also carry default-y. There are 113 rest anchors and 70 pitched tie-stop anchors. No barline or same-event duplicate appears in these files.

The tests preserve these incompatible interval patterns without choosing a synchronization rule:

- I, measures 2 and 5: 18 part copies at each opening anchor; the sounding measure-5 notes end ties begun earlier.
- I, measure 21: one sounding 0–2-quarter anchor and sixteen 1–2-quarter rest anchors.
- II, measure 123: eleven sounding 0–1-quarter anchors and five 0–½-quarter rests.
- I, measure 268: separate 0–2, 5–6 and 9–10-quarter anchors in the cadenza-like passage.
- III, measure 257: 0–3 and 1–2-quarter rest anchors.
- IV, measure 446: eighteen 0–4-quarter sounding anchors and a timpani 2–4-quarter anchor.

Neither equal-onset nor equal-end grouping is a universal interpretation of these signs. The four-file audit is evidence for authored mapping, not full-score import or an auditory test. Integration/CLI fixtures are explicitly synthetic; no original is sanitized to claim success.

## Later performance policy

A future interpreter must expose and report its hold/breath policy separately from authored data. It must operate on performed event occurrences after repeat traversal, retain written references, handle tied sounding intervals and rest anchors, reconcile simultaneous/unequal part annotations deliberately, and advance one global performance clock. It must not add one independent pause per staff copy, silently move tie-stop signs, infer an unbounded hold, or apply skipped/repeated controls on the wrong visit. Unsupported groups require an explicit diagnostic. Hold seconds, tempo warps, breaths and boundary ownership need direct scheduler/offline tests before any audio claim.

## Checks

```sh
cargo test --locked -p scorestream-core --test fermata_extension
cargo test --locked -p scorestream-formats --test fermatas
cargo test --locked -p scorestream-cli --test fermatas
SCORESTREAM_MXL_FIXTURES=/path/to/pinned/corpus cargo test --locked -p scorestream-formats original_beethoven_fermata_fragments -- --ignored
cargo test --workspace --locked
```

Primary references: [MusicXML fermata](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/fermata/), [fermata shapes](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/fermata-shape/), [notations](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/notations/), [barline](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/barline/), and [duration](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/duration/).
