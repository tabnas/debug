# How to describe a grammar

Goal: get a readable dump of a parser's active configuration — its
tokens, rules, alternates, lexer matchers and loaded plugins — without
running a parse or producing trace noise.

## Steps

1. Load the plugin with both printing and tracing off. You only want the
   `describe` method, not its side effects.

   TypeScript:

   ```js
   const am = new Tabnas()
   am.use(Debug, { print: false, trace: false })
   ```

   Go:

   ```go
   am := tabnas.New()
   am.Use(debug.Debug, &debug.Options{Print: false, Trace: nil})
   ```

2. Call `describe` and use the returned string however you like — print
   it, write it to a file, or compare it against a previous run.

   ```js
   const report = am.debug.describe()
   console.log(report)
   ```

   ```go
   report := am.Debug.Describe()
   fmt.Println(report)
   ```

## Reading the output

The report is divided into labelled sections in a fixed order:
`TOKENS`, token sets, `RULES`, `ALTS`, `LEXER` and `PLUGIN`. The
[Reference](../reference.md#describe-output) explains each section.

## Diffing two grammars

Because `describe` returns a plain string and the section order is
stable, you can capture it before and after adding a plugin or rule and
diff the two strings to see exactly what changed. The TypeScript and Go
implementations use identical section headers, so their output can be
diffed against each other as a parity check.
