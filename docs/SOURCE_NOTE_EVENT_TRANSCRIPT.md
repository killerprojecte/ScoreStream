# Independent source note-event acceptance check

`python3 scripts/compare_musicxml_transcript.py original.musicxml exported.musicxml`
compares two XML documents independently of the Rust typed model. It does not
replace successful conversion, listen to audio, or claim full performance fidelity.
The original must remain unchanged. To run only after successful authentic imports:

```
python3 scripts/check_beethoven_import.py /path/to/scorestream /path/to/originals \
  --movement-i-contract crates/scorestream-cli/tests/fixtures/fifth-i-irregular-measures.json \
  --compare-note-events
```

The pinned gate verifies original XML/MXL payload hashes and import parity first.
Only successful imports with nonempty SFS and untruncated diagnostics are exported
using the same explicit contract. Failed imports report `not_run_import_failed`;
they cannot become a transcript pass. Export failures and unresolved comparison
structures are likewise nonpasses. This opt-in gate succeeds only when every
movement imports and its transcript compares equal.

## What is compared

- Declared part order, measure ordinal/label and encoded measure extent.
- Exact rational quarter-note onsets and durations derived from divisions,
  backup and forward, including division changes; no floating-point tolerance.
- Rest events and pitched note groups. Chord membership is explicit, while order
  of pitches inside a chord is irrelevant. Each member retains its own duration.
- Actual chromatic pitch from step/octave/alter plus explicit chromatic/octave
  transposition. Accidental display glyphs and enharmonic spelling do not set
  pitch; this is not a written-spelling fidelity check.
- Ordered pre-principal grace groups and chord members, with presence and exact
  values of steal-time-previous/following and make-time (division-normalized).
  It does not invent realized grace durations. Slash engraving is not timing.
- Sound `tie` start/stop endpoints, separately from engraving `tied` notation.
  Tie endpoints are preserved, not interpreted as new retriggered attacks.
- Staff membership. Voice strings may change: complete per-part voice histories
  are compared as a multiset, with measure positions and staff inside each lane.
  A lane cannot be split, merged or reallocated without failing comparison.
  Identical competing lanes are explicitly ambiguous rather than guessed.

Lane serialization order and division scaling may differ. Chord heads must have
matching adjacent voice/grace context. Unattached grace groups, grace principals
that are rests, conflicting same-onset attacks in one lane, cue notes, unpitched
routing, double transposition and unsupported sound tie attributes fail with
source context. No event-count or file-size ceiling is added.

This is deliberately not complete audible equivalence: tempo/sound-dynamics,
articulation, instrument routing, repeat traversal, pedal, fermata realization,
notation styling and synthesis/audio are outside this transcript. Importing and
retaining those features still needs its separate gates. A changed excluded
feature may therefore compare equal here, never proving full performance equality.

## Evidence

Compact tests cover changed division scaling, voice labels and lane order;
intentional pitch, duration, tie, chord, grace timing and forward-extent corruption;
accidental glyph independence; transposition; contextual ambiguity; and the
successful-import-only harness condition.

The unchanged four pinned originals parse into transcripts with 18/18/18/23 parts
and 18/22/19/28 voice lanes. This verifies source-side transcript coverage only.
No successful original→SFS→XML comparison is claimed while real import is blocked.
