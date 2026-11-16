# ScoreStream desktop editor

A native **Tauri 2 + Rust** application with a **React + shadcn/ui** view layer and Monaco source editor. The UI has no ScoreStream parser. Every live draft is validated by `scorestream-app`; only typed, validated snapshots enter the disposable MEI projection and pinned Verovio worker.

## What works in this slice

- Native SFS file picker, valid-source Save with destination picker, syntax-highlighted source editing and Rust diagnostics.
- Automatic valid-draft preview, explicit Apply, expected-revision conflicts, session undo/redo.
- Last good SVG visibly retained for invalid or unsupported drafts; obsolete jobs cannot overwrite newer input.
- Built-in synthesized Web Audio playback driven by the authoritative Rust performance plan: play/pause/stop, Space outside editing/modal controls, seconds and measure seeking, score-time cursor/following, and note/rest click-to-seek.
- **File → Export WAV…** renders the click-time source through the built-in synth to 48 kHz stereo PCM16. The native save dialog chooses a new destination; existing files are not replaced. Export has no arbitrary duration or note-count cap, is cancellable before finalization, and never applies a draft or changes SFS save/undo state. See [desktop WAV export](../docs/DESKTOP_WAV_EXPORT.md).
- Distinct synthesized piano, flute and bowed-string timbres, selected by the note's exact instrument identifier. `cello`, `violin` and `strings` share the bowed-string profile; other identifiers use a neutral triangle tone. These are not recorded instrument samples.
- Stable event selection, Rust-provided source line navigation, committed pitch/duration inspector edits.
- Bounded renderer queue: one running job and one replaceable pending job, off the UI thread.
- In-process HTTP + SSE MCP service with listen IP/port controls, an optional session-only key, start/stop/status, and a secret-free connection URL. MCP reads the same committed workspace as the editor; private drafts are excluded. See [MCP service](MCP.md).
- Compact split/score views, optional page title, collapsible inspector, 60–200% zoom and width-aware engraving. These presentation controls never mutate score data. See [UI refinement](../docs/UI_REFINEMENT.md).

The initial projection supports aligned measured staves, ordinary pitches, rests/spacers, equal-duration chords, basic clefs/key/meter, and ordinary/dotted durations. It rejects unsupported rhythmic/layout cases and reports omitted markings. It is **not MusicXML interchange** and not a full milestone D implementation. MEI projection currently lives in an isolated TypeScript adapter over Rust-typed DTOs; moving it into a Rust rendering crate is planned. No renderer output becomes authoritative score data.

Session history is in memory. Save persists the SFS source, not revision history. No external MIDI, SoundFont/sample output, MCP score mutation, or complete notation editing is claimed. Built-in tones are not sampled instruments. Native Windows/macOS verification remains pending unless recorded in a later test report.

See [React architecture and unchanged backend boundaries](../docs/REACT_DESKTOP.md) and [platform window chrome](../docs/WINDOW_CHROME.md).

For historical build/runtime results and blockers, see [desktop validation](../docs/DESKTOP_VALIDATION.md).

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

`npm run test:pcm` uses standalone Node.js 22.6+ tests for the [bounded PCM16/WAV data preparation component](../docs/PCM_WAV.md). It validates a fixed 48 kHz stereo output budget and checks header/chunk bytes without a browser, audio renderer or file writer. The desktop export controller reuses this component without allocating a second whole-file buffer.

`npm run test:wav-render` checks the [offline WAV rendering API](../docs/OFFLINE_WAV_RENDER.md) using injected fake audio contexts and sinks in standalone Node.js 22.6+. It checks shared live/offline graph scheduling, leading rests, ordered bounded byte delivery, backpressure, cancellation and failure cleanup. The desktop export controller connects this API to the existing shadcn menu/dialog and a session-bound native sink. These mocks do not validate real offline audio, hardware or desktop export.

`npm run test:wav-export` checks click-time source capture, repeated starts, binary IPC shape, backpressure, cancellation, finalization, cleanup and safe localized errors with fake native commands/rendering in standalone Node.js. It does not launch a native picker, WebView, browser or Vite, and does not verify native file persistence.

`npm run test:audio-render` is an optional real `OfflineAudioContext` signal regression using an already installed Chromium (`CHROMIUM_PATH` can specify its executable). It checks spectra, routing, gain, seek and note boundaries without opening an audio device or producing an audio file. It requires Chromium to run with its normal sandbox and does not install a browser. This test is separate from the mocked frontend suite; a browser-launch failure is not a signal-render pass.

An opt-in `smoke-test` Cargo feature adds a test-only report command. Native smoke runs also require the explicit Vite smoke mode and the isolated `io.scorestream.desktop.qa` configuration; a report request under the production identifier is rejected before profile access. Follow [the isolated native smoke build instructions](../docs/NATIVE_SMOKE.md). The real WebKit harness covers validation, drafts, previews, apply, undo/redo, conflicts, selection, and property edits. Ordinary builds contain neither the report command nor frontend smoke/observer entry points. Native file-picker cancellation/overwrite and OS packaging still need manual testing.

## Architecture

- `src/playback/`: cancellable audio-clock transport, bounded Web Audio scheduling and presentation of Rust timeline anchors.
- `src/components/PlaybackControls.tsx` and `ScorePlayhead.tsx`: existing shadcn transport controls and a source/sequence-guarded score overlay.
- `src/bridge.ts`: Tauri command boundary.
- `src/editor/source.ts`: Monaco presentation and Rust diagnostics.
- `src/render/project.ts`: bounded typed snapshot → MEI + identity map, no source parsing.
- `src/render/exact.ts`: rational comparison only at the projection boundary.
- `src/render/worker.ts`: exclusive Verovio instance and full layout.
- `src/render/scheduler.ts`: bounded latest-only publication.
- `src/render/sanitize.ts`: inert SVG-only publication.
- `src/main.tsx` and `src/App.tsx`: React root and app-owned titlebar/pane/dialog composition.
- `src/app/workbench.ts`: observable editor-session state and stale-safe async orchestration.
- `src/components/`: React Inspector, Preferences, source/score panes, menus and OS window controls.
- `src/components/ui/`: checked-in official shadcn components over Radix primitives.
- `src/lib/window.ts`: platform-specific native window operations and state subscription.
- `src/mcp/model.ts`: testable, stale-response-safe service lifecycle controller.
- `../crates/scorestream-desktop`: thin native command/dialog/lifecycle adapters over the shared Rust authority.

Each pending preview is invalidated on **every** keystroke, even if the new source is invalid. Draft keys distinguish session, sequence, SHA-256 content hash, base revision and renderer/layout identity. Graphical mutations are disabled for dirty text. Save writes source only after Rust validation; no unrestricted filesystem API is exposed to the WebView. SFS Open and Save use compatible file handling without a separate editor byte cutoff.

The contextual inspector and explicit metronome/dynamics candidate are described in [Contextual refinement](../docs/UI_CONTEXTUAL_REFINEMENT.md). Those records describe the pre-React UI; consult the React migration validation record for current acceptance.
