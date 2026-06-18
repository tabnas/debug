# tabnas-debug (Go)

Debug plugin for the [`tabnas`](https://github.com/tabnas/parser) parser
engine.

Adds parse tracing and a `Describe` function that dumps a parser
instance's active grammar. This is the Go port of the canonical
TypeScript implementation in [`../ts`](../ts); the TypeScript version is
authoritative and this package tracks it. Because the Go engine exposes
tracing and introspection through different idioms than TypeScript, the
Go surface differs in shape — see [the reference](../docs/reference.md)
for the details.

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

	// Trace a parse (lex + rule events go to stdout).
	j.Use(tabnasdebug.Debug, map[string]any{"trace": true})
	j.Parse("a:1")
}
```

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
