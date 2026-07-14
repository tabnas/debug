# How to silence the per-`use` grammar dump

Goal: stop the plugin from printing the whole grammar every time another
plugin is loaded, while keeping `describe()` and tracing available.

## Why this happens

When `print` is enabled, every later plugin load prints the current
grammar — in TypeScript because the plugin wraps the instance's `use`
method, in Go for loads made through `debug.Use(j, plugin, opts...)`
(the Go engine's `Use` method cannot be wrapped in place, so plugins
loaded directly via `j.Use` never trigger the dump). In a project that
loads several plugins, this produces a large dump per load. The `print`
option controls only this; it is independent of tracing.

## Turn printing off

Set `print: false` when loading the plugin:

```js
tn.use(Debug, { print: false, trace: true })
```

```go
j.Use(debug.Debug, map[string]any{"print": false, "trace": true})
```

You keep `describe` and tracing; you just stop the automatic dump.

## Get the description on demand instead

With printing off, call `describe` yourself when you want it — see
[Describe a grammar](describe-a-grammar.md).

## Note on load order

Printing wraps `use` at the moment the debug plugin is loaded, so it only
reports plugins loaded *after* it. Load the debug plugin first to see the
effect of every later `use`.
