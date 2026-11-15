# Native offline PCM adapter: isolated first milestone

This crate consumes the real `scorestream_app::playback::PlaybackPlan` and renders
interleaved stereo PCM through an already-installed Linux FluidSynth 2 runtime.
`examples/render_sfs.rs` uses the real Rust `plan_source` parser/compiler, not a
hand-written interpretation of SFS. The adapter does not import MusicXML.

It is a standalone nested workspace for review. It is **not wired into desktop
playback, the main CLI, preferences, the main workspace or default CI**. No new
bank, native library, audio driver or plug-in is downloaded or bundled. Existing
Web Audio behavior is unchanged. This is not system-MIDI output or a VST3 host.

## Contract

- Each `(part, instrument)` requires an explicit bank and **zero-based** program.
  Names never imply patches. Actual available presets are enumerated and the
  selected native patch is checked back. Unknown/missing/unused routes fail.
- One selected SF2 is loaded once into a render instance. Up to 256 independent
  routes map to native channels, all explicitly melodic, including channel 10.
  Recorded SF2 samples work through the same API as GM. Exact key/velocity coverage
  and articulation realism still require a separate audited bank profile; finding
  a preset is not proof that every note produces the intended sound.
- The compiler owns musical interpretation, durations, repeats and movement
  joins. The adapter does not add silence at section boundaries. Whole-work
  callers should use one combined timeline for attacca sections, with a release
  tail only at the final endpoint.
- Integer MIDI pitch, matching equal-tempered frequency, valid nonzero velocity,
  finite times and positive rounded durations are required. Microtones,
  independent gain envelopes, and overlapping identical keys on one route fail
  explicitly, without substituting another pitch or channel-wide expression.
  Constant unity gain envelopes are accepted. Already-realized score gates are
  used as-is; this layer adds no pedal/slur/fermata/articulation interpretation.
- Events split output at integer frame boundaries, note-offs before retriggers.
  **FluidSynth internally quantizes event response** (64 frames on the tested
  runtime), so this is not a sample-accurate plug-in host. The report records the
  internal quantum. Test block sizes 1, 64, 257 and 4096 produce identical PCM,
  including deliberately off-quantum events on the installed fixture.
- Output is streamed in bounded blocks, not accumulated for a whole movement.
  There is no 120-second or 4096-total-note cap. Default budgets are 1 million
  note events, 4096 simultaneous native voices, and a 2 GiB bank input; callers
  may set explicit limits. Native voice count reaching its budget fails instead
  of presenting a known saturation as success. This is not a complete proof
  against every form of layered-patch voice stealing. The bank byte budget is
  not a native resident-memory guarantee.
- Effects are off; gain and release-tail duration are explicit. No normalization,
  randomization or hidden instrument substitutions occur. Nonfinite/clipping PCM
  fails. The final 50 ms peak flags a potentially truncated release; fixed tails
  cannot prove all voices are finished.
- `render_pcm` sends private partial data to a caller sink and checks cancellation
  between blocks. A failed/cancelled stream must be discarded by its consumer.
  Bank hashing is bounded and checks cancellation every 64 KiB. Native library
  loading, SoundFont parsing/sample allocation and an individual sink call cannot
  be interrupted inside those foreign/blocking calls.
- `render_wav` writes PCM16 stereo to a same-directory temporary file, flushes and
  verifies byte count, then publishes without overwriting any existing file or
  symlink. Cancellation/errors before publication discard the temporary file.
  Once publication starts, cancellation is too late. This provides atomic
  visibility, not guaranteed crash durability of the parent directory. Standard
  RIFF's 4 GiB size bound is checked; RF64 is not implemented. The PCM sink itself
  is not subject to that file-format limit.

## Native trust and lifetime

The only unsafe boundary is `native.rs`. No caller-supplied library filename is
accepted. The loader checks fixed canonical `/usr` locations, ELF magic, owner
and non-writable path components, and rejects any nonempty `LD_*` override. The
system image's `/usr` owner is the trust anchor (normally root; mapped owner IDs
are supported), never the current user. It records the checked on-disk library
hash and reported runtime version. The hash is not an attestation of resident
code: the OS may reuse an already-loaded image, including across a package update.
This trusts the installed OS and its dependencies, including the process's prior
loading history. It is **not** a sandbox or an integrity proof against root, a
compromised process, or malicious native dependencies.

Known FluidSynth 2 symbols have typed ABI declarations. The module is thread-bound;
settings/synth objects use RAII. Native code and dependency statics remain resident
with `RTLD_NODELETE`, because GLib/libinstpatch process-global registries are not
safe to unload/reload. Repeated sequential open/render/drop is tested. This
intentional process-lifetime code residency is not a leaked render instance.

Only existing authorized local SoundFonts may be selected. The bank is a regular,
size-bounded RIFF SF2, hashed and held open. Native loading uses its held
`/proc/self/fd` handle, not a reopened user pathname. Pre/post metadata and bounded
hash checks detect ordinary replacement or in-place changes, with eager sample
loading. This does not protect against a hostile writer racing and restoring the
same inode during parsing. **Native bank parsing is not sandboxed.** Broader
untrusted uploads need process containment and an immutable asset snapshot first.
There is no enduring copy or redistribution of the selected bank.

## Build and checks

Use a normal environment with the Rust toolchain on PATH. From the repository:

```sh
cargo test --manifest-path crates/scorestream-audio/Cargo.toml --locked --offline
cargo clippy --manifest-path crates/scorestream-audio/Cargo.toml --locked --offline --all-targets -- -D warnings
python3 crates/scorestream-audio/tests/run_native_tests.py
```

Ordinary tests need no installed synth. Native tests are explicitly opt-in and
require the already-installed library plus a standard local GM bank. Cargo sets
`LD_LIBRARY_PATH` for its child executables; therefore running native tests through
`cargo test ... -- --ignored` is intentionally rejected. The helper builds using
Cargo JSON output, then launches the verified test artifact directly under the
unchanged caller environment. It does not remove loader overrides or weaken the
native checks. Do not use `cargo run` for the example for the same reason.

To use the example, build it then execute the built artifact directly:

```sh
cargo build --manifest-path crates/scorestream-audio/Cargo.toml --locked --offline --example render_sfs
# Run the render_sfs executable reported by Cargo (target layout may vary):
render_sfs score.sfs routes.json new.wav /path/to/authorized-existing.sf2
```

Example routes JSON: `[ { "part": "p", "instrument": "clarinet", "bank": 0,
"program": 71 } ]`. The example reports actual backend, sample hash, preset names,
voice count, timing quantum, PCM statistics and propagated compiler diagnostics.
It does not claim whole-orchestra coverage from one successful patch.

## Production integration gates

1. Review this isolated crate, then decide workspace membership, opt-in desktop
   feature and CI runners with authorized installed banks. Normal builds must
   still work when the backend is unavailable. Native loading stays off until
   explicitly selected; do not use a silent fallback for export.
2. Define audited sample profiles (key/velocity coverage, sustain behavior,
   articulations, source licenses/hashes), per-route bank assignment and job-level
   resource/progress/cancellation ownership. Preserve upstream expression or fail
   before export until the selected backend supports it.
3. Integrate the actual imported-score compiler output and preserve its diagnostics
   and movement timeline. Validate complete works with sustained dynamics and
   attacca seams; a synthetic long test is capacity evidence, not music fidelity.
4. Implement system MIDI-device output and a separate isolated VST3 scanner/host
   as distinct capabilities. VST3 requires native component/controller lifecycle,
   sample-offset events/parameter automation, patch state, latency/tail handling,
   and a separate offline instance. No plug-in is discovered or loaded here.

API references: [FluidSynth rendering](https://www.fluidsynth.org/api/group__audio__rendering.html),
[SoundFont loading](https://www.fluidsynth.org/api/group__soundfont__management.html),
[synth settings](https://www.fluidsynth.org/api/settings_synth.html),
[MIDI messages](https://www.fluidsynth.org/api/group__midi__messages.html).
