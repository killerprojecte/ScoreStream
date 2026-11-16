# Repeated preservation receipts versus semantic failures

Automatic authored-articulation retention is one repeated decision, not one
unsupported semantic per note. The importer now emits a compact preservation
receipt with an exact `occurrences` count. Its path/source is explicitly the first
sample; all retained notes still exist independently in the typed model. Missing
`occurrences` in older JSON means one, and singleton JSON keeps its old shape.
Append preserves aggregate counts, source sample and severity. This aggregation
changes neither source acceptance policies nor consumer capability claims.

The existing 1024-entry diagnostic receipt budget does not cap score size, notes,
or retained articulations. Exhausting informational diagnostics alone is not an
error. A later real error formerly upgraded only the final truncation marker,
concealing the actionable feature/path. Saturated reports now keep the first
concrete error in addition to the severity-bearing omission marker. Unknown,
invalid and resource-failure semantics never become successful preservation.
The error-report safety budget is unchanged, not raised.

Regression receipts cover 2048 retained notes with one exact notice and unchanged
full model cardinality; a late unsupported attribute still fails. JSON/append
roundtrip counts, old JSON, and a concrete late error after 4096 informational
notices are checked independently. This is diagnostic correctness, not evidence
that every retained articulation is rendered or sounded.

Pinned untouched Beethoven IV XML/MXL CLI conversion after this change produces
115 diagnostics with no truncation and equal outputs/reports. The actual first
failure is SFS `navigation.route_policy`: `UnclosedFinalEnding` at `/barlines`.
No SFS or roundtrip semantic transcript is emitted; no ending endpoint or route
has been inferred to make the score pass. This was a previously concealed real
semantic limitation, not a newly introduced rejection.
