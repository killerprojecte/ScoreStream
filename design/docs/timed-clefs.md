# Source-backed interior clefs

`ScoreMeasure.clef_changes` retains exact rational local offset, staff and classic
G/F/C sign/line separately from measure-start state. Native SFS uses the existing
ordinary `= part.sN clef=F4` at the exact absolute frame time, not a new extension.
Offsets must be strictly inside the measure, ordered by (offset, staff), unique.
Pitch, rest identity, event onset/duration, written rhythm and ties are untouched.
Other interior state (key, meter, divisions, transpose) remains unsupported.

The MusicXML profile accepts a clef-only attributes block on one staff/voice,
with an unambiguous positive cursor. Following a backup it accepts only the exact
immediate unadorned backup/clef/forward excursion, equal durations and restored
furthest cursor. It does not guess multi-voice/staff state, relocate changes to
measure start, or split sustained notes. Unknown clef presentation still fails.

The [W3C attributes reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/attributes/)
defines mid-measure state in score order, not MusicXML document order. Export
therefore emits exact closed excursions after the measure's voice, restoring its
end cursor each time. Divisions include the exact clef offset denominator.
Document placement is normalized; musical position is not. Explicit/omitted
clef staff number follows the existing normalized staff identity convention.

Preview currently refuses timed-clef engraving with a specific message, rather
than displaying the wrong clef or claiming fidelity. This rendering boundary does
not prevent source/SFS roundtrip or musical audio. General mid-score preview
limitations are otherwise unchanged.

## Pinned source receipt

`audit_beethoven_timed_clefs.py` checks all four original SHA-256 hashes and exact
XML/MXL root byte parity, audits every cursor, and extracts 62 timing/pitch/rest/
written-rhythm/clef projections: 22/9/12/19. The two IV P8/P9 ordinal269 cases use
closed backup/forward excursions at 5/2; IV P22 also exercises thirds. Projection
omits unrelated engraving, directions, ties, and notations; it is explicitly not
whole-score acceptance. Opt-in `timed_clefs` test compares every owner, offset,
sign and line, SFS/XML event equality and unchanged playback with the clef lane
removed. Separate tests verify tied continuation/no reattack and unsupported
mixed state/ambiguous cursor rejection. Whole original imports remain a separate
integration receipt, and other unsupported families may still block them.
