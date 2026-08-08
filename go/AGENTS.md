# Agent guide: go/ (parity)

This is the Go port of `@tabnas/debug`. It is **not** canonical: it
tracks the TypeScript implementation in `../ts`, which is the source of
truth. See [../AGENTS.md](../AGENTS.md) for the parity rules and the list
of intentional TS/Go differences.

- Source: `debug.go` (the `Debug` plugin, `Describe(j)`, `Abnf(j)`, the
  ABNF emitter, and `Defaults`), `model.go` (`Model(j)` and the nine
  `Debug*` types), `trace.go` (the six trace kinds).
- Tests: `debug_test.go` and `model_test.go`, mirroring
  `../ts/test/debug.test.js`; `parity_test.go` runs the shared
  `../test/spec/*.tsv` fixtures, as `../ts/test/parity.test.js` does.
- Module `github.com/tabnas/debug/go`, requiring the engine module
  `github.com/tabnas/parser/go`.

```bash
GOWORK=off go build ./... && GOWORK=off go vet ./... && GOWORK=off go test ./...
```

(`../scripts/fetch-parser.sh` is legacy and is NOT a prerequisite — this
module has no `replace` pointing at `vendor/`, so fetching changes
nothing here. See [../AGENTS.md](../AGENTS.md).)

### A local green is not a CI green

**`go.mod` carries no `replace`, so the `GOWORK=off` commands above
resolve the engine at its pinned published version from the module
proxy** — not the `vendor/` copy `fetch-parser.sh` downloads, and not the
sibling checkout CI builds. Anything added to the engine on `main` but
not yet released is invisible here and enforced there.

(Dropping `GOWORK=off` resolves the sibling `../../parser/go` instead,
because the repo-set `../../go.work` lists `./debug/go`. That is a
useful second opinion, but the Makefile deliberately uses `GOWORK=off`,
so a green `make test` alone does not exercise sibling `main`.)

That gap has already cost one red build: a test bound a fixed literal to
`#AA`, which an unreleased engine change rejects (matcher tokens cannot be
rebound). It passed locally and panicked in CI.

Before pushing a Go change, re-run against the engine CI actually uses:

```bash
go mod edit -replace github.com/tabnas/parser/go=/path/to/parser/go
go test ./...
go mod edit -dropreplace github.com/tabnas/parser/go    # do not commit the replace
```

The TypeScript side does not have this gap in the fleet checkout:
`ts/package.json` asks for `"@tabnas/parser": "*"`, but
`ts/node_modules/@tabnas/parser` is a symlink to the sibling
`../../parser/ts`, wired by `admin/scripts/link.sh`, so `npm test`
already runs against sibling `main`. An `npm ci` or a wiped
`node_modules` silently replaces that symlink with a registry copy and
reintroduces the gap.

## API notes

The Go engine differs from the TypeScript engine, so this port uses Go
idioms, not a literal translation:

- Tracing installs `Tabnas.Sub(lexSub, ruleSub)` for the `lex`/`rule`
  streams, and derives the other four kinds (`step`, `parse`, `node`,
  `stack`) from a parse-`Prepare` hook plus after-open/after-close rule
  state actions — the closest hooks to the TS engine's post-match log
  points. All six kinds are individually selectable via `opts["trace"]`,
  and output goes to `opts["out"]` (an `io.Writer`) or `os.Stdout`.
- The `print` option exists, but as the package function
  `Use(j, plugin, opts...)`: `(*Tabnas).Use` is a concrete method and
  cannot be reassigned the way the TS plugin reassigns `tabnas.use`.
- Introspection for `Describe`/`Model` reads exported accessors:
  `j.Config()` (`LexConfig`: `TinNames`, `FixedTokens`, `CustomMatchers`,
  lex flags), `j.RSM()` (rule specs and their `OpenAlts()`/`CloseAlts()`
  `[]*AltSpec`), `j.Plugins()`, `j.TinName(tin)`, `j.TokenSet(name)`,
  `j.Options()`.
- Every exported entry point returns `(value, error)` and recovers
  panics into an `"internal"`-code `*tabnas.TabnasError`, mirroring the
  engine's no-panic guarantee. Keep that property when adding one.
- Slices in `DebugModel` are initialised empty, never left nil: a nil
  slice marshals to `null` and would break the cross-runtime JSON
  comparability that `../test/spec/model.tsv` pins.

Keep the `describe` section headers identical to TS. When TS gains or
loses behaviour, port it here if the engine API allows; if it cannot be
matched, record the difference in `../docs/reference.md`.
