# How to choose which events to trace

Goal: cut trace noise down to only the event kinds you care about.

## TypeScript

The `trace` option accepts, instead of `true`, a map of kind → on/off.
Any kind set to a falsy value is suppressed. The recognised kinds are
`step`, `rule`, `lex`, `parse`, `node` and `stack`.

Trace only lexing and rule events. The engine deep-merges
`Debug.defaults` (all kinds on) with your object, so a partial map cannot
turn other kinds off implicitly — disable them explicitly:

```js
tn.use(Debug, {
  print: false,
  trace: { lex: true, rule: true, step: false, parse: false, node: false, stack: false },
})
```

Start from "everything" and switch a few off:

```js
tn.use(Debug, {
  print: false,
  trace: { step: true, rule: true, lex: true, parse: true, node: false, stack: false },
})
```

Turn tracing off entirely — pass `false`:

```js
tn.use(Debug, { print: false, trace: false })
```

## Go

The `"trace"` option accepts the same shapes: `true` (all kinds),
`false` (off), or a per-kind map merged over the all-true defaults (so,
as in TypeScript, disable unwanted kinds explicitly):

```go
j.Use(debug.Debug, map[string]any{"trace": true})  // all kinds
j.Use(debug.Debug, map[string]any{"trace": false}) // off
j.Use(debug.Debug, map[string]any{"trace": map[string]any{
	"rule": true,
	"lex":  false, "parse": false, "node": false, "stack": false, "step": false,
}}) // rule lines only
```

See the [Reference](../reference.md#trace-output) for what each kind
logs.
