# How to choose which events to trace

Goal: cut trace noise down to only the event kinds you care about.

## TypeScript

The `trace` option accepts, instead of `true`, a map of kind → on/off.
Any kind set to a falsy value is suppressed. The recognised kinds are
`step`, `rule`, `lex`, `parse`, `node` and `stack`.

Trace only lexing and rule events. The engine deep-merges
`Debug.defaults` (all kinds on) with your object, so a partial map cannot
turn other kinds off implicitly. Disable them explicitly:

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

Turn tracing off entirely by passing `false`:

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

## Rust

Rust options are a typed struct rather than an option map, so a selection
is a `TraceKinds` value with the same six fields:

```rust
use tabnas_debug::{apply, DebugOptions, TraceKinds};

// All kinds.
apply(&mut parser, DebugOptions::default())?;

// Off.
apply(&mut parser, DebugOptions::default().without_trace())?;

// Rule and lex lines only. `TraceKinds::none()` starts from everything
// off, so nothing has to be disabled explicitly.
apply(
    &mut parser,
    DebugOptions::default().with_trace(TraceKinds {
        rule: true,
        lex: true,
        ..TraceKinds::none()
    }),
)?;
```

That is the one shape difference worth knowing: TypeScript and Go merge a
partial selection over an all-on default, so an unwanted kind has to be
turned off by name, while a Rust `TraceKinds` literal states the whole
selection at once.

`step` is accepted for option-name parity but never logs, because the
Rust engine emits no per-step event. Selecting `step` alone therefore
installs no tracing at all, not even the per-parse banner.

Re-applying the plugin replaces the selection rather than adding to it,
including with `without_trace()`, which turns tracing off on an instance
that was already tracing.

See the [Reference](../reference.md#trace-output) for what each kind
logs.
