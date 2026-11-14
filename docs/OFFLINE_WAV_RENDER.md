# Bounded offline WAV rendering API

`desktop/src/playback/offline.ts` adds `renderPcm16Wav(plan, sink, options?)`, a frontend component for rendering the audio-facing Rust `PlaybackPlan` to an explicitly supplied transactional byte sink. The production default creates an `OfflineAudioContext`. It is not connected to desktop commands, UI, a save picker, native IPC or a file writer. User-facing desktop audio export and professional sampled instruments remain unimplemented.

## Audio and allocation contract

- `validatePcm16Plan` runs before creating any context, graph or audio/PCM buffers. Its existing limits remain unchanged: positive score duration up to 120 seconds, 4,096 total voices, 32,768 total expression points, 48 kHz stereo PCM16. See [PCM data contract](PCM_WAV.md).
- One context has exactly `ceil(duration_seconds * 48000) + 12000` frames. Notes start at their original score time with offset zero; leading rests are not trimmed. The fixed 250 ms output tail is retained, while note gates and oscillator stop times remain at the original note ends. No extra latency compensation, normalization or resampling is applied.
- Every note is scheduled once using the same `scheduleBuiltInVoice` used by live playback. Both paths use `createBuiltInMaster`, which creates and connects the compressor with unchanged Web Audio defaults. Instrument identifiers, neutral fallback, harmonic definitions, Rust expression envelopes, pitch, velocity and note gates are shared, not copied into a second audio engine.
- One full stereo Float32 render is required by Web Audio (up to 46,176,000 channel bytes, excluding graph/runtime overhead). The two channels are borrowed with `getChannelData`, never copied. The existing PCM encoder produces one independently owned chunk at a time, at most 65,536 bytes, with no full PCM file buffer. The sink must also avoid accumulating all chunks.
- Rendered buffer sample rate, channel count, frame count and channel array lengths are checked before the header is delivered. Only the validated frame range is encoded.
- The module has one active slot, shared even across injected factories. A concurrent request rejects immediately; there is no retained pending queue. This is exclusivity within one loaded JavaScript module/realm, not an OS-wide lock across application windows or processes.

The caller must keep the plan unchanged through the operation. Graph scheduling is synchronous after validation, before the first await; changing a score requires a separately validated request. `options.createContext` is a host/test seam and must return a fresh, unscheduled context for the supplied options. It is not a way to share a live context.

## Transactional sink and completion

All four `Pcm16WavSink` callbacks are required. Each returns a promise and is awaited before the next callback or chunk encoding:

1. `writeHeader(header, layout)` receives one 44-byte header after rendering and buffer validation succeed.
2. `writeChunk(bytes, startFrame, frameCount)` receives consecutive non-overlapping frame ranges from zero to exactly `layout.frameCount`. Header and chunk bytes remain owned by that callback until its promise settles. No callback overlaps another.
3. `complete(layout)` runs only after all bytes have been acknowledged. It must atomically publish/finalize the private output or reject without publishing. The renderer resolves with frozen layout metadata only after this callback succeeds; rendered audio or partial writes alone are never success.
4. `abort(reason)` discards incomplete output on any accepted-operation failure, including failures before the first header, or on cancellation. It is awaited before the active slot is released. If abort also fails, the returned `AggregateError` contains both the original and discard errors. The caller must report possible leftover private output rather than claiming successful cleanup.

Preflight validation, invalid sink, pre-aborted signal and busy-slot rejections do not call sink callbacks. A sink must not publish partial output during write callbacks. The component cannot undo arbitrary side effects in a sink that violates its contract. Future native persistence must provide temporary storage, atomic publication and discard behavior, not write directly to the final destination during rendering.

## Cancellation and lifecycle

Pass `options.signal` to request cancellation. Before finalization, an abort disconnects scheduled voices and the master and prevents further header/chunk/completion callbacks. A pending render or sink callback is allowed to settle before `abort` is called; the slot stays occupied throughout. Web Audio has no `OfflineAudioContext.close()` or reliable rendering cancellation API, so the function does not race the pending render against an early rejection and allocate another context while the old one is still running. A hung runtime or sink therefore keeps the request and slot pending; no timeout or false success is fabricated.

Cancellation is checked after the final chunk acknowledgement and immediately before entering `complete`. Entering `complete` is the commit point: later cancellation is ignored and the finalization result wins. This avoids reporting an already published output as cancelled. If finalization rejects, `abort` still runs. Callers should stop offering cancellation once their completion callback begins.

Graphs and abort listeners are cleaned up on success, render/setup failure, sink failure and cancellation. Runtime graph disconnection is best effort and never masks an operation error. A later request can run only after all outstanding work and sink discard have settled.

## Verification and remaining acceptance

Run with Node.js 22.6+ without dependency installation:

```sh
cd desktop
npm run test:pcm
npm run test:wav-render
```

The renderer suite injects fake contexts and sinks. It checks preallocation rejection, exact context sizing, leading-rest timing, one scheduling call per note, live/offline graph equivalence, exact ordered bytes from synthetic channels, the partial final chunk, no whole-channel or full-PCM copy, callback backpressure, malformed buffer rejection, serialization, cancellation before and during rendering/writes, finalization semantics, failure cleanup, retry and abort-error reporting.

This is orchestration and byte-contract coverage, **not real OfflineAudioContext rendering, audible playback, native hardware/WebView acceptance or a written WAV file**. Actual compressor behavior/latency and whether the reserved tail fully drains the real graph still need signal tests. Existing browser signal tests are separate; they are not run or expanded by this standalone suite. Desktop integration, safe native persistence and sampled instrument support are separate later work.
