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
tn.use(Debug, options)
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
| `"trace"` | `true` / `false` / object (any non-`false` value) / absent | Enable parse tracing (lex + rule events). |
| `"out"` | `io.Writer` | Where trace output is written. Defaults to `os.Stdout`. |

The `trace` option mirrors the canonical TypeScript `true | false | object`
handling: an explicit `false` (or `*bool` false) disables tracing; any
other non-`nil` value (`true`, or a per-kind flag object) enables it; and
when the key is **absent** (or `opts` is `nil`) the value falls back to
`Defaults["trace"]` (i.e. on). Because the Go engine surfaces only two
event streams, a per-kind object cannot select individual kinds — it
simply turns both streams on.

The Go engine drives tracing through instance subscribers
(`Tabnas.Sub`), which expose two event streams — token (`lex`) and rule.
The finer TypeScript kinds (`step`, `parse`, `node`, `stack`) and the
`print` behaviour (which wraps `use`) have no equivalent in the Go engine
API and are intentionally absent.

Trace output is capturable: pass any `io.Writer` under `opts["out"]` and
the subscribers write there via `fmt.Fprintf` instead of `os.Stdout`.

## Defaults

| | TypeScript | Go |
|---|---|---|
| symbol | `Debug.defaults` | `debug.Defaults` (a `map[string]any`) |
| `print` / — | `true` | n/a |
| `trace` | all kinds `true` | `true` |

## Describing a grammar

| Language | Form |
|---|---|
| TypeScript | `tn.debug.describe()` — method attached to the instance, returns `string` |
| Go | `debug.Describe(j)` — package function taking the instance, returns `(string, error)` |

The Go form returns an `error` alongside the report to uphold the
engine's no-panic guarantee: a malformed grammar spec (nil config, nil
rule spec, nil alternate) is rendered defensively, and any remaining
panic is recovered and returned as an `"internal"`-code
`*tabnas.TabnasError` with an empty report string. On success the error
is `nil`.

Both produce a snapshot of the instance's active configuration with no
side effects, organised into these sections, in this order, with these
exact headers:

| Header | Contents |
|---|---|
| `========= INSTANCE ========` | The instance tag (`tag:`), empty when unset. |
| `========= TOKENS ========` | Each token: name, tin, and fixed source text (if any). Followed by a token-set sub-block (`IGNORE`, `VAL`, `KEY`, plus any custom set) listing member token names. |
| `========= RULES =========` | Each rule's push/replace transition tree: the distinct rule-name targets reached by an open-push (`op`), open-replace (`or`), close-push (`cp`) and close-replace (`cr`) alternate. Empty categories are omitted; single-character rule names are valid targets. Function-valued (`PF`/`RF`) targets render as `<F>`. |
| `========= ALTS =========` | Each rule's open and close alternates: token sequence, push (`p`), replace (`r`), backtrack (`b`), counters (`n`), group (`g`), the action/condition/modifier presence flags (`A`/`C`/`H`), and the declarative condition (`CD`). Function-valued push/replace render as `p=<F>` / `r=<F>`. Per-position multi-token sets render as `[a,b]`, a single token bare. |
| `========= LEXER =========` | Lexer matchers. TS lists every matcher; Go lists only the custom matchers its public API exposes (the built-in enable flags are reported under `CONFIG`). |
| `========= CONFIG ========` | Key parser settings: rule `start`, `finish`, `safeKey`, and the built-in lex enable flags (`lex.fixed`, `lex.space`, `lex.line`, `lex.text`, `lex.number`, `lex.comment`, `lex.string`, `lex.value`). |
| `========= PLUGIN =========` | Loaded plugins. TS lists each plugin and its options; Go reports the plugin count (the Go engine stores plugins as bare functions). |

Section headers are identical across both implementations so output can
be diffed.

## Trace output

Under tracing, each event prints one line to the instance's console
(TypeScript) or to `opts["out"]` / `os.Stdout` (Go).

- **TypeScript** logs the enabled kinds (`step`, `rule`, `lex`, `parse`,
  `node`, `stack`); most lines lead with the parse state — upcoming
  source, the token window `[t0 t1]~[tin0 tin1]`, and the parse depth.
- **Go** logs two kinds: `[lex]` lines (token name, tin, source, value,
  row:col) and `[rule]` lines (rule name, instance, state, depth, node).
  Output goes to the `io.Writer` passed as `opts["out"]`, defaulting to
  `os.Stdout`, so it can be captured (e.g. in tests).

## Parity and remaining differences (Go vs. canonical TypeScript)

The Go port now closes most of the prior gaps. The `TOKENS` token-set
sub-block, the `RULES` op/or/cp/cr transition tree (including
single-character rule names and function-valued `<F>` targets), the
`ALTS` `A`/`C`/`H` presence flags, declarative-condition (`CD`) rendering,
function-valued push/replace (`p=<F>` / `r=<F>`), and per-position
multi-token sets all mirror `debug.ts`. Tracing is configurable
(`true | false | object | absent`, honouring `Defaults["trace"]`) and
capturable (`opts["out"]`).

The remaining differences are imposed by the Go engine's public API:

1. **Tracing kinds.** Go emits `lex` and `rule` only; the finer TypeScript
   kinds (`step`, `parse`, `node`, `stack`) are not surfaced by the engine
   `Sub` API. A per-kind trace object therefore just turns both streams on
   rather than selecting individual kinds.
2. **No `print` option in Go** (the engine does not expose a `use` hook to
   describe after each `use`).
3. **`Describe` is a package function** in Go, a method in TypeScript, and
   returns `(string, error)`: it upholds the engine's no-panic guarantee,
   surfacing any internal failure as an `"internal"`-code error (with an
   empty string) instead of panicking. Malformed specs (nil config, nil
   rule spec, nil alternate) render defensively (`***INVALID***`).
4. **`LEXER` and `PLUGIN` sections are summarised** in Go: the engine
   exposes only custom lexer matchers (built-in enable flags appear under
   `CONFIG`) and stores plugins as bare functions, so the plugin count is
   reported rather than per-plugin names and options.
5. **`ALTS` condition counter map (`CN=`).** The canonical TS renders the
   normalised condition's counter map as `CN=` (from `a.c.n`). The Go
   engine has no equivalent `AltSpec` field — it folds counter conditions
   into the `C` function rather than retaining a separate map — so `CN=`
   is not emitted. The presence of a condition is still flagged by `C`,
   and declarative conditions are rendered via `CD=`.
6. **Token ordering.** The Go engine exposes token sets through Go maps
   (e.g. `IGNORE` is a `map[Tin]bool`) and custom token names through
   `cfg.TinNames` (a `map[Tin]string`), neither of which preserves
   insertion order. Exact TS insertion-order parity is therefore not
   possible without engine changes; the Go port instead orders tokens and
   token-set members by tin (built-in tins in their canonical
   `TinBD..TinCA` order, then custom tins ascending) so the output is
   deterministic and diffable.
