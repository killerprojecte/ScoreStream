# Explicit local GM pizzicato approximation

This opt-in Linux offline route is **not** wired into desktop playback, browser
preview, default compilation or default rendering. It downloads/bundles no bank.

## Verified installed asset

The reviewed local `/usr/share/sounds/sf2/TimGM6mb.sf2` is 5,969,788 bytes, SHA-256
`c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854`.
The installed Debian `timgm6mb-soundfont/copyright` declares the bank GPL-2 with
Tim Brechbill (2004) and David Bolton (2010) attribution. This feature neither
copies nor redistributes that bank and does not make new licensing promises.

Static RIFF pdta inspection found bank 0, zero-based program 45, exact preset
`Pizzicato`, referencing instrument 128 and sample IDs 238/237/236. Four key
regions are 0–68, 69–83, 84–93, 94–108; no velocity-region restriction. Samples
are named `Pizz Violin E3`, `Pizz Violin C4`, `Pizz Violin E5`. Sample names are
not tuning evidence: the sample headers/generators, not those labels, govern
actual pitch. The existing FluidSynth adapter renders them and validates the
loaded bank hash and actual preset name before emitting PCM.

This is a **shared GM pizzicato approximation for violin/viola/cello/double_bass/
strings (including exact catalog aliases)**, not separately recorded viola/cello/bass, not acoustic modeling, and
not a guarantee of orchestral realism. Natural sample decay remains audible;
notated note/hold duration is preserved, not forced into a sustained bowed sound.
Key coverage is structural bank evidence, not a claim of convincing timbre over
the entire admitted range. The caller must explicitly choose this policy.

## API and invariants

- `plan_source_with_pizzicato(source, GeneralMidiSharedStringsApproximation)`
  returns an opaque `TechniquePlaybackPlan`, carrying the original plan and exact
  performed attack IDs needing pizzicato. Source text and instrument IDs stay
  unchanged. Default `plan_source` still rejects active unresolved pizzicato.
- `plan_source_with_joint_policies_and_pizzicato` requires all prior explicit
  articulation, grace allocation and synchronized hold choices plus this sound
  policy. It delegates to the same qualified compiler and final global clock.
- `validate_pizzicato_plan` checks route identities, key zones and overlap before
  loading a bank. `render_pcm_with_pizzicato` / `render_wav_with_pizzicato` require
  `InstalledTimGm6mbSharedStringsV1`, the exact bank digest, and caller-selected
  exact preset names for the original route palette.
- The plan getter is inspection data, **not authorization to strip the receipt**.
  The ordinary native renderer rejects extracted plans carrying active technique
  diagnostics. No browser/default dispatcher exposes this opt-in plan.
- New native channels are allocated only for routes with actual pizzicato attacks.
  The same part/instrument identity remains in the report; `technique_channels`
  identifies the extra channels and attack counts. No part-wide program change
  can mutate an already sounding bowed note. Arco attacks keep the original route.
- Tie continuations are not new attacks; repeats restore their prior explicit or
  unspecified state. Grace attacks consult the qualified actual onset, not the
  zero-length written marker. Selected holds retain one unchanged global clock.
- Independent same-key arco/pizzicato attacks use separate channels. Unsupported
  same-key overlap within one technique lane still reports the existing error.
  Native gain-envelope, microtonal, palette and capacity constraints are not bypassed.
- WAV output retains the existing atomic no-clobber publication and frame checks.
  The report states the approximation and verified preset; it does not claim a
  complete original Beethoven import or a production cross-platform audio host.

## Focused evidence

Tests cover exact attack receipts, unchanged source notes/timing, tie continuation,
repeat checkpoint restoration, independent concurrent same-key lanes, same-lane
collision, key/profile rejection, explicit joint grace/hold/articulation clock,
wrong preset before any sink output, isolated actual pizzicato PCM distinct from
bowed PCM, and real atomic WAV byte/frame count. Native tests must be built then
launched directly, preserving the normal environment and existing loader checks.
