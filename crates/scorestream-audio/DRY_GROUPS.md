# Internal native dry-group primitive

This is **not a new rendering mode** and does not change the ordinary renderer.
`native::dry::DrySynth` is a crate-private foundation for later source-qualified,
per-note dry-lane expression. No full-score fidelity claim or new WAV is made.

The constructor requires the original source bytes, matching sealed local MIDI
mixer, exact bank SHA and preset names. It reuses the trusted installed-library
loader and pinned bank handle. Optional FFI symbols are loaded only on this path.
One admitted MIDI channel maps to each stereo output group; out-of-range channels
are rejected rather than allowed to wrap modulo group count. Native group/channel
counts and disabled reverb/chorus settings are read back. Every process call uses
all distinct zero-filled dry output buffers and no effects output buffers.

Only whole native quanta are consumed. The frame cursor therefore describes PCM
actually returned, not an arbitrary request within a partially consumed native
buffer. Full configured polyphony plus a null sentinel is inspected synchronously;
no voice pointer escapes, no truncated list is treated as empty, released-but-playing
voices remain occupied. Native voice-budget saturation fails rather than permitting
voice-stealing to become an accepted render. NoteOff must match its admitted owner;
a new owner requires both previous NoteOff and zero playing voices. There is no
fixed tail timeout, sample-amplitude threshold, forced kill, delayed attack, or
modulo alias. A later adapter must enforce the actual source-plan schedule and
connect the separately prepared immutable envelope/lane policy; this primitive
alone is not that compiler authorization.

## Installed-bank proof

Ignored native unit fixture, executed directly without loader overrides, passed
on FluidSynth 2.4.4 and TimGM SHA
`c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854`.
Source/mixer SHA:
`be1e7a014f6afc99c62cca7801976caf76d7900df0461fe7650057396f83560b`.
Two bank-0/program-40 Violin channels play the same key with independent releases;
channel 0 receives a third synthetic attack at the fixed frame 32768. The source
fixture records those note times at a 120-quarter/minute clock; native synthetic
attack velocity is explicitly 80, not a claim of compiling source dynamics.

- First release at frame 4096 still owns audible voices; attempted reuse fails.
- Full voice snapshot first reports channel 0 empty at frame 17088.
- Until fixed next attack, that output is exactly zero.
- All dry samples including release tails equal isolated-channel references with
  the **same voice history**, and the concurrent other lane is unchanged.
- Incorrect source/bank, channel-2 modulo alias, incorrect NoteOff owner, and
  partial-quantum PCM requests fail.
- 8 existing unit tests also pass. No public API, ordinary option, or backend
  capability advertisement was relaxed.

No claim of bit-identical output across fresh versus reused synth instances, wet
FX support, general channel-pool capacity, or whole-movement coverage is made.

ABI references: [FluidSynth process](https://www.fluidsynth.org/api/group__audio__rendering.html),
[voice-list lifetime](https://www.fluidsynth.org/api/group__voice__control.html),
[output group modulo routing](https://www.fluidsynth.org/api/settings_synth.html).

## Composed opaque-plan proof

The private `LaneVoiceState` implementation now reads live complete snapshots
from `DrySynth`, never a caller-created cached empty certificate. A second native
unit fixture compiles a real opaque `TechniquePlaybackPlan` through the explicit
practice joint constructor and binds the matching source/mixer and envelope
schedule. Three performed attacks use the two verified Violin presets. Both pitch
and velocity come from the compiled plan, and event times come from its immutable
schedule. The fixture chooses written durations whose actual compiler gates fall
on native quantum boundaries; it **does not round, move or delay compiled events**.
The existing ordinary 0.92 gate, selected staccato half gate, and explicitly
selected 1/750-second fermata hold remain compiler decisions, not adapter fixes.

- `occ0:a`: frames 0–4096, original source dynamics step at frame 2001, gain 1→0.5.
- `occ0:b`: frames 64–4160, unit envelope, independent overlapping attack/tail.
- `occ0:c`: frames 32768–36928, safe reuse after native tail completion, including
  the selected 64-frame hold at its written endpoint.
- Every original envelope point, source/plan hash, pitch, integer velocity,
  duration and complete serialized plan is retained unchanged.
- Shaped lane 0 differs from the raw native baseline and matches independent
  pointwise evaluation exactly (maximum error 0), including its release tail.
- Lane 1 is bit-identical before/after gain projection. Both lanes separately
  match same-history isolated references through the complete test extent.
- Attempted admission at still-occupied frame 4096 is rejected, not delayed.
- A schedule from source differing only by a newline is rejected against the
  earlier opaque plan witness.

Fixture source SHA:
`c6bdd07ca7cf1485fe515cff5cbf870d1954f6d1dff12bb16a439addd4b003fb`.
Plan SHA:
`ed7670678de1c98be72fb4210028908561a3938ba0f98c03b0930d883e67b7be`.
This adds a trait adapter and test, not a production plan renderer. General
capacity allocation, arbitrary nonquantum event admission, technique-preset lane
changes, whole-score peak/tail budgeting and output publication remain unwired.

## Authorized idle route re-selection

A lane can now reselect only an index in the constructor's immutable, verified
source palette. Native bank/program and CC7/CC10 are read back at initialization,
including actual baseline values when the original source omitted those fields.
Re-selection cannot accept an arbitrary bank/program, mutable mixer, or controller
value. It requires the prior NoteOff and a fresh complete empty-voice observation,
then reads the selected preset and both restored controllers back. The original
sealed source mixer, including absent-versus-explicit fields, is unchanged.

The primitive exposes no sustain, bend, pressure, tuning, arbitrary CC, or MIDI
message entry point. Those channel states remain at their deterministic melodic
initialization. Future support for such controllers must re-prove safe rebinding;
this is not a general MIDI channel-reset guarantee.

Native multi-preset fixture: Violin bank 0/program 40 with explicit source volume
100% (CC7=127), Flute bank 0/program 73 with absent source volume/pan (native
baseline CC7=100, CC10=64). The test first switches the idle flute lane to violin
and back, proving that absent-source baseline is restored rather than inheriting
127. It then releases violin at frame 4096, rejects immediate re-selection while
its tail is playing, and only at the fixed frame 32768 selects flute for the next
attack. Previous violin samples/tail and the concurrently playing other flute
lane remain bit-identical. New flute samples differ from the unchanged-violin
control. The result matches an isolated same-history reference exactly.

Fixture source SHA:
`cf131f74e0f65651a39ed249a85bf6008c7326ab4b7008674e16a2e77b89f8ff`.
14 unit/native tests pass; external scratch-JSON fixture not rerun here.

This remains an internal re-selection primitive, **not real-I scheduling
admission**. A source-plan-derived pool of more independently initialized channels
than source routes, pizzicato palette ownership, and immutable ideal/rendered
64-frame event receipts still need composition. No capacity is inferred from
peak synth sample-voice counts, no whole-movement native admission was claimed,
and no full WAV was generated by this step.
