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

The MCP binary defaults to newline-delimited JSON-RPC on stdin and writes responses only on stdout. It exposes `score_get_format` and a finite set of versioned language resources. It intentionally does not advertise unimplemented score editing, rendering or conversion tools.

### Strict format conversion

A first MusicXML 4.0 score-partwise adapter now maps through the shared typed `ScoreDocument`, alongside an SFS adapter. This is a **limited conversion profile**, not complete MusicXML support. Unknown symbols/attributes block conversion with structured diagnostics; XML is never passed to a renderer. See [supported mappings, Chinese design and runnable examples](docs/FORMAT_ADAPTERS.md).

```sh
cargo run -p scorestream-cli -- inspect-format musicxml crates/scorestream-formats/tests/fixtures/two-staff.musicxml
cargo run -p scorestream-cli -- convert musicxml sfs crates/scorestream-formats/tests/fixtures/two-staff.musicxml
```

### Fast format discovery

Call `score_get_format` with `{}` (or `{"detail":"quick"}`) for a compact ScoreStream authoring baseline: record syntax, pitch and exact-time rules, common event attributes, scope/lexical guidance, and a complete ensemble example. This teaches the score language, not the MCP request schema.

- `{"topics":["time","relations"]}` adds only those sections and relevant examples while keeping the baseline. Available topics are `syntax`, `pitch`, `time`, `polyphony`, `state`, `marks`, `relations`, `editing`, `extensions`, and `validation`.
- `{"topics":[]}` is identical to the default. Topic order does not change the response; shared examples and constraints are included once.
- `{"detail":"full"}` returns the complete canonical language descriptor. If `topics` is also supplied, it is validated but does not narrow `full`.
- `structuredContent` and the JSON text fallback contain the same selected application result, so text-only clients do not receive hidden/unrequested sections.

The full descriptor remains at `scorestream://spec/1.1/language`, with `/grammar`, `/ast-schema`, and `/agent-reference` available independently. Every quick result includes these complete resource URIs. Editing-related language documentation does not advertise operational MCP score-edit tools.

Run `python3 scripts/verify_format_reference.py` after building the MCP binary to validate the emitted quick/full/topic responses against the output schema and report serialized UTF-8 byte sizes. This optional acceptance check requires Python `jsonschema`; size reports are bytes, not model-token estimates.

### MCP over HTTP + SSE

For clients configured with an SSE server URL, start the standalone server:

```sh
cargo run -p scorestream-mcp -- --transport sse
# Optional explicit address and port (port 0 selects an available port):
cargo run -p scorestream-mcp -- --transport sse --bind 127.0.0.1 --port 3210
# Optional authentication; supply the secret through your environment, not argv:
cargo run -p scorestream-mcp -- --transport sse --token-env SCORESTREAM_MCP_TOKEN
```

Connect to **http://127.0.0.1:3210/sse**. [Example client configuration](examples/mcp-sse-client.json) uses the common `mcpServers` / `type: sse` shape; adapt the surrounding configuration to your client. If authentication is enabled, the client must send `Authorization: Bearer <your token>` on both GET and POST. Clients that cannot set this header cannot connect to an authenticated service. Tokens are not accepted from URLs and are not printed by the server.

This implements the [2024-11-05 HTTP+SSE transport](https://modelcontextprotocol.io/specification/2024-11-05/basic/transports): GET `/sse` opens a stream and returns an `endpoint` event with a session-specific POST `/messages?session_id=…` URI. POSTs return HTTP 202; JSON-RPC responses arrive as `message` events on that session's stream. It is **not Streamable HTTP** and does not expose a `/mcp` endpoint. A disconnected client starts a fresh connection and initializes again; replay/resumption is not supported.

The default listener is loopback only. Binding to another interface requires a token; plain HTTP is not encrypted, so use loopback or a separately secured TLS tunnel rather than exposing it publicly. Browser `Origin` and HTTP `Host` are checked against the actual listener address/port (also `localhost` for loopback). Missing Origin is allowed for native clients, but untrusted origins are rejected even with a token. Arbitrary browser origins and reverse-proxy hostnames are not enabled. The server limits each request to 1 MiB, active sessions to 32, and each session's pending replies to 32; slow clients receive HTTP 429 before their request is processed. Stop with Ctrl-C.

The desktop hosts the same Rust service in-process. Its optional `scorestream://workspace/current` resource reads the **current committed** source, document and revision from the desktop's existing workspace. It never reads a private editor draft or exposes file paths. Standalone stdio/SSE remain format-only because they have no desktop workspace. No score mutation tools or resource subscriptions are advertised. See [desktop setup](desktop/README.md) for configuration and lifecycle controls.

Transport verification:

```sh
cargo test -p scorestream-mcp --locked
cargo build -p scorestream-mcp --locked
python3 scripts/verify_mcp_sse.py
# Optional, if the official Python MCP SDK is installed:
python3 scripts/verify_mcp_sdk.py
```

The Rust tests exercise the HTTP router and stream bodies in-process. The Python acceptance test additionally uses real loopback HTTP connections, compares two SSE sessions with stdio, checks authentication/origin rejection and disconnect cleanup, and verifies shutdown releases the listening port. The optional SDK test uses two simultaneous authenticated `sse_client` / `ClientSession` connections from the official Python MCP SDK (verified with 1.29.0). These process tests need permission to bind a local socket.

The separate desktop crate keeps platform WebView dependencies out of headless core/CLI builds. Desktop setup and platform verification are described in `desktop/README.md` when available.

## Orchestral instrument identities

The shared [instrument catalog](docs/ORCHESTRAL_INSTRUMENTS.md) now covers 51 standard orchestral and auxiliary identities. Live built-in playback and WAV export use 25 simple synthesis profiles for 40 pitched identities; the 11 unpitched percussion identities are cataloged but not yet rendered. Custom IDs and explicit pitch/transposition semantics are preserved. This is not a professional sample library or system MIDI/VST3 backend.

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
