# Rust implementation plan

Status: proposed implementation architecture and milestones. This bundle contains design documents and contracts, not an implemented or compiled Rust application. No Rust build, product runtime test, editor session, renderer integration, MCP service, or performance result is claimed as executed. The finite Python package QA has run; its scope and results are recorded in `validation/README.md` and `validation/format-report.json`.

## 1. Scope and implementation boundary

The first contract is the ScoreStream format standard that an agent retrieves through `score_get_format`. It provides rapid understanding of the text language and references to its complete grammar, machine-readable descriptor, normalized AST schema, concise agent guide, and complete examples. Operational score tools are auxiliary to that language contract.

Rust owns exact musical data, parsing, interchange conversion, validation, immutable revisions, region resolution, edit transactions, I/O, storage, and MCP integration. The planned desktop client is Tauri 2 with a proposed TypeScript WebView UI: Monaco for SFS source editing and an SVG notation surface for selection, properties, and keyboard editing. The renderer is a replaceable external dependency. There is no Python production component.

Human editing and MCP calls share one application service and revision history. The desktop milestone includes automatic debounced preview of valid dirty text using private draft snapshots; this does not commit revisions or expose drafts through MCP. Playback/performance and the physically separate structured region-annotation store are deferred. No annotation table, service, or annotation handler is part of the first implementation.

## 2. One workspace with a small set of boundaries

Use one Cargo workspace. Keep modules inside these crates until an independently testable boundary justifies another crate.

| Proposed crate | Responsibility | Must not own |
| --- | --- | --- |
| `scorestream-core` | Exact numbers, typed IDs, score objects, state, relations, regions, validation, immutable revisions, patch semantics | XML parsing, a renderer document, HTTP/MCP objects, analytical annotations |
| `scorestream-codec` | ScoreStream text parsing, canonical writing, frame commit state, diagnostics, projection/event maps | A second musical model |
| `scorestream-musicxml` | Versioned MusicXML import/export, coverage manifest, source bindings, precision and loss reports | Rendering as a substitute for conversion |
| `scorestream-rendering` | Renderer-specific projection, stable ID map, render contract, capability diagnostics, backend interface | Authoritative edits or source round trips |
| `scorestream-app` | Snapshot persistence, region/edit orchestration, bounded jobs, render scheduling, application APIs | Audio scheduling inside request handlers |
| `scorestream-cli` | Import, inspect, validate, convert, and local workflow entry points | Reimplemented parsing or edit rules |
| `scorestream-desktop` | Tauri lifecycle, the workspace authority, frontend command/channel adapters, editor session coordination | A second revision model |
| `scorestream-mcp` | Format discovery and implemented operational handlers; stdio bridge and optional host/transport integration | A private editable workspace competing with the desktop |

The desktop UI is a small adjacent TypeScript package. It hosts Monaco, loads SVG resources, manages selection/property controls, and hosts the Verovio worker. It sends text and typed edits to Rust rather than maintaining an independent editable musical model. Tauri's documented Rust core/WebView split and command/channel APIs support this arrangement. [R5, R6]

Create these crates as their boundaries become useful; the table is an ownership plan, not a requirement to start with eight empty crates. Deferred performance and annotation components should not become placeholder services.

## 3. Domain types and exact arithmetic

Use distinct Rust types for `ScoreId`, `RevisionId`, `EventId`, `MeasureId`, `VoiceId`, `RelationId`, `RenderId`, and application `JobId`. The private desktop interface also needs `EditorSessionId`, a draft sequence, and a tagged draft-snapshot key distinct from `RevisionId`. Type distinctions prevent passing a render ID as an edit target, a draft key as a committed revision, or a display measure label as a persistent identity.

The core number type must represent arbitrary-precision reduced rationals, including exact decimal imports. A candidate implementation is `BigRational` backed by `BigInt`; the exact crate versions and enabled features are an implementation decision recorded in the lockfile. Avoid fixed-width multiplication for numerator/denominator arithmetic and avoid a floating-point intermediate. [R8]

In the v1.1 API contracts, exact numeric fields use canonical reduced rational strings only. Numerator/denominator pairs may be used internally; they are not an alternative accepted API encoding unless a future version explicitly defines one. Do not serialize exact score coordinates as JSON floating-point numbers. Timestamps used for UI or later audio transport are separate types and have their own precision contract.

Keep these identities and values distinct:

* A measure object ID, its displayed label, and its local order.
* A written pitch, its explicit accidental object, and a derived sounding pitch.
* An event's score length, written duration/dots, and a performance offset.
* A written object, a repeat traversal occurrence, and a rendered SVG instance.
* Common-clock score positions and positions in an optional local-clock extension.

The core should not require unsafe code. If a native renderer FFI is later selected, put the small ownership-sensitive bridge behind the rendering backend boundary rather than exposing foreign pointers to score objects.

## 4. Parser and conversion design

Implement the language standard as a versioned registry distributed with the application and exposed by `score_get_format`. The human reference, EBNF, language descriptor, AST schema, and examples must refer to the same format version. Add conformance fixtures with expected parses and expected failures.

The EBNF and lexer rules describe raw SFS text. JSON Schema validates the normalized AST representation after parsing; it does not establish that arbitrary SFS text is valid. Semantic checks then cover frame references, exact values, state scope, and musical relationships. A passing AST schema is not a substitute for those checks.

The text parser emits typed records into a frame builder. A frame is not published until its closing commit record has been parsed and frame-local references are resolved. The builder then applies the frame atomically to a candidate snapshot. Incomplete input produces diagnostics while retaining the authoritative committed state. The desktop separately retains its last good rendered snapshot, visibly marked out of date until the current buffer is valid and rendered.

Canonical writing is defined by the protocol, not by hash-map iteration order. Preserve meaningful local ordering, stable identities, and the distinction between unspecified values and explicit values.

Use an XML parser that exposes the events and source positions needed by the MusicXML adapter. Keep original bytes and source bindings independently; a convenient XML deserializer alone is not a preservation strategy. The adapter maintains document cursor state, divisions changes, voices/staves, chord groups, offsets, notation relationships, and source ordering before merging events by exact musical position.

For export, check the requested fidelity and target capability before producing a success result. Choose divisions that exactly express all relevant time quantities. Do not assume the same technique encodes an arbitrary rational microtonal alteration: MusicXML's decimal pitch-related fields have a different representability constraint. [R3, R4]

The renderer must not be the importer/exporter of record. Its MEI input is a disposable projection with a separate coverage report.

## 5. Revisions, storage, and scoped edits

Persist immutable revision snapshots and their supporting source/resource blobs. SQLite plus immutable files is sufficient for the first local implementation; database access belongs behind an application repository interface. Store exact values losslessly and treat indices as rebuildable projections.

The score database contains musical objects, revisions, source bindings, ordinary project metadata, and application job metadata. It does not contain the deferred external descriptions of regional musical effects. Future annotations use their own physical store and reference score identities; the score application has no dependency on that store.

An edit transaction binds `score_id` and `expected_revision_id`, resolves its region and object targets, validates preconditions, applies typed operations to a candidate snapshot, and atomically commits a new revision plus a change report. A stale expected revision returns a conflict; it is not silently rebased.

In v1.1, `replace_range` supports only `duration_policy=preserve`. The replacement fragment must match the frozen target span and cover each bound voice under the fragment contract. Ripple replacement is deferred and is rejected by the current contract. Report changed objects outside the requested region, including affected relationship endpoints, through the operation's declared boundary semantics; never shift later material to accommodate a different fragment length.

The change report tracks inserted/deleted/modified IDs, old-to-new identities for split/merge operations, relationship endpoints, source preservation validity, and affected render/index regions. The planned `score_preview_edit` is a pure validation/diff operation: it returns diagnostics and a proposed change report, without creating a branch or returning a render result. Automatic desktop rendering of a valid uncommitted draft uses a separate private application interface. Preview is never an added permission requirement for an already authorized edit.

An editor session stores `editor_session_id`, its text buffer's `base_revision_id`, dirty state, a monotonically increasing `draft_seq`, and submitted content hashes. Incoming MCP commits update the workspace head and notify the UI but cannot replace dirty text or silently rebase its preview. Applying the buffer is a compare-and-commit operation; a stale base requires reconciliation and revalidation rather than an overwrite. Initial notation mutations target committed models, so dirty text must first be applied/reconciled or explicitly discarded, even when its draft preview is valid. Draft inspection and source navigation remain available. Committed undo/redo is an application transaction shared across human and MCP history, not a renderer-local undo stack.

Keep one authoritative Rust service per workspace. The desktop holds its exclusive writer lock and exposes local IPC for the stdio MCP bridge. A headless owner may acquire the lock only when no other owner exists. A bridge forwards requests; it does not silently create a second core or a second writer. Format discovery is stateless and does not require taking this lock.

## 6. Preview runtime and worker ownership

The first backend is a pinned Verovio WebAssembly worker, fed by the Rust-generated MEI projection and mapping. Rendering uses full layout initially. Browser output can arrive page by page, but that does not establish partial-layout capability.

The application schedules bounded jobs with distinct committed and draft input keys. A clean buffer uses its committed revision with scope, options, renderer version, projection version, and font identity. A dirty buffer automatically debounces to Rust parsing and semantic validation; when valid, it produces a private immutable draft snapshot keyed by `editor_session_id`, `draft_seq`, `content_hash`, and `base_revision_id`, plus score identity and the same layout/backend dependencies. This live valid-draft preview is required for the desktop milestone.

Every text change invalidates earlier draft requests, including work still parsing or rendering. A single view retains the newest pending request and publishes a result only when its complete key and input kind still match. Invalid or incomplete text keeps the last good preview visibly marked out of date and shows diagnostics; an older pending result cannot become current merely because the new draft is invalid. Each worker owns its renderer instance; do not share a mutable toolkit across unrelated jobs without an explicit ownership/concurrency design.

If a native host becomes preferable, Verovio documents exported C wrapper symbols in `tools/c_wrapper.h` and shared-library builds. A Rust backend can wrap that interface or use a worker process while keeping the application contract unchanged. This is a possible implementation route, not an implemented Rust binding in this bundle. [R1]

The committed render result carries `render_id`, `score_id`, `revision_id`, `options_hash`, renderer identity, SVG page resource URIs, stable event/measure maps, and diagnostics. Private draft results instead carry their tagged draft key and a session-local preview handle; they must not fabricate a persisted revision or masquerade as a committed `render_id`/revision record. Draft resources and maps remain bound to that private snapshot. SVG content and mappings are published together, and UI selection resolves within the correct input snapshot. MCP reads and `score_render` continue to use committed revisions only; `score_preview_edit` remains pure validation/diff.

Do not use Verovio's experimental edit interface as the editing substrate. Its native rendering state is a disposable view. See [Desktop score editing, live preview, and future playback](04-preview-and-playback.md) for the verified renderer boundaries, licensing references, and scheduling contract.

## 7. Standard discovery and shared application APIs

Implement `score_get_format` as the primary agent entry point. Return the supported format version and the bundle-defined standard resources needed to understand and produce SFS quickly. The descriptor and examples must be retrievable without an existing score. Do not require an agent to reverse-engineer operational JSON schemas to learn the musical language.

Expose typed application operations for import, inspection, region reads, validation, dry-run edits, committed edits, export, rendering, and application job queries/cancellation. The CLI, desktop UI, and MCP adapter use the same implementations.

The planned tool set is:

`score_get_format`, `score_import`, `score_inspect`, `score_read_region`, `score_validate`, `score_preview_edit`, `score_apply_edit`, `score_export`, `score_render`, `job_get`, and `job_cancel`.

Only advertise tools once their handlers actually exist. Tool inputs use `score_id` with `revision_id` for reads or `expected_revision_id` for edits. Region handles are revision-bound and resolve to musical anchors, not screen coordinates. `score_preview_edit` is an optional validation/diff dry run with no branch or render output.

The normal local connection is a stdio MCP process bridging to the active desktop authority through local IPC. An explicit headless mode can own a workspace under the same single-writer rule. Streamable HTTP is optional, initially loopback-oriented, and must reach the same application service rather than a separate copy of its state.

Application jobs and MCP extension tasks have different IDs and lifecycle contracts. An adapter may map between them explicitly; it must not use the terms interchangeably or claim protocol features that have not been implemented. The desktop WebAssembly renderer is available only while its host/backend is available; a headless `score_render` requires a configured headless backend and an honest capability declaration.

## 8. Milestones and exit conditions

| Milestone | Concrete work | Exit condition |
| --- | --- | --- |
| A. Standard and conformance | Versioned EBNF, language descriptor, AST schema, agent reference, complete examples, `score_get_format`, parser fixtures | An agent can retrieve the complete language contract; raw-text grammar, AST shape, and semantic validation have distinct tests |
| B. Core and codec | Typed identities, exact numbers, declared state/relationship semantics, frame parser, canonical writer, region reads | Core fixtures parse/write/parse without semantic changes; unsealed frames remain unpublished |
| C. MusicXML vertical slice | Fixed-version coverage manifest, source preservation, exact import/export for a declared useful subset, CLI diagnostics | Known source facts survive first import; supported cases round-trip with no fidelity violations; unsupported cases are explicit |
| D. Desktop editor and live preview | Tauri Rust authority, Monaco source pane, SVG notation/property/keyboard editing, automatic debounced valid-draft preview, worker renderer, typed committed/draft maps, common history | Valid text changes preview without committing; invalid text keeps a visibly stale last good view; obsolete draft results are discarded; notation edits cannot mix with dirty text; dirty text survives concurrent commits; a viewer alone does not pass |
| E. Shared MCP and partial editing | Local stdio bridge, single-writer ownership, implemented auxiliary handlers, revision preconditions, typed patches, optional dry runs and change reports | Desktop and MCP see the same head/history; stale edits conflict; authorized scoped edits are applied without unintended changes |
| F. Coverage completion | Work through the fixed MusicXML matrix, complex notation fixtures, independent source checks, declared renderer coverage | Structure and declared musical semantics meet their separate completion criteria; raw preservation does not count as semantic interpretation |
| Later: performance | Traversal occurrences, interpretation/tempo mapping, transport, audio scheduling and score following | The audio clock is independent of tool/network/UI timing; playback is reproducible for a pinned plan |
| Later: independent annotations | Separate physical database, region anchoring, provenance, search and optional overlays | The score application still works with that store absent; descriptions never overwrite score facts |

Preview development should begin with the supported vertical slice. It must not wait for every MusicXML feature, and its coverage must not be mistaken for the conversion adapter's coverage.

## 9. Verification that protects the architecture

Use a compact corpus of high-information fixtures with explicit expected facts. Include two parts sharing an onset, independent voices on one staff, a voice crossing staves, unequal-duration chord members, tuplets, accidentals, ties, offset directions, and a range crossing a relationship boundary. Expand it when a concrete unsupported feature is implemented.

Check first-import correctness against those facts as well as import/export/re-import equality. The latter alone can pass if both passes consistently omit the same feature.

Rendering checks resolve visible objects through the correct committed or draft map, preserve identities through a width change, prevent stale-result publication, and keep unsupported score objects in the authoritative snapshot. Check draft session/sequence/hash/base isolation and that old draft results are discarded even when current text is invalid. Same-engine visual comparisons are useful diagnostics; they are not the definition of interchange fidelity.

Editor/integration checks include automatic debounced preview of valid dirty text without a commit, invalid drafts retaining a visibly out-of-date last good view, committed-only MCP reads, a dirty buffer surviving a concurrent MCP commit, source/notation selection mapping, shared revision history, expected-revision rejection, and enforcement of one workspace writer. A graphical mutation must not silently consume or overwrite a dirty text draft. Test `score_get_format` examples with the actual parser; validating only the AST JSON Schema is insufficient.

Measure core parsing/conversion separately from projection, layout, SVG production, and UI publication. Use measured score sizes and hardware before choosing performance budgets. The design does not claim a particular frame latency, memory bound, compression ratio, or complete incremental engraving capability.

For later playback, add traversal and timing fixtures rather than testing audio behavior through MCP call timing. The audio callback consumes prepared events/resources and does no XML parsing, engraving, database access, or model invocation.

## 10. Dependency and packaging decisions

Pin exact dependency versions when creating the implementation and preserve the lockfile. Record the renderer release/build, supported input profile, font set, notices, source/build references, and adapter version alongside reproducible preview fixtures. Do not depend on a mutable `latest` renderer URL in production.

Choose exact-number and serialization dependencies without making their data structures part of the public musical protocol. Keep third-party XML and renderer error types inside adapters and expose stable domain diagnostics to callers.

The browser worker and any future native C bridge are replaceable integrations. LGPL distribution requirements still apply to the Verovio dependency; choosing a worker process or WebAssembly module does not by itself settle those obligations. The preview document records the upstream license and an alternative renderer for comparison.

## Official references

* R1 — [Verovio native C/C++ library interface and wrapper header](https://book.verovio.org/installing-or-building-from-sources/library.html)
* R2 — [Verovio JavaScript/WebAssembly distribution](https://book.verovio.org/installing-or-building-from-sources/javascript-and-webassembly.html)
* R3 — [MusicXML divisions](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/divisions/)
* R4 — [MusicXML semitones data type](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/semitones/)
* R5 — [Tauri 2 process model](https://v2.tauri.app/concept/process-model/)
* R6 — [Tauri commands and channels](https://v2.tauri.app/develop/calling-rust/)
* R7 — [Monaco Editor official repository and MIT license](https://github.com/microsoft/monaco-editor)
* R8 — [num-rational BigRational API](https://docs.rs/num-rational/latest/num_rational/type.BigRational.html)

All crate boundaries, application policies, and milestone exit conditions above are this project's proposals. They are not upstream library guarantees.
