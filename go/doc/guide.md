# How-to guide (Go)

Focused recipes for real tasks with the Go debug package. This is the Go
port of the [TypeScript how-to guide](../../ts/doc/guide.md); the Go API
differs in shape (see [concepts](concepts.md)). The package is imported as
`tabnasdebug`.

```go
import (
	tabnas "github.com/tabnas/parser/go"
	tabnasdebug "github.com/tabnas/debug/go"
)
```

## Dump a grammar description

`Describe` returns the grammar as a string with labelled sections, plus an
error (it never panics):

```go
report, err := tabnasdebug.Describe(j)
if err != nil {
	// "internal"-code *tabnas.TabnasError on a recovered panic.
	panic(err)
}
fmt.Println(report)
```

The sections, in order, are `INSTANCE`, `TOKENS`, `RULES`, `ALTS`,
`LEXER`, `CONFIG`, `PLUGIN`, `ABNF`.

## Assert on a section

`Describe` returns text, so assert on a header or a known row. (Unlike the
TypeScript port there is no `model()` — see [concepts](concepts.md) — so
in Go you read the text.)

```go
report, _ := tabnasdebug.Describe(j)

if !strings.Contains(report, "========= RULES =========") {
	t.Error("missing RULES section")
}
if !strings.Contains(report, "op: add") {
	t.Error("expected val to open-push into add")
}
```

To pull out one section, slice between its header and the next:

```go
func sectionOf(report, start, end string) string {
	si := strings.Index(report, start)
	ei := strings.Index(report, end)
	if si < 0 || ei < 0 || ei < si {
		return report[max(si, 0):]
	}
	return report[si:ei]
}

rules := sectionOf(report, "========= RULES =========", "========= ALTS =========")
```

## Render a grammar as ABNF

`Abnf` emits the live grammar as ABNF text. It reads only the running
engine — it never imports an ABNF library — so it works on any grammar.

```go
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
```

Constructs ABNF cannot express (an arbitrary match regex) are emitted as
ABNF comments (`; /.../`) so the output stays valid text; such a grammar
will not round-trip. See [concepts](concepts.md) for the round-trip
contract.

## Trace a parse to stdout

Load the plugin with tracing enabled. With no `out` writer, the lines go
to `os.Stdout`:

```go
if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": true}); err != nil {
	panic(err)
}
j.Parse("1+2")
```

## Capture trace output (e.g. in a test)

Pass an `io.Writer` under `opts["out"]` and the trace subscribers write
there instead of `os.Stdout`:

```go
var buf bytes.Buffer
if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": true, "out": &buf}); err != nil {
	t.Fatal(err)
}
if _, err := j.Parse("1+2"); err != nil {
	t.Fatal(err)
}

out := buf.String()
if !strings.Contains(out, "[rule]") {
	t.Error("expected rule trace lines")
}
if !strings.Contains(out, "[lex]") {
	t.Error("expected lex trace lines")
}
```

The Go engine surfaces two streams: `[lex]` lines (token name, tin,
source, value, row:col) and `[rule]` lines (rule name, instance, state,
depth, node). The finer TypeScript kinds (`step`, `parse`, `node`,
`stack`) are not available in the Go engine — see [concepts](concepts.md).

## Disable tracing

`trace` defaults to on (`Defaults["trace"]` is `true`). Pass an explicit
`false` to attach the plugin without installing trace subscribers:

```go
j.Use(tabnasdebug.Debug, map[string]any{"trace": false})
```

A per-kind object (the TypeScript shape) is accepted but, because the Go
engine exposes only two streams, any non-`false` value simply turns both
streams on; it cannot select individual kinds.

## Handle the error returns

Every error-returning entry point upholds the engine's no-panic guarantee:
a recovered panic becomes an `"internal"`-code `*tabnas.TabnasError`.

```go
report, err := tabnasdebug.Describe(j)
if err != nil {
	var te *tabnas.TabnasError
	if errors.As(err, &te) && te.Code == "internal" {
		// an internal failure was recovered, not a crash
	}
}
```

A malformed grammar (a nil rule spec, a nil alternate) is rendered
defensively — a nil alternate shows as `***INVALID***` — rather than
returning an error, so `Describe` still produces a useful dump while you
are mid-edit.
