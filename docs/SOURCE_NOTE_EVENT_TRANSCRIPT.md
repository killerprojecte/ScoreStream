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

## Verified movement III extent discrepancy

A real untouched III XML→SFS→XML run at integration `07e94f4` retained every
per-measure note/voice transcript but changed encoded extents in all18 parts:
ordinal/label1: 1→3 quarters;141: 2→3;142: 1→3;162: 2→3. The source gives3/4
and omits `implicit`; the importer used nominal meter length and the exporter
added trailing forwards. The independent comparator now identifies the first
exact measure and both extents rather than merely reporting a part mismatch.
It does not discard the six extra encoded quarters as voice/division normalization.

MusicXML's [measure attributes](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/measure-partwise/)
describe `implicit` in terms of measure numbering, with pickups *typically*
implicit. Absence alone is not an independent proof that source cursor time
should be padded. A future explicit short-bar contract can preserve verified
source lengths without changing numbering or weakening ordinary conversion.

With the four exact short-bar contracts in
`crates/scorestream-cli/tests/fixtures/fifth-iii-irregular-measures.json`, subsequent
native CLI runs on both pinned III XML and MXL successfully imported and exported.
Their SFS bytes and exported XML bytes matched. The independent source/export
note-event comparator passed all18 parts in each run, including exact measure
extents. This is note-event preservation, not an audio/listening or complete
performance-equivalence claim.

A separate real movement-II SFS export showed the same class of discrepancy
only at ordinal248/label247:1quarter became3/2quarters in all18 parts; all
per-measure note lanes and the explicit implicit pickup were unchanged.
The II-only fixture selects that exact terminal bar without reclassifying the
pickup or inferring an automatic short-measure policy.

With that single exact II contract and the authored boundary-dynamics lane,
real pinned II XML and MXL independently imported/exported successfully. Both
independent note-event comparisons passed all18 parts; SFS bytes and exported
XML bytes matched across containers. The pickup retained1/2quarter and the
terminal bar retained1quarter. Boundary-dynamics expression/performance remains
subject to its own explicit policy, not certified by this note-only comparison.
