# Document metadata in development SFS 1.1

This increment extends 1.1 directly by explicit product decision. No new file version, dependency, raw foreign markup, UI inference or filename-to-title conversion is introduced.

## Authoring

    !meta title="春日练习" description="两位作者的短练习曲。" authors=["林青", "Alex Chen"]
    !meta header="排练版" footer="仅供学习 · 2026"

Place metadata after the score header and before frames. All fields are optional. One or several declarations are allowed, but repeated fields fail, including identical values. Fragments cannot declare metadata. Strings use JSON quoting. Authors use a JSON array with optional horizontal whitespace, preserve order and spelling, and require nonblank single-line names without controls. No role or identity inference occurs. Legacy composer, lyricist and arranger are separate role fields.

The canonical writer emits a single line with lexically sorted keys and compact array syntax. Empty arrays disappear. Explicit empty strings remain. No normalization removes content from strings. Legacy escaped controls remain readable with a portability warning; consumers must report text their output format cannot represent, never silently remove it.

## Shared Rust and JSON contract

Document.metadata is scorestream_core::DocumentMetadata. String fields are Option<String>; authors is Vec<String> with an empty default. The old flat metadata strings are accepted unchanged, and unknown fields, wrong JSON types and null values are rejected. All 13 fields are:

- title, description, authors, header, footer
- subtitle, composer, lyricist, arranger, work_number, movement, rights, language

Only present fields are serialized; an empty authors list is omitted. Source provenance is an internal BTreeMap<String, SourceRange> on metadata.source, with the exact field token's character range. It is not exported into the wire AST. Metadata validation errors name metadata.authors or the relevant field; text portability diagnostics are warnings.

    {"title":"春日练习","description":"两位作者的短练习曲。","authors":["林青","Alex Chen"],"header":"排练版","footer":"仅供学习 · 2026"}

The same object appears in CLI inspect output, desktop ViewResponse.document.metadata and committed snapshots. Existing revision, preview, undo and redo rules apply. Metadata does not affect musical timing or playback.

## Display meaning

- Title is an explicit work title. No filename, product label, header or description becomes a title automatically.
- Description is editor/catalog context. It is not automatically printed as notation.
- Authors are ordered generic attribution. Display may join them beneath a first-page title, but they are not automatically composers.
- Header and footer are deliberate running page text. They do not imply page numbering, variables, layout coordinates, or template expansion.
- All strings are plain text, including strings that look like HTML, MEI or placeholders. Escape them when generating another format. Renderer placement is a separate frontend capability, not a parser claim.

## Modules and verification

core/metadata.rs owns the typed values, provenance and validation. codec/metadata.rs owns text decoding and canonical serialization. The generic scalar grammar remains unchanged: author arrays do not enable arbitrary arrays on events or directions. The descriptor, EBNF, AST schema and syntax-scoped MCP reference describe the same fields. The descriptor generator and package validator include the full document-metadata fixture.

Run the CLI on design/examples/sfs/document-metadata.sfs with inspect, canonical or validate. The format tool's default quick reference includes the metadata declaration; topics=["syntax"] adds all field definitions and the full fixture. Full reference and AST-schema resources reflect the same development 1.1 contract.

Tests cover Unicode, quoted names, whitespace, source ranges, deterministic canonicalization, no inferred title, legacy fields, strict JSON types, invalid and repeated authors, declaration boundaries, plain markup-like content, direct AST validation, existing conformance fixtures, and desktop preview/commit/history propagation.
