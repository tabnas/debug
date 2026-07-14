# Tutorial: inspect your first grammar (Go)

This walkthrough takes you from nothing to a working result: load the
debug plugin onto a parser instance, dump a description of its grammar,
render it as ABNF, and trace a parse. One happy path, step by step.

This is the Go port of the [canonical TypeScript tutorial](../../ts/doc/tutorial.md).
The TypeScript version is authoritative; the Go API differs in shape (see
[concepts](concepts.md) for the differences). The Go package is imported
as `tabnasdebug`.

You need the [`tabnas`](https://github.com/tabnas/parser) parser engine
and the `github.com/tabnas/debug/go` package. In this repository the
engine is consumed from source; fetch it first:

```bash
./scripts/fetch-parser.sh
cd go && go test ./...
```

## 1. Load the plugin

```go
package main

import (
	"fmt"

	tabnas "github.com/tabnas/parser/go"
	tabnasdebug "github.com/tabnas/debug/go"
)

func main() {
	j := tabnas.Make(tabnas.Options{Tag: "demo"})

	// Describe returns (string, error): it never panics.
	report, err := tabnasdebug.Describe(j)
	if err != nil {
		panic(err)
	}
	fmt.Println(report)
}
```

A bare instance carries no grammar of its own — the engine ships none — so
the report is mostly empty. Add a grammar so it has content.

## 2. Build a small grammar by hand

Here is a recogniser for sums like `1+2+3`: a `val` rule that pushes into
an `add` rule, where `add` matches a number and then optionally a `+`
followed by another `add`.

```go
package main

import (
	"fmt"

	tabnas "github.com/tabnas/parser/go"
	tabnasdebug "github.com/tabnas/debug/go"
)

func buildAddGrammar() *tabnas.Tabnas {
	plus := "+"
	j := tabnas.Make(tabnas.Options{
		Fixed: &tabnas.FixedOptions{Token: map[string]*string{"#PL": &plus}},
		Rule:  &tabnas.RuleOptions{Start: "val"},
	})
	zz := j.Token("#ZZ")
	nr := j.Token("#NR")
	pl := j.Token("#PL")

	j.Rule("val", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "add"})
	})
	j.Rule("add", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{nr}}})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{pl}}, R: "add"})
		rs.AddClose(&tabnas.AltSpec{})              // epsilon close: optional
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{zz}}}) // #ZZ end
	})
	return j
}

func main() {
	j := buildAddGrammar()
	report, _ := tabnasdebug.Describe(j)
	fmt.Println(report)
}
```

## 3. Read the description

`Describe` returns the grammar as a single string, organised into labelled
sections in this exact order: `INSTANCE`, `TOKENS`, `RULES`, `ALTS`,
`LEXER`, `CONFIG`, `PLUGIN`, `ABNF`.

For the `add` grammar above, the `RULES` section shows the push/replace
transition tree — `val` open-pushes into `add` (`op: add`), and `add`
close-replaces back into `add` (`cr: add`) — and the `ALTS` section shows
each alternate's token sequence and actions.

The error return is the Go difference: `Describe` upholds the engine's
no-panic guarantee, so a malformed grammar is rendered defensively and any
internal failure comes back as an `"internal"`-code error rather than a
crash. For a well-formed instance the error is `nil`.

## 4. Render the grammar as ABNF

`Abnf` re-expresses the live grammar as
[ABNF](https://www.rfc-editor.org/rfc/rfc5234) text — productions, then a
legend defining each token:

```go
package main

import (
	"fmt"

	tabnasdebug "github.com/tabnas/debug/go"
)

func main() {
	j := buildAddGrammar() // from step 2

	out, err := tabnasdebug.Abnf(j)
	if err != nil {
		panic(err)
	}
	fmt.Println(out)
	// val = add
	// add = NR [ PL add ]
	//
	// NR = <number>
	// PL = "+"
}
```

The optional `+ add` continuation became `[ PL add ]`, `PL` is the literal
`"+"`, and the number token — which has no ABNF literal — is shown as the
prose value `<number>`. Like `Describe`, `Abnf` returns `(string, error)`.

## 5. Trace a parse

Enable tracing and the plugin logs one line per parse event. By default
the lines go to `os.Stdout`; pass an `io.Writer` under `opts["out"]` to
capture them:

```go
package main

import (
	"bytes"
	"fmt"

	tabnas "github.com/tabnas/parser/go"
	tabnasdebug "github.com/tabnas/debug/go"
)

func main() {
	j := buildAddGrammar() // from step 2

	var buf bytes.Buffer
	if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": true, "out": &buf}); err != nil {
		panic(err)
	}

	if _, err := j.Parse("1+2"); err != nil {
		panic(err)
	}
	fmt.Print(buf.String())
	// ========= TRACE ==========
	//   step   0:
	//   stack  1+2 ...
	//   rule   1+2 ... val~0:OPEN  prev=-1 parent=-1 child=-1
	//   lex    +2  ... #NR  1  0  1:1
	//   parse  ...     alt  ...
	//   node   ...     why=O <...>
	// ... and so on
}
```

The trace mirrors the TypeScript kinds: `step` (loop counter), `stack`
(the rule stack and partial nodes), `rule` (a rule opening or closing),
`lex` (one line per token), `parse` (the alternate match result) and
`node` (the node built so far). Most lines lead with the upcoming source,
the token window and the parse depth. Read them top to bottom to watch
the parser descend into the input and come back out. Pass a per-kind map
under `"trace"` to select individual streams — see the
[guide](guide.md).

## What you have learned

You loaded the plugin, dumped a grammar (`Describe`), rendered it as ABNF
(`Abnf`), and traced a parse (`trace: true`, captured via `out`). From
here:

- The [how-to guide](guide.md) has focused recipes.
- The [reference](reference.md) is the exact API surface.
- [Concepts](concepts.md) explains how it works and how the Go API differs
  from the TypeScript original.
