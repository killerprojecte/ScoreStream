# Native WAV destination and streaming contract

This component adds dedicated WAV commands to the pinned Tauri 2.9.4 desktop host, using rfd 0.15.4 and tempfile 3.23.0 already in its dependency lockfile. `scorestream-app::wav` supplies the platform-independent output layout, canonical header and fail-closed sequential writer. It does not change SFS open/save, score history, live playback, format conversion or external sampled-instrument routing. Frontend/menu wiring is a separate component.

## Destination authority and commands

The native chooser is the only way to select the destination. No command accepts a path, directory, filename, arbitrary format or caller-supplied layout. All command errors are JSON objects with `code` and a path/source-free `message`.

1. `invoke('wav_export_begin', {source})` reserves one slot for the calling window, plans that exact source on a blocking worker and validates the audio layout **before** opening the native save chooser. The layout matches `desktop/src/playback/pcm.ts`: finite positive duration, fixed 48000 Hz stereo PCM16, and `ceil(duration * 48000) + 12000` frames. No fixed source-size, duration, voice or expression-count cap applies. File size and stream offsets must fit uint32; offset headers accept all canonical uint32 decimal values (up to ten digits). Invalid input never opens a chooser or creates a file.
2. Cancelling the native chooser resolves `null`, releases the slot and creates no temporary/final file. No rendering is required before this result. Success returns `{sessionId, path, frameCount, dataBytes, fileBytes, plan}`. Metadata keys use camelCase; `plan` retains the existing Rust PlaybackPlan field names. The frontend validates and renders this exact returned plan. `path` is display-only and never sent back to the writer.
3. `invoke<number>('wav_export_write', bytes, {headers: {'x-scorestream-wav-session': sessionId, 'x-scorestream-wav-offset': String(offset)}})` takes a raw `Uint8Array`, never JSON arrays or base64. Await the next byte offset before encoding/sending another chunk. The first write is exactly the canonical 44-byte header at offset 0. Every later write is 1..65536 bytes, a multiple of four, with the next exact byte offset. Offset includes the header, so PCM begins at 44. The final acknowledgement equals `fileBytes`.
4. `invoke<string>('wav_export_finish', {sessionId})` verifies exact completion and physical file size, flushes and synchronizes the file, then persists it without overwriting another path. Only this command publishes the destination. It resolves the saved path after publication. Once finalization owns the lock, a later abort waits and cannot undo a successful commit.
5. `invoke<void>('wav_export_abort', {sessionId})` removes private output. Missing/previous generations are idempotent no-ops, never affecting a newer session. The caller must abort after a post-begin validation failure, pre-aborted render, busy renderer rejection, render failure or user cancellation, including failures before the offline renderer starts calling the sink. A matching-session malformed write or failed finish already removes the private file.

Headers are bounded: session IDs have exactly 20 ASCII characters (`wav-` plus sixteen lowercase hexadecimal digits); offsets contain one through ten canonical decimal digits, without a sign, whitespace, exponent or leading zero. Duplicate required headers are rejected. The Tauri transport can allocate an incoming request before the handler checks its size; this is a handler-level 64 KiB limit, not a claim of transport-level admission control.

## Isolation and cleanup

Session IDs distinguish generations and are **not secret bearer capabilities**. Ownership is checked using the actual `WebviewWindow` injected by Tauri, never a JSON/header-provided window name. Only the main editor WebView can invoke this initial slice. A valid ID from another window does not authorize access. One slot covers planning, the pending native chooser, writing and finalization. There is no retained application chunk queue or full PCM buffer.

The output is a same-directory `.scorestream-wav-*` named temporary file. A byte error, wrong format, sequence gap/replay, incomplete finish, overrun, write/flush/sync/persist failure or abort cannot publish an incomplete file. Explicit discard reports `WAV_CLEANUP_FAILED` if removal fails. Navigation/reload, actual window destruction and normal app exit also discard pending output. A cancelled close request does not discard a still-running export. A pending chooser reservation is invalidated on teardown, so a late chooser result cannot open a file. `NamedTempFile` drop is a final best-effort safety net.

No overwrite is supported in this slice. Existing files, directories and dangling symlinks are rejected even if the OS dialog offered a Replace button. The final `persist_noclobber` prevents a competing file created after selection from being overwritten. Choose a new `.wav` filename instead. This deliberately does not infer platform overwrite consent from the rfd result.

File data is synchronized before publication. No directory-fsync or power-loss durability guarantee is claimed. tempfile documents that no-clobber persistence can leave a second temporary hard link on unusual filesystems or interruption; normal publication exposes the complete file, but universal atomic movement/cleanup across all filesystems is not claimed. Forced process termination does not run destructors. Native OS/filesystem acceptance remains required. Per-chunk writes are synchronous, borrow the bounded IPC payload, and may briefly stall the host on slow storage; only planning and final sync/persist use blocking workers.

## Stable error codes

- `WAV_SOURCE_INVALID`: the submitted source cannot produce a playback plan.
- `WAV_LAYOUT_INVALID`: audio fields or output size cannot be represented by the WAV format.
- `WAV_DESTINATION_EXISTS`: existing destination or a competing file at final publication.
- `WAV_BUSY`: this window already owns a pending/active export.
- `WAV_BAD_REQUEST`: malformed protocol, header/layout/count mismatch or non-WAV destination.
- `WAV_SESSION_NOT_FOUND`: this caller has no matching active session.
- `WAV_CANCELLED`: window teardown invalidated the pending chooser reservation.
- `WAV_IO`: native temporary write, flush, sync or publication failed.
- `WAV_CLEANUP_FAILED`: private temporary output could not be removed. Do not present this as successful cleanup.

Unknown errors should remain generic. Native errors do not contain raw source, parser diagnostics, OS error messages or paths; the successful picker result and final saved path are returned separately for normal display.

## Tests and verification limits

The headless writer adds tests for layout/limits, canonical bytes, mono/wrong-format rejection, whole-frame chunks, exact full/partial completion, replay/gap/truncation/overrun rejection and injected partial-write/flush failures. Run with an existing Rust toolchain:

```sh
cargo test -p scorestream-app wav
```

The native module adds local-filesystem/session tests for private-before-finish output, no-overwrite races, invalid paths, dangling symlinks on Unix, window ownership, stale IDs, busy chooser reservations, invalid protocol cleanup, abort/drop/teardown and source preflight. They require the desktop build prerequisites:

```sh
cargo test --manifest-path crates/scorestream-desktop/Cargo.toml wav_export
```

**These Rust tests and native compilation were not run in the implementation environment because no Rust toolchain was installed.** No dependencies were installed. No actual chooser, Tauri IPC, WebView, browser/audio rendering, platform packaging or CI validation was performed. Source-level API review is not a native runtime pass.

Pinned primary-source checks: [Tauri Request and InvokeBody](https://github.com/tauri-apps/tauri/blob/tauri-v2.9.4/crates/tauri/src/ipc/mod.rs), [injected WebviewWindow owner](https://github.com/tauri-apps/tauri/blob/tauri-v2.9.4/crates/tauri/src/webview/webview_window.rs), [raw invoke arguments and header options](https://github.com/tauri-apps/tauri/blob/tauri-v2.9.4/packages/api/src/core.ts), [builder lifecycle hooks](https://github.com/tauri-apps/tauri/blob/tauri-v2.9.4/crates/tauri/src/app.rs), [rfd save dialog](https://github.com/PolyMeilex/rfd/blob/0.15.4/src/file_dialog.rs), and [tempfile persistence and cleanup](https://github.com/Stebalien/tempfile/blob/v3.23.0/src/file/mod.rs).
