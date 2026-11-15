# Exact numeric dynamics and independent printed symbols

This is the next bounded MusicXML import domain, **not full original Beethoven import or a natural sampled-orchestra result**. Wedges, compound dynamics, arbitrary words and source layout still fail closed. No source directions are deleted or rewritten to claim full import.

## Two typed lanes

- `ScoreSoundDynamics` preserves a part/staff scope, exact quarter-note position and nonnegative rational percentage. MusicXML `sound/@dynamics` is a percentage of default forte velocity 90, not a MIDI integer and not a symbolic dynamic level.
- `ScoreDynamic` preserves the independently positioned printed symbol, part/staff scope, optional above/below placement, and whether a symbolic performer policy may act. MusicXML display marks import with `sound=false`; an explicit numeric sound remains authoritative. `sf` is distinct from `sfz`; neither spelling is silently substituted.

The SFS additions are a registered `sound_dynamics` part/staff state and a boolean `sound` dynamic-mark field (default true for existing SFS). For example:

```sfs
= p sound_dynamics=1111/25
+ p dynamic value=p sound=false placement=below
```

The source value 44.44% remains exactly 1111/25 in both the shared model and SFS. It means 39.996 MIDI-relative velocity units. It is not replaced with the existing printed-p policy 48. The original Beethoven corpus also uses 106.67% for both sf and ff, and varies printed p from 31.11% through 54.44%; label-based substitution would change the authored sound suggestions.

Missing initial numeric sound stays unassigned. Import/export never invent a 100% source control or derive sound from a display symbol, and unrelated parts retain their existing playback behavior. The W3C MIDI tutorial describes a default velocity 90, whereas the existing SFS renderer uses 80 until an explicit state applies. This is an acknowledged renderer-default gap, not equivalent MusicXML playback: affected conversions report `sound_dynamics.initial_unassigned`. The same absence and later first assignment survive SFS→MusicXML→SFS. A separate, explicitly selected import/render policy could implement the tutorial default later; this slice does not change the legacy default.

## Positions, conflicts and roundtrips

Tempo and dynamics now share one tested position resolver. Direction offsets always move display; absent `sound` or `sound="no"` does not move associated playback. `sound="yes"` does. An offset inside sound overrides the direction's playback offset and remains relative to the current cursor. Backup/forward and current divisions are respected. Directions never advance notes or enlarge pickups. Imported display and sound positions must remain inside their containing measure.

Part and staff states remain distinct. Staff numeric state overrides part state; later same-scope assignments replace earlier ones. Simultaneous same-scope assignments must agree exactly; equal duplicates are normalized, and conflicts reject. Changes in different parts do not become score-wide. Printed marks cannot conflict with or replace numeric sound.

Common/SFS storage preserves arbitrary nonnegative rational percentages. XML export requires an exact finite decimal within its existing precision budget. Part-scoped sound exports independently of display. Staff-scoped XML sound currently requires a printed dynamic at exactly the same position so it can use that direction's staff; unmatched staff export explicitly rejects rather than inventing an invisible direction or fake symbol. SFS retains unmatched staff states without losing their scope. Symbolic `sound=true` policy marks remain representable in SFS but are not falsely exported as explicit numeric MusicXML sound.

## Playback and renderer boundary

The plan keeps exact rational percentages until projection: percentage × 9/10 is the MIDI-relative magnitude. The existing plan has a u8 attack velocity. At that final boundary it rounds/clamps the integer carrier and preserves the actual requested magnitude in the gain envelope. Thus 39.996 becomes velocity 40 with gain 0.9999. Zero uses a nonzero carrier with gain 0, avoiding MIDI note-on 0/note-off ambiguity. Values above 127 MIDI-relative units, nonfinite projection and underflow reject rather than clip.

Sustained changes produce ordered equal-time envelope anchors, an explicit step rather than an invented crescendo or an epsilon ramp. WebAudio's shared live/offline scheduler uses `setValueAtTime` for the second equal-time anchor. Seeking at a boundary uses the right-hand value. Printed dynamics, including sf/fp, cannot trigger a transient or decay when marked sound=false. Gate-only articulation, ties, rests and tempo changes remain usable.

For this first numeric profile, combinations with any sounding symbolic dynamic, hairpin, or attack-changing accent/strong-accent reject. Existing symbolic-only expression behavior remains unchanged. Wedge import and a complete explicit-versus-symbolic precedence model are separate work, not hidden approximations.

**Native sampled/offline rendering still rejects non-unit gain envelopes.** Exact fractional dynamics therefore remain a known native-renderer prerequisite, not evidence of completed sampled playback. The gain-compensated plan and existing built-in WebAudio PCM path support these values; no new native sound-library, MIDI or VST behavior is claimed. Sample-layer velocity quantization and actual instrument realism require independent device-level work.

## Supported display and explicit rejections

The narrow MusicXML display set is pppp, ppp, pp, p, mp, mf, f, ff, fff, ffff, sf, sfz and fp; exactly one symbol in exactly one direction-type. Optional direction placement and staff are typed. The existing preview draws sf without renaming it and accepts the sound flag only as a boolean; the flag changes playback ownership, not engraving.

Unknown direction fields, layout coordinates/fonts, compound `other-dynamics`, Leland Text private-use characters, sound repeat conditions, MIDI/device controls, pedals and wedges remain errors. Knowing a font or a private-use codepoint does not prove a semantic dynamic; this slice never interprets those words. Source errors produce no partial CLI conversion output. Shared nested JSON rejects unknown fields rather than dropping display information.

## Verification

```sh
cargo test --locked -p scorestream-formats --test dynamics
cargo test --locked -p scorestream-app playback::numeric
cargo test --workspace --locked
cd desktop && npm test && npm run build
```

Tests cover exact original-source numeric pairs in explicitly synthetic minimal fixtures, independent display/playback offsets, nested offset precedence, unassigned-initial/default-gap reports, display-only and unrelated-part stability, scoped agreement/conflicts, MusicXML/shared/SFS roundtrips, zero, held-note steps, triplet/tempo/tie/gate interactions, forbidden mixtures, native-range limits, non-finite projection, strict nested JSON and UI step scheduling. These are semantic/regression assertions, not cleaned-original imports or auditory quality acceptance.

Primary references: [MusicXML sound](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/sound/), [MIDI-compatible sound suggestions](https://www.w3.org/2021/06/musicxml40/tutorial/midi-compatible-part/), [offset](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/offset/), [dynamics](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/dynamics/), and [wedge](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/wedge/).
