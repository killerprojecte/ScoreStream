# Authored declared-part label details

Optional ScorePart.labels preserves plain unstyled part-abbreviation
and the verified part-name-display shape: exactly display-text,
accidental-text with value flat, then display-text. This fixed typed order
retains the three distinct tokens and all decoded text, including leading,
trailing and internal whitespace. No text prefix is parsed as a pitch, instrument
or transposition. An absent abbreviation differs from an explicitly empty one.

Labels are separate from existing part-name / SFS display_name, instrument
metadata (including instrument-abbreviation), catalog identity and routing.
Those fields and playback policy remain unchanged; native audio/plugin ABI code
is untouched. Additional label
rendering is not implemented: part_labels.authored_only makes the preview
boundary explicit instead of claiming font or layout fidelity.

Canonical SFS uses scorestream.part_labels@1: one closed
scorestream.part_labels mark per owning declared part at time zero. It contains
optional abbreviation and/or the complete name_leading_text, name_accidental,
name_trailing_text triple. Missing owners, other times, duplicate marks,
incomplete triples, undeclared capability and invalid XML text reject.

The profile intentionally rejects styling, nested markup, other accidental
tokens, other display sequences, part-abbreviation-display and later print-based
label changes. It does not flatten foreign XML or infer values from ordinary
part names. MusicXML export places name-display then abbreviation after part-name
in schema order; original lexical XML formatting is not preserved.

## Untouched originals

The ignored original_77_abbreviations_and_four_ordered_name_displays_roundtrip
test checks all four pinned original SHA-256 values. The labels-only projection
keeps original part ownership and existing part names, but replaces notes and
instrument declarations with explicit synthetic rest/piano fixtures. It is not
a claim that the entire original score imports or performs.

Counts: 18/18/18/23 abbreviations (77 total), plus four movement-I name displays.
Their accidental token is flat; exact suffixes include " Clarinet",
" Clarinet 2", " Horn 1" and " Horn 2 " (the last trailing space is retained).
Typed/SFS/XML equality and unchanged playback plans apart from diagnostics
are checked for each movement.

Run:
SCORESTREAM_MXL_FIXTURES=/path/to/originals cargo test -p scorestream-formats original_77_abbreviations --lib -- --ignored --nocapture

## Primary references

- [Part-name-display](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/part-name-display/)
- [Accidental-text](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/accidental-text/)
- [Part-abbreviation](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/part-abbreviation/)
- [MusicXML schema](https://www.w3.org/2021/06/musicxml40/listings/musicxml.xsd/)

MusicXML distinguishes ordinary part names/abbreviations from explicit display
text and accidental glyph sequences. These are labels, not instrument assignments.
