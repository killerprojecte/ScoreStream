# Experimental authenticated dry-lane admission probe

`dry_admission::probe` computes and discards native PCM. It writes no audio file,
opens no output device, and does not modify ordinary/default rendering. The
example `dry_admission_probe SOURCE POLICY BANK hold_last_gain_at_noteoff_v1`
explicitly selects the existing qualified practice policies and prints JSON.

## Binding and capacity

- Original source bytes must match both the opaque `TechniquePlaybackPlan` and
  sealed original MIDI mixer. The existing TimGM pizzicato policy supplies the
  validated program-45 palette, source witness, and inherited CC proof; it is not
  recreated by arbitrary route overrides.
- Normal plan validation remains: exact pitch/frequency, source route identity,
  velocity, note extent, caller resource budget and selected bank/preset assertions.
  Only this crate-private opaque-plan route accepts retained independent envelopes
  and defers old shared-route same-key admission to disjoint native lanes.
- Native settings report the physical audio-group/channel range. The pool uses
  that real installed maximum. Extra lanes are unbound until selected from the
  initially verified source palette; they cannot sound with a default preset.
  Source mixer routes are never duplicated or fabricated to expand authority.
- Every admission needs the fixed clock boundary, previous NoteOff, and complete
  synchronous native playing-voice snapshot including released voices. No lane is
  freed by timeout, small amplitude or inferred tail length. Exhaustion reports
  the exact event/frame and physical capacity. It never queues or delays attacks.
- Rebinding restores verified preset and deterministic CC7/CC10, including captured
  initial defaults where source fields were absent. The proof remains restricted
  to the backend's NoteOn/Off + these controllers, not general MIDI history.

## Clock and report

Each ideal nearest-sample event and its ceil-to-native-quantum rendered event are
retained. A note collapsing within one quantum fails rather than disappearing.
Quantization is explicit; no extra admission delay is introduced. This coarse
native response is not advertised as sample-accurate. Per-note gain is evaluated
on the ideal source-derived sample clock, retaining every original point and
right-continuous step; after NoteOff, the selected policy holds its endpoint gain
through the actual native tail. The source plan is never rewritten.

The report includes source/plan/bank hashes, backend identity, physical capacity,
all admitted event/palette/lane/CC assignments, note-off count, measured peak owned
and playing lanes/voices, remaining playing lanes at the observation endpoint,
original envelopes, practice omissions, compiler diagnostics, exact NoteOn receipts,
and computed (discarded) PCM peak/RMS. Any remaining voices at the selected final
observation frame are reported, not assumed finished. The probe's selected tail
observation duration is not a lane eviction rule or whole-tail completeness claim.

The configured synth voice budget is still explicit. Saturation fails. Public
voice counts do not exhaustively prove absence of all SoundFont exclusive-class
behavior. General SoundFonts, wet effects, live MIDI, VSTs, arbitrary controllers,
resampling and publication are outside this experiment.

## Focused validation

The true opaque three-note fixture exercises the full pool/probe, original gain
step, concurrent unmodified lane, exact admitted frames, safe same-lane reuse,
and complete final tail observation. Separate native fixtures cover different
presets, missing-controller restoration and rejected busy-tail rebinding. Legacy
contract tests still reject independent gain and shared-route same-key overlap
through ordinary/default entry points.

## Untouched movement I admission evidence

The first end-to-end native probe admitted all 10,922 attacks and 10,922 NoteOffs,
including the existing 38 program-45 pizzicato attacks, with no capacity failure
or post-quantization delay. Actual installed capacity was 128 groups; measured
peak owned/playing lanes was 89 and peak sample voices 124. These are separate
counts. Maximum ideal-to-rendered displacement in this source was 62 frames.

Source SHA `ffac34c86e31df50b028ca7afe0e91f822c2b122af28b55c5e0c1ba10769f77b`;
plan SHA `876e1e7545018b5c8e97768563d0066d50ecd97e509a3cc795d4d0acd99ce2f4`.
The 17,516,352-frame observation computed/discarded PCM peak 0.09171499062638219
and RMS 0.009624511601293939. **Seven lanes still had playing voices at the end
of the explicitly selected two-second tail observation.** This is not a proof of
complete final tail release, and no WAV was written or silently extended.

The initial JSON was generated before additive diagnostic/CC-readback report
fields and an untouched-zero-lane allocation optimization. Final code retains the
same admission/curve/preset algorithm; 15 unit/native plus 7 legacy contract tests
passed. Exact final-code corpus receipt is a separate re-verification, not inferred
from that earlier JSON. The scratch JSON and SoundFont are not repository assets.

## Explicit exhaustion observation

`probe_until_idle` / example suffix `until_native_idle` extends observation until
all scheduled NoteOffs have been consumed and the full authoritative native voice
snapshot is empty (and the written score extent has elapsed). It does not use
amplitude, predicted patch-tail seconds, or a timeout to release ownership. The
caller can cancel an unending probe; cancellation is not reported as completion.
The report records the original selected-window endpoint separately, last ideal
and rendered NoteOff, and the first truly idle completed frame. No audio is saved.
A real-bank synthetic regression deliberately selects zero extra tail, then proves
this mode continues past that window and stops exactly at measured voice exhaustion.

## Verified atomic WAV artifact

The separate explicit `render_verified_wav` path accepts the successful exhausted
observation as an **expected result**, not an imported authorization token. It
checks the opaque plan/source/envelopes and full-note readiness, then freshly
replays the bank, palette, controller readback, lane admissions, every gain sample
and authoritative tail observation. The entire resulting JSON observation must
match the expected observation before publication. JSON comparison uses the same
serializer/parser normalization on both sides, avoiding an incidental one-ULP
parser round-trip difference. A changed frame extent, program/controller receipt,
source, native backend, gain result or still-playing tail fails publication.

The pre-existing atomic WAV writer is reused with only its private report return
type generalized. It still refuses an existing file/symlink, writes a temporary
file, checks exact frame and byte counts, flushes and fsyncs, and publishes with
no-clobber semantics. Original envelope processing and summation remain f64; the
explicit artifact format converts the final stereo sum to f32, rejects any newly
rounded clipping/nonfinite sample, then uses the existing 16-bit PCM encoding.
The report's computed peak/RMS describes the f64 sum; verify encoded PCM separately.

Example suffix `until_native_idle READINESS_JSON NEW_WAV` selects this artifact
path. Ordinary probes still discard PCM and ordinary product rendering is unchanged.
Real-bank synthetic tests verify successful nonzero PCM, complete measured tail,
existing-file preservation, and no file or leftover temporary file after a full
replay with deliberately mismatching expected peak. No source score is modified.

The untouched I exhaustion proof measured last rendered NoteOff 17,405,952 and
first idle frame 17,617,152, exactly 4.4 seconds later. The complete observation is
367.024 seconds at 48 kHz, all 10,922 attacks/releases admitted, 0 unmatched live
note-offs, 89 occupied lanes out of 128 physical groups, and 124 peak sample voices.
This observation alone is not an assertion that an output artifact was published.
