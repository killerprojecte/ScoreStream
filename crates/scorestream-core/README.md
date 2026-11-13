# scorestream-core

A UI- and storage-independent ScoreStream 1.1 musical model. No unsafe code.

- `Rational` uses `BigRational`/`BigInt`, exact comparison and arithmetic, and canonical string-only JSON. No machine integer or float is used for score time. `to_f64()` is explicitly a display projection.
- `Pitch` preserves written step, exact alteration, and an arbitrary-size signed octave. `Duration` separates measured values from grace events. Rest, spacer, and unpitched identities remain distinct.
- Typed record variants serialize to the supplied AST schema. Flat attributes are typed strings, signed 64-bit notation integers, or booleans, validated against the bundled version-pinned vocabulary.
- `validate` checks aliases, staff ranges, frame ordering, namespaces, state scope/conflicts, transposition tuples, event tails, measure ownership, references, grace/chord groups, relation phases/closure, sounding ties, and fragment coverage.
- Source locations are one-based line/column ranges, retained on domain records but excluded from protocol AST JSON. Diagnostics carry lowercase stable codes and severity. Source locations are zero when an AST was created without source text.

`Document::events()` returns exact onset/event pairs. `active_events()` uses half-open intervals and never invents rests or treats a sustained event as a new attack.

## Boundaries

This crate does not authorize fragment bindings, commit revisions, render engraving, or infer playback. The application must validate before committing a directly constructed/deserialized AST. Unknown required semantic extensions are rejected, not treated as safely interpreted. No MusicXML conversion support is implied.

Explicit normal tuplet notation is retained with a warning rather than a speculative consistency calculation. Sounding ties in a document containing a sounding octave relation currently return an explicit unsupported diagnostic. Written rhythm disagreement is a warning preserving both measured and written values. Notation integers outside signed 64-bit and staff numbers outside unsigned 32-bit are unsupported; musical times, alterations, and octaves are arbitrary precision.
