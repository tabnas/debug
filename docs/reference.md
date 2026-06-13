# Reference

Exact behaviour of the `@tabnas/debug` plugin. The TypeScript
implementation (`ts/src/debug.ts`) is canonical; the Go implementation
(`go/debug.go`) mirrors it. Names are given in both spellings where they
differ only by language casing.

## Entry point

| Language | Symbol | Form |
|---|---|---|
| TypeScript | `Debug` | `Plugin` function: `(tabnas, options) => void` |
| Go | `Debug` | `func(t *tabnas.Tabnas, options *Options)` |

Load it with the parser's `use` / `Use` method:

```js
am.use(Debug, options)
```

```go
am.Use(debug.Debug, options)
```

Loading the plugin:

1. attaches a `describe` / `Describe` method to the instance;
2. wraps `use` / `Use` to print the grammar after each load, when
   `print` is enabled; and
3. registers a parse-prepare hook that installs a trace logger, when any
   trace kind is enabled.

## Options

| Field (TS) | Field (Go) | Type | Meaning |
|---|---|---|---|
| `print` | `Print` | boolean | Print the grammar description after every `use` call. |
| `trace` | `Trace` | per-kind flags | Which parse events to log. See [Trace kinds](#trace-kinds). |

The `trace` option may be:

- `true` (TypeScript) — enable all kinds. The plugin copies the default
  kind set.
- `false` (TypeScript) / `nil` (Go) — disable tracing entirely. No trace
  hook is registered.
- a map/object of kind → boolean — enable only the listed kinds; unlisted
  kinds are off.

## Defaults

| | `print` / `Print` | `trace` / `Trace` |
|---|---|---|
| value | `true` | all six kinds `true` |

In TypeScript the defaults are exposed as `Debug.defaults`; in Go as the
package variable `debug.Defaults`. The parser merges these with
caller-supplied options before invoking the plugin.

## Methods

### `describe()` / `Describe()`

Returns a `string`: a snapshot of the instance's active configuration.
Takes no arguments and has no side effects. Safe to call at any time
after the plugin is loaded.

## Trace kinds

Each enabled kind logs one line per event, under a
`========= TRACE ==========` banner, to the parser's configured console.

| Kind | Logged when | Key fields |
|---|---|---|
| `step` | low-level step | the raw step arguments |
| `rule` | a rule opens or closes | rule name, instance, open/close state, prev/parent/child links, rule counters |
| `lex` | a token is produced | token name, source, offset, row:col, matcher name, the active alternate |
| `parse` | an alternate is evaluated | matched alternate index, its token sequence, push/rule/back actions, condition result, counters |
| `node` | a node is attached | the reason (`why`), the node value, rule counters |
| `stack` | the rule stack is reported | parse state, indented rule stack, indented node stack |

Most lines begin with the parse state: the upcoming source text, the
current token window `[t0 t1]~[tin0 tin1]`, and the parse depth.

## `describe()` output sections

The report contains these sections, in this order, with these exact
headers:

| Header | Contents |
|---|---|
| `========= TOKENS ========` | Each named token, its tin, and its fixed source text (if any). |
| (token sets) | Each named token set and the tokens it contains. |
| `========= RULES =========` | Per rule, the distinct push/rule targets for open and close states. |
| `========= ALTS =========` | Per rule, every open and close alternate: token sequence, actions, counters, condition and group. |
| `========= LEXER =========` | The ordered lexer matchers and their make-function names. |
| `========= PLUGIN =========` | Each loaded plugin and its options. |

Section headers are byte-for-byte identical across the TypeScript and Go
implementations so their output can be diffed.
