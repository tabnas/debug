#!/bin/bash
# SessionStart hook: prepare the TypeScript and Go sides to build, lint
# and test. It installs ts/'s dependencies, the published engine among
# them, and warms the Go build cache; go.mod requires the published engine
# too. It also runs scripts/fetch-parser.sh, which fetches and builds the
# engine's GitHub main in vendor/, though nothing in the build or test path
# reads vendor/ any more (AGENTS.md). rs/ needs a sibling ../parser
# checkout, which this hook does not provide.
#
# Runs only in Claude Code on the web (remote) sessions, which start from
# a fresh container. Safe to run repeatedly.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

ROOT="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$ROOT"

echo "session-start: fetching and building the tabnas parser engine ..."
./scripts/fetch-parser.sh

echo "session-start: installing TypeScript dependencies ..."
( cd ts && npm install --no-audit --no-fund )

echo "session-start: warming the Go build cache ..."
( cd go && go build ./... )

echo "session-start: ready."
