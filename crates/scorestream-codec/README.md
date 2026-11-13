# scorestream-codec

Strict production Rust reader and deterministic writer for the initial ScoreStream 1.1 core measured profile. It is independent of the design pack's Python QA utility.

```rust
use scorestream_codec::{parse, canonical};
let document = parse("!score 1.1 unit=q\n!end at=0\n")?;
let text = canonical(&document);
# Ok::<(), Vec<scorestream_codec::Diagnostic>>(())
```

`parse(&str) -> Result<Document, Vec<Diagnostic>>` accepts complete scores. `parse_with_context(&str, &FragmentContext)` additionally accepts bounded fragments with explicit caller-supplied `FragmentBinding { alias, part, staff, staves }` values. Bindings are not inferred from fragment contents. Authorization and entry-state ownership belong to the application.

A quote-aware lexer handles JSON escaping, surrogate pairs, whitespace, sharp pitches, and event references without naïve space/comment splitting. LF and CRLF are accepted. Only complete frames and a final `!end` can produce a successful document. Syntax errors are fail-fast; a syntactically complete document receives aggregate semantic validation.

Missing authoring event IDs are assigned deterministic collision-safe names after validation. Canonical output preserves given identities, written pitch, exact times, member order, and notation attributes. It normalizes formatting and orders measures/state/events/marks/relations. Comments are source trivia and are not retained in the normalized AST.

## Verification

`cargo test -p scorestream-core -p scorestream-codec` exercises:

- All four positive design fixtures with exact supplied JSON AST equality and canonical round trips.
- All 33 negative fixtures with their specific expected diagnostic codes.
- Exact huge fractions, immutable relation attributes, crossing slurs, transposition scope, same-frame forward references, state after events, grace chords, unequal chord durations, fragment contexts, namespace rejection, Unicode JSON strings, collision-safe IDs, interval queries, source errors, and incomplete drafts.

The supplied negative manifest uses uppercase QA labels. The public diagnostic code is that exact label in lowercase, following the normative format document's stable-lowercase requirement.

## Explicit limits

Only version 1.1 and quarter-note time are supported. Unknown extensions and fields are errors. A full streaming/incremental API, source-trivia preservation, extension preservation mode, and permissive rational normalization are not exposed. The reader accepts an explicit empty score; the application decides where that state is appropriate. See `scorestream-core/README.md` for semantic limits. The canonical writer expects a validated document; arbitrary mutated ASTs must be validated before writing/committing.
