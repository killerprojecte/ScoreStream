# Ordinary barline import and score replay planning

This slice maps actual MusicXML boundary barlines into `ScoreDocument.barlines`, exports them back to MusicXML, and connects that common model to the typed repeat traversal core. The subsequent [SFS navigation capability](SFS_NAVIGATION.md) adds authored-only closed-repeat SFS roundtrip. Preview repeat engraving, performed note realization and audio export of repeated scores remain unsupported; playback lowering explicitly rejects instead of silently playing the written sequence once.

## Authored source and derived route

Each `ScoreBarline` retains its part, measure identity, left/right location, optional XML identity, optional typed line style, repeat direction and optional authored count, and optional ending sign. An ending sign retains its pass list, start/stop/discontinue phase, display text and exact `default-y` placement. An omitted line style stays omitted. The absence of a closing ending sign stays absent; the model does not invent one at a measure or movement boundary.

The XML mapper requires a left barline before other musical measure content and a right barline after it. It accepts the MusicXML default right location, enforces supported child order and cardinality, and rejects middle placement, unknown attributes/children and malformed values. Repeat counts are bounded nonnegative integers; count zero is retained as authored but cannot compile into a supported performance plan. Unknown ending passes are explicitly unsupported. Explicit ending lists are limited to 1,024 distinct positive integers. Exact vertical placement export is limited to terminating decimals of at most 32 places and 64 characters; unsupported values are rejected, never rounded.

Authored signs remain per part. `navigation::from_score` checks exact measure-time alignment and derives a single route by comparing navigation families independently. Every nonempty part repeat sequence must agree, and every nonempty ending sequence must agree. A wholly omitted family is permitted and its omission remains in the authoring model. This admits an orchestral score with repeat signs on all parts but endings only in the top part. Partial conflicting sequences are rejected; there is no selected part that silently wins. Ending display text and layout can differ by part without changing the performed pass list.

The first profile supports nonnested ordinary repeats without endings, or a closed first ending followed by an adjacent final ending. Closed endpoints preserve stop versus discontinue. Nested/overlapping repeats, unmatched forward signs, malformed endpoints, complex multiple endings and conflicting part navigation cannot produce a successful plan. The source model may still preserve a structurally valid unsupported route for MusicXML roundtrip.

## Exact replay contract

`navigation::plan(score, movement_id, policy)` returns:

- The reconciled navigation model, plus the parts that supplied each family.
- The reviewed typed traversal steps, including capture, restore, replay, skip and release boundaries.
- Unique measure occurrences that reference the original measure index, written start, performed start and exact length, all in quarter-note rationals.
- The total performed quarter length, with no seconds or silence policy inferred.

Checkpoint capture occurs before the entry measure's authored controls. A backward edge restores that checkpoint before replaying the entry. A skipped first ending executes none of its notes or controls. In particular, loading a precomputed linear state snapshot at the second ending would leak skipped controls and is forbidden by the contract. The source states and notes are not duplicated or rewritten. Later realization must project authored controls onto each performed occurrence and rebind ties, slurs, pedals and other relations across the actual jump/skip edges.

The repeat occurrence budget is checked before output. Derived navigation uses ordered maps and sorted mark lists, followed by the traversal core's bounded emission; ending assignment does not scan every ending for each repeat. Measure lengths come from the common model, not from labels or inferred meter. The separate cadenza-duration import limitation remains unchanged.

## Untouched Beethoven evidence

An optional unit test runs this production barline mapper against all four original pinned MusicXML streams, verifies their exact SHA-256 fingerprints, and checks all 36 / 18 / 93 / 140 barlines. It confirms the actual repeat boundaries and top-part ending phases: I backward after 124 with no forward sign; III forward at 142, first ending 162 and closed second ending 163; IV forward at 2, first ending 85–86 and second ending beginning 87 with no endpoint. It also verifies that the full strict original conversion still rejects other unsupported meaning. Extracting boundary fragments in this test is not full-score import.

The source attribution, original hashes, navigation fixture and route counts remain in [the original traversal evidence](REPEAT_TRAVERSAL_PROTOTYPE.md). The default traversal policy rejects IV's unclosed final ending. The explicitly selected final-continuation policy reports its inference and keeps the source unclosed. It is never enabled automatically by the importer.

```sh
cargo test --locked --offline -p scorestream-formats --test barlines
SCORESTREAM_MXL_FIXTURES=/path/to/pinned/inputs cargo test --locked --offline -p scorestream-formats original_beethoven_barline_fragments_use_production_mapper -- --ignored
```

## Subsequent SFS capability and remaining performance work

The registered SFS capability stores authored boundary records, not pre-expanded notes, state snapshots or a second redundant route. Its typed vocabulary carries part/measure-boundary identity, left/right placement, line style, optional explicit forward/backward count, ending pass/phase/display fields, and preserved open endpoints. Engraved placement remains separate from the score-wide performed graph. The versioned required capability prevents older readers from silently ignoring these fields.

The SFS reader, validator and common-model bridge now agree on authored-only serialization; notation lowering explicitly rejects this capability until performed replay exists. Unsupported readers must reject the capability rather than play once. The playback layer then consumes the exact occurrence plan, restores scope-specific state at backward edges, replays controls in written order, omits skipped controls, and rebinds sounding relations by occurrence. Movement joins remain explicit; III→IV attacca inserts no pause and cannot infer instrument identity from repeated local part IDs. Full original import and listening acceptance remain later end-to-end checks.

Primary references: [MusicXML barline](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/barline/), [repeat](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/repeat/), [ending](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/ending/).
