# Local notation integration checkpoint

This is an isolated integration of two complete local checkpoints, not a claim
that notation coverage is finished and not a master, remote, PR or CI update.

Ordered source parents:

1. `d6041ed7dcaed6ddffd0924a4f4473e206c662a6`: notation ledger, already containing
   authored slurs `53dfbcefd3e7d34d95c312bc7575068143c6158e`.
2. `5555cfdd6769873ea62a7a321fa0ec6ed5c921df`: bounded simple event articulations.

Shared conflicts were resolved in the MusicXML reader, writer, schema boundary
and SFS reader by retaining both implementations. A note can contain slur
endpoints and supported articulation marks together. Export includes both in
its notations wrapper; SFS keeps both representations and their validation.
The separate articulation/slur implementation modules, existing Rust tests,
articulation snapshots and reference catalog remain byte-identical to their
source checkpoints. No source checkpoint was amended.

New joint tests cover:

- The synthetic two-staff articulation duet with authored slur endpoints added
  on the chord head and following note: JSON, SFS, MusicXML and canonical SFS
  retain both families, event identities and endpoint geometry.
- The same combined input still rejects ordinary playback with
  `slur.performance_unsupported`.
- A preview snapshot containing both supported articulation and authored slur
  still refuses preview rather than hiding the slur.
- A legal repeated-articulation-container grouping remains explicitly refused
  by the bounded profile. This does not call the MusicXML construct invalid.

The ledger now binds the existing four-symbol contract to exact executable
model/import/export/render/playback evidence. All 17 MusicXML articulation
choices are enumerated with their pinned declaration and schema type, a scope
class, incomplete-work reason and import evidence. Four choices remain partial;
13 remain blocked on import. Other layers of those 13 choices are not credited.
Unknown inventory and other unassessed domains remain visible.

## Verification on the combined source

Focused verification only, using an existing compact Rust target with debug
information and incremental compilation disabled. Frontend packages were reused
through dependency links while this worktree owns its test cache.

- Offline inventory/reference gate: passed; 5,041 reference entries, 22 bounded
  profiles, 70 exact evidence entries. The static gate does not execute behavior.
- Python integrity/false-claim/complete-choice inventory tests: 17 passed.
- Rust integration targets: 50 passed, 0 failed, 0 ignored:
  `articulations` 11, `authored_slurs` 7, `xml_slurs` 15,
  `notation_combination` 2, `qualified_fermata_playback` 15.
- Exact ledger evidence for `articulations` and `authored-slurs`: 33 passed.
  This reruns selected tests above plus exact frontend cases, and must not be
  added to the other totals as disjoint coverage.
- Frontend: 102 passed in four files: `articulations.test.ts` 12,
  `slur-fidelity.test.ts` 3, `accidental-appearance.test.ts` 84,
  `beam-fidelity.test.ts` 3. The articulation/accidental suites execute pinned
  Verovio 5.6.0 SVG assertions.
- TypeScript `tsc --noEmit`: passed.
- New joint Rust test formatting and Git whitespace check: passed.

Reproducible commands (run at repository root unless otherwise indicated):

```sh
python3 scripts/notation_coverage.py check
python3 -m unittest discover -s scripts/tests -p 'test_notation_coverage.py' -v
cargo test --offline --locked -p scorestream-formats \
  --test articulations --test authored_slurs --test xml_slurs \
  --test notation_combination --test qualified_fermata_playback
CARGO_NET_OFFLINE=true python3 scripts/notation_coverage.py test-evidence \
  --feature articulations --feature authored-slurs
(cd desktop && npm test -- tests/articulations.test.ts \
  tests/slur-fidelity.test.ts tests/accidental-appearance.test.ts \
  tests/beam-fidelity.test.ts)
(cd desktop && ./node_modules/.bin/tsc --noEmit)
rustfmt --edition 2024 --check crates/scorestream-formats/tests/notation_combination.rs
git diff --check
```

Not run for this combined checkpoint: full Rust workspace, full frontend suite,
workspace Clippy/format gate, production build, browser/manual visual QA,
external unchanged-score acceptance, CI, remote publication or a PR. Shared disk
had about 723 MB free after focused checks, so full Rust builds were deliberately
not attempted. Historical broader articulation results in
`ARTICULATION_IMPORT.md` are source-checkpoint results, not a combined full pass.
