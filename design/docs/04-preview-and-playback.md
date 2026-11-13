# Desktop score editing, live preview, and future playback

Status: design proposal for the Rust implementation. The desktop editor, live preview, and a shared MCP service belong to the planned product. Playback and live performance are later work. No editor, renderer integration, benchmark, or runtime verification has been performed for this design bundle.

## 1. Rendering decision

Use Tauri 2 for the desktop client, with a Rust application core and a proposed TypeScript WebView UI. The UI contains a Monaco ScoreStream text editor and an interactive SVG notation surface. Verovio is the first replaceable engraving backend, with a pinned release and an explicit rendering capability profile. Run its WebAssembly build in a dedicated worker and return SVG pages to the UI. A native worker can implement the same rendering contract later.

Rust owns musical semantics, I/O, persistence, editing, and MCP integration. TypeScript is a proposed UI integration choice, not a second musical implementation. There is no Python production component. Tauri documents a Rust core that owns global state and coordinates WebViews, while its command and channel mechanisms connect frontend interaction to Rust. Monaco is a browser-based editor distributed under the MIT license. [V12, V13, V14]

The renderer receives a disposable projection of either a committed score revision or a validated private desktop draft snapshot. A draft snapshot is not a persisted score revision. The renderer does not own the score, editing history, source preservation records, or interchange export. Generate a renderer-specific MEI projection with explicit stable IDs rather than depending on undocumented MusicXML-to-SVG identity preservation. MusicXML import/export remains an independent ScoreStream adapter.

This is an integration proposal. The worker packaging, pinned renderer build, projection coverage, and browser support must be checked during implementation.

## 2. Verified upstream capabilities and boundaries

| Upstream capability | Consequence for this design |
| --- | --- |
| Verovio provides a WebAssembly/JavaScript distribution and a native C interface. | Browser and native hosts can use one application-level rendering contract. [V1, V2] |
| MEI is its native input. Unsupported MEI elements and their descendants can be discarded when loading and re-exporting. | Never reconstruct authoritative data from the renderer's internal document or `getMEI()` output. [V3] |
| An MEI note's `xml:id` corresponds to its SVG group's `id`. | Supply our own IDs and maintain an explicit mapping to score objects. [V4] |
| `renderToSVG()` renders a page; `getPageWithElement()` locates an element in the current layout. | Pages and navigation are derived from a specific rendering result. [V5] |
| `redoLayout()` recalculates layout. `redoPagePitchPosLayout()` recalculates only note vertical positions. | Neither method establishes general incremental editing support. [V5] |
| The documented `edit()` API is experimental and is explicitly not recommended as a dependable interface. | All edits go through the Rust revision model. [V5] |
| `select()` supports a single measure range. | Excerpt rendering is available; it is not a guarantee of incremental full-score layout. [V6] |

Verovio is distributed under LGPL v3. Keep the exact upstream license, release identity, source/build references, and modifications with the distributed dependency. Engine and font resources need their own recorded license information. A worker boundary is an engineering choice, not an assertion that distribution obligations disappear. [V7]

OpenSheetMusicDisplay is a credible alternative when direct MusicXML rendering and a BSD-3-Clause dependency are preferred. Its official documentation describes a renderer with incomplete MusicXML coverage, not a complete score editor. Its progressive `renderNext()` draws successive batches of a loaded score; that capability must not be described as arbitrary edit-aware incremental layout. Evaluate it against the same fixtures and ID mapping contract before changing the default backend. [V8, V9]

## 3. Authoritative data and disposable views

The application derives a rendering projection from an identified immutable input snapshot and submits it to the renderer. That input is either a committed revision or the latest parse- and semantic-valid text draft in a desktop editor session. The result consists of SVG resources, layout metadata, object mappings, and diagnostics. The two input kinds remain distinct throughout scheduling, mapping, and publication; viewing a draft does not commit it.

The following objects remain separate:

| Object | Authority and lifetime |
| --- | --- |
| Score revision | Persistent musical and notational truth, owned by Rust |
| Desktop draft snapshot | Private, immutable, validated buffer state identified by its editor session, draft sequence/content hash, and base revision; not a persisted revision |
| Rendering projection | Rebuildable MEI input and mapping for one explicitly typed input snapshot and scope |
| Render result | Rebuildable SVG pages and layout mappings for one input snapshot and renderer/options combination |
| UI selection | References bound either to a committed revision or to a private draft snapshot; these are different selection types |
| Playback plan | Later, a derived interpretation with occurrence identities and physical timing |

A missing glyph is not evidence that the corresponding score object is absent. Diagnostics distinguish unsupported notation, hidden objects, objects folded into other graphical groups, and actual projection failures.

### 3.1 A human editor, not only a viewer

The desktop exposes two coordinated editing surfaces:

* The SFS text pane supports typing, syntax highlighting, diagnostics, region navigation, and source-to-score selection. Highlighting can use the language descriptor, but Rust parsing and semantic validation remain authoritative.
* The notation surface supports note/measure/range selection, a property inspector, keyboard insertion/deletion and supported pitch/rhythm/notation edits. Every change becomes the same typed command used by the application service and MCP. Direct SVG changes are limited to presentation such as highlighting; moving a glyph does not edit a note.

Maintain source-span-to-object mappings in addition to SVG mappings. Text replacement must explicitly reconcile identities: preserve unambiguously retained objects, create IDs for new objects, and report deletions/splits/merges. Do not decide identity solely from pitch and onset when several voices can contain the same note.

The UI provides a shared committed revision history for text, notation, and MCP edits. Uncommitted keystroke undo can remain local to the text buffer; undoing a committed score edit is a new validated transaction against the current revision. It cannot silently erase intervening changes from another client.

### 3.2 Dirty text and concurrent MCP edits

A document session records `editor_session_id`, the current committed revision, the text buffer's `base_revision_id`, its dirty state, a monotonically increasing `draft_seq`, and the content hash of each submitted buffer. Every text change invalidates earlier draft-preview requests. A clean buffer previews its committed revision. A dirty buffer that passes Rust parsing and semantic validation automatically schedules a debounced preview of a private immutable draft snapshot. This live valid-draft preview is part of the desktop milestone, not a later optional feature; it creates no committed revision.

If a buffer is invalid or incomplete, retain the last successfully published preview, which may depict an older draft or a committed revision. Mark it visibly out of date, identify what it depicts, and show current diagnostics. A previously queued result must not publish after the buffer has advanced, even if the newer buffer is invalid.

If an MCP edit commits while a buffer is clean, the UI can refresh the text and score through the normal revision subscription. If the buffer is dirty, preserve it exactly, mark its base as stale, and display the new committed revision separately. Its draft preview remains explicitly tied to its existing base; do not silently rebase the draft. Never overwrite a dirty buffer with canonical text from the remote edit. MCP reads continue to expose committed score revisions only.

Committing the draft uses `expected_revision_id`. A mismatch requires explicit reconciliation and revalidation, with conflict diagnostics. Initial graphical editing operates on committed models: while text is dirty, require the user to apply or reconcile a valid draft before a notation mutation, or explicitly discard it. Draft notation can still support inspection and source navigation, but its selection cannot be passed directly into a committed edit. This rule applies to valid and invalid dirty text alike, preventing graphical edits from silently mixing with an uncommitted draft. These are data-conflict rules, not an extra approval requirement for MCP actions.

### 3.3 One workspace authority

The desktop's Rust service owns an open workspace. A stdio MCP bridge connects to that service over local IPC; it does not open an independent editable copy of the score. A headless service may own the workspace only after acquiring its exclusive writer lock. If another owner exists, connect to it or return a clear ownership result.

All entry points use the same application operations, revisions, jobs, and event notifications. A later Streamable HTTP transport can begin on loopback; it is not required for the local desktop/stdio arrangement. Stateless `score_get_format` discovery can serve the versioned standard without claiming score-workspace ownership.

## 4. Committed render contract and private draft identity

The committed `score_render` wire schema is defined by the shared bundle contract. These semantics are required:

| Field or record | Meaning |
| --- | --- |
| `render_id` | Identity of this render request/result; not a score revision |
| `score_id`, `revision_id` | Exact authoritative input identity |
| Scope | Full score or an explicit region expressed with stable musical anchors |
| `options_hash` | Identity of layout settings and selected parts; width changes produce a new value |
| Renderer version | Pinned engine/build identity, included in cache compatibility |
| SVG page resources | Resource URIs and page metadata belonging to this result |
| Object map | Stable event/measure IDs to renderer IDs, SVG IDs, and page references |
| Diagnostics | Unsupported or approximated projection features and other rendering issues |

Font resources and projection-version changes also affect cache compatibility. Do not reuse a render merely because its score revision matches.

Ordinary application job IDs remain distinct from any future MCP protocol task IDs. A render can be delivered synchronously or through an application job without changing its revision semantics.

The private desktop draft interface is a separate internal design, not an alternative payload for this committed render record. Use an explicitly tagged draft key containing `editor_session_id`, `draft_seq`, `content_hash`, and `base_revision_id`, together with score identity, scope, layout options, renderer/build, projection version, and font identity. Use a local draft-preview handle for its result. Do not fabricate a `revision_id` for the draft or store it as a committed `render_id`/revision record; `base_revision_id` identifies only the revision from which the buffer was edited.

Draft SVG resources and object maps belong to that exact private snapshot and editor session. New draft objects may have session-local identities; retained committed IDs do not make the draft mapping valid for committed edits. Publication, hit-testing, and source navigation must check the full draft key. This interface neither exposes private draft snapshots through MCP nor changes `score_preview_edit`.

## 5. Preview scheduling

Live preview follows the active editor buffer. A clean buffer displays the committed revision; a dirty, valid buffer automatically renders an ephemeral draft snapshot after debouncing. Invalid or incomplete text keeps the last good preview visibly marked out of date. The UI labels draft content and its base revision, and cannot present it as a committed score. Automatic draft rendering uses the private desktop interface described above, not the MCP `score_preview_edit` contract.

The initial scheduling policy is:

1. Resolve the active input kind. For a clean buffer, key the committed snapshot by revision, scope, options, renderer/build, projection version, and font set. For dirty text, advance the draft sequence, debounce parsing and semantic validation, and use the distinct draft key only after validation succeeds.
2. Coalesce queued work for the same view so only its newest pending request is retained. Keep bounded worker capacity. Advancing the buffer invalidates every older draft request, including requests still parsing or rendering.
3. Generate a complete, self-contained rendering projection. Do not send a half-written XML document or an unsealed event frame to the renderer.
4. Run layout away from the UI thread and the ordinary request path. Render the visible page first when the backend permits page-level output, then obtain other requested pages.
5. Publish SVG and its mappings together only if the request still matches the active input kind and full view key. Discard obsolete draft results; committed results may populate a compatible cache but must not overwrite a newer view. A newer invalid draft does not make an older pending draft result current again.
6. Restore focus and selection using stable musical anchors. Pixel coordinates and page numbers are layout outputs and cannot serve as persistent identities.

Cancellation removes queued work where possible. Do not assume that an executing synchronous WebAssembly call is interruptible; stale-result suppression is still required. Retaining a warm worker is an optimization to measure, not a reason to publish stale output.

During streaming generation, a sealed frame makes musical data available; it does not require immediate engraving of every arriving frame. Batch preview requests adaptively and prioritize the visible region. Open musical relationships and incomplete regions must remain marked as such. Any temporary presentation aid belongs only to the rendering projection and cannot silently become a new score event.

## 6. Full layout first; partial rendering as a separate capability

Initial correctness uses full layout of the supplied document or self-contained excerpt. Updating only the displayed page is a UI optimization and does not imply that the backend avoided whole-document layout.

| Change | Initial handling |
| --- | --- |
| Selection highlight or playback highlight | Update an overlay or SVG styling; do not mutate the score |
| Score edit, including pitch, rhythm, lyrics, or notation | Produce a new projection and perform a full layout of the requested scope |
| Page width, font, staff visibility, or layout options | Re-layout with a new options identity |
| Request for a musical excerpt | Resolve the region in the core, include entry context, and render it as a distinct scoped result |
| Independent analytical description changes | No score re-layout by default; refresh only the optional annotation UI |

A local musical edit can change spacing, line breaks, collision avoidance, and later page positions. The application must not claim that only the edited measure needs re-layout. Future incremental work requires a tested dependency model and a full-layout fallback. The renderer's vertical pitch-position shortcut is not a safe general implementation of this feature.

An excerpt carries its entry clef, key, meter, instrumentation, transposition context, and boundary relationships. Cross-boundary ties and slurs remain linked to their real endpoints; the projection may indicate clipping but cannot invent editable replacement notes. An excerpt's pagination is its own layout and is not guaranteed to match the same region inside the full score.

## 7. Stable selection and region mapping

For committed content, use a maintained correspondence from `(score_id, revision_id, object_id)` to renderer `xml:id`, then to one or more SVG elements. For draft content, bind the correspondence to the complete private draft key and draft object identity instead; never substitute the base revision as though it described the draft. Encode renderer IDs in an XML-compatible form rather than copying arbitrary IDs into XML without validation.

The correspondence is not universally one-to-one: a score object can have no visible representation, several visual pieces, or repeated rendered instances. Conversely, graphical layout objects may have no editable score counterpart. Preserve those distinctions in the mapping.

A click resolves through the SVG map into an object in the displayed snapshot. A committed region selection resolves to musical boundaries and part/staff/voice filters in the core. Future MCP calls receive those committed stable references, not DOM selectors, screen rectangles, or private draft selections. If the input snapshot changes, either reconcile the selection through an explicit object mapping or require a fresh resolution; do not silently reinterpret stale measure indices.

For overlapping noteheads, chords, or other ambiguous hit targets, return the candidate score objects and their context. Coordinate guesses must not become an authoritative edit target.

`score_render` will expose the declared committed revision-bound rendering contract when implemented. The planned `score_preview_edit` only validates a proposed edit and returns its diff and diagnostics; it neither creates a branch nor returns a render result. Automatic desktop rendering of valid uncommitted text uses the separate private draft interface and is part of the desktop milestone. `score_preview_edit` is not a mandatory approval step before an already authorized `score_apply_edit`.

A desktop-attached WebAssembly worker can service application rendering jobs while that backend is available. A headless host needs a configured native or equivalent renderer backend before advertising headless rendering. A protocol handler alone must not claim that a rendering runtime exists.

## 8. Later playback and performance support

Playback is outside the preview MVP. Preserve its architectural boundary now:

* A playback plan binds a score revision, an interpretation/tempo policy, and a traversal of repeats and jumps.
* Written event IDs remain stable. Each sounding occurrence has a distinct occurrence identity; one written note can appear several times during playback.
* The performance layer computes physical onset/release and controller events. It does not replace exact score positions with milliseconds.
* The audio transport and scheduling clock operate independently of MCP requests, rendering jobs, and UI refresh. MCP starts, stops, or queries a transport; it is not the audio clock.
* Visual highlighting follows the actual transport position and its occurrence map. Re-layout changes visual geometry, not transport timing.
* Editing while playing requires an explicit policy: continue the pinned playback revision or restart/switch at a declared boundary. Do not splice a new revision into an already scheduled plan implicitly.

Verovio can generate basic MIDI and timing maps, but playback requires a separate player. Its timing-map documentation records restrictions on tempo handling; validate a pinned build before relying on its timing for any particular case. Treat renderer-generated timing as a supported-subset preview aid, not as the authoritative future performance model. [V10, V11]

Repeat expansion, ties, ornaments, grace timing, pedal behavior, microtonal playback, and multiple clocks require explicit interpretation policies. Rendered note geometry cannot determine these policies. The eventual audio callback must receive prepared events and resources rather than perform parsing, layout, database work, or model/MCP calls.

## 9. Independent region descriptions

Structured descriptions of the musical effect of a measure range or region are a separate future annotation subsystem. Its records reference score/revision/region identities and carry their own provenance and lifecycle. They are not inserted into the ScoreStream event stream or score database as musical truth.

The score application, renderer, and editing path must work without that subsystem. A later annotation overlay can consume its records, but this milestone creates neither the annotation database nor its handlers. Source notation such as written dynamics and lyrics still belongs to the score; external analytical descriptions do not.

## 10. Preview acceptance conditions

Before claiming the MVP works, verify that a committed edit causes a new revision-bound preview and that dirty valid text automatically produces a debounced draft preview without committing a revision. Verify draft-key isolation, discarded obsolete results, invalid text retaining a visibly out-of-date last good view, and MCP reads continuing to see only committed content. SVG selection must resolve within the correct snapshot; selection must survive re-layout by identity; independent edits must not depend on renderer internals; and unsupported notation must appear in diagnostics without disappearing from authoritative data.

Also verify source/notation selection in both directions, basic property and keyboard edits, common committed history, dirty-buffer preservation during an MCP edit, expected-revision conflicts, and the absence of two simultaneous workspace writers. A notation mutation must require applying/reconciling or explicitly discarding dirty text instead of silently combining the two states. A working viewer alone does not satisfy the desktop-editor milestone.

Measure projection, loading/layout, SVG generation, mapping, and UI publication separately on identified score sizes and hardware. Record responsiveness before selecting latency targets. No latency or incremental-layout guarantee is established by this document.

## Official references

* V1 — [Verovio JavaScript and WebAssembly](https://book.verovio.org/installing-or-building-from-sources/javascript-and-webassembly.html)
* V2 — [Verovio C and C++ library interface](https://book.verovio.org/installing-or-building-from-sources/library.html)
* V3 — [Verovio input formats and preservation limitations](https://book.verovio.org/toolkit-reference/input-formats.html)
* V4 — [Verovio SVG structure and element identity](https://book.verovio.org/advanced-topics/internal-structure.html)
* V5 — [Verovio toolkit methods](https://book.verovio.org/toolkit-reference/toolkit-methods.html)
* V6 — [Verovio content selection](https://book.verovio.org/interactive-notation/content-selection.html)
* V7 — [Verovio LGPL license](https://github.com/rism-digital/verovio/blob/develop/COPYING.LESSER)
* V8 — [OpenSheetMusicDisplay official repository, license, features, and limitations](https://github.com/opensheetmusicdisplay/opensheetmusicdisplay)
* V9 — [OpenSheetMusicDisplay API, including progressive rendering](https://opensheetmusicdisplay.github.io/classdoc/classes/OpenSheetMusicDisplay.html)
* V10 — [Verovio MIDI playback tutorial](https://book.verovio.org/interactive-notation/playing-midi.html)
* V11 — [Verovio output formats and timing map](https://book.verovio.org/toolkit-reference/output-formats.html)
* V12 — [Tauri 2 process model](https://v2.tauri.app/concept/process-model/)
* V13 — [Tauri commands and channels](https://v2.tauri.app/develop/calling-rust/)
* V14 — [Monaco Editor official repository and MIT license](https://github.com/microsoft/monaco-editor)
