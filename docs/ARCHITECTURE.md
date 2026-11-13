# Architecture and ownership

ScoreStream follows the supplied 1.1 design package. The implementation is intentionally separated by responsibility:

- `scorestream-core`: exact arbitrary-precision rational values, typed musical objects, stable authoring identities, semantic validation.
- `scorestream-codec`: SFS lexer/parser, source diagnostics and deterministic canonical serialization.
- `scorestream-app`: the single in-process editor authority, committed history, compare-and-commit revisions, and private draft validation.
- `scorestream-cli`: local validation, AST inspection and canonicalization through the same codec.
- `scorestream-mcp`: stateless MCP language discovery. It advertises only implemented discovery handlers and never opens an independent editable score workspace.
- `scorestream-desktop`: Tauri lifecycle and native adapters. The adjacent `desktop/` TypeScript application is a projection of Rust data, not a second parser or score authority.

## Revision and draft invariants

1. Validating dirty source does not mutate committed history.
2. Invalid source retains the last good projection but returns `valid=false` and current diagnostics. The view must display its stale status.
3. Draft sequence and frontend request sequence invalidate obsolete preview responses, including when the newest text is invalid.
4. Apply requires the current expected revision. Stale edits fail without replacing the user's buffer.
5. Undo and redo create fresh revision identities. Returning to an older musical state never makes an obsolete revision token current again.
6. Property edits mutate typed events, canonicalize and revalidate the complete score before committing. They are blocked while a dirty source buffer exists.
7. Input event IDs and written pitch spelling survive the projection. SVG IDs are rendering identities only.

## Explicit first-milestone limits

History is currently in memory. Native source-file save/open is separate from a durable workspace repository. The current application must not be advertised as crash-recoverable storage, multi-process exclusive ownership or a deployed operational MCP score-edit server.

The renderer is a replaceable, bounded projection. Unsupported notation must be reported, never silently removed from the authoritative SFS. MusicXML import/export, archive handling, full rendering coverage, durable snapshots, shared desktop/MCP IPC, bounded edit jobs, playback and analytical annotations remain separate work.

The bundled `design/` files are the original proposal and fixtures. Their historical design-package validation reports are not implementation test evidence. Current implementation results belong in `docs/VALIDATION.md`.
