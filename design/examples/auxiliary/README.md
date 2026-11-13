# Auxiliary schema examples

See `manifest.json` for the authoritative schema and expected outcome of each JSON example. A manifest entry may select a named `$defs` definition rather than the schema's root.

`valid/` examples must pass JSON Schema. They are fixtures and still require referenced score/render objects to exist before a real command can run.

`invalid/` examples must fail JSON Schema. They exercise closed tagged unions, exact-time field types, unsupported ripple edits, and prevention of caller-invented actor authority.

`semantic-invalid/` examples must pass JSON Schema and fail the stated semantic rule. JSON Schema cannot calculate rational reduction, compare exact start/end values, look up immutable revisions, parse a fragment's complete musical meaning, or verify a render's score identity.

Files ending in `.future.json` illustrate an independent future annotation contract only. They do not enable annotation storage or annotation MCP tools in the current delivery.
