# Beethoven Fifth, I: local rehearsal render receipt

The unchanged, verified movement-I SFS was compiled and rendered from beginning to end with the isolated Linux ScoreStream audition example. This is a **rehearsal interpretation**, not a complete symphony render or a fidelity claim.

- Source SFS SHA-256: `ffac34c86e31df50b028ca7afe0e91f822c2b122af28b55c5e0c1ba10769f77b`.
- Installed TimGM6mb SHA-256: `c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854`.
- 10,922 audible attacks; 38 used the explicit shared GM pizzicato approximation. 18 source mixer routes were verified against the installed bank.
- Plan: 362.922926 seconds. WAV: 17,516,300 stereo PCM16 frames at 48 kHz, 364.922917 seconds including the selected two-second tail. Native report counted zero NoteOffs without live voices; tail truncation was not reported.
- Raw WAV SHA-256: `8d4e80f28b94a1fe768a5ef94552930cd12361e0624d545c2da73a8b985693c3`. The WAV and source bank are not included in this repository. The listening MP3 was made with a uniform level increase; source release and onset times were not changed in that copy.

The source-qualified rehearsal adds a half-second at each of 13 selected release groups and a quarter-second at each of the three distinct cadenza releases. These choices total 7.25 seconds and are not authored hold durations. Plain notes that end exactly at an unselected hold boundary release before it; sound ties remain one attack.

The opt-in native output realizes MIDI attack velocities, not independent per-note gain. The retained plan has 10,877 constant non-unit multipliers near unity and eight changing step envelopes, which are not applied to the rehearsal PCM. The report also identifies 904 fractional NoteOn residuals. The separate practice policy discloses unrepresented wedge/dashes/ornament/slur effects. The original notation and full compiler trajectories remain available; the ordinary renderer still does not silently drop them.

See [the explicit native rehearsal policy](../crates/scorestream-audio/ATTACK_REHEARSAL.md), [the local audition harness](../crates/scorestream-audio/examples/audition/README.md), and [the independent four-movement score round-trip](BEETHOVEN_FIFTH_ROUNDTRIP.md).
