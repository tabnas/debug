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

This repository consumes the engine from source. From the repository
root, fetch it first, then build and test:

```bash
./scripts/fetch-parser.sh   # downloads + builds the engine into vendor/
cd go && go build ./... && go vet ./... && go test ./...
```

Or, from the repository root, `make test-go` does all of the above. The
`go.mod` `replace` directive points the `github.com/tabnas/parser/go`
requirement at the fetched copy in `../vendor`.

## License

MIT.
