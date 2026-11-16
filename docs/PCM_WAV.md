# Bounded PCM16 data preparation

`desktop/src/playback/pcm.ts` is a pure frontend data component used by the [separate offline rendering API](OFFLINE_WAV_RENDER.md). It validates the audio-facing portion of the Rust `PlaybackPlan`, calculates bounded output sizes, builds a classic RIFF/WAVE header, and converts one stereo Float32 range into PCM16 bytes. This data component does **not** render audio, create an audio context, add an export command, invoke desktop IPC, open a save dialog, or write a file. The desktop export uses this API; external sampled instruments remain unimplemented.

## Fixed profile and allocation limits

- 48,000 samples per second, two channels, signed 16-bit PCM, little-endian. No alternative format, sample-rate conversion, mixing, normalization, dither or metadata chunks.
- Finite positive score duration, rounded up to a complete sample, followed by exactly 12,000 frames (250 ms). This reserved tail lets the offline graph drain after the score, not extend planned notes. Its adequacy for that graph still needs real signal validation.
- No fixed score-duration, total-note or expression-point ceiling. Invalid timing, pitch and envelope fields still report errors.
- Output must fit the native unsigned 32-bit file-size/offset protocol and classic RIFF fields: at most floor((2^32 - 1 - 44) / 4) frames, including the tail. Header/layout tests exercise that boundary without allocating the payload. RF64 is not implemented.
- Each encoded chunk is at most 16,384 complete stereo frames (65,536 bytes). A future writer must wait for each transfer before producing the next and must not collect every chunk in memory. The 64 KiB limit applies to raw PCM bytes, not any future IPC serialization overhead.
- Frame counts must be safe integers within the profile. Header generation also checks classic RIFF's unsigned 32-bit size fields before allocating bytes. No RF64 support is implied.

## Contract

1. Call `validatePcm16Plan(plan)` **before** allocating an offline audio graph or rendered buffers. It returns frozen `{frameCount, dataBytes, fileBytes}` metadata. The caller must retain that same unchanged plan until rendering completes; this function does not copy or freeze caller-owned score data. Mutating or replacing the plan requires validation again.
2. The offline renderer uses that frame count and the fixed 48 kHz stereo format. Only `duration_seconds` and `notes` are consumed by the validator. Timeline anchors, warnings and other UI metadata are not parsed or copied. The Rust plan remains the source of performance timing, pitch, routes and expression.
3. Call `createPcm16WavHeader(frameCount)` for the 44-byte PCM format-1 header. This primitive permits zero frames for a valid empty RIFF, although plan validation rejects zero-duration scores. It independently validates its size argument.
4. Call `encodePcm16StereoChunk(left, right, startFrame, frameCount)` for consecutive non-overlapping frame ranges. Channel arrays must have equal lengths, be Float32 arrays at 48 kHz, and fit the profile. The encoder cannot infer their sample rate; the caller must ensure it. Write the header once, then chunks in order. The caller/writer must verify the total frames exactly match the header and handle cancellation, backpressure and errors. No transport protocol is implemented here.

The validator rejects non-finite numeric fields, negative or zero note durations, notes extending beyond score time, unordered onsets, invalid MIDI-pitch/frequency/velocity ranges, and invalid envelope points. Fractional MIDI pitches are accepted. Initial velocity is an integer from 1 through 127, as in the Rust DTO. Frequency must be positive and below 24 kHz. Envelope offsets must be nondecreasing and within the note, with duplicate offsets allowed because Rust may emit them. Relative gains can range from 0 through 127, rather than being incorrectly clamped to unity. Only a small floating-point roundoff allowance (four machine epsilons scaled by score duration) is permitted at time endpoints. It does not re-derive frequency from pitch or interpret instrument names.

The encoder writes `L0, R0, L1, R1, ...` as little-endian signed 16-bit values. Finite samples clip to `[-1, 1]`; negative values scale by 32,768 and nonnegative values by 32,767, then use JavaScript `Math.round`. Thus -1 maps to -32,768 and +1 to +32,767. NaN and either infinity become silence. Inputs are unchanged and each returned chunk owns its bytes. This is deterministic quantization, not a mastering or loudness policy.

## Standalone verification

With Node.js 22.6 or later (built-in TypeScript stripping), no dependencies are needed:

```sh
cd desktop
npm run test:pcm
```

The tests cover valid scores beyond the former duration/count ceilings and exact/overflowing representable output sizes, overflow-scale and non-finite inputs, ordering, fractional pitch, relative gain, header fields and sizes, endpoint quantization, clipping, little-endian channel order, subarray offsets, independent chunk buffers, range rejection before PCM allocation, and exact full/partial chunk assembly. This is byte-level and validation coverage only. No Vite, browser, audio device, native save dialog, file write or external sample engine runs in this harness. Audio rendering and desktop export acceptance require separate implementation and testing.
