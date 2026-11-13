# Format Discovery and MCP Contract

**Status:** design only. This package does not contain a running MCP server. The tool and resource manifests describe a planned surface; they are not evidence that any operation is implemented.

**Protocol baseline:** MCP `2026-07-28`, with a separate adapter for `2025-11-25`. **Implementation language:** Rust. **Review date:** 2026-11-13.

## 1. Responsibilities and boundaries

The principal agent entry point is `score_get_format`: one call teaches an agent the exact ScoreStream syntax, semantics, supported profile, examples, and versioned normative references. The primary machine contract is the **score format standard**, not merely a collection of tool argument schemas.

The MCP adapter exposes the same score application services used by the human desktop editor and command-line interface. It must not contain a second parser, music model, edit engine, or renderer. Every score operation names its score and revision explicitly; connection-local state is not a score workspace.

The intended first server can import, inspect, read, validate, edit, export, and render supported scores. It also exposes application job status and cancellation when asynchronous work is implemented. Annotation storage, annotation retrieval, generation models, performance rendering, audio synthesis, and playback control remain separate future modules. Their tools must not appear in the planned MVP catalog or in a deployed server until their implementations exist.

Musical markings that belong to the score, such as a source crescendo or printed text direction, remain score data. Analytical prose about the effect of a passage belongs to the separate annotation design. The MCP score API does not create an annotation knowledge base as a side effect of editing a score.

### 1.1 A format standard that an agent can discover

`score_get_format` returns a concise, directly usable description first: selected format version/profile, lexical rules, frame and lane syntax, pitch/time conventions, essential record types, scope/inheritance rules, complete small examples, and common mistakes. Topic selection can request more detail on, for example, polyphony, relations, region editing, or preserved extensions. Its default response must be sufficient to write a valid small score without forcing the agent through a long resource-fetch sequence.

The response also points to five complementary, versioned resources:

| Format artifact | What it defines |
|---|---|
| `scorestream.ebnf` | The grammar of the actual plain-text score stream |
| `scorestream-language.json` | Machine-readable vocabulary, syntax templates, rules, defaults, scopes, and cross-references |
| `scorestream-ast.schema.json` | JSON Schema for the parsed abstract syntax tree, not for the raw SFS text |
| `agent-reference.md` | A compact authoring reference with examples and semantic constraints |
| Valid `.sfs` examples | Complete, profile-labeled inputs covering common and advanced constructs |

A JSON Schema containing only `type: string` does not define the score language. Tool input schemas likewise cannot substitute for its grammar and musical semantics. All five artifacts and the short discovery response must derive from the same versioned language specification and be checked for consistency.

### 1.2 One desktop workspace authority

The desktop application embeds the Rust workspace authority. Its human editor, notation preview, and MCP-facing command executor use the same application services, revision store, validators, and change stream. The desktop shell may use Tauri 2 and a thin TypeScript view layer; domain logic remains Rust.

A local stdio MCP process acts as a bridge to that authority over local IPC. It does not open an independent writable score store. A headless authority may own the workspace only when no desktop authority owns it. An exclusive workspace ownership mechanism and authority discovery prevent simultaneous desktop/headless writers. Optional loopback Streamable HTTP exposes the same authority; it is another adapter, not another workspace copy.

Human uncommitted text is an editor draft, separate from the persisted score head. The preview may display that draft with its validation state. AI reads and commits target explicit persisted revisions unless a future draft API is deliberately added. Human and AI commits execute the same atomic commands. If either advances the head, a stale AI patch fails its compare-and-swap precondition. An external commit must not overwrite the human's unsaved draft; the UI keeps the draft and makes its base revision visible so it can be reconciled. These concurrency rules do not impose a confirmation dialog on every authorized AI edit.

## 2. Protocol and Rust SDK decisions

The published `2026-07-28` protocol uses per-request version and client metadata rather than requiring a connection handshake. A modern server implements `server/discover`; modern result envelopes include `resultType`. The official Rust SDK, `rmcp`, documents support for this revision and older compatibility. Use a tested published SDK release and commit the dependency lockfile when implementation begins; this design does not pin a moving minor release. [S1][S2][S3]

Use the SDK's protocol types and transport adapters. Rust application input/output types use `serde`; schema generation should use the SDK-compatible `schemars` version. Keep protocol compatibility in the adapter, including modern versus legacy response envelopes. The domain edit command and stored score do not depend on an MCP protocol date. [S3]

### 2.1 Transport

Use stdio for a local child-process server and Streamable HTTP at one `/mcp` endpoint for a remote server. In the modern HTTP binding, requests use POST; responses are JSON or a request-scoped SSE stream. The old standalone GET stream and protocol sessions are not the modern transport contract. [S4]

MCP is an agent control and context interface. It is not the editor's mandatory rendering transport, a WebSocket protocol, a musical clock, or a hard real-time audio bus. A browser editor may use its own HTTP/SSE/WebSocket application channel. Both adapters call the same Rust application services and observe the same revisioned change events.

The `2025-11-25` adapter retains its negotiated legacy initialization and transport behavior. Do not mix modern per-request assumptions, legacy session headers, and legacy resource subscription methods in one undocumented wire profile.

### 2.2 Modern wire envelope

The application tool name is carried by `tools/call`; it is not a new MCP method. With default format selection, an illustrative modern request is:

```json
{
  "jsonrpc": "2.0",
  "id": "format-1",
  "method": "tools/call",
  "params": {
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientInfo": {
        "name": "scorestream-agent",
        "version": "1.0.0"
      },
      "io.modelcontextprotocol/clientCapabilities": {}
    },
    "name": "score_get_format",
    "arguments": {}
  }
}
```

The request's protocol metadata belongs in `params._meta`, outside tool `arguments`. The modern response has the same JSON-RPC ID and a `result` with `resultType: "complete"`, `content`, and the schema-valid application envelope in `structuredContent`. The SDK supplies the corresponding HTTP metadata headers and transport-specific framing. The legacy adapter uses the legacy wire envelope; callers must not copy modern metadata conventions into an older session by assumption. [S2][S3][S4]

## 3. Planned tool inventory

The following eleven names are the planned MVP tool family. `score_get_format` is the central discovery operation; the remaining ten tools are auxiliary score and job operations. A deployed `tools/list` exposes only the subset actually implemented and authorized for the caller. It returns tools in deterministic order. Importing a score must not install temporary tools or change the tool list for a connection. [S5]

| Tool | Purpose | Required domain constraints |
|---|---|---|
| `score_get_format` | Teach the agent the exact format, with concise syntax, semantics, vocabulary, examples, and normative resource links | Select a supported version/profile and optional topics; always report the selected version; never claim a roadmap extension is supported |
| `score_import` | Import inline SFS/MusicXML or a registered asset; create an immutable initial revision | Explicit source format, supported profile, idempotency key; MXL uses an asset rather than an inline binary field |
| `score_inspect` | Discover score identity, head or selected revision, parts, regions, coverage and diagnostics | `score_id`; optional `revision_id`; response always identifies the exact revision inspected |
| `score_read_region` | Read a bounded region and register/reuse its immutable frozen selection | The exact shared `SelectionRequest`: nested `score` reference, selector, explicit expansion policy, and optional filters |
| `score_validate` | Validate a pinned revision under a named profile | `score_id`, `revision_id`; no claim that a schema check proves musical completeness |
| `score_preview_edit` | Validate prospective edits and describe their consequences without committing | The edit's score/base/frozen selection/operations, without an idempotency key; no artifact, job, or render is created by this pure preview operation |
| `score_apply_edit` | Atomically commit typed score edits | `score_id`, `expected_revision_id`, `idempotency_key`, typed operations |
| `score_export` | Export one immutable revision using a supported adapter and fidelity policy | `score_id`, `revision_id`, target format and fidelity policy; return diagnostics and artifact references |
| `score_render` | Produce a revision-specific score preview and event-to-graphic mapping | `score_id`, `revision_id`, region/layout options; may return an application job |
| `job_get` | Inspect a durable application job and locate its result | Application `job_id`; authorization checked on every call |
| `job_cancel` | Request cooperative cancellation of an application job | Application `job_id`; return the observed state, not a false guarantee that work stopped |

There is no general-purpose filesystem, shell, remote-URL fetch, SQL, or arbitrary code tool. Import accepts inline text or an asset already registered with the application. Remote ingestion, if added later, is a distinct product capability with its own contract.

Tool annotations describe actual effects. Format discovery, inspection, validation, and the pure edit preview are read-only. A region read registers/reuses a content-addressed frozen selection; this application effect is declared non-read-only even though score revisions remain unchanged. Creating scores, revisions, artifacts, or durable jobs is also a mutation. Render/export therefore do not claim `readOnlyHint: true`. `idempotentHint: true` is appropriate only when the implementation actually provides repeat-call deduplication or an operation is naturally idempotent. Hints are not authorization checks.

The planned catalog is `mcp/tools-list.planned.json`; `mcp/deployment-notes.json` records its non-deployed status and renderer condition. `tools/build_mcp_catalog.py` is an offline packaging utility, not a Rust product implementation. It emits the catalog and individual self-contained schemas in `schemas/mcp/` from the authoritative format, selection, edit, job, and rendering contracts. `examples/mcp/manifest.json` enumerates structural validation expectations. Its successful checks do not establish a running server or validate the music semantics of SFS text.

## 4. Auxiliary tool JSON Schema contract

All agent-facing schemas use JSON Schema 2020-12 and declare that dialect. MCP defaults to this dialect when `$schema` is absent. Every advertised tool carries a self-contained `inputSchema` and `outputSchema`; reusable definitions are bundled with local `#/$defs/...` references. Network `$ref` resolution is not required or enabled. [S6]

The separately published tool-schema resources are documentation and validation artifacts. A client must not have to fetch them to interpret a tool definition. The manifest generator should derive both the embedded schemas and those resources from the same versioned definitions. These auxiliary schemas are distinct from the format grammar, language descriptor, and AST schema returned or referenced by `score_get_format`.

Use object-shaped structured results even though modern MCP permits other JSON values. This preserves a useful common subset with the `2025-11-25` adapter. The common application envelope has mutually exclusive branches:

- Success: `ok: true` and `data` matching the specific tool's output definition.
- Failure: `ok: false` and a typed `error`; never also return a success-shaped `data` value.

Each branch includes only the fields allowed by its schema. Diagnostics are bounded and report truncation or a continuation reference when applicable. Large score streams, images, and exported files are resources/artifacts; they are not unbounded strings repeated in a tool result.

Return the application object in `structuredContent` and a serialized JSON TextContent block for compatibility. The object must conform to the tool's `outputSchema` on both success and domain failure. The modern wire envelope adds `resultType: "complete"`; it is not a field inside the application's `data` object. [S5]

Additional schema rules:

1. IDs and revisions are opaque strings. Clients compare revisions for equality and never derive ordering from their spelling.
2. Musical time uses the canonical rational representation from the core schema, not a JSON floating-point approximation. Lexical validation does not replace reduction, positivity, or cross-field checks.
3. Edit operations are a bounded, discriminated set of domain commands. Do not expose raw JSON Patch paths into an internal Rust serialization.
4. Optional fields have documented semantics. Omission, an explicit reset, and an empty collection must not accidentally mean the same thing.
5. Unknown operation kinds and unsupported semantic extensions produce actionable errors. Schema defaults are documentation unless the application explicitly applies them.
6. The core semantic validator checks event references, temporal constraints, boundary context, and profile-specific rules. JSON Schema cannot prove those properties by itself.

## 5. Region reads and partial edits

Use the common core selector and anchor definitions everywhere. A selector may refer to stable event/region identities and precise time boundaries, with part/staff/voice filters. Printed measure numbers are labels, not unique identifiers. Region intervals are half-open where time ranges are used; the response reports the normalized selection.

`score_read_region` must return enough context for a subsequent edit: relevant instrument and voice declarations; state at entry; continuing notes; crossing relations; and identifiers for affected boundaries. It also identifies omitted context and returns a continuation when a result is truncated. It must never silently cut a relation and present the fragment as an independent complete score.

The output includes the full `FrozenSelection` and one or more local-time inspection fragments. Each fragment carries an explicit revision-bound `text_map` from the SFS aliases or source lines in its fragment/context fields to opaque application IDs. It covers events, measures, relations, intrinsic marks, chord groups, parts, voices, and staff references where those objects occur. Unlabelled objects can use an exact 1-based line locator in the named snippet; event-list order is never an identity map. A truncated context or map must identify its full resource, and an agent must retrieve that information before depending on it. A frozen selection itself is never silently truncated. The output marks whether each inspection fragment is complete enough for replacement; a sparse selection with holes is not automatically a valid `replace_range` payload.

### 5.1 Commit preconditions

An edit transaction follows this order:

1. Authenticate the caller and authorize access to the operation's score and idempotency namespace.
2. Compute the normalized request digest and look up an existing idempotency receipt. The same key and request return the recorded outcome **before** stale-head or expired-selection checks; a different request under the same key is rejected.
3. For a new invocation, authorize supplied assets, resolve its frozen selection against `expected_revision_id`, and validate its operations.
4. Compute affected events, boundaries, relations, source-preservation attachments, and downstream layout invalidation, then validate the prospective score.
5. At commit, compare the current head with `expected_revision_id` again and atomically publish exactly one new immutable revision.
6. Record the request digest and idempotency result in the same transactional boundary, then publish change notifications.

The MVP uses conservative whole-score compare-and-swap. It does not silently merge concurrent edits. A future per-region conflict mechanism can be added without changing the meaning of existing revision preconditions.

The same idempotency key and same normalized request return the original outcome during the advertised retention period. Reusing a key with a different request is an error. After a lost response, callers retry with the same key or inspect the score; they must not assume the edit failed.

### 5.2 Crossing boundaries

Replacing a region requires an explicit duration policy and explicit treatment of affected relationships. A duration change can shift later music, and deleting a note can invalidate a tie, lyric attachment, tuplet group, or layout anchor outside the selected region. Do not silently drop such relationships or fill all empty voices with rests.

A preview reports these impacts and any required related operations. The commit rejects unresolved dependencies. Successful edits return the new revision, affected IDs/ranges, relevant ID mappings, invalidated render regions, and bounded diagnostics.

Preview is a useful review facility, not a mandatory human approval workflow. `score_apply_edit` can validate and commit an already authorized operation directly. The current shared `EditRequest` does not accept a preview receipt or approval token. A future optional preview-reuse facility would need to bind the exact base revision and operation digest; it must not be mistaken for authorization.

## 6. Error behavior

Malformed MCP requests, unknown methods/tools, and protocol failures use JSON-RPC errors. Domain failures such as an invalid region, unsupported notation, failed score validation, or a revision conflict return a tool result with `isError: true` and the typed application error branch. This gives the agent actionable feedback without pretending the call succeeded. [S5]

The error object should contain a stable code, a concise message, a recoverability indicator, relevant score/revision identifiers, and bounded diagnostics. The following are recommended domain codes, not MCP-standard error numbers:

| Code | Meaning | Agent recovery |
|---|---|---|
| `revision_conflict` | The requested base is no longer head and no matching receipt exists | Read the returned current revision and re-plan the edit |
| `selection_mismatch` | A boundary or ID does not match the pinned selection/revision | Read the region/manifest and use returned IDs |
| `validation_failed` | Prospective score violates the selected profile | Correct the reported operations; no partial commit occurred |
| `crossing_relation` | An edit leaves a crossing relationship unresolved | Include the required relation or boundary operations |
| `unsupported_feature` | An adapter or profile cannot represent requested semantics | Choose a supported path or an explicitly allowed fidelity policy |
| `idempotency_conflict` | A key was reused with different arguments | Use the original request or a new key for a new operation |
| `handle_expired` | A temporary artifact/job handle exceeded retention | Recreate the operation from the immutable score revision |

Error messages must not expose unrelated scores or inaccessible content. A resource URI or opaque ID identifies an object; it is not, by itself, authorization to read it.

## 7. Resources and templates

The accompanying `mcp/resources.json` and `mcp/resource-templates.json` are example result bodies for a future server. A format index lists supported versions and profiles. Versioned templates expose the agent reference, EBNF grammar, language descriptor, AST schema, and examples. Additional resources describe editing rules and the capability matrix. Other templates expose auxiliary JSON schemas, revision manifests, SFS streams, diagnostics, render manifests/pages, export artifacts, and application job status.

Use immutable revision URIs for reproducible reads. A separate `latest` resource is a mutable head pointer. An immutable revision resource never changes to mean a newer score. A render resource identifies its source `revision_id`, renderer version, layout settings hash, and event mapping.

Dynamic resources need not all be enumerated in `resources/list`. Tools return the precise readable URIs they create or discover. The capability matrix distinguishes semantic support, preservation-only support, export limitations, and unsupported features; importing an unknown extension cannot promote it to semantic support.

If resource change notifications are implemented, advertise `resources.subscribe` and accept modern `subscriptions/listen` requests for mutable head/job resources. Modern subscriptions use an acknowledged, opt-in filter; legacy `resources/subscribe` behavior belongs only to the legacy adapter. Clients reconnect and read current state after a broken subscription; notification streams are not an authoritative edit log. [S7][S8]

## 8. Application jobs and MCP Tasks

Keep a durable **application job** abstraction independent of MCP. A job records its `job_id`, kind, source score/revision, normalized request digest, status, stage/progress, result references, error, timestamps, and retention. Application states may distinguish queued, running, cancellation requested, succeeded, failed, and cancelled. These names are application values, not invented MCP task statuses.

The portable fallback returns an application job handle in the normal success `data` branch. `job_get` obtains its current state and final result references; `job_cancel` requests cancellation. The submitted job is durably registered before its handle is returned. Every job reads an immutable input revision, and derived artifacts retain that revision identity.

The modern MCP Tasks extension is an optional adapter over this job system. When both peers advertise `io.modelcontextprotocol/tasks`, the server can return `resultType: "task"` and an MCP `taskId`; the client uses `tasks/get`, `tasks/update` when input is required, and `tasks/cancel`. The current extension includes terminal results in `tasks/get`. Do not use the older per-request task flag, per-tool `taskSupport`, or `tasks/result` as if they were the modern contract. [S9]

An application `job_id` and an MCP `taskId` have distinct namespaces and lifecycles. Do not pass one to the other's method. The adapter may retain an internal mapping. The tool's `outputSchema` describes its final application result, not the protocol's intermediate `CreateTaskResult`.

Cancellation is cooperative and race-prone. A committed score revision is never rolled back merely because a connection closed or cancellation arrived late. The job reports the actual final outcome. Synchronous HTTP response-stream closure and stdio cancellation follow their respective protocol bindings; durable jobs survive loss of the client connection that originally submitted them. [S10]

## 9. Interactive rendering

Interactive rendering is an application capability with the following contract:

- Each render targets an exact revision and declared layout/render options.
- Output is a manifest plus independently retrievable pages or systems, preferably SVG for editable previews, with event/measure IDs mapped to graphic bounds.
- The editor may render validated incremental drafts, but committed resources identify a complete immutable revision. Incomplete input is labeled as a draft, not silently repaired into a different score.
- Commit events invalidate derived render results. Reflow can affect later systems or pages; invalidation is not necessarily confined to the edited measure.
- A newer preview can supersede an older queued preview. This scheduling rule applies to derived rendering, never to already committed score edits.
- The UI discards stale results whose revision/options no longer match its requested view.

MCP can request a render, fetch its resources, or observe resource invalidation. It does not promise a frame rate or deterministic audio scheduling. Future playback uses a separate performance timeline and playback engine; no transport/play/seek/stop tools are advertised in this MVP.

## 10. Deployment and conformance gates

Before advertising a tool, implement its input validation, output-schema validation, revision behavior, error branch, and authorization. Before advertising resource subscriptions or Tasks, implement their lifecycle and cancellation behavior. Capability flags must reflect the deployed binary and configured services rather than the roadmap.

The implementation should verify: modern and legacy envelope interoperability; schema self-containment; conflict and idempotency behavior; edit atomicity; boundary diagnostics; resource revision identity; job recovery; stale-render handling; and the absence of annotation/playback tools from the MVP tool list. These are focused contract checks, not a request for a large generic test framework.

## Official references

- [S1] [MCP 2026-07-28 release announcement](https://blog.modelcontextprotocol.io/posts/2026-07-28/).
- [S2] [Versioning and compatibility](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning.md) and [server discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover).
- [S3] [Official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk).
- [S4] [Streamable HTTP binding](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http) and [transport overview](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports).
- [S5] [Tools, structured output, stateful workflows, and error handling](https://modelcontextprotocol.io/specification/2026-07-28/server/tools).
- [S6] [JSON Schema usage](https://modelcontextprotocol.io/specification/2026-07-28/basic#json-schema-usage).
- [S7] [Resources](https://modelcontextprotocol.io/specification/2026-07-28/server/resources).
- [S8] [Subscriptions](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/subscriptions).
- [S9] [MCP Tasks extension overview](https://modelcontextprotocol.io/extensions/tasks/overview) and [official extension repository](https://github.com/modelcontextprotocol/ext-tasks).
- [S10] [Cancellation](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/cancellation).
