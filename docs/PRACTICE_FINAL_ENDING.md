# Explicit practice final-ending continuation

The new core entry points
`discover_document_practice_joint_with_final_ending` and
`perform_document_practice_joint_with_final_ending` accept the existing typed
`UnclosedFinalEndingPolicy`. The ordinary practice functions still select Reject;
strict performance, ordinary playback and native/audio CLI defaults are unchanged.

ContinueToMovementEnd is permitted only inside the existing explicit practice
profile. It uses the same constrained repeat traversal as the trace: an unclosed
final ending must be the last navigation structure. No missing endpoint is added,
no arbitrary jump is accepted, and the source Document is retained unchanged.
Both practice receipts expose `final_ending_policy()` alongside their immutable
source reference. Performed diagnostics disclose
`practice.navigation_unclosed_final_ending_continues` and its repeat/start boundary.

This seam does not select a palette, waive source MIDI checks, change articulation
or other practice nonrealizations, invent grace allocations/hold seconds, or open
an audio device. A matching sealed native mixer/sample receipt is still required
for eventual output. Native callers need a separate explicit wiring step; existing
app/native entry points intentionally remain on their Reject-default wrapper.

## Verification

The synthetic open-ending fixture combines an authored grace, repeated principal,
fermata, articulation, and explicit bank0/program0 test route. Default practice
and strict projection reject. Explicit discovery and pure joint timing succeed,
retain the source and policy, and preserve the ordinary5-quarter route. Missing
routes and unknown source capabilities still reject. Closed-route and existing
qualified-joint tests remain unchanged and pass.

Initially, before a II–IV palette selection, the ignored original-source
negative test verified all three hashes and reported the real first discovery
guard: `practice.midi_selection: Require exact selected route for every source
part`. Authored one-based MIDI program alone does not establish a GM bank or a
sample-palette choice. No placeholder routes were supplied to get past that guard.
The subsequent explicitly selected probe below advances past that guard, without
claiming successfully compiled practice plans. No WAV, render, playback, or full
native preflight ran.

```
cargo test -p scorestream-formats --test practice_navigation --test qualified_joint_articulation --test sfs_navigation
SCORESTREAM_REHEARSAL_WORKSPACE=/path/to/verified/workspace cargo test -p scorestream-formats --test practice_navigation actual_rehearsal -- --ignored --nocapture
```


## Explicitly selected actual-source probe

After an explicit first-rehearsal selection of installed TimGM6mb bank0 as an
approximation, the installed bytes were rehashed to
`c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854`.
Actual RIFF preset headers provided unique names for every source MIDI program:
18/18/23 exact routes for II/III/IV. No program was inferred from a label. Named
devices/noncentered pan were checked and absent; original part, instrument,
volume, pan, channel and port metadata were retained. This evidence is not a
sealed native mixer or bank receipt.

The `probe_practice` example binds supplied route/timing drafts to the complete
source SHA, then independently attempts core discovery and pure joint timing.
It contains no synthesis or audio-device API. Actual first errors were:

- II: `authored_dynamics.performance_required` for retained distinct terminal/opening
  dynamic owners, in both discovery and timing.
- III: `articulation.performance_capability`, in both discovery and timing. The
  current profile's closed family list does not include `part_groups@1`.
- IV: the same capability guard; `part_groups@1` and `tuplets@1` are outside that
  profile. The earlier guard prevents the actual source from reaching traversal.

No capability was stripped, reclassified as passive, or musically reinterpreted
just to pass these gates. The real timing drafts therefore remain proposals, not
compiled plans. The explicit ending seam itself is verified by the targeted
source-preserving synthetic test above.

```
cargo run -p scorestream-formats --example probe_practice -- source.sfs selected.json
cargo run -p scorestream-formats --example probe_practice -- IV.sfs IV-selected.json continue-to-movement-end
```
