# Explicit practice numeric accent composition

Only the existing qualified joint-practice compiler, with its explicit SimpleStaccatoAccentV1 articulation choice and named note-expression policy, enables this composition. Ordinary/default compilation still rejects numeric sound dynamics combined with accents. Sounding symbolic dynamics, legacy hairpins, and strong accents remain unsupported. No score data is changed.

This is an editorial rehearsal interpretation: a fresh simple-accent attack adds 16 velocity steps to the current numeric baseline, clamped at 127. A numeric mute remains muted. The initial envelope starts at unity and returns to the exact numeric baseline within the minimum of 0.08 seconds, one quarter of the sounding duration, and the first existing numeric-change offset. Subsequent paired numeric steps retain their exact offsets and gains. An accent on a tied continuation does not manufacture a new attack. An accented event with its own authored NoteOn dynamics is rejected rather than assigning ambiguous precedence.

Focused numeric tests cover unchanged strict rejection and existing numeric semantics, preserved paired steps including decay coincident with a step, short duration, saturation, zero mute, and NoteOn conflicts. Seventeen targeted tests pass. Actual III is probed independently using its unchanged SFS and previously selected route/timing inputs; pure unit coverage is not a claim of rendering fidelity.

The attack-only offline approximation may separately omit sustained envelope evolution, including this decay. Its sealed omission receipt remains required; this policy does not claim the attack-only bank renderer delivers the full numeric sustain trajectory.
