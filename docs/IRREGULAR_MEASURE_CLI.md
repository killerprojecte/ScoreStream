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
