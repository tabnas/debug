See [AGENTS.md](AGENTS.md) for the full agent guide to this repository.

Quick reminders:

- `ts/` (TypeScript) is canonical; `go/` tracks it. Change TypeScript
  first, then update Go to match as far as the Go engine API allows.
- The engine is already wired: TS via the `node_modules/@tabnas/parser`
  symlink to the sibling `../parser/ts`, Go via a pinned published
  module version in `go/go.mod`. `scripts/fetch-parser.sh` and `vendor/`
  are legacy and are NOT prerequisites — nothing in the build reads them.
- `make build` / `make test` cover both implementations. Don't run
  `npm ci` or delete `node_modules`: it breaks the symlink wiring.
- Some TS/Go differences are intentional (engine API limits) and are
  recorded in `docs/reference.md` — the authoritative divergence
  register.
