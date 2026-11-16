# Complete Beethoven Fifth offline rehearsal

This is a 25:26 offline rehearsal from the four unedited imported scores, not
the desktop player's default and not an orchestral recording. All four use
the verified TimGM6mb bank. The selected timing and approximate instrument
mappings are rehearsal policies rather than assertions about the score.
Wedges, dashes, ornaments and slurs are not acoustically reconstructed.

| Movement | Score start in suite | Synthesis | Independent PCM frames |
| --- | ---: | --- | ---: |
| I | 0 | Per-note gain, native release through idle | 17,617,152 |
| II | 17,617,152 | Per-note gain, native release through idle | 10,720,832 |
| III | 28,337,984 | Attack-only approximation, selected release window | 12,016,000 |
| IV | 40,257,984 | Attack-only approximation, selected release window | 32,988,571 |

The whole decoded track has 73,246,555 stereo frames at 48 kHz. The III–IV
overlap follows the score clock, sums the existing 96,000 frames of III release
into IV, and adds neither spare frames nor a pause. See the [fourth movement
report](BEETHOVEN_FIFTH_IV_REHEARSAL.md).

## Upgraded first and second movements

I's original gain schedule and all 10,922 NoteOn/Off pairs were replayed.
Authoritative idle arrived at frame 17,617,152, 4.4 seconds after the last
native NoteOff at frame 17,405,952. The verified output WAV SHA-256 was
`7cc581ce380635a1f11100633079c661ac445f4ae5a3c52340d85e30d25a78cf`.
This is a different, more complete render than the earlier attack-only I.

II's 6,876 NoteOn/Off pairs and nine selected boundary-attack trajectories were
retained. The original two overlapping P16 viola A-flat attacks were admitted
to independent native lanes, not joined, discarded or shortened. Native idle
arrived at frame 10,720,832, 189,696 frames after the final native NoteOff.
The independently scanned WAV SHA-256 was
`de158a950c4e0e0bdde70c8b982712ed96ac6bc7bb63b3e5484349ee2f2b0ce1`.

These two WAV exports required transactionally equal readiness and fresh
render receipts. Source-clock positions and native 64-frame quantization are
recorded separately. This does not imply III and IV already have native-idle
or complete continuous expression proof.
