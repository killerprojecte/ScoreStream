# Authored part-list boundaries v1

`scorestream.part_groups@1` is a preserve-only ordered boundary bridge, not a
computed group tree. `ScoreDocument.part_groups` contains typed
`PartGroupBoundary {before_part, phase, number, symbol, barline}` values.
`before_part` names the next declared part; absence is the boundary after all
parts. This preserves stop-after-one-part and start-before-the-next placement,
including multiple ordered boundaries at the same gap.

[W3C part-group](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/part-group/)
places start before the first score-part and stop after the last, and defines
number as a token distinguishing overlapping/nested groups, not a sequential
ordinal. Absent number implies 1 but remains absent in this bridge. Explicit 1,
other canonical tokens and an explicitly empty token remain distinct; no number
renumbering or start/stop pairing is performed.

## Closed presentation profile

Start supports optional unstyled `group-symbol` with the actual source value
`bracket`, followed by optional unstyled `group-barline` yes/no. Stop is empty.
Symbol/barline absence is not replaced with a guessed default. Names, display
names, abbreviations, group-time, footnote/level, symbol styling, other symbols,
Mensurstrich and presentation on stop reject the whole conversion rather than
silently disappear. W3C says stop presentation may be ignored by consumers; this
bridge deliberately rejects it instead of discarding authored material.

Number accepts canonical XML-token text (no leading/trailing/doubled ASCII space
or XML tab/newline/CR) and valid XML characters. Noncanonical whitespace is
unsupported rather than silently normalized. The token is not interpreted as an
integer and no arbitrary numeric or string-length cap is added.

## Wire and validation

Each `scorestream.part_group` mark targets score at time zero in a complete score,
with required contiguous zero-based index and start/stop phase. Optional
before_part references an existing declared part; optional number is a quoted
string; optional symbol is bracket; optional barline is boolean. Fields are
closed. Declaration boundaries must be nondecreasing, but same-boundary authored
order is preserved. Start must precede an existing part; stop must follow at
least one part. Unsupported presentation, duplicate indices, unknown parts,
misordered boundaries, missing capability, wrong scope/time or fragments reject.
Part IDs are remapped with the native part declarations during SFS export.

The bridge does not fabricate a span/tree or repair overlapping, nested, unmatched
or repeated group numbers. Endpoint preservation is not a claim that malformed
source sequences form meaningful complete groups. Multi-staff part braces are
independent existing behavior and are not invented from these endpoints.

## Consumers and original receipt

The desktop warns that authored cross-part brackets and common-barline choices
are not drawn; otherwise supported preview remains unchanged. Playback and
instrument routing are identical with these metadata removed. No fake glyphs,
joined barlines, score hierarchy or sound behavior are introduced.

The ignored `original_part_group_boundaries_and_presentation_are_exact` test
verifies all four pinned hashes and literal XML/MXL equality. Boundary counts are
0/0/2/2. III brackets P14–P18, IV brackets P19–P23; both have common barline yes and
explicit number1 at start/stop. A neutral-note part-list family projection checks
all declaration IDs, boundary placements and group presentation/order through
typed Score/SFS/XML and compares playback. It is not full original-score import.
