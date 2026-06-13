# How to choose which events to trace

Goal: cut trace noise down to only the event kinds you care about.

## TypeScript

The `trace` option accepts, instead of `true`, a map of kind → on/off.
Any kind set to a falsy value is suppressed. The recognised kinds are
`step`, `rule`, `lex`, `parse`, `node` and `stack`.

Trace only lexing and rule events:

```js
am.use(Debug, {
  print: false,
  trace: { lex: true, rule: true }, // others default to off
})
```

Start from "everything" and switch a few off:

```js
am.use(Debug, {
  print: false,
  trace: { step: true, rule: true, lex: true, parse: true, node: false, stack: false },
})
```

Turn tracing off entirely — pass `false`:

```js
am.use(Debug, { print: false, trace: false })
```

## Go

The Go engine drives tracing through two subscriber streams, so tracing
is all-or-nothing across the `lex` and `rule` kinds rather than
selectable per kind. Turn it on or off with the `"trace"` flag:

```go
j.Use(debug.Debug, map[string]any{"trace": true})  // lex + rule
j.Use(debug.Debug, map[string]any{"trace": false}) // off
```

If you need only one stream, subscribe directly with the engine's
`Sub` method instead of loading the plugin, passing `nil` for the stream
you do not want.

See the [Reference](../reference.md#trace-output) for what each kind
logs and the [documented differences](../reference.md#documented-differences-go-vs-canonical-typescript).
