# Authored note visibility

Direct MusicXML `note/@print-object` now maps to the existing SFS event boolean
`visible` and the optional typed `ScoreEvent.visible` / `ScoreGraceNote.visible`.
Absent, explicit `yes`, and explicit `no` remain distinct through common-model
JSON, SFS and canonical MusicXML export. Existing JSON that omits the field is
unchanged. Notes, rests, chord members and grace notes retain their own owners.

The [official MusicXML note reference](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/note/)
defines this as printing visibility, with yes as the effective omitted default.
The [yes-no datatype](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/yes-no/) derives from XML token: surrounding XML whitespace is
normalized, but non-XML Unicode whitespace and malformed values are rejected.
It separately defines spacing and printing controls for dots, lyrics and leger
lines. This bridge does not infer or accept those separate controls: explicit
`print-spacing`, `print-dot`, `print-lyric`, `print-leger`, malformed yes/no values,
unknown print fields and dot/accidental child `print-object` still reject rather
than silently drop data. Note visibility does not rewrite accidental records,
augmentation dots, full-rest identity, pitch, duration, velocity or grace timing.

Hidden pitched notes still sound. Hidden rests still occupy their exact time.
No visibility option is translated into a spacer or a mute. Tests compare the
complete playback plans with visibility absent, true and false. Authored grace
performance remains subject to its unchanged explicit unsupported-policy error.

## Preview boundary

The current desktop preview **shows hidden events** and explicitly warns that
print-object visibility is not reproduced. It retains source and event IDs and
uses the same MEI timing/pitches as the visible baseline. No visual fidelity is
claimed. A single renderer attribute is not treated as proven support across
shared chord stems/dots, separately projected controls, or grace layout. Existing
unsupported engraving checks remain in force.

## Original evidence and tests

`python3 scripts/audit_beethoven_note_visibility.py <originals-directory>` checks
the original SHA-256 pins shared with the navigation audit. It finds 80 explicit
`print-object="no"` attributes: 73 in movement I and 7 in III, none in II/IV, none
on other element owners. All 80 are authored rests, have no explicit
`print-spacing`, and remain events rather than deleted intervals.

- `cargo test -p scorestream-formats --test note_visibility`: seven focused cases,
  covering tri-state round trips, normal notes/rests/chord members/graces, visual
  independence from accidentals/dots, exact playback equality, invalid native
  booleans, strict unsupported-field boundaries, and real original rest samples.
- `SCORESTREAM_MXL_FIXTURES=<originals-directory> cargo test -p scorestream-formats
  --lib original_beethoven_stems_and_full_rests_are_exact_independent_fields --
  --ignored`: pinned production XML decoder audit of all 68,559 original notes,
  including the 80 hidden rests and their unchanged positive durations.
- `desktop/tests/note-visibility.test.ts`: explicit warning, unchanged MEI,
  identities/source, and separate mixed-chord visibility ownership.

This field-family bridge is not a claim that any full original score imports:
unrelated unsupported structures remain independent blockers.

On this baseline, the real CLI's four untouched XML/MXL pairs all retain identical
XML/MXL diagnostics and zero partial SFS output. No `attribute.print-object`
diagnostic remains. Counts are I: 248, II: 197, III: 102 (not truncated), IV: 1024
(truncated). Most remaining diagnostics are unrelated ornaments; therefore full
original-score acceptance still fails intentionally.
