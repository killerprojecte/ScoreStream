# Authored common and cut time display

`Meter.symbol: Option<TimeSymbol>` retains the authored `common` or `cut`
MusicXML time-signature display separately from `beats` and `beat_type`.
Absence is distinct from either explicit symbol. Numeric meter, nominal measure
length, actual event durations and performance timing do not change.

[W3C MusicXML 4.0 time-symbol](https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/time-symbol/)
defines a display choice, with ordinary numeric display implicit when omitted.
This implementation intentionally supports only common with exactly 4/4 and cut
with exactly 2/2, the shapes present in the pinned originals. That pairing
restriction is this implementation's supported profile, not a claim that W3C
requires it. Explicit normal, single-number, note/dotted-note, mismatched numeric
pairs, extra display attributes, composite/interchangeable time and mid-measure
time changes remain unsupported rather than inferred or silently rewritten.

## Native form and ownership

`= part.s1 meter=4/4 meter_symbol=common` and
`= part.s1 meter=2/2 meter_symbol=cut` are exact staff-state assignments.
`meter_symbol` must occur in the same state record as the matching numeric meter;
it cannot independently mutate a previously assigned meter or live at part,
voice or score scope. At one staff/onset, conflicting symbol values or conflicting
presence across duplicate meter records reject. A new meter assignment without
the companion explicitly has no authored display symbol; it does not inherit a
stale symbol. XML and SFS preserve this absence/reset behavior.

The existing reader applies an unnumbered MusicXML time node to all current
staves of its owning part. Numbered time nodes retain their selected staff.
Measure-start ownership and numeric-meter changes use the existing strict state
mapping. Irregular extent policy compares the numeric meter independently of
this display metadata; a displayed common sign cannot resize a measure.

## Preview and playback

The current preview shows numeric meter and explicitly warns that the authored
common/cut glyph is not drawn. It does not hide the normal initial preview solely
because this display metadata is present. Its existing mid-score state-change
limitation is unchanged and remains an explicit error. No glyph-rendering or
mid-score engraving coverage is claimed. Playback ignores the display field and
uses the existing numeric/actual timing, with equality regression tests.

## Evidence

The opt-in `original_69_symbols_preserve_part_measure_staff_onset_and_meter`
receipt verifies all four pinned originals' SHA-256 and XML/MXL byte equality.
Only IV contains symbols: 46 common and 23 cut, across 23 parts. Per part, common
occurs at measures 1 and 209, cut at measure 364. All are unnumbered, measure-start
nodes. The intervening numeric 3/4 at measure 155 has no symbol.

A clearly labeled meter-family projection retains all 92 original meter states,
part/measure/staff ownership, and exact onsets 0, 616, 778, 1398 quarters in a
neutral-note timeline. Typed Score/SFS/XML round trips retain numeric state and
symbol presence, and playback is compared with symbols removed. Other original
note/direction content is not claimed imported by that projection. The untouched
original import report is run separately and may still fail on other families.
