# Agent guide: @tabnas/debug

This file orients AI coding agents working in this repository. Keep it
accurate: when the structure, commands, or conventions below change,
update this file in the same change.

## What this repository is

A debug plugin for the [`tabnas`](https://github.com/tabnas/parser)
parsing engine. It provides:

- a grammar dump — `describe()` (TypeScript) / `Describe(j)` (Go) — that
  reports tokens, rules, alternates, lexer matchers and plugins; and
- parse tracing that logs events as the parser runs.

The plugin is a developer tool, not part of the parse path.

## Layout

| Path | What it is |
|---|---|
| `ts/` | TypeScript / JavaScript implementation (`@tabnas/debug`). **Canonical.** |
| `go/` | Go implementation (`github.com/tabnas/debug/go`). Tracks `ts/`. |
| `docs/` | Cross-language documentation (see `docs/README.md`). |
| `scripts/fetch-parser.sh` | Downloads + builds the engine into `vendor/`. |
| `vendor/` | The fetched engine (git-ignored; created by the script). |
| `.github/workflows/build.yml` | CI: builds and tests both implementations. |

## The parser engine dependency

The engine lives at `github.com/tabnas/parser` — npm package `tabnas`
(in `parser/ts`) and Go module `github.com/tabnas/parser/go`. It is **not
published to a registry**, so both implementations consume it from
source:

- `scripts/fetch-parser.sh` downloads the engine's GitHub `main` branch
  over HTTPS into `vendor/tabnas-parser` and builds its TypeScript
  `dist/`. Pin a different ref with `TABNAS_PARSER_REF`; set
  `TABNAS_PARSER_SKIP_TS_BUILD=1` to skip the TS build (Go-only).
- TypeScript references it as `"tabnas": "file:../vendor/tabnas-parser/ts"`
  in `ts/package.json`.
- Go requires `github.com/tabnas/parser/go` with a `replace` pointing at
  `../vendor/tabnas-parser/go` in `go/go.mod`.

Always run the fetch script before installing/building; the Makefile and
CI do this automatically.

## Build and test

From the repository root:

```bash
make build   # fetch engine, build both implementations
make test    # fetch engine, build + test both
```

Targeted: `make test-ts`, `make test-go` (each fetches the engine first).
Both currently pass: TS via Node's test runner, Go via `go test ./...`.

## The parity rule

**TypeScript is canonical.** `ts/src/debug.ts` is the source of truth for
behaviour, option names, defaults, output format and section ordering.

When you change behaviour: change TypeScript first, then update Go to
match as far as the Go engine API allows.

The two engines are not API-identical, so some divergence is real and
**intended**, not drift:

- TypeScript traces six kinds (`step`, `rule`, `lex`, `parse`, `node`,
  `stack`) via a context-log hook; the Go engine exposes two streams
  (`lex`, `rule`) via `Tabnas.Sub`.
- TypeScript has a `print` option that wraps `use`; the Go engine has no
  such hook, so the Go plugin omits it.
- TypeScript attaches `describe` as an instance method; in Go,
  `Describe(j)` is a package function.
- Go's `LEXER`/`PLUGIN` sections are summarised — limited to what the
  engine's public accessors expose.

These are documented in `docs/reference.md`. Keep the shared parts —
option semantics, `Defaults`, and the `describe` section headers — in
lockstep, and record any new divergence in the reference.

## Conventions

- Keep the `describe` section headers (`========= TOKENS ========`, etc.)
  byte-for-byte identical across both implementations so output diffs.
- Tests mirror each other: `ts/test/debug.test.js`, `go/debug_test.go`.
- Go: run `gofmt` and `go vet ./...` before committing.

## Documentation

Docs in `docs/` are organised by purpose: a learning-oriented tutorial,
task-oriented how-to guides, a reference, and an explanation. When you
add a capability, extend the reference and add a how-to if it introduces
a new task.
