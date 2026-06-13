# Reference

Exact behaviour of the `@tabnas/debug` plugin. The TypeScript
implementation (`ts/src/debug.ts`) is canonical. The Go implementation
(`go/debug.go`) tracks it functionally, but the two engines expose
tracing and introspection through different idioms, so the surfaces
differ in shape. Both are documented below.

## Entry point

| Language | Symbol | Form |
|---|---|---|
| TypeScript | `Debug` | `Plugin` function: `(tabnas, options) => void` |
| Go | `Debug` | `tabnas.Plugin`: `func(j *tabnas.Tabnas, opts map[string]any) error` |

Load it with the engine's `use` / `Use` method:

```js
am.use(Debug, options)
```

```go
j.Use(debug.Debug, opts)   // opts is map[string]any
```

## Options

### TypeScript

| Field | Type | Meaning |
|---|---|---|
| `print` | boolean | Print the grammar description after every `use` call. |
| `trace` | `true` / `false` / per-kind flags | Which parse events to log. |

`trace` may be `true` (all kinds), `false` (off), or an object of
kind → boolean (only the listed kinds). The kinds are `step`, `rule`,
`lex`, `parse`, `node`, `stack`.

### Go

| Key | Type | Meaning |
|---|---|---|
| `"trace"` | bool | Enable parse tracing (lex + rule events). |

The Go engine drives tracing through instance subscribers
(`Tabnas.Sub`), which expose two event streams — token (`lex`) and rule.
The finer TypeScript kinds (`step`, `parse`, `node`, `stack`) and the
`print` behaviour (which wraps `use`) have no equivalent in the Go engine
API and are intentionally absent. This is the main documented divergence
from the canonical behaviour.

## Defaults

| | TypeScript | Go |
|---|---|---|
| symbol | `Debug.defaults` | `debug.Defaults` (a `map[string]any`) |
| `print` / — | `true` | n/a |
| `trace` | all kinds `true` | `true` |

## Describing a grammar

| Language | Form |
|---|---|
| TypeScript | `am.debug.describe()` — method attached to the instance, returns `string` |
| Go | `debug.Describe(j)` — package function taking the instance, returns `string` |

Both produce a snapshot of the instance's active configuration with no
side effects, organised into these sections, in this order, with these
exact headers:

| Header | Contents |
|---|---|
| `========= TOKENS ========` | Each token: name, tin, and fixed source text (if any). |
| `========= RULES =========` | Each rule with its open/close alternate counts (TS also shows push/rule transition targets). |
| `========= ALTS =========` | Each rule's open and close alternates: token sequence, push (`p`), replace (`r`), backtrack (`b`), counters (`n`), group (`g`). |
| `========= LEXER =========` | Lexer matchers. TS lists every matcher; Go lists the built-in enable flags plus any custom matchers (the only ones its public API exposes). |
| `========= PLUGIN =========` | Loaded plugins. TS lists each plugin and its options; Go reports the plugin count (the Go engine stores plugins as bare functions). |

Section headers are identical across both implementations so output can
be diffed.

## Trace output

Under tracing, each event prints one line to the instance's console
(TypeScript) or stdout (Go).

- **TypeScript** logs the enabled kinds (`step`, `rule`, `lex`, `parse`,
  `node`, `stack`); most lines lead with the parse state — upcoming
  source, the token window `[t0 t1]~[tin0 tin1]`, and the parse depth.
- **Go** logs two kinds: `[lex]` lines (token name, tin, source, value,
  row:col) and `[rule]` lines (rule name, instance, state, depth, node).

## Documented differences (Go vs. canonical TypeScript)

1. Tracing kinds: Go emits `lex` and `rule` only (engine `Sub` API).
2. No `print` option in Go (the engine does not expose a `use` hook).
3. `Describe` is a package function in Go, a method in TypeScript.
4. `LEXER` and `PLUGIN` sections are summarised in Go, limited to what
   the engine's public API exposes.
