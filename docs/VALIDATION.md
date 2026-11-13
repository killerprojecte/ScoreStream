# Validation checkpoint

## Published Rust foundation — 2026-11-13

Tested locally on Linux x86_64 with Rust/Cargo 1.99.1. The five-crate headless workspace deliberately excludes the native desktop and its WebView dependencies.

Passed against the committed sources:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --workspace --locked --offline
python3 scripts/verify_runtime.py
```

- 36 Rust tests passed: 2 core, 24 codec/conformance, 5 application, and 5 MCP tests.
- Codec conformance includes all four positive supplied AST fixtures and canonical round trips, all 33 negative fixtures with expected diagnostic codes, exact large-number cases, fragment context, identity preservation, and incomplete drafts.
- Application tests cover private valid/invalid drafts, revision conflicts, validation-before-commit, undo/redo with fresh revisions, dirty-buffer mutation gates, and frame-wide staff state.
- Independent CLI process checks compare the three complete score fixtures with the supplied ASTs, verify canonical semantic preservation and idempotence, and reject all 33 negative inputs.
- Independent MCP stdio process checks send 7 messages and receive 6 correctly addressed responses. Notifications stay silent, only the implemented format-discovery tool is advertised, an unimplemented edit is rejected, and arbitrary filesystem resource access is rejected.

`--offline` used already cached, locked dependencies in this Linux environment. On a fresh computer, omit `--offline` and let Cargo retrieve the locked dependencies. `scripts/check.sh` runs the complete headless acceptance sequence without requiring those caches.

## Platform and product limits

- Linux headless core/CLI/MCP: built and run, checks above passed.
- Linux native desktop: separate validation in progress; no successful runtime or screenshot is claimed here.
- Windows and macOS: portable source paths and separate native adapters are planned, but no native build, packaging, or runtime acceptance has run on those systems.
- Rust 1.85 is the declared minimum; this checkpoint was tested on 1.99.1, not on the minimum toolchain.

The application history is in memory. Durable snapshots, exclusive multi-process workspace ownership, operational MCP edits/region reads, MusicXML conversion, full engraving coverage, playback, and annotation storage are not implemented by this checkpoint. No manual CI workflow was triggered.

The original `design/validation/` Python reports describe the supplied design package only. They are not implementation or platform acceptance evidence.
