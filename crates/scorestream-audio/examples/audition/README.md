# Experimental local audition harness

This is an operable Linux-only development example in the **isolated**
`scorestream-audio` workspace, not desktop UI or a default playback command.
It reads authored SFS without rewriting it, compiles explicit policies, checks a
caller-selected palette and renders through the already-installed native backend.
No package, bank, plug-in or sample is downloaded. No complete Beethoven import
or exact orchestral realism is claimed.

## Build and run

From the repository, with the installed Rust toolchain on PATH:

```sh
cargo build --manifest-path crates/scorestream-audio/Cargo.toml \
  --locked --offline --example audition -j2
```

The binary is normally `crates/scorestream-audio/target/debug/examples/audition`,
or `$CARGO_TARGET_DIR/debug/examples/audition` if that variable was set. The
exact path is also Cargo's `compiler-artifact.executable` with
`--message-format=json`. **Run that binary directly**, not `cargo run`: Cargo
injects `LD_LIBRARY_PATH`, which the native loader deliberately rejects. Do not
remove or override the loader checks. Use the normal unchanged launch environment.

```sh
AUDITION=/actual/build/path/debug/examples/audition
CASE=crates/scorestream-audio/examples/audition

"$AUDITION" trace-route "$CASE/fixture.sfs"
"$AUDITION" discover-joint "$CASE/fixture.sfs" simple_staccato_accent_v1
"$AUDITION" preflight "$CASE/fixture.sfs" "$CASE/fixture.json"
"$AUDITION" render "$CASE/fixture.sfs" "$CASE/fixture.json" \
  /usr/share/sounds/sf2/TimGM6mb.sf2 /a/new/output.wav
```

The final command refuses any existing output, including a symlink. It verifies
bank bytes and actual native preset names before PCM, then publishes an atomic
PCM16 stereo WAV. Failure returns nonzero and no new final WAV. JSON success
output contains source SHA, backend/bank evidence, exact route/preset/channel
assignments, warnings, approximation labels, frame counts, clipping/peak and tail
status. Partial temporary streams are never presented as a completed output.

The bundled **synthetic** SFS is adapted from the repository's
`joint-articulation-playback.musicxml` fixture, not from Beethoven's score. It has
an explicit violin route, staccato, a short rest anchor synchronized to a later
held note release, one authored grace and an explicit pizzicato command before
the grace. The configuration deliberately chooses a 1/4-quarter grace, one added
second at quarter 3, and the shared GM pizzicato approximation. These choices are
fixture editorial decisions, never defaults offered for other scores. Five
attacks occupy a three-second score; the explicitly selected two-second final
tail makes a five-second WAV. The silent rest-anchor part needs no audio palette
entry. Source labels never choose a patch.

## Configuration contract

`fixture.json` is a complete concrete example. Every field is required; unknown
fields, enumeration names and partial policies are rejected.

- `source_sha256` binds **exact SFS bytes**, not a path or approximate score match.
  A changed source requires new discovery and an explicitly reviewed policy.
- `bank_sha256` plus `palette` choose bank/program (zero-based) and exact native
  preset names for every audible `(part,instrument)`. There is no name-to-program
  inference; arbitrary extra routes fail the native contract.
- `timing.kind` is `ordinary` or `joint`. Ordinary is not a fallback for unsupported
  authored features. Joint explicitly selects the named articulation and sound
  policies, exact grace allocations and complete synchronized hold groups.
- `discover-joint` requires the explicit articulation policy argument and reports
  performed occurrence IDs, original IDs, authored grace timing and exact quarter
  coordinates. It does **not** choose allocations, grouping, seconds or palettes.
  Do not reuse written IDs as performed IDs or infer hold duration from a glyph.
- Grace allocation is either `authored_following_percent` (the core validates its
  authored prerequisites) or `explicit_following_quarters` with exact member IDs
  and rational-string quarters. Hold `boundary` and `added_seconds` are rational
  strings. The qualified compiler checks identities, complete consumption and
  musical compatibility; no detached schedule bypass exists.
- `pizzicato` is `reject_unresolved` or
  `installed_tim_gm6mb_shared_strings_v1`. The latter requires the reviewed local
  bank hash, exact Pizzicato preset and supported key/identity profile documented
  in [PIZZICATO.md](../../PIZZICATO.md). It is not an exact cello/bass recording.
- Rendering rate, block size, gain, final tail, polyphony and input capacities are
  explicit. There are no hidden audition-specific source byte/event/duration caps.
  The example's 100-note/256-voice limits are deliberate small-fixture choices,
  not production limits; select and review appropriate values for another score.
- Preflight is pure compiler/route/channel validation, **not a claim that the
  supplied bank exists or passed native checks**. It reports `native_bank_verified:
  false`. Render performs those remaining checks. Compiler-omitted sound is an
  error in this harness, not silently accepted as a faithful audition.

## Current full-work blockers and transparent next steps

These are existing renderer limits, not claims that all are present in a fully
imported Fifth. Source-qualified compilation may fail earlier on unsupported
notations, incomplete explicit policies, cross-jump ties or timing ambiguity.

1. **Independent per-note gain envelopes:** native currently accepts unity only.
   A numeric dynamic change during an already sounding note can produce a genuine
   non-unity envelope and fails. The test demonstrates this with authored numeric
   dynamics. Next work must implement independently controlled voices/channels or
   another verified per-note gain renderer; do not replace the curve with one
   velocity or shared CC gain that changes unrelated voices.
2. **Same MIDI key overlapping in the same route/technique lane:** fails, because
   one channel's note-off cannot safely identify separate held instances. The test
   demonstrates it. A future interval-colored subchannel allocator must preserve
   original IDs, note-off ownership, patch state and capacity accounting. The
   existing bowed/pizzicato separation does not solve same-lane collisions.
3. **Microtones:** only integer MIDI keys with matching equal-tempered frequency
   are admitted; no rounding. Independent tuning would need an explicit supported
   renderer contract.
4. **Palette/coverage:** every audible route needs explicit known presets in one
   existing bank. Pizzicato is currently the reviewed shared-string approximation,
   key 0–108. A named GM patch alone is not proof of convincing range/timbre or
   independent sampled orchestral instruments. No unsupported percussion map is
   inferred, and compiler-omitted notes prevent audition.
5. **Native capacity and clock:** at most 256 native channels including technique
   lanes, selected voice budget, internal event quantization (64 frames on the
   tested runtime), explicit fixed final tail, standard RIFF's 4 GiB limit.
   Tail flags are evidence, not proof every voice finished. A future full-work WAV
   may require RF64 or a separately declared output container, not truncation.

Do not loosen these checks to make a full-score command appear successful. First
produce the unchanged full SFS, discover its actual performed route, select exact
policies/palette, and run preflight. Work on the first observed semantic or native
blocker with a source-qualified regression. The example is already reusable for
supported sources; whole-work fidelity still needs those independent milestones.

## Non-playable route trace versus qualified sound discovery

`trace-route source.sfs` validates the complete unchanged source and reuses the
closed navigation reconciler/traversal. It returns written-event and repeat-visit
counts, exact fermata occurrence IDs/quarter intervals, capability inventories
and disclosed navigation conventions. It returns **no resolved score or playback
plan**, does not choose hold seconds, and cannot authorize sound. In particular,
slurs, wedges, dashes and ornaments remain in the source and capability inventory.
A successful trace does not certify tie realization across jumps, gain envelopes,
articulation semantics, pitches, timbres or rendering.

`discover-joint` is the stricter policy-entry route. It now accepts an actually
grace-free source without inventing `grace@1` or a grace event. For actual joint
compilation such a source must receive an explicitly empty grace-group policy;
a nonempty invented allocation still fails. The same qualified hold clock is
used. Typed passive provenance, metronome presentation, part labels, defaults,
credits, lyric text, print layout and geometry may coexist, with a retained
passive-receipt disclosure. This does not turn a printed metronome into sound tempo.

See [qualified route audit](../../../../docs/QUALIFIED_ROUTE_CAPABILITIES.md) for
the individual family decisions and the observed original Beethoven I trace.

### Explicit practice review, not render permission

```
audition review-practice source.sfs written_notes_without_wedge_dashes_ornaments_slur_v1
```

This closed opt-in produces a durable `scorestream.practice-review.v1` JSON report
bound to the exact SFS SHA. It validates the complete source and route, retains
original typed instrument metadata (including external channel/port, one-based
program, exact volume percent and pan degrees), and names all four deliberately
unrepresented acoustic behaviors. Unknown policy names and malformed source fail.
The report is **not** a playable plan, a changed score, or an acoustic qualifier.
`contract_renderable` and `playback_plan_created` are always false. It also records
the actual strict joint-discovery errors and independently checks the existing
MIDI-intent guard; that latter observation is not a claim that all preceding
performance checks have passed.

The unchanged movement-I source SHA
`ffac34c86e31df50b028ca7afe0e91f822c2b122af28b55c5e0c1ba10769f77b`
contains 18 authored MIDI assignments, each with volume `393701/5000` and pan `0`.
The next route obstacle is `instrument.performance_unsupported`, not missing
notes or a file-size ceiling. Merely choosing a bank/preset does not realize the
source mixer intent. P11/P12 have no source port; absence must not become port 1.

A subsequent sealed local-MIDI receipt must separate external addresses from
internal channels, compare source programs against explicit selected presets,
and disclose any volume/pan quantization. A subsequent practice compiler must
carry each nonrealization into its immutable result and final native report,
retain the original source, and reuse existing joint articulation/grace/fermata
qualification without guessed holds. No such permissive compiler or render
profile is introduced by this review command. Native overlap, independent gain
envelope, sample coverage, tie/clock and output checks remain unresolved until a
real plan reaches them. Do not describe a review report as a faithful rendition.
