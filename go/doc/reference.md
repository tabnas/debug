# Reference (Go)

The exact API surface of `github.com/tabnas/debug/go` (package
`tabnasdebug`). This is the Go port of the canonical TypeScript
implementation; the TypeScript [reference](../../ts/doc/reference.md) is
authoritative. The Go surface differs in shape — those differences are
listed here and explained in [concepts](concepts.md).

## Import

```go
import tabnasdebug "github.com/tabnas/debug/go"
```

## Exported symbols

| Symbol | Type | Description |
|---|---|---|
| `Debug` | `tabnas.Plugin` | The plugin. Load it with `j.Use(tabnasdebug.Debug, opts)`. |
| `Describe(j *tabnas.Tabnas) (string, error)` | func | A sectioned, human-readable dump of the instance's grammar. |
| `Abnf(j *tabnas.Tabnas) (string, error)` | func | The live grammar rendered as ABNF text. |
| `Defaults` | `map[string]any` | The default options (`{"trace": true}`). |
| `Version` | `string` | The module version. |

> **No `model()` in Go.** The TypeScript port also exposes a structured
> `model()` that returns the grammar as data. The Go engine's
> introspection API does not support an equivalent typed projection, so
> the Go package offers only the text forms (`Describe`, `Abnf`). See
> [concepts](concepts.md).

## `Debug` — the plugin

```go
var Debug tabnas.Plugin = func(j *tabnas.Tabnas, opts map[string]any) error
```

Loading it with `j.Use(tabnasdebug.Debug, opts)` installs parse-trace
subscribers when tracing is enabled. Loading itself never panics: a panic
while wiring subscribers is returned as an `"internal"`-code
`*tabnas.TabnasError`.

### Options

| Key | Type | Default | Meaning |
|---|---|---|---|
| `"trace"` | `true` / `false` / `*bool` / object / absent | `true` (from `Defaults`) | Enable parse tracing (lex + rule streams). |
| `"out"` | `io.Writer` | `os.Stdout` | Where trace output is written. |

`trace` handling mirrors the TypeScript `true | false | object`:

- an explicit `false` (or a `*bool` false) — off;
- any other non-`nil` value (`true`, or a per-kind flag object) — on;
- absent (or `opts` is `nil`) — falls back to `Defaults["trace"]` (on).

Because the Go engine surfaces only two event streams, a per-kind object
cannot select individual kinds; any non-`false` value turns both streams
on.

## `Describe(j) (string, error)`

Returns the instance's active grammar as one string, organised into these
sections, in this order, with these exact headers:

| Header | Contents |
|---|---|
| `========= INSTANCE ========` | The instance tag (`tag:`), empty when unset. |
| `========= TOKENS ========` | Each token: name, tin, fixed source text (if any). Then a token-set sub-block (`IGNORE`, `VAL`, `KEY`) listing member token names. |
| `========= RULES =========` | Each rule's push/replace transition tree: distinct rule-name targets reached by open-push (`op`), open-replace (`or`), close-push (`cp`), close-replace (`cr`). Empty categories omitted; function-valued targets render as `<F>`. |
| `========= ALTS =========` | Each rule's open/close alternates: token sequence, push (`p`), replace (`r`), backtrack (`b`), counters (`n`), group (`g`), the action/condition/modifier flags (`A`/`C`/`H`), declarative condition (`CD`). A nil alternate renders as `***INVALID***`. |
| `========= LEXER =========` | The custom lexer matchers (`name (priority=N)`); built-in matchers are not enumerable in Go and appear as enable flags under `CONFIG`. |
| `========= CONFIG ========` | `start`, `finish`, `safeKey`, and the built-in lex enable flags. |
| `========= PLUGIN =========` | The loaded plugin count (the Go engine stores plugins as bare functions). |
| `========= ABNF =========` | The grammar rendered as ABNF (same as `Abnf`). |

The headers are identical to the TypeScript port's, pinned by a shared
golden fixture, so dumps from either runtime are diffable.

The `error` return upholds the engine's no-panic guarantee: a malformed
grammar (nil config, nil rule spec, nil alternate) is rendered
defensively, and any remaining panic is recovered and returned as an
`"internal"`-code `*tabnas.TabnasError` with an empty string. On success
the error is `nil`. (A `nil` instance, for example, surfaces as an
internal error rather than a crash.)

## `Abnf(j) (string, error)`

Returns the live grammar as [ABNF](https://www.rfc-editor.org/rfc/rfc5234)
text: the productions first (the real start rule leading), then a blank
line, then a legend defining each token as its own ABNF rule.

Token legend forms:

| Token kind | Legend form |
|---|---|
| Fixed literal containing a letter | `%s"<lit>"` |
| Fixed literal, punctuation only | `"<lit>"` |
| Case-insensitive literal | `"<lit>"` |
| Char-range match | `%xLO-HI` |
| Built-in lexer token | `<number>`, `<string>`, `<text>`, … |
| Anything else | `; /<regex>/` (an ABNF comment) |

Like `Describe`, `Abnf` returns `(string, error)` to uphold the no-panic
guarantee. The emitter reads only the running engine; Go has no ABNF
library port to depend on.

## `Defaults`

```go
var Defaults = map[string]any{"trace": true}
```

Used when the plugin is loaded with no explicit `trace` option. Mirrors
the TypeScript `Debug.defaults`, where tracing is on by default.

## Trace output

When tracing is on, the plugin installs two engine subscribers
(`Tabnas.Sub`) that write one line per event to `opts["out"]` (default
`os.Stdout`):

| Prefix | Format |
|---|---|
| `[lex]` | `[lex]  <name> tin=<n> src=<q> val=<v> at <row>:<col>` — one per token. |
| `[rule]` | `[rule] <name>~<i>:<state> d=<depth> node=<node>` — one per rule open (`o`) / close (`c`). |

The finer TypeScript kinds (`step`, `parse`, `node`, `stack`) are not
surfaced by the Go engine's `Sub` API and are not emitted.

## Differences from the TypeScript surface

| Area | TypeScript | Go |
|---|---|---|
| Structured `model()` | present | **absent** (text only: `Describe`, `Abnf`) |
| `describe` form | `tn.debug.describe()` method, returns `string` | `Describe(j)` package func, returns `(string, error)` |
| `abnf` form | `tn.debug.abnf()` method | `Abnf(j)` package func, returns `(string, error)` |
| `print` option | present (wraps `use`) | **absent** (no engine `use` hook) |
| Trace kinds | `step`, `rule`, `lex`, `parse`, `node`, `stack` | `lex`, `rule` only |
| Trace destination | instance console (`get_console`) | `opts["out"]` / `os.Stdout` |
| `LEXER` section | every matcher | custom matchers only |
| `PLUGIN` section | each plugin + options | plugin count |
| Token ordering | engine insertion order | sorted by tin (built-ins first), for determinism |

These divergences are imposed by the Go engine's public API; they are
explained in [concepts](concepts.md) and were carried over from the
project's combined `docs/reference.md`.
