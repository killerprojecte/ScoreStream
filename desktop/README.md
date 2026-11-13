# ScoreStream desktop editor

A native **Tauri 2 + Rust** application with a TypeScript/Monaco view layer. The UI has no ScoreStream parser. Every live draft is validated by `scorestream-app`; only typed, validated snapshots enter the disposable MEI projection and pinned Verovio worker.

## What works in this slice

- Native SFS file picker, valid-source Save As, syntax-highlighted source editing and Rust diagnostics.
- Automatic valid-draft preview, explicit Apply, expected-revision conflicts, session undo/redo.
- Last good SVG visibly retained for invalid or unsupported drafts; obsolete jobs cannot overwrite newer input.
- Stable event selection, Rust-provided source line navigation, committed pitch/duration inspector edits.
- Bounded renderer queue: one running job and one replaceable pending job, off the UI thread.
- Compact split/score views, optional page title, collapsible inspector, 60–200% zoom and width-aware engraving. These presentation controls never mutate score data. See [UI refinement](../docs/UI_REFINEMENT.md).

The initial projection supports aligned measured staves, ordinary pitches, rests/spacers, equal-duration chords, basic clefs/key/meter, and ordinary/dotted durations. It rejects unsupported rhythmic/layout cases and reports omitted markings. It is **not MusicXML interchange** and not a full milestone D implementation. MEI projection currently lives in an isolated TypeScript adapter over Rust-typed DTOs; moving it into a Rust rendering crate is planned. No renderer output becomes authoritative score data.

Session history is in memory. Save As persists the SFS source, not revision history. No audio/playback, shared desktop/MCP writer, packaged installers, or complete notation editing is claimed. Native Windows/macOS verification remains pending unless recorded in a later test report.

For the latest actual build/runtime results and blockers, see [desktop validation](../docs/DESKTOP_VALIDATION.md).

## Requirements

- Rust stable, Node.js 22+ and npm.
- Linux (Debian/Ubuntu): `build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.
- macOS: Xcode command-line tools (`xcode-select --install`).
- Windows: Microsoft C++ Build Tools with the Desktop C++ workload, Rust MSVC toolchain, and WebView2 Runtime.

```sh
cd desktop
npm ci
npm test -- --maxWorkers=1
npm run build
npm run desktop:dev
```

`npm run desktop:build` invokes the platform's Tauri packager. Native installers must be built and tested on their respective OS. Code signing and distribution credentials are not configured.

To build just the native executable after the frontend bundle:

```sh
cargo build --manifest-path crates/scorestream-desktop/Cargo.toml
```

This desktop crate is deliberately a separate Cargo workspace so headless/core builds do not require a WebKit installation. Both workspaces use the same application/core path dependencies and have their own committed lockfile.

## Tests and native smoke

`npm test -- --maxWorkers=1` tests exact projection, refusal of unsupported content, stable mapping, and stale-job rejection. `npm run build` typechecks and bundles locally pinned dependencies with no CDN runtime dependency.

An opt-in `smoke-test` Cargo feature adds a test-only report command. Set `SCORESTREAM_SMOKE_REPORT` to an absolute writable JSON path and run the native executable built with `--features smoke-test`. The actual WebKit window exercises validation, live drafts, stale previews, apply, undo/redo, conflicts, selection, and typed property edits; the JSON report records observed assertions. This is not a browser-only mock. Normal builds do not register the report command or inject the smoke flag. Native file-picker cancellation/overwrite and OS packaging still need manual testing.

## Architecture

- `src/bridge.ts`: Tauri command boundary.
- `src/editor/source.ts`: Monaco presentation and Rust diagnostics.
- `src/render/project.ts`: bounded typed snapshot → MEI + identity map, no source parsing.
- `src/render/exact.ts`: rational comparison only at the projection boundary.
- `src/render/worker.ts`: exclusive Verovio instance and full layout.
- `src/render/scheduler.ts`: bounded latest-only publication.
- `src/render/sanitize.ts`: inert SVG-only publication.
- `src/main.ts`: editor-session state and UI orchestration.
- `../crates/scorestream-desktop`: thin native command/dialog/lifecycle adapters over the shared Rust authority.

Each pending preview is invalidated on **every** keystroke, even if the new source is invalid. Draft keys distinguish session, sequence, SHA-256 content hash, base revision and renderer/layout identity. Graphical mutations are disabled for dirty text. Save writes source only after Rust validation; no unrestricted filesystem API is exposed to the WebView. File opens are bounded to 4 MiB.

The contextual inspector and explicit metronome/dynamics candidate are described in [Contextual refinement](../docs/UI_CONTEXTUAL_REFINEMENT.md). Its frontend/native build checks pass; fresh native visual acceptance is still pending.
