# Explicit initial local MIDI mixer (narrow practice seam)

`midi_mixer::prepare_local_midi_mix` accepts exact source bytes + expected SHA,
an explicitly chosen one-route-per-part bank-0 palette, and the nondefault
`InitialGmCc7CenteredV1` policy. It returns a private-construction, non-deserializable
`LocalMidiMix`. This token is **not** proof of musical playback readiness. A higher
source-qualified wrapper must match `verify_source` before using it alongside
separately approved articulation/hold/unsupported-family policies. Ordinary core
lowering and existing source-assignment gates are deliberately unchanged.

The selected native program must equal the authored one-based program minus one.
The program alone does not imply a bank: choosing bank 0 is part of this explicit
local interpretation, with exact bank hash/preset-name assertions still required.
Full authored instrument metadata, including absent vs explicit MIDI port and
external channel, remains in the receipt. Native channels are route indices;
no external MIDI device is opened or addressed. Nonempty device names are rejected.
Dynamic metadata, absent programs, duplicates, mismatches and noncentral pan fail.

The [MusicXML volume specification](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/volume/)
defines percentage scaling of MIDI channel volume, not a linear PCM multiplier.
This chosen policy rounds `volume * 127 / 100` to nearest integer, half upward,
using exact rationals. It reports source percent, CC7, realized controller percent
and signed rounding error. I's `393701/5000` maps to CC7=100, realized `10000/127`,
error `-27/635000` percentage points. It does **not** claim exact source amplitude
or replace note velocity/expression with channel volume. FluidSynth/SoundFont
controller response is responsible for acoustic output; master gain is separate.

Only authored pan exactly zero maps to CC10=64. Nonzero/3D pan is not interpreted.
Absent volume/pan stays absent and no corresponding controller is sent. This does
not promise every stereo preset produces equal left/right energy. A mono Flute
probe verifies the supported center path. Native `fluid_synth_cc` is followed by
`fluid_synth_get_cc` readback for each applied controller before PCM. See the
[official FluidSynth MIDI API](https://www.fluidsynth.org/api/group__midi__messages.html).

Attach the token explicitly as `RenderOptions.local_midi_mix`; default is None.
A hash/name-verified bank is mandatory and the full route vector must match the
token exactly. Arbitrary expanded technique routes fail this check. Only the dedicated
source-witness-bound pizzicato path below qualifies controller inheritance; no
general patch-program exception is enabled. All existing per-note expression rejection,
resource/cancellation checks, voice limits and PCM clipping guards remain.

Tests cover source/route/hash mismatch, unsupported source semantics, external vs
internal addressing, exact CC7 quantization including half steps, absence, and
unchanged strict compiler rejection. Native tests use only installed TimGM6mb,
verify source palette names/controllers and synthetic PCM centered/muted routes.
The 18-route C4 probe is not Beethoven audio, an instrument range/timbre fidelity
proof, or permission to bypass remaining musical interpretation gates.

## Dedicated pizzicato lane inheritance

The dedicated TimGM pizzicato renderer may extend an initial mixer token only
when its opaque compiler `TechniquePlaybackPlan` has the exact same source SHA,
the original routes match, and the independently selected bank is the exact
TimGM6mb hash. Each additional route must be bank0/program45 for an existing part
and instrument. A private receipt copies that part's CC7 and CC10 unchanged and
records `inherited_from_native_channel`. The original program/port/channel in
`authored_metadata` is not rewritten; the lane has a separately reported local
program45. Expanded mixer receipts are also bound to the bank hash.

No global program-change events are emitted. Existing source-qualified attack
identities and tie consolidation determine the lane; simultaneous bowed/pizzicato
attacks remain on separate native channels. Original-route timing preflight runs
before native bank loading; expanded native palette and controllers are verified
before PCM. Wrong-source tokens, wrong banks, absent routes and malformed
technique attacks still fail. This path does not create a qualified musical plan:
source MIDI intent must independently pass the explicitly authorized compiler
path; stripping metadata and reusing another source's plan is rejected.
