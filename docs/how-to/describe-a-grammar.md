# How to describe a grammar

Goal: get a readable dump of a parser's active configuration — its
tokens, rules, alternates, lexer matchers and loaded plugins — without
running a parse.

## TypeScript

1. Load the plugin with printing and tracing off; you only want the
   `describe` method:

   ```js
   const tn = new Tabnas()
   tn.use(Debug, { print: false, trace: false })
   ```

2. Call `describe` and use the returned string however you like:

   ```js
   console.log(tn.debug.describe())
   ```

## Go

`Describe` is a package function — you do not need to load the plugin to
call it. It returns `(string, error)`: unlike the TypeScript
`describe()`, the Go form never panics, returning an `"internal"`-code
error instead if the grammar spec is unrenderable:

```go
j := tabnas.Make()
report, err := debug.Describe(j)
if err != nil {
	// handle the error; report is "" on failure
}
fmt.Println(report)
```

## Rust

`describe` is a free function here too, and an infallible one: the Rust
engine's accessors cannot fail, so there is no error to return. As in
Go, no plugin has to be installed to call it:

```rust
use tabnas::Tabnas;

let parser = Tabnas::new();
println!("{}", tabnas_debug::describe(&parser));
```

For the same information as data rather than text, call
`tabnas_debug::model(&parser)`, which returns a serialisable
`DebugModel`.

## Reading the output

The report is divided into eight labelled sections in a fixed order:
`INSTANCE`, `TOKENS`, `RULES`, `ALTS`, `LEXER`, `CONFIG`, `PLUGIN` and
`ABNF`. The
[Reference](../reference.md#describing-a-grammar) explains each, and
notes where the Go and Rust output is summarised relative to TypeScript.

## Diffing two grammars

Because the section order and headers are stable and identical across
all three implementations, you can capture the output before and after a
change — or one language against the other — and diff the strings to see
what differs. The shared `test/spec/sections.tsv` fixture is what keeps
that true: every runtime runs it, over the same named grammars.
