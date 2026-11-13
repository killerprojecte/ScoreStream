# ScoreStream

A modular Rust score editor and agent-readable music notation toolkit, implemented from the supplied ScoreStream 1.1 design pack.

**Development checkpoint, not a complete 1.1 implementation.** The desktop and core share one Rust authority. The first published slice includes exact-time SFS parsing/validation, a shared editor application service with private drafts, atomic revision-bound changes, typed note edits, undo/redo, CLI tools and read-only MCP format discovery. The desktop UI is being validated as a separate checkpoint. See [validation](docs/VALIDATION.md) for what was actually built and run, and [architecture](docs/ARCHITECTURE.md) for boundaries and remaining work.

## Rust tools

Install a current stable Rust toolchain, then run:

```sh
cargo test --workspace
cargo run -p scorestream-cli -- validate examples/first-light.sfs
cargo run -p scorestream-cli -- inspect examples/first-light.sfs
cargo run -p scorestream-cli -- canonical examples/first-light.sfs
cargo run -p scorestream-mcp
```

The MCP binary reads newline-delimited JSON-RPC on stdin and writes responses only on stdout. It exposes `score_get_format` and a finite set of versioned language resources. It intentionally does not advertise unimplemented score editing, rendering or conversion tools.

The separate desktop crate keeps platform WebView dependencies out of headless core/CLI builds. Desktop setup and platform verification are described in `desktop/README.md` when available.

## Design and samples

- [Original design package](design/README.md)
- [Notation language](design/docs/02-format-standard.md)
- [Agent reference](design/spec/agent-reference.md)
- [First Light piano demo](examples/first-light.sfs)
- [Conformance fixtures](design/examples/)

Do not interpret passing the supplied finite fixtures as exhaustive MusicXML, notation, renderer or platform compatibility. Windows and macOS runtime acceptance require builds on those systems.

## Current roadmap

1. Complete and harden the SFS model/parser and fixture coverage.
2. Validate the native Linux editor, draft isolation, property edits and notation mapping.
3. Expand rendering coverage with exact projection diagnostics.
4. Add durable workspace snapshots and exclusive writer/IPC ownership.
5. Add operational MCP region reads and atomic typed edits through that same authority.
6. Add the MusicXML adapter, strict fidelity reports and packaging acceptance on Linux, Windows and macOS.

Playback and independent analytical annotations are later subsystems, as specified in the design.
