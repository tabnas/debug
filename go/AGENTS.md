# Agent guide: go/ (parity)

This is the Go port of `@tabnas/debug`. It is **not** canonical: it
mirrors the TypeScript implementation in `../ts`, which is the source of
truth. See [../AGENTS.md](../AGENTS.md) for the parity rules.

- Source: `debug.go`. Names mirror the TS originals in Go casing
  (`Describe`, `Options`, `Defaults`, `Trace`).
- Tests: `debug_test.go`, mirroring `../ts/test/debug.test.js`.
- The `tabnas` parser module (`github.com/rjrodger/tabnas/go`, pinned in
  `go.mod`) must be available to build or test.

```bash
go build ./...
go test ./...
```

Do not add behaviour here that does not exist in `../ts`. If the TS side
gains a feature, port it; if it loses one, remove it here too.
