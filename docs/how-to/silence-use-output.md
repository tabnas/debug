# How to silence the per-`use` grammar dump (TypeScript)

Goal: stop the plugin from printing the whole grammar every time another
plugin is loaded, while keeping `describe()` and tracing available.

> This applies to the **TypeScript** implementation. The Go engine has no
> `use`-wrapping hook, so the Go plugin has no `print` option and never
> dumps the grammar automatically — call `debug.Describe(j)` when you
> want it.

## Why this happens

When `print` is enabled, the plugin wraps the instance's `use` method so
that every subsequent `use(...)` call prints the current grammar. In a
project that loads several plugins, this produces a large dump per load.
The `print` option controls only this; it is independent of tracing.

## Turn printing off

Set `print: false` when loading the plugin:

```js
am.use(Debug, { print: false, trace: true })
```

You keep `describe` and tracing; you just stop the automatic dump.

## Get the description on demand instead

With printing off, call `describe` yourself when you want it — see
[Describe a grammar](describe-a-grammar.md).

## Note on load order

Printing wraps `use` at the moment the debug plugin is loaded, so it only
reports plugins loaded *after* it. Load the debug plugin first to see the
effect of every later `use`.
