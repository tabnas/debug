# tabnas-debug (Go)

Debug plugin for the [`tabnas`](https://github.com/rjrodger/tabnas) parser.

Adds tracing helpers and a `Describe()` method to a `*tabnas.Tabnas`
instance. This is the Go port of the canonical TypeScript implementation
in [`../ts`](../ts); the TypeScript version is authoritative and this
package is kept at parity with it.

## Install

```bash
go get github.com/rjrodger/tabnas/go
go get github.com/rjrodger/tabnas-debug/go
```

## Use

```go
package main

import (
	"fmt"

	tabnas "github.com/rjrodger/tabnas/go"
	debug "github.com/rjrodger/tabnas-debug/go"
)

func main() {
	am := tabnas.New()
	am.Use(debug.Debug, &debug.Options{Print: false, Trace: nil})
	fmt.Println(am.Debug.Describe())
}
```

## Build and test

```bash
go build ./...
go test ./...
```

Both require the `tabnas` parser module to be available; see
[`go.mod`](go.mod) for the pinned version.

## License

MIT.
