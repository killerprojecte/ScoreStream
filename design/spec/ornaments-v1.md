# Authored ornament bridge v1

`scorestream.ornaments@1` is a lossless, preserve-only notation capability. It
is not ornament realization, a wavy-line span model, or a generic XML container.

## Closed source shape

A measured note/rest owns one unadorned `notations` wrapper. Its nonempty,
unadorned `ornaments` groups retain child and group order. Supported children:

- empty `trill-mark`, no attributes;
- `tremolo type="single"`, integer strokes 1 through 4;
- empty `wavy-line type="start|stop"`, optional `number="1"`, optional exact
  finite-decimal signed `default-y`. Absence differs from explicit zero.

This is a bounded supported semantic profile, not a claim to cover all MusicXML
ornaments. Other tremolo types/stroke values, continue wavy lines, other numbers,
accidental-marks, turn/mordent/etc., styling, extra attributes/children, empty
ornament groups, grace ownership and multiple/adorned notation wrappers reject.
No unsupported child is silently discarded. No sequence is repaired or paired:
open, repeated and stop-before-start wavy endpoints remain authored endpoints.

## Typed ownership, topology and wire form

The existing ordered notation bridge (`ScoreDocument.tuplets`, `TupletNotation`)
is shared for compatibility. `TupletChild::Ornament` has a typed `Ornament` and
`group_index`. This internal field name does not imply that an ornament needs an
actual tuplet. Contiguous zero-based ornament group indices preserve grouping;
one group's children must be contiguous in the complete wrapper sequence.

All native marks on an ornament-bearing host use `scorestream.ornaments.child`,
with contiguous zero-based `index` at the measured owner's exact onset. Payload
`kind` is `trill_mark`, `tremolo`, `wavy_line`, or the existing `tuplet`,
`articulations`, `tied`, `slur`, `fermata` sibling. Ornament payloads require
`ornament_group`; tremolo requires `strokes`; wavy requires `phase` and optionally
`number` and rational `default_y`. Every variant has closed fields. Canonical
export requires the ornaments capability exactly when these records occur.
Actual tuplet endpoints additionally require `scorestream.tuplets@1`. Hosts
without ornaments keep the previous tuplet wire representation.

Sibling references must bijectively match existing ordinary ties, authored
slurs/fermatas and canonical-simple or authored articulation groups, including
existing mixed-notation topology. Canonical articulation remains canonical;
ornaments do not force a sound-blocking authored articulation mode. Duplicate,
missing, foreign, reordered group indices or conflicting native topology reject.

## Rendering and sound contract

Import reports preservation and explicitly reports omissions. Desktop retains
normal preview where its written-rhythm renderer is otherwise supported, warns
that ornaments are omitted, and does not interpret these marks as generic signs.
Existing unsupported rhythm/other semantic guards remain unchanged.

Playback uses the same unornamented note pitches, durations, ties and existing
articulation policies. No trill/tremolo attacks, tempo, velocity, wavy-line
pairing or playback subdivisions are synthesized. This is not a claim that the
performed ornaments are faithful. Unsupported independent performance families
still retain their existing checks.

## Evidence boundaries

The opt-in original receipt uses the four pinned `SCORESTREAM_MXL_FIXTURES`
MusicXML/MXL pairs, verifies byte equality and SHA-256, and tests all 1,569
ornament groups (I203 / II162 / III61 / IV1143) through a neutral-note family
projection and typed SFS/XML round trips. It separately retains the complete
original notation wrapper for all 103 tuplet+ornament hosts, including the one
articulation sibling, and verifies order and exact unornamented playback equality for 102 hosts.
One of those hosts has a positioned authored accent: its existing articulation
policy rejects playback with and without ornament metadata, rather than silently
downgrading the accent to canonical simple articulation. The child receipt is
20 trill marks, 1,549 single tremolos and 14 wavy endpoints.
Those projections are not a claim that the complete untouched files import.
Other independent original blockers remain and are measured separately.
