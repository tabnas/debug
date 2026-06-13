See [AGENTS.md](AGENTS.md) for the full agent guide to this repository.

Quick reminders:

- `ts/` (TypeScript) is canonical; `go/` is kept at parity with it.
- Change TypeScript first, then mirror the change in Go.
- The `tabnas` parser is pinned to its GitHub `main` branch and is
  required to build or test either implementation.
- `make build` / `make test` cover both implementations.
