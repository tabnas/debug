# How to silence the per-`use` grammar dump

Goal: stop the plugin from printing the whole grammar every time another
plugin is loaded, while keeping `describe()` and/or tracing available.

## Why this happens

When `print` is enabled, the plugin wraps the instance's `use` method so
that every subsequent `use(...)` call prints the current grammar
description. In a project that loads several plugins, this produces a
large dump per load. The `print` option controls only this behaviour;
it is independent of tracing.

## Turn printing off

Set `print: false` (TypeScript) or `Print: false` (Go) when loading the
plugin.

TypeScript:

```js
am.use(Debug, { print: false, trace: true })
```

Go:

```go
am.Use(debug.Debug, &debug.Options{Print: false, Trace: debug.Defaults.Trace})
```

You keep the `describe` method and tracing; you just stop the automatic
dump on each `use`.

## Get the description on demand instead

With printing off, call `describe` yourself exactly when you want it —
see [Describe a grammar](describe-a-grammar.md).

## Note on load order

Printing wraps `use` at the moment the debug plugin is loaded, so it only
reports plugins loaded *after* the debug plugin. Load the debug plugin
first if you want to see the effect of every later `use`.
