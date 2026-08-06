# Agent guide: go/ (parity)

This is the Go port of `@tabnas/debug`. It is **not** canonical: it
tracks the TypeScript implementation in `../ts`, which is the source of
truth. See [../AGENTS.md](../AGENTS.md) for the parity rules and the list
of intentional TS/Go differences.

- Source: `debug.go`. Provides `Debug` (a `tabnas.Plugin`), `Describe(j)`
  (a package function), and `Defaults` (a `map[string]any`).
- Tests: `debug_test.go`, mirroring `../ts/test/debug.test.js`.
- Module `github.com/tabnas/debug/go`, requiring the engine module
  `github.com/tabnas/parser/go`.

```bash
TABNAS_PARSER_SKIP_TS_BUILD=1 ../scripts/fetch-parser.sh
go build ./... && go vet ./... && go test ./...
```

### A local green is not a CI green

**`go.mod` carries no `replace`, so the commands above resolve the engine
at its pinned published version from the module proxy** — not the
`vendor/` copy `fetch-parser.sh` just downloaded, and not the sibling
checkout CI builds. Anything added to the engine on `main` but not yet
released is invisible here and enforced there.

That gap has already cost one red build: a test bound a fixed literal to
`#AA`, which an unreleased engine change rejects (matcher tokens cannot be
rebound). It passed locally and panicked in CI.

Before pushing a Go change, re-run against the engine CI actually uses:

```bash
go mod edit -replace github.com/tabnas/parser/go=/path/to/parser/go
go test ./...
go mod edit -dropreplace github.com/tabnas/parser/go    # do not commit the replace
```

The same gap exists on the TypeScript side — `ts/package.json` asks for
`"@tabnas/parser": "*"`, which npm resolves from the registry — so point
`ts/node_modules/@tabnas/parser` at `../vendor/tabnas-parser/ts` to test
against `main` there too.

## API notes

The Go engine differs from the TypeScript engine, so this port uses Go
idioms, not a literal translation:

- Tracing is installed via `Tabnas.Sub(lexSub, ruleSub)` — two streams
  (`lex`, `rule`). There is no per-kind selection or `print` option.
- Introspection for `Describe` reads exported accessors: `j.Config()`
  (`LexConfig`: `TinNames`, `FixedTokens`, `CustomMatchers`, lex flags),
  `j.RSM()` (rule specs and their `OpenAlts()`/`CloseAlts()` `[]*AltSpec`),
  `j.Plugins()`, `j.TinName(tin)`.

Keep the `describe` section headers identical to TS. When TS gains or
loses behaviour, port it here if the engine API allows; if it cannot be
matched, record the difference in `../docs/reference.md`.
