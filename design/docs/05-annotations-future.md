# Future knowledge annotations and revision-bound anchors

**Status: design contract only. Do not implement annotation storage, annotation MCP tools, an annotation editor, indexing, or automatic rebasing in the current Rust delivery.**

The current product is the shared Rust score authority, its human desktop editor and live score preview, and the MCP interface through which an agent reads the format and edits the same score. Future playback belongs to a separate performance interpretation. This document reserves a compatible, independent home for structured textual knowledge about musical passages.

## 1. Keep score marks and knowledge annotations separate

A ScoreStream `+` record is a score-owned notation object: for example, a lyric, a printed accidental, or a direction. Such an object is part of a score revision and participates in score import, editing, export, and rendering.

A future knowledge annotation explains a passage, such as “the rising upper line increases anticipation over a sustained bass.” It lives in a separate knowledge store, has its own revision history, and refers to an immutable score revision or a deliberately tracked set of score identities. Creating, editing, rebasing, or deleting it never changes a score revision or inserts a `+` record.

The future schema describes stored/output records, not a currently available mutation API:

- Schema: `schemas/annotation.future.schema.json`.
- Schema identity: `urn:scorestream:schema:annotation-future:1.1`.
- Root definition: `KnowledgeAnnotation`.
- Referenced score identities and exact time values use `common.schema.json`.

Annotation provenance is derived from an authenticated human or agent context. A future client must not grant itself authority by supplying `created_by`; that field is output provenance. In the present score editor the same rule is already reflected in `edit.schema.json`: mutation input has no actor field, while committed output reports the authority-derived actor.

## 2. Minimal independent record

| Field | Meaning |
| --- | --- |
| `annotation_id` | Stable identity of the knowledge item. |
| `annotation_revision_id` | Immutable revision of that item, independent of the score revision. |
| `binding` | A pinned score reference or an explicit follow-identity binding. |
| `effect.summary` | A concise textual explanation. |
| `effect.musical_features` | Concrete features supporting the explanation. |
| `effect.perceived_effect` | The claimed musical or perceptual result. |
| `effect.conditions` | Context, style, performance, or interpretation limits. |
| `effect.evidence_kind` | `source_statement`, `computed`, `human_analysis`, or `model_analysis`. |
| `effect.method` | Method and version for computed/model claims; optional attribution detail for other claims. |
| `effect.confidence` | Optional value with a documented interpretation; not an automatic probability of musical truth. |
| `resolution` | Separate anchor validity and explanation-review status. |
| `created_by` | Provenance derived by the service, not caller-controlled authority. |

Original score facts, deterministic computations, human analyses, and model analyses remain distinguishable. An explanation about a musical effect must not overwrite the original notes or be presented as a source fact merely because it points to them.

## 3. Four anchor shapes

The anchor is a discriminated union. A schema must not accept one object that ambiguously mixes pixels, time, and event identities.

| `kind` | Fields | Semantics |
| --- | --- | --- |
| `events` | `event_ids` | Exactly these stable event identities. An event set does not select all events between its first and last time. |
| `measures` | `measure_ids` | Exactly these stable local measure objects. Printed labels such as `1`, `1a`, or a repeated `8` are not identities. |
| `score_region` | `clock_id`, `start`, `end`, optional `filters` | A musical region in exact score time `[start,end)`, intersected with explicit part/voice/staff filters. |
| `visual_snapshot` | `render_id`, `page_id`, `coordinate_space`, `rectangle` | A region in one immutable rendered page, useful when a graphic region has no interpreted score objects yet. This is evidence tied to that render, not a musical-time anchor. |

Times are canonical rational strings in the named score clock. `"1/3"` is exact; the floating-point JSON number `0.3333` is not a score-time value. JSON Schema checks lexical shape, while semantic validation checks reduction, ordering, object existence, scope, and revision consistency.

### Region boundaries and filters

A score region is half-open: an event beginning exactly at `end` is outside its onset range. Selection tools separately declare whether they match an onset, an overlapping event, or a fully contained event. Overlap selects a whole event; it never silently clips the event at the region boundary.

Part, voice, and staff filters are intersected. An omitted filter is unrestricted; empty filter arrays are invalid. A staff filter uses the event’s resolved staff at its onset, including a local staff override, rather than the original default staff of its voice. A stable voice can cross between staves without changing identity.

Grace notes need an explicit selection rule. A region may select them by their principal event or by their score anchor. Their display order and attachment are not inferred from a zero duration. Ties, chords, and slurs do not silently pull all connected notes into a selected region; expansion policies and boundary context must be explicit.

Selection resolution first matches the selector, then intersects filters, then applies requested grace/chord/tie expansions. A whole-chord or whole-chain expansion may intentionally extend outside the original time interval, and the frozen result must show that fact. It cannot escape an explicitly supplied part, voice, or staff filter: an incompatible filter/expansion combination is a conflict, not permission to silently truncate the group. Expanded membership does not enlarge the allowed duration-preserving replacement span.

### Visual coordinates are not persistent musical truth

A desktop drag rectangle first identifies a particular `render_id`, `page_id`, and page-viewBox rectangle. Zoomed viewport pixels must be transformed to that coordinate system before resolution. The render’s score revision must match the requested score revision.

The renderer’s semantic hit map can turn note/rest glyphs into stable event IDs. Once that succeeds, persist a musical anchor and retain the original rectangle only as optional provenance. A later layout, font, page size, or line break must not be used to reinterpret the same rectangle as a new musical subject.

If no semantic mapping exists, the future annotation may retain a pinned `visual_snapshot`. It cannot use follow-identity mode, claim event-level understanding, or drive score mutation. A stale render must be rejected for editing, not silently resolved against the newest page.

## 4. Pinned versus follow-identity bindings

### Pinned

```json
{
  "mode": "pinned",
  "score": {"score_id": "score_demo", "revision_id": "rev_001"},
  "anchor": {
    "kind": "score_region",
    "clock_id": "score",
    "start": "0",
    "end": "4",
    "filters": {"part_ids": ["part_p"]}
  }
}
```

A pinned item always describes that historical revision. Editing the current score head does not make its historical anchor invalid. The interface may say “written for an earlier revision,” and may offer a deliberate new analysis or rebase, but must not change its subject in place.

All four anchor kinds are allowed in pinned mode. A visual snapshot’s render revision must equal the binding revision.

### Follow identity

```json
{
  "mode": "follow_identity",
  "score_id": "score_demo",
  "origin_revision_id": "rev_001",
  "resolved_revision_id": "rev_001",
  "anchor": {
    "kind": "events",
    "event_ids": ["event_c4", "event_e4"]
  }
}
```

Only event and measure identity sets can follow identity. Absolute times and visual rectangles cannot. To turn a time or visual range into a following item, resolve and explicitly accept stable event/measure subjects first.

Following means tracking the same identities, not selecting the closest notes, reusing a printed measure label, or rerunning an old range query. The stored `resolved_revision_id` advances only after a successful explicit rebase. If rebasing fails, retain the previous valid binding and report the failed attempt separately; do not overwrite it with a guessed subject.

A measure anchor describes that measure’s content. New notes inserted into the same measure therefore change its subject matter and can require explanation review. A newly inserted *different measure* is not automatically added to an existing measure-ID set. For an event-ID set, newly inserted neighboring events are never automatically included.

## 5. Anchor validity and explanation validity are different

`anchor_status` and `content_status` answer different questions:

- `anchor_status`: can the referenced musical objects or render evidence be located unambiguously?
- `content_status`: is the explanation still applicable to the located content?

A note can retain its identity after its pitch changes. The anchor remains `resolved`, but a claim that the melody rises may need `review_required`. Similarly, a measure can retain its ID while its harmony, rhythm, orchestration, or dynamics changes.

The future service must use the score change set and the annotation’s declared subject/dependencies. For example, a claim about orchestration can depend on an instrument outside a narrow list of pitched events. If dependency coverage is incomplete, flag review rather than asserting that the claim is unaffected.

| Change | Anchor treatment | Explanation treatment |
| --- | --- | --- |
| Layout only, stable musical IDs | Event/measure anchor remains resolved. | Usually unchanged; a visual-layout claim may need review. |
| Note moved with same ID | Follow that identity to its new position. | Review claims that depend on position, rhythm, or context. |
| Pitch or duration changed with same ID | Identity can remain resolved. | Review affected musical claims. |
| Referenced note deleted | Report missing identity; do not select a neighboring note. | Unresolved until explicitly repaired or superseded. |
| Referenced note replaced by a new ID | Replacement lineage is evidence, not equality. | Require explicit subject acceptance and review. |
| Note split or several notes merged | Do not choose one descendant or expand to all automatically. | Rebase conflict until a deliberate mapping is supplied. |
| Measure renumbered, ID unchanged | Continue to resolve by ID. | No review solely for a display-label change. |
| Measure split/merged | Explicit lineage conflict. | Review/reanchor deliberately. |
| Render regenerated | Old snapshot remains pinned to the old render. | No automatic rectangle relocation. |

The rebase schema describes a future operation with annotation-level compare-and-swap. It takes an expected annotation revision and a source/target score revision. A conflict or unresolved outcome does not publish a replacement binding. A successful rebase creates a new annotation revision and records the decision. Neither outcome mutates the score.

## 6. Current selection and edit contracts that make this possible

These score-side contracts belong to the Rust editor/MCP design, not to an implemented annotation subsystem:

Opaque external IDs and SFS lexical aliases are different types. Use the alias/reference mapping returned with score text; never guess an alias from an external ID. Fragment voice aliases and fragment-local event IDs use the SFS `TextIdentifier` grammar, while their bindings point to existing external identities. A part and a voice cannot share a bare scope alias, and `score` is reserved.

1. Resolve a `SelectionRequest` against an immutable `ScoreRef`.
2. Materialize all selection expansions and boundary context into a `FrozenSelection`.
3. Return its `selection_id`, exact score revision, and digest. The digest covers canonical selection policy and resolved membership; it is not a mutable query token.
4. Submit edits with `expected_revision_id`, `idempotency_key`, and that frozen reference.
5. The shared desktop/MCP command authority stages the entire command privately, validates its final graph, and compares-and-swaps the score head once.
6. If a human edit moved the head while an agent was reasoning, reject the stale command. The agent must read the new revision, deliberately resolve a new selection, and submit a new command. Do not replay the old selector against the new head.
7. Publish changed, created, and deleted identities plus explicit lineage. Renderer updates and any future knowledge service consume this versioned change result independently.

Retries with the same authenticated principal, score, idempotency key, and canonical request return the original result. Reusing that key with a different request is an error. An idempotent retry remains a retry even if a later command has already advanced the head.

### Section replacement

In v1.1, replacement is explicitly duration-preserving. It uses a frozen contiguous musical span, a ScoreStream `!fragment 1.1 unit=q span=...`, explicit fragment-voice bindings, and matching local `!end at=...`. It retains existing state and measure objects; the fragment cannot change those objects. Incoming/outgoing sounding events cannot be silently cut.

Crossing relations are not silently discarded. An unaffected relation spanning the region can remain intact. If a replaced event is a referenced endpoint/member, the command must resolve that dependency explicitly or fail. A retarget operation can refer either to an existing event or to a named event inside a specific replacement operation in the same atomic command; the service allocates the final ID before validation.

Ripple replacement is deferred and is not an accepted v1.1 schema value or advertised capability. Its future design would need agreed aligned scopes, changes to all affected states and boundaries, and explicit treatment of independent clocks and cross-part relationships. The current format does not pretend that moving a few selected notes is a complete ripple operation.

## 7. Validation and examples

`examples/auxiliary/manifest.json` labels three categories:

- `valid`: must pass the referenced JSON Schema. Some examples assume fixture objects exist; production still performs semantic checks.
- `invalid`: must fail JSON Schema, such as floating-point score time, caller-supplied actor authority, or an attempted ripple policy.
- `semantic-invalid`: intentionally passes structural schema but must fail a named semantic check, such as an unreduced fraction, reversed time range, stale selection base, fragment-length mismatch, or wrong render revision.

The schema `$id` values are local URNs. Register package schemas in the validator; do not fetch schemas from arbitrary URLs or treat the standard draft URI as a runtime network dependency.

These examples and schemas are contracts for future implementation. They do not create annotation records, implement an annotation service, train a knowledge model, or claim that a Rust server is already running.
