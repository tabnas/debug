# Agent guide: @tabnas/debug

This file orients AI coding agents working in this repository. Keep it
accurate: when the structure, commands, or conventions below change,
update this file in the same change.

## What this repository is

A debug plugin for the [`tabnas`](https://github.com/rjrodger/tabnas)
parser. It decorates a parser instance with:

- a `describe()` / `Describe()` method that dumps the active grammar
  (tokens, token sets, rules, alternates, lexer matchers, plugins); and
- optional parse tracing that logs `step`, `rule`, `lex`, `parse`,
  `node` and `stack` events as the parser runs.

The plugin reaches into parser internals (`internal().config`, rule
specs, lexer matchers). It is a developer tool, not part of the parse
path.

## Layout

| Path | What it is |
|---|---|
| `ts/` | TypeScript / JavaScript implementation (`@tabnas/debug`). **Canonical.** |
| `go/` | Go implementation (`github.com/rjrodger/tabnas-debug/go`). Kept at parity. |
| `docs/` | Cross-language documentation (see `docs/README.md`). |
| `.github/workflows/build.yml` | CI: builds and tests both implementations. |

## The parity rule

**TypeScript is canonical.** `ts/src/debug.ts` is the source of truth
for behaviour, option names, defaults, output format and section
ordering.

When you change behaviour:

1. Change `ts/src/debug.ts` first.
2. Mirror the change in `go/debug.go`.
3. Keep the option set, the `Defaults`, the trace kinds, and the
   `describe()` section order and labels identical across both.
4. Update `docs/` and the per-language READMEs if the public surface
   changed.

If you can only change one side, say so explicitly in your summary and
flag the parity gap — do not silently let the two drift.

## Build and test

The `tabnas` parser is a peer dependency pinned to its GitHub `main`
branch (`github:rjrodger/tabnas#main` in `ts/package.json`,
`github.com/rjrodger/tabnas/go` in `go/go.mod`). Both implementations
need the parser present to build or test; neither can be exercised
without it.

From the repository root:

```bash
make build   # build both implementations
make test    # test both implementations
```

Per language:

```bash
make -C ts build && make -C ts test     # or: cd ts && npm i && npm run build && npm test
make -C go build && make -C go test     # or: cd go && go build ./... && go test ./...
```

## Conventions

- Mirror the canonical TS names: TypeScript `describe` ⇄ Go `Describe`,
  `DebugOptions` ⇄ `Options`, `DEFAULTS` ⇄ `Defaults`, `trace` ⇄ `Trace`.
- Trace kinds are exactly: `step`, `rule`, `lex`, `parse`, `node`,
  `stack`. Add or remove kinds in both implementations together.
- Keep the `describe()` section headers (`========= TOKENS ========`,
  etc.) byte-for-byte identical so output can be diffed across languages.
- Tests mirror each other: see `ts/test/debug.test.js` and
  `go/debug_test.go`.

## Documentation

Docs live in `docs/` and are organised by purpose: a learning-oriented
tutorial, task-oriented how-to guides, a reference, and an explanation
of how the plugin works. When you add a capability, extend the
reference and add a how-to if it introduces a new task.
