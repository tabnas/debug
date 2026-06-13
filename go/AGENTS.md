# Agent guide: go/ (parity)

This is the Go port of `@tabnas/debug`. It is **not** canonical: it
tracks the TypeScript implementation in `../ts`, which is the source of
truth. See [../AGENTS.md](../AGENTS.md) for the parity rules and the list
of intentional TS/Go differences.

- Source: `debug.go`. Provides `Debug` (a `tabnas.Plugin`), `Describe(j)`
  (a package function), and `Defaults` (a `map[string]any`).
- Tests: `debug_test.go`, mirroring `../ts/test/debug.test.js`.
- Module `github.com/tabnas/debug/go`. The engine module
  `github.com/tabnas/parser/go` is required with a `replace` pointing at
  `../vendor/tabnas-parser/go`; fetch it with `../scripts/fetch-parser.sh`
  first.

```bash
TABNAS_PARSER_SKIP_TS_BUILD=1 ../scripts/fetch-parser.sh
go build ./... && go vet ./... && go test ./...
```

## API notes

The Go engine differs from the TypeScript engine, so this port uses Go
idioms, not a literal translation:

- Tracing is installed via `Tabnas.Sub(lexSub, ruleSub)` — two streams
  (`lex`, `rule`). There is no per-kind selection or `print` option.
- Introspection for `Describe` reads exported accessors: `j.Config()`
  (`LexConfig`: `TinNames`, `FixedTokens`, `CustomMatchers`, lex flags),
  `j.RSM()` (rule specs and their `Open`/`Close` `[]*AltSpec`),
  `j.Plugins()`, `j.TinName(tin)`.

Keep the `describe` section headers identical to TS. When TS gains or
loses behaviour, port it here if the engine API allows; if it cannot be
matched, record the difference in `../docs/reference.md`.
