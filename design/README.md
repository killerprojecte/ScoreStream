# ScoreStream 1.1 — Rust Desktop and Agent Design Package

**Status:** proposed design baseline, 2026-11-13. **Language:** English. **Deliverable:** readable Markdown, JSON contracts, EBNF, and examples. No PDF is included.

ScoreStream is a proposed flat, time-ordered notation language for music generation, score exchange, and precise editing. The product is a **desktop score editor for humans plus an MCP service for agents**, backed by the **same Rust workspace authority**. This package specifies the product and its contracts; it is not a working desktop application or MCP server.

## Start here

| Reader or task | Read first |
|---|---|
| Understand the final architecture and tradeoffs | [System design](docs/01-system-design.md) |
| Implement the actual ScoreStream language | [Format standard](docs/02-format-standard.md) and [EBNF](spec/scorestream.ebnf) |
| Teach an agent the language in one call | [Agent reference](spec/agent-reference.md), [language descriptor](spec/scorestream-language.json), and [example `score_get_format` result](mcp/examples/score_get_format.structured.json) |
| Implement MCP, partial reads, and edits | [MCP contract](docs/03-mcp-contract.md), [planned catalog](mcp/tools-list.planned.json), and `schemas/` |
| Build the desktop UI and live notation preview | [Preview and playback](docs/04-preview-and-playback.md) |
| Reserve future knowledge annotations | [Independent annotation design](docs/05-annotations-future.md) |
| Plan Rust implementation and acceptance gates | [Implementation plan](docs/06-rust-implementation-plan.md) |
| Inspect what was actually checked in this package | [Validation notes](validation/README.md) and `validation/` reports |

`ScoreStream_Design.md` combines the English design chapters and agent reference for convenient reading or AI ingestion. The individual files remain the canonical package locations for code-oriented references.

## The primary schema is the music format standard

The agent-facing format contract consists of:

1. `spec/scorestream.ebnf`: grammar for **raw ScoreStream text**.
2. `spec/scorestream-language.json`: syntax templates, registered fields, semantic rules, defaults, examples, and resource locations.
3. `schemas/scorestream-ast.schema.json`: JSON Schema for the **parsed AST**.
4. `spec/agent-reference.md`: short authoring instructions.
5. `examples/sfs/`: complete valid score and replacement-fragment examples.

`score_get_format` returns useful syntax and examples inline, with versioned resource references for details. The planned MCP tool input schemas are additional contracts; they do not substitute for the music language definition. JSON Schema by itself cannot validate raw SFS text or musical timing and reference integrity.

## Settled design choices

- Quarter-note time uses exact reduced rational strings, such as `1`, `1/2`, and `1/3`.
- All events beginning at one score time share an `@time` frame. Instrument/voice lines appear vertically within it. A dot closes the frame.
- Written pitch spelling is authoritative. Numeric pitch and relative pitch are derived views for models and search.
- Flat text is an interchange and authoring surface. Internally, Rust uses typed musical objects, indexes, stable IDs, and immutable revisions.
- The desktop editor and MCP perform the same atomic domain commands. A stale base revision fails explicitly.
- Rendering is a projection of the score. Renderer output never replaces the authoritative score.
- Analytical prose and passage-effect descriptions have an independent future store. Their schema is included; their persistence, retrieval, editor, and MCP implementation are deferred.
- Playback is a later subsystem with an independent performance timeline and real-time engine.
- Full source preservation and full interpreted notation support are different capabilities. No adapter may silently claim the latter merely because it retained source XML.

## What is included and what is not

Included are architecture decisions, normative contracts within this proposal, valid/invalid fixtures, schemas, a planned MCP catalog, implementation milestones, and package QA utilities. Python files under `tools/` are reproducible **artifact generation and validation utilities**, not a production component. The proposed runtime is Rust with a Tauri/WebView desktop integration.

No compiled Rust application, production parser, MusicXML converter, running MCP service, renderer integration, knowledge database, or audio engine is claimed to exist in this package. The environment used for this package did not contain a Rust toolchain, so no Cargo build is reported. Test reports state their actual coverage and do not certify full MusicXML interoperability.

The catalog is deliberately named `tools-list.planned.json`. A future server must advertise only handlers and backends that are implemented, configured, and authorized. The `.future.schema.json` annotation file is a design reservation, not an enabled feature.

## Reproduction

The QA environment needs Python 3 and `jsonschema` with `referencing`. From the package root:

```sh
python3 tools/build_format_reference.py
python3 tools/build_mcp_catalog.py
python3 tools/validate_package.py
```

Read each report's stated scope. These checks validate distributable contracts and fixtures; the Rust milestones add production parser, converter, concurrency, UI, renderer, and performance tests.

The package manifest lists the delivered files and SHA-256 hashes. Example scores in this package are original miniature fixtures, not excerpts from a third-party score collection.
