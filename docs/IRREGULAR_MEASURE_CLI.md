# Exact irregular measure contracts in the CLI

The default converter remains strict about overfull bars. For an intentionally
irregular bar backed by exact source evidence, append:

```
--musicxml-irregular-measures contracts.json
```

This optional suffix works on `convert` and `inspect-format`, when the input or
output is MusicXML. XML and MXL inputs use the same contract. SFS → MusicXML
export uses the same explicit contract, not a rewritten meter. The file is a JSON
array of the existing typed `IrregularMeasure` records:

```json
[{"ordinal":268,"label":"268","written_meter":{"beats":[2],"beat_type":4},"actual_length":"10"}]
```

Ordinal is one-based within each declared part; actual length is an exact rational
string in quarter-note units. This is not a maximum, an inheriting rule, or an
instruction to trim/extend notes. Every part/staff must match the selected bar's
label, written meter, encoded extent and alignment. Duplicate, stale, unused or
mismatching contracts fail atomically. Other bars retain ordinary validation.
No contract is inferred from a title, filename or overfull extent.

For the pinned Beethoven Fifth movement I only, the checked-in
`crates/scorestream-cli/tests/fixtures/fifth-i-irregular-measures.json` selects
ordinal/label268, written2/4 and actual10 quarters across all18 parts. It is
source-specific evidence, not a generic converter default. The original-family
harness verifies XML and extracted MXL hashes before applying it, and does not
pass it to movements II–IV:

```
python3 scripts/check_beethoven_import.py /path/to/scorestream /path/to/originals \
  --movement-i-contract crates/scorestream-cli/tests/fixtures/fifth-i-irregular-measures.json
```

That harness reports XML/MXL parity and the first actual error. Applying this
contract is not a claim that all other original-score features are supported.

At this change's pinned-source check, default movement I stops at
`measure.overfull` in part1/measure268 (line5218). With the exact contract,
XML and MXL both advance to `attributes.mid_measure` in
part7/measure110/attributes1 (line56875). Both failures emit zero converted
bytes. Thus the CLI option exposes the existing contract but does not yet
establish complete movement-I import or consumption across all18 parts.


## Exact short numbered bars

The same typed contract also permits positive source extents shorter than the
written meter, but never equal to it. The ordinary converter's default behavior
is unchanged. A contract must match the complete encoded cursor extent, including
any authored forward, in every part/staff; it cannot cut notes or discard trailing
cursor movement. Export retains the written meter, number and non-implicit status.

For pinned movement III, `fifth-iii-irregular-measures.json` in the same fixture
directory selects exactly ordinal/labels1,141,142,162, all written3/4, actual
quarter lengths1,2,1,2. These match all18 parts. Use `--movement-iii-contract`
with the original-family harness, alongside `--compare-note-events`; that contract
is never passed to I, II or IV. This avoids the formerly added6quarters per part
without inferring a general pickup policy or silently changing unrelated bars.

## Pinned movement II terminal bar

`fifth-ii-irregular-measures.json` selects only ordinal248, label247, written3/8,
actual1quarter in every one of18 parts. `--movement-ii-contract` passes that
contract only to movement II in the pinned harness. Ordinal1/label0 is an
explicit `implicit=yes` pickup of1/2quarter and is deliberately not selected.
Its existing import length must remain unchanged.

The independent pinned source census found exactly these two uniform
non-nominal bars in II and no asymmetric counterexamples. All446 movement-IV
bars across23 parts have nominal extents, so this evidence supplies no IV
short-bar contract. Uniformity alone does not establish authorial intent:
II's terminal contract is additionally grounded in the real prior SFS export,
which added a trailing1/2quarter to that bar in all18 parts while retaining
all note-event lanes. No other II bar is changed by this scoped contract.
