See [AGENTS.md](AGENTS.md) for the full agent guide to this repository.

Quick reminders:

- `ts/` (TypeScript) is canonical; `go/` tracks it. Change TypeScript
  first, then update Go to match as far as the Go engine API allows.
- The `tabnas` parser engine (`github.com/tabnas/parser`) is not
  published; run `scripts/fetch-parser.sh` to download + build its
  GitHub `main` branch into `vendor/` before building or testing.
- `make build` / `make test` fetch the engine and cover both
  implementations.
- Some TS/Go differences are intentional (engine API limits) and are
  recorded in `docs/reference.md`.
