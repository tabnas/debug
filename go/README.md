# tabnas-debug (Go)

Debug / introspection plugin for the
[`tabnas`](https://github.com/tabnas/parser) parser engine, package
`tabnasdebug`.

It makes a grammar *visible*: `Describe` dumps an instance's installed
grammar (tokens, rules, plugins) as labelled text, `Model` returns the
same information as structured, JSON-serialisable data, `Abnf`
re-expresses it as ABNF, and `trace` logs a parse step by step (the
TypeScript trace kinds `step`, `rule`, `lex`, `parse`, `node`, `stack`).
A dev/test aid — never a runtime dependency.

This is the Go port of the canonical TypeScript implementation in
[`../ts`](../ts); the TypeScript version is authoritative and this package
tracks it. The Go engine exposes tracing and introspection through
different idioms, so the surface differs in shape — package functions
returning errors instead of instance methods, and a `tabnasdebug.Use`
wrapper for the `print` option. See [the concepts doc](doc/concepts.md)
and [reference](doc/reference.md) for the details.

## Install

```bash
go get github.com/tabnas/parser/go
go get github.com/tabnas/debug/go
```

## Use

```go
package main

import (
	"fmt"

	tabnas "github.com/tabnas/parser/go"
	tabnasdebug "github.com/tabnas/debug/go"
)

func main() {
	j := tabnas.Make()

	// Describe the grammar. Describe returns (string, error): it never
	// panics, surfacing any failure as an "internal"-code error instead.
	report, err := tabnasdebug.Describe(j)
	if err != nil {
		panic(err)
	}
	fmt.Println(report)

	// Trace a parse (lex + rule events go to stdout by default).
	j.Use(tabnasdebug.Debug, map[string]any{"trace": true})
	j.Parse("a:1")
}
```

## Documentation

- [Tutorial](doc/tutorial.md) — zero to a working inspection, step by step.
- [How-to guide](doc/guide.md) — focused recipes.
- [Reference](doc/reference.md) — the exact exports, options and output.
- [Concepts](doc/concepts.md) — how it works and how it differs from the
  TypeScript version.

## Build and test

The engine is an ordinary module requirement pinned in `go.mod` — there
is no `replace` and nothing to fetch:

```bash
cd go && GOWORK=off go build ./... && GOWORK=off go vet ./... && GOWORK=off go test ./...
```

Or, from the repository root, `make test-go`. `GOWORK=off` pins the
engine to that published version; omitting it resolves the sibling
`../../parser/go` via the repo-set `go.work`. CI does the latter — it
generates a workspace over the cloned siblings and tests against parser
`main` — so both resolutions need to pass.

## License

MIT.
