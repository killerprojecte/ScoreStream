# Explicit final-ending propagation to an audition plan

The old `plan_source_with_practice_joint_policies_and_pizzicato` function still
selects `Reject`. The new
`plan_source_with_practice_joint_policies_pizzicato_and_final_ending` function
adds only an explicit `UnclosedFinalEndingPolicy` argument and delegates to the
already qualified core practice implementation. It never inserts or rewrites
an authored ending endpoint.

The opaque, source-hashed `TechniquePlaybackPlan` retains a
`PracticeFinalEndingReceipt`: exact source digest, selected policy name and the
original missing-endpoint performance disclosures. These also remain in the
PlaybackPlan diagnostics, with the selected policy attached. The source and
performed occurrence clock are not substituted.

The experimental audition JSON accepts an optional top-level `final_ending`:
`reject` (also the absent-field default) or `continue_to_movement_end`. The latter
is accepted only on the authenticated practice joint path with a matching sealed
local MIDI mixer and the existing explicit technique interpretation. Ordinary
and strict commands cannot use this option as a bypass. Unknown names reject.
The practice preflight report includes `final_ending_receipt`.

The same CLI now exposes the separately selected grace policy
`practice_principal_staccato_accent_in_allocated_remainder_v1`. Neither option is
a product default. Existing configuration files require no changes.

A successful compiler/preflight result is not proof of soundbank loading,
capacity throughout the whole synth render, or a completed WAV. These remain
separate checks. No renderer/backend behavior changes in this patch.
