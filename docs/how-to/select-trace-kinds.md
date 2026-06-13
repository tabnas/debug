# How to choose which events to trace

Goal: cut trace noise down to only the event kinds you care about — for
example, lexer matches but not the rule stack.

The trace option accepts, instead of `true`, a map of kind → on/off. Any
kind set to a falsy value is suppressed; the recognised kinds are
`step`, `rule`, `lex`, `parse`, `node` and `stack`.

## Trace only lexing and rule events

TypeScript:

```js
am.use(Debug, {
  print: false,
  trace: { lex: true, rule: true }, // others default to off
})
```

Go:

```go
am.Use(debug.Debug, &debug.Options{
	Print: false,
	Trace: map[string]bool{"lex": true, "rule": true},
})
```

Kinds you do not list are treated as off, so only `lex` and `rule` lines
appear.

## Start from "everything" and switch a few off

TypeScript:

```js
am.use(Debug, {
  print: false,
  trace: { step: true, rule: true, lex: true, parse: true, node: false, stack: false },
})
```

Go — copy the defaults, then disable what you do not want:

```go
trace := map[string]bool{}
for k, v := range debug.Defaults.Trace {
	trace[k] = v
}
trace["node"] = false
trace["stack"] = false

am.Use(debug.Debug, &debug.Options{Print: false, Trace: trace})
```

## Turn tracing off entirely

Pass `false` (TypeScript) or `nil` (Go) for the trace option. With no
kind enabled, the plugin does not register its trace hook at all.

```js
am.use(Debug, { print: false, trace: false })
```

```go
am.Use(debug.Debug, &debug.Options{Print: false, Trace: nil})
```

See the [Reference](../reference.md#trace-kinds) for what each kind logs.
