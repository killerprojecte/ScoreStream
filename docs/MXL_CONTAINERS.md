# Compressed MusicXML container input

The existing `musicxml` CLI input format accepts plain UTF-8 XML and compressed
`.mxl` containers. This adds archive ingestion, not complete MusicXML notation
support. No desktop Open/menu behavior changes in this slice.

```sh
cargo run -p scorestream-cli -- scan-musicxml movement.mxl
cargo run -p scorestream-cli -- inspect-format musicxml movement.mxl
cargo run -p scorestream-cli -- convert musicxml sfs movement.mxl
```

Scanning reports structural counts independently of the strict semantic adapter.
Unsupported music still fails conversion with diagnostics and no partial stdout.
A ZIP signature also identifies a compressed input with a different extension;
an `.mxl` extension requires a valid archive. Neither errors nor missing manifest
members trigger fallback to another XML file. Output remains uncompressed.

## Selection and integrity

Per the [W3C MusicXML container specification](https://www.w3.org/2021/06/musicxml40/container-reference/elements/rootfiles/),
`META-INF/container.xml` selects the root. Its first `rootfile` is the primary
MusicXML document, not the first ZIP entry or first `.xml` filename. An omitted
`media-type` means MusicXML. A different first media type is an error, even when
a later root is compatible. Additional PDF/audio/part renditions are not imported.
Older containers without `mimetype` are accepted. If present, that marker must
be stored, unencrypted, without a local extra field, and contain the exact
MusicXML archive media type.

The reader validates every directory member before processing the root:

- Exact UTF-8 relative paths. No traversal, empty/dot components, backslashes,
  drive/URI prefixes, controls, normalization, percent decoding, or duplicate names.
- Regular files and directories only; no symlinks, devices, encryption, or
  unsupported flags/compression methods. STORE and DEFLATE are supported.
- Matching local/central headers, framed extra fields, valid ZIP64 sizes and end
  records, consistent directory counts/extents, and nonoverlapping entry ranges
  including data descriptors. Split archives and executable prefixes are rejected.
- Declared sizes are checked before writes; selected entries also require exact
  expanded size, CRC, and a complete DEFLATE stream with no trailing data.
- Manifest structure, duplicate attributes, XML characters/declarations, and
  root references are checked. Manifest DTDs, entity declarations, processing
  instructions, and external resources are never resolved.

Only the manifest, optional MIME marker, and chosen root are decompressed.
Unselected media payloads are not decompressed or integrity-certified. No archive
path is ever used as a filesystem destination. The root is copied into an
anonymous temporary file and rewound for the XML scanner/adapter. Errors drop
that file; successful readers delete it when closed. This does not promise crash
cleanup on every OS/filesystem. No resource referenced by score XML is followed.

## Resource behavior

There is no total score byte/event limit. The spool consumes disk proportional
to the selected XML, with fixed 64 KiB copy/decompression buffers. Runtime is
proportional to metadata and selected payload bytes; insufficient disk or I/O
failure returns an error. Applications serving untrusted uploads should also
apply their own disk quotas, scheduling, cancellation, and execution budgets.

Default configurable `mxl::ContainerLimits` cover metadata only: 100,000 ZIP
members, 1 MiB manifest, 4,096-byte individual paths, and 16 MiB aggregate paths.
These bound membership indexes and manifest parsing, independently of score
length. The DEFLATE plausibility check uses its theoretical 1032:1 upper bound,
not a heuristic score-compression cutoff. XML token/depth/measure checks remain
in the separate streaming scanner; no safety checks were removed.

## Dependencies and validation

The container reader uses `rawzip 0.5.2` (Rust 1.85, zero dependencies) for ZIP
structure, `flate2 1.1.11` with only the pure-Rust `rust_backend`, and
`tempfile 3.27.0` for private temporary files. Default ZIP encryption and unrelated
compression stacks are not pulled in. These add twelve lockfile packages; their
declared MSRVs, where supplied, are at most 1.85. Existing workspace dependencies
are unchanged. Actual validation used the repository's pinned Rust 1.100 toolchain,
not a Rust 1.85 build; workspace members do not currently inherit its declared
`rust-version` value.

The alternative `zip` crate's newest 9.5.1 release requires Rust 1.88; its older
7.3.0 supports 1.83 but loads directory metadata eagerly and supplies many
unneeded archive/extraction features. `rawzip` permits bounded, streaming
central-directory processing and explicit integrity policy without extraction.
Sources: [rawzip documentation](https://docs.rs/rawzip/0.5.2/rawzip/),
[flate2 package](https://crates.io/crates/flate2/1.1.11),
[tempfile package](https://crates.io/crates/tempfile/3.27.0),
[zip 9.5.1](https://docs.rs/zip/9.5.1/zip/).

Focused tests cover ordered roots, invalid/ambiguous paths, encryption, symlinks,
malformed metadata/XML, CRC, truncated streams, ZIP64/descriptors, and large
root bytes. Two tiny deterministic fixtures package existing repository XML.
An ignored corpus test requires locally supplied, pinned originals and verifies
the exact XML SHA-256 before independently scanning all four Beethoven Op.67
movements. It does not download or redistribute the corpus:

```sh
SCORESTREAM_MXL_FIXTURES=/path/to/pinned/inputs \
  cargo test -p scorestream-formats --test mxl --locked -- --include-ignored
```

The four inputs contain 18 / 18 / 18 / 23 parts and 15,261 / 10,908 / 11,488 /
30,902 notes. All four archived roots scan successfully. This is not a claim
that their complete musical contents convert, render, or play correctly.
