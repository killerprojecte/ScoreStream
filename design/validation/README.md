# Design-package validation

Run from the bundle root:

```sh
python3 tools/validate_package.py
```

The utility requires Python 3.10 or newer plus `jsonschema` and `referencing`. Their actual versions and the checked input hashes are recorded in `format-report.json`. This is packaging QA only. It is not production Python code and does not claim a Rust implementation, running editor, renderer, MCP service, performance engine, or annotation store.

## What is checked

The finite reader parses the four files in `examples/sfs/` into `examples/ast/`, validates the ASTs against the local Draft 2020-12 schema, compares their descriptor copies, and checks independently recorded event facts and counts. Its lexer preserves pitch `#`, references, and JSON-quoted values. Declarations, closed frames, record kinds, and end records follow the documented ordinary core subset.

Semantic checks cover reduced exact timing rationals, strictly increasing frame times, part/voice aliases and staff bounds, registered field names and state scopes, duplicate object IDs, reference resolution by frame commit, grace anchors and order, positive measured durations, declared ends, chord onset/voice cohesion, relation phase/endpoints, and simple sounding-tie continuity. Replacement fixtures receive explicit external voice bindings and must cover their local span without implicit gap filling.

Written type, dots, and time-modification values remain separate from measured duration. Their ordinary numeric calculation produces a warning on disagreement and never rewrites or automatically rejects the data. The four documented positive examples are expected to be numerically consistent. Separate acceptance probes verify that disagreement is preserved, that different chord-member lengths are legal even when the head is shorter, and that the registered normal-type/normal-dot fields remain distinct from the written glyph's type and dots.

The deliberate invalid corpus is enumerated in `examples/sfs-invalid/manifest.json`, with the expected diagnostic code for each case. A negative case passes QA only when it is rejected for the recorded reason. The report lists the actual diagnostics.

Auxiliary JSON examples are checked according to their own manifest, including named schema definitions. Fixture-local semantic checks cover exact span ordering/reduction, frozen-selection versus edit revisions, replacement fragment spans and voice coverage, fragment-local retarget references, and the seeded render/score revision correspondence. Future annotation examples validate a proposed data contract only.

## Explicit limits

This handwritten line reader is **not a complete implementation of the EBNF** and is not generated from it. The grammar is included in the report's input hashes; passing QA does not prove the grammar unambiguous or all possible valid SFS accepted. A production Rust lexer/parser and its broader conformance suite remain necessary. JSON Schema validates parsed structure, not raw SFS.

The checker executes only ordinary common-clock core fixtures. It does not interpret unknown extensions, free-time or multi-clock content, every symbol/instrument vocabulary, arbitrary metadata meanings, complete nested-tuplet semantics, unusual grace/chord combinations, or all relation kinds' musical behavior. Relation checks outside simple ties and tuplets primarily establish identity, ordering, closure, and endpoint existence. A standalone open relation is rejected because these fixtures supply no external relationship boundary context; that is not a ban on contextual excerpts.

Nominal pitch comparison for simple ties is not a general sounding-pitch engine. Changing transposition/tuning contexts receive a coverage warning. Written-duration consistency is illustrative because intended duration can differ from written duration. Full registry scalar, enum, and requiredness validation remains outside this finite QA's executed coverage, including symbol and metadata semantics documented by the format descriptor.

`fixture-context.json` is explicitly synthetic: it supplies fragment aliases, expected example facts, and the two render revision records needed by the auxiliary fixtures. The utility does not query a real workspace, verify actual object existence or selection digests, resolve arbitrary visual geometry, or exercise transaction authorization, idempotency, concurrency, rendering, or audio behavior.

The report is reproducible for the same package inputs and dependencies. Regenerate it after changing the grammar, descriptor, schemas, examples, or QA utility. A passing report establishes consistency of this finite design corpus, not full ScoreStream or MusicXML support.
