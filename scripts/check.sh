#!/usr/bin/env sh
set -eu
python3 scripts/notation_coverage.py check
python3 -m unittest discover -s scripts/tests -p 'test_notation_coverage.py'
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -q -p scorestream-cli -- validate design/examples/sfs/ensemble.sfs
cargo run -q -p scorestream-cli -- validate design/examples/sfs/tuplets-and-ties.sfs
cargo run -q -p scorestream-cli -- validate design/examples/sfs/grace-percussion-cross-staff.sfs
cargo build --workspace --locked
python3 scripts/verify_runtime.py
