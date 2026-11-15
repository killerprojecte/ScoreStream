# Authored beam spelling

This bounded conversion profile retains direct MusicXML note/beam records through
ScoreDocument and SFS 1.1. It does not infer beams from meter or rewrite notes to
fit grouping. It is not full MusicXML, full Beethoven import, or sampled-audio
acceptance.

## Typed contract

ScoreDocument.beams is an optional, omitted-when-empty collection of ScoreBeam:

- event: the owning measured note/rest, supported chord head or grace event identity.
- number: optional integer 1 through 8. Absence and explicit 1 remain distinct.
- value: begin, continue, end, forward hook, or backward hook.

SFS uses the registered required capability scorestream.beam@1 and per-event
scorestream.beam marks at each event's exact onset. SFS uses the atom spellings
forward-hook and backward-hook; XML uses its defined spaced spellings. Grace marks target the grace
identity, not the principal, although both share the principal's logical onset.
Native legacy beam relations remain unchanged. Combining legacy beam relations
with this authored profile is rejected rather than coalescing conflicting models.

The source number identifies a beam level, not a group ID. Group topology is
validated from the complete event stream in an actual measure, voice and effective
staff. Grace groups additionally retain their principal and explicit order.
Graphic cue size does not make a note a silent cue or alter its clock.

## Deliberate limits

- Runs must be complete and contiguous within one measure and lane. Unmarked
  intervening notes/rests, gaps, simultaneous ambiguous events, cross-staff or
  cross-measure runs fail instead of being repaired.
- Each event has at most one record per effective level. Secondary connections
  need lower-level support. Secondary hooks retain their authored direction as
  local partial beams, not inferred connections to neighboring stems.
- Every lower level must be present on a stem bearing a higher-level mark.
  Written rhythm must support the declared level.
- Ordinary measured pitched chords are supported only when all members share
  positive actual duration, explicit written type, dot count, time-modification
  (including normal notation), graphic size, actual measure, onset, voice and
  effective staff. Stems must all be absent or all the same authored up/down value.
- The first declared member is the head. Only that head may carry beam levels.
  Ordered member events, pitches, accidentals, ties and other independently
  supported metadata remain separate; nothing is copied from a head to a member.
  Only the topology view collapses checked members to a single rhythmic anchor.
  XML/SFS/canonical/JSON conversion preserves the authoritative member order.
- Singleton, reused, interleaved or ambiguous chord groups; split/member beams;
  mixed-duration, cross-staff and grace chords remain outside this profile.
  Shorter chord members are legal MusicXML but deliberately unsupported here.
- Primary hooks, silent cues, unpitched events, spaces and full-measure rests are
  outside this profile. Beamed graces still require a non-chord principal.
- Only number and the five role values are mapped. Unknown descendants and
  attributes, including explicit fan, repeater, color and beam identity, remain
  blocking. They are never silently defaulted or classified as geometry.
- All other import boundaries stay strict. Existing coordinate regeneration is
  neither expanded nor selected automatically.

XML export emits beams after staff and before notations on measured and grace
notes. SFS conversion remaps targets through the same stable identity map as the
events. Missing and explicit default levels survive XML, SFS, canonical and JSON
round trips.

Validated beam metadata has no performance effect. Ordinary and existing qualified
navigation/grace/fermata pathways retain their separate policy requirements and
MIDI-intent guards. Beam registration does not authorize grace allocation, fermata
holds, tremolo, slurs, external routing or sample assignment overrides.

The current preview deliberately withholds this new authored-beam profile before
engraving. Automatic engraving is not evidence that authored runs, breaks and
hooks have been preserved. A later projection needs its own pinned-renderer tests.

## Original-source receipts

An ignored regression reads the untouched original MusicXML files and each MXL
root, verifies their pinned fingerprints and equality, then exercises only the
beam field decoder on the actual source nodes. It does not strip other notation,
return a partial score, or claim complete import.

| Movement | Beam records | Owning notes | Beamed rests | Beamed graces |
| --- | ---: | ---: | ---: | ---: |
| I | 3,855 | 3,851 | 0 | 0 |
| II | 10,841 | 4,931 | 36 | 6 |
| III | 1,246 | 998 | 0 | 124 |
| IV | 11,046 | 9,056 | 298 | 64 |
| Total | 26,988 | 18,836 | 334 | 194 |

Independent source inventory found 8,656 complete per-level runs, including 6,258
primary groups, and 801 hooks. Some beamed events are chord heads: an independent
scan found 623 beamed chord heads with 648 additional chord-member notes: 598 dyads
and 25 triads. No member itself carries a beam. These heads carry 931 beam records.
All 623 groups satisfy the new same-rhythm/staff/stem chord field profile, including
131 with 3:2 time-modification and 25 dotted heads. Dot geometry is not rhythm;
unsupported dot coordinates remain blocking. Independent member accidentals and
ties are retained, not inferred from the head. Per-movement head/member counts are
117/117, 196/216, 46/48 and 264/267. These receipts check source fields, not complete
source acceptance. An exact raw Movement I/P14/measure 55 fragment, with its original
inherited attributes restored in a separate contextual measure, tests conversion
of all notes and its three chord heads under the existing explicit coordinate
regeneration policy. Strict import still rejects its coordinates. This fragment
is not a whole-movement substitute or a claim of preserved source geometry.

Six actual Movement II notes (P16/P17/P18,
measure 237 note 3 and measure 238 note 2) carry a forward level-3 hook where
level 2 ends. The typed representation retains this direction; it does not invent
a level-2 forward connection. Beams do not make the originals pass the remaining
header, print, articulation, ornament, direction and slur requirements.

## Definitions

[MusicXML 4.0 beam](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/beam/)
defines eight levels, the five roles, default level one, and grouping by voice and
presence of grace/cue elements. The supported numeric and role subset is explicit;
this document is not permission to accept all attributes of that element.

[MusicXML 4.0 chord](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/chord/)
defines an additional tone and leaves timeline advancement to the preceding
non-chord head. The format permits shorter member durations, but this bounded
beam profile requires equality rather than guessing a shared stem.
[MusicXML 4.0 stem](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/stem/)
distinguishes up, down, none and double; the last two remain unsupported for
these authored chord-head beams.
