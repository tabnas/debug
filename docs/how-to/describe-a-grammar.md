# How to describe a grammar

Goal: get a readable dump of a parser's active configuration — its
tokens, rules, alternates, lexer matchers and loaded plugins — without
running a parse.

## TypeScript

1. Load the plugin with printing and tracing off; you only want the
   `describe` method:

   ```js
   const am = new Tabnas()
   am.use(Debug, { print: false, trace: false })
   ```

2. Call `describe` and use the returned string however you like:

   ```js
   console.log(am.debug.describe())
   ```

## Go

`Describe` is a package function — you do not need to load the plugin to
call it:

```go
j := tabnas.Make()
report := debug.Describe(j)
fmt.Println(report)
```

## Reading the output

The report is divided into labelled sections in a fixed order:
`TOKENS`, `RULES`, `ALTS`, `LEXER` and `PLUGIN`. The
[Reference](../reference.md#describing-a-grammar) explains each, and
notes where the Go output is summarised relative to TypeScript.

## Diffing two grammars

Because the section order and headers are stable and identical across
both implementations, you can capture the output before and after a
change — or one language against the other — and diff the strings to see
what differs.
