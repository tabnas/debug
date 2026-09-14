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

# Assert the lock's entry for THIS crate still matches the manifest,
# BEFORE anything runs cargo. Without `--locked` (see below) a cargo
# command silently rewrites Cargo.lock in the runner, so a version bump
# that updates rs/Cargo.toml and forgets rs/Cargo.lock passes every
# check and ships a stale lock. Reproduced: bump the manifest, leave the
# lock, `cargo build` compiles the new version and rewrites the lock
# without a word. This has to come first -- after a cargo command the
# lock has already been fixed up and the check can never fail.
#
# Only this crate's entry is asserted. The engine's entry legitimately
# moves whenever the sibling checkout does, which is the same reason
# blanket `--locked` is wrong here.
CRATE=$(awk -F'"' '/^name = /{print $2; exit}' Cargo.toml)
WANT=$(awk -F'"' '/^version = /{print $2; exit}' Cargo.toml)
HAVE=$(awk -v c="$CRATE" -F'"' '
  $0 == "name = \"" c "\"" { f = 1; next }
  f && /^version = / { print $2; exit }
' Cargo.lock)

if [[ "$WANT" != "$HAVE" ]]; then
  echo "Cargo.lock records $CRATE ${HAVE:-<missing>}, but Cargo.toml says $WANT" >&2
  echo "run a cargo command and commit the updated rs/Cargo.lock" >&2
  exit 1
fi

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
# NOT `--all`. cargo defines it as "all packages, and also their local
# path-based dependencies", and the engine IS such a dependency, so
# `--all` reaches into the sibling parser checkout: an unformatted file
# over there fails this gate even when every file here is clean, and a
# contributor with dirty sibling work cannot run it at all. Verified both
# ways -- with the sibling made unformatted, `--all --check` reports a
# diff in parser/rs and plain `--check` stays silent, while a dirty file
# in THIS crate still fails plain `--check`. The engine repo has no path
# dependency, which is why `--all` is safe there and not here.
cargo fmt --check
cargo build --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
