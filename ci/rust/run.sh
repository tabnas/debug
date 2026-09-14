#!/usr/bin/env bash
# Rust port gate. Kept in one script so local and hosted validation cannot
# quietly drift apart: ci/workflows/rust.yml runs this file, and so can
# you. `make test-rs` is the fast inner loop; this is the full gate.
#
# The engine is a PATH DEPENDENCY on the sibling checkout
# (rs/Cargo.toml: `tabnas = { path = "../../parser/rs" }`), and the crate
# is unpublished, so there is no registry version to fall back on. Clone
# https://github.com/tabnas/parser next to this repo before running.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
ENGINE="$ROOT/../parser/rs"

if [[ ! -f "$ENGINE/Cargo.toml" ]]; then
  echo "no engine checkout at $ENGINE" >&2
  echo "clone https://github.com/tabnas/parser as a sibling of $(basename "$ROOT")" >&2
  exit 1
fi

cd "$ROOT/rs"

# NOT `--locked`, deliberately, and this is the one place the plugin gate
# differs from the engine's own (parser ci/rust/run.sh does pass it).
#
# Cargo.lock records the engine by version, and the engine is resolved
# from a sibling checkout of MAIN. So the day parser bumps its crate
# version, `--locked` here fails with "cannot update the lock file" on
# every pull request in this repo, including ones that touch no Rust at
# all -- a red build caused by another repository's release. Reproduced
# by bumping the sibling's version and re-running: `--locked` fails, the
# plain build succeeds. The engine repo has no path dependency of its
# own, which is why `--locked` is right there and wrong here.
cargo fmt --all --check
cargo build --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
