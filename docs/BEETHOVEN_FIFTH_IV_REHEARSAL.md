# Beethoven Fifth, movement IV: source-bound rehearsal

This documents an offline rehearsal, not the desktop player's default and not a
faithful orchestral recording. The original imported ScoreStream source was
unchanged (SHA-256 `39044fe2ed9a3d138b67b3c5d5db64794dfb64627d07258bc0854658017f71c9`).
The source-bound policy explicitly retains the missing endpoint receipt for the
final ending; it does not invent an ending marker in the score. The 112 actual
grace occurrences use the selected editorial grace allocation. Of their 80
principal groups, 48 have the selected staccato gate within the remaining
interval. The one selected 0.5-second hold is an editorial rehearsal choice.

The real TimGM6mb SoundFont (SHA-256
`c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854`) covered all
23 source routes. The render admitted 27,055 notes including 71 pizzicato attacks,
using 27 native routes, with no missing NoteOff and a peak of 138 native voices.
The selected attack-only rehearsal did not reproduce 27,055 per-note gain
envelopes: 27,051 constant and four time-varying. Wedge, dashes, ornaments and
slur are outside the selected acoustic review policy.

The WAV contained 32,988,571 PCM frames, stereo 48 kHz PCM16, including the
selected 96,000-frame release window. All frames were independently scanned;
peak −16.74 dBFS and no full-scale samples. Output SHA-256:
`e8b00b4db270184076f5ef3caa163e11f671d97990da30c1d278b0ac54d80079`.
The end of this selected window is not a proof that every native voice had
already become idle.

## III to IV without an inserted pause

The third movement's authored Attacca is in part P1 at 1177 quarter-note units.
Under the unchanged selected rehearsal tempo and holds its **written end** is
1180 quarter-note units, exactly 745/3 seconds or frame 11,920,000 at 48 kHz.
Its last scheduled NoteOff is frame 11,917,600, not the written boundary. All
23 fourth movement parts start at zero without initial rest or grace delay.

For the joined rehearsal, position IV frame zero at III frame 11,920,000 and
add III's 96,000 exported release frames to the matching opening frames of IV.
Keep the original samples before and after this overlap; do not cross-fade,
trim III's tail or add its standalone two-second window as a silent gap.
A sample-by-sample check verified both unaffected sections and every sum in the
overlap. The resulting 44,908,571-frame PCM16 rehearsal SHA-256 was
`43d9764be4d2208080ef3baf5d57ba78b2e408aabeb2d54830b338d08d551646`.
This is an offline join of the selected rehearsal versions. It does not claim
a new acoustic sustain or infer native oscillator state across the two files.
