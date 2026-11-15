# Desktop WAV export

Use **File → Export WAV…** in the native desktop editor. Choose a **new** `.wav` filename in the operating system save dialog. A destination that already exists is rejected, even if the operating system dialog offered to replace it. The export dialog reports completion or an actionable error. Cancelling the native save dialog closes the export quietly.

On Linux, WAV export uses a GTK save dialog parented to the invoking editor window. It runs through main-thread response signals rather than a nested blocking dialog loop, accepts only local paths, and does not offer an overwrite confirmation. Explicit Cancel or window-close responses are quiet; an abnormal response, missing local filename, unavailable dispatch or unexpected destruction reports the stable `WAV_CHOOSER_FAILED` error in English/Chinese. Windows/macOS retain their existing native chooser route. Ordinary SFS Open/Save are unchanged.

The export captures the current source at the menu action, including an unapplied draft. Rust validates and plans that source independently; invalid or over-budget input fails without changing the document. Export does not call SFS Open/Save, apply, undo, or redo. Editor file identity, saved-source marker, revision and undo history are unchanged.

## Output and limits

- Built-in synthesized instruments, using the same voices/master graph as playback. This is **not** an external sampled instrument, SoundFont, VST or Audio Unit host.
- Classic RIFF/WAV, 48 kHz, stereo, signed 16-bit PCM, with the existing 250 ms tail.
- Up to 120 seconds of score time, 4,096 planned notes and 32,768 expression points.
- One operation at a time, including the native chooser, rendering, writing, cancellation cleanup and finalization.
- Cancel is available during rendering/writing. `OfflineAudioContext` cannot stop its pending render immediately, so cancellation waits for it or the current write to drain before discarding incomplete output. The finalization result is authoritative once commit starts.

## Frontend/native contract

The narrow bridge accepts source only for planning and does not accept a caller-selected writable path:

1. `wav_export_begin({ source })` returns `null` for chooser cancellation, or `{ sessionId, path, frameCount, dataBytes, fileBytes, plan }`. Native independently validates the plan and controls the destination/session.
2. The frontend validates the returned plan/layout before offline allocation, then calls the existing `renderPcm16Wav` API with a transactional sink.
3. `wav_export_write` receives a **top-level `Uint8Array` raw body**, not JSON/base64/number arrays. Invoke options carry `x-scorestream-wav-session` and decimal `x-scorestream-wav-offset` headers. It returns the next acknowledged byte offset.
4. The first write is the 44-byte RIFF header at offset zero. Further writes are whole PCM frames, at most 65,536 bytes each. Every invocation is awaited before another chunk is requested. No extra full PCM/WAV buffer or chunk queue is retained.
5. `wav_export_finish({ sessionId })` publishes and returns the saved path. `wav_export_abort({ sessionId })` discards incomplete output and is idempotent. Failed or cancelled operations are cleaned up even when the renderer rejects before invoking a sink callback.

The dialog uses existing shadcn Menubar, Dialog, Alert, Button and Spinner components, with English/Chinese text. It displays only the output basename and safe messages chosen from stable native error codes; arbitrary native diagnostics, score source and private directory paths are not echoed into export errors.

## Validation scope

Run with already installed pinned dependencies:

```sh
cd desktop
npm run test:wav-export
npm run test:pcm
npm run test:wav-render
npx --no-install tsc --noEmit
```

Standalone Node tests inject a fake native bridge/renderer. They cover captured source, duplicate starts, exact raw IPC shape, ordered offsets, one in-flight write, acknowledgement mismatch, cancel during chooser/render/write, dispose, finalization wins, plan limits, cleanup, safe errors and narrow source-level UI wiring checks. Existing standalone PCM/offline tests cover the byte encoding and mock graph separately.

These tests do **not** launch a native chooser, Vite, browser, WebView or audio device and do not test actual native file persistence. Real native chooser, operating-system filesystem behavior, real offline audio and Windows/macOS/Linux desktop acceptance require separate validation. Do not interpret these mocked tests as full platform acceptance.

The desktop Rust test filter `wav_export::tests` also includes pure Linux response classification and single-reply channel lifecycle checks (no GTK initialization or widgets), plus reservation cleanup/stale-operation checks. The GTK/Tauri adapter was checked against the committed GTK 0.18.2 and Tauri 2.9.4 source APIs. That source review does not replace compilation or native interaction: chooser presentation, focus, Save/Cancel/Escape/window-close, parent teardown and repeated exports still require native runtime validation.
