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
| `"print"` | `true` / `false` / `*bool` / absent | Log `USE:` plus the full `Describe` dump when a later plugin is loaded via `debug.Use`. |
| `"trace"` | `true` / `false` / `*bool` / per-kind map / absent | Which parse events to log. |
| `"out"` | `io.Writer` | Where trace and print output is written. Defaults to `os.Stdout`. |

The `trace` option mirrors the canonical TypeScript `true | false | object`
handling: an explicit `false` (or `*bool` false) disables tracing; `true`
enables every kind; a per-kind map (`map[string]any` or
`map[string]bool`) enables tracing with the map merged over the all-true
defaults — a partial map cannot turn other kinds off implicitly (set them
`false` explicitly), matching the TS engine-side deep-merge of
`Debug.defaults`; and when the key is **absent** (or `opts` is `nil`) the
value falls back to `Defaults["trace"]` (i.e. on). The kinds are the
TypeScript six: `step`, `rule`, `lex`, `parse`, `node`, `stack`.

The `print` behaviour is exposed as the package function
`debug.Use(j, plugin, opts...)`: the Go engine's `(*Tabnas).Use` is a
concrete method that cannot be wrapped in place (the TS plugin reassigns
`tabnas.use`), so later plugin loads must go through `debug.Use` to get
the `USE:` log.

Trace output is capturable: pass any `io.Writer` under `opts["out"]` and
the trace streams write there instead of `os.Stdout`.

## Defaults

| | TypeScript | Go |
|---|---|---|
| symbol | `Debug.defaults` | `debug.Defaults` (a `map[string]any`) |
| `print` | `true` | `true` |
| `trace` | all kinds `true` | `true` (all kinds) |

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
| `========= PLUGIN =========` | Loaded plugins. TS lists each plugin and its options; Go lists each plugin by its function symbol name, plus options registered via `Tabnas.SetPluginOptions`. |

Section headers are identical across both implementations so output can
be diffed.

## Structured model

| Language | Form |
|---|---|
| TypeScript | `tn.debug.model()` — returns `DebugModel` |
| Go | `debug.Model(j)` — returns `(*DebugModel, error)` |

Both return the same information as `describe()` / `Describe` as a
typed, JSON-serialisable object: the token table (`tokens`), token sets
(`tokenSets`), rules and alternates as data (`rules`), the
rule-reference graph (`graph`), lexer matchers (`lexer`), key config
(`config`), plugins (`plugins`) and the ABNF text (`abnf`). Both
runtimes export the full type set: `DebugModel`, `DebugTokenInfo`,
`DebugTokenSet`, `DebugAltInfo`, `DebugRuleInfo`, `DebugRuleEdges`,
`DebugLexMatcher`, `DebugConfigInfo`, `DebugPluginInfo` — the Go structs
carry JSON tags matching the TS field names, so serialised output is
comparable across runtimes.

A nil/null alternate renders as the seq entry `***INVALID***` in both. A
function-valued push/replace target is `"<fn>"`. The Go `Back` field
omits an explicit `b: 0` (Go zero-value semantics), and Go rule/token
ordering is deterministic (rules by name, tokens by tin) rather than TS
insertion order.

## Trace output

Under tracing, each event prints one line to the instance's console
(TypeScript) or to `opts["out"]` / `os.Stdout` (Go). Both runtimes begin
each parse with a `========= TRACE ==========` banner and log the enabled
kinds (`step`, `rule`, `lex`, `parse`, `node`, `stack`); most lines lead
with the parse state — upcoming source, the token window
`[t0 t1]~[tin0 tin1]`, and the parse depth.

Go derives the streams from the engine's hooks (rule/lex subscribers, a
parse-prepare hook, and after-open/after-close rule state actions); the
`parse` lines say `alt` / `no-alt` without the TS alt index, and `lex`
lines omit the matcher name, because the Go engine does not expose them.

## Parity and remaining differences (Go vs. canonical TypeScript)

The Go port now closes most of the prior gaps. The structured `Model`
(with all nine `Debug*` types), the `print` option (via `debug.Use`),
the six granular trace kinds, the `TOKENS` token-set sub-block, the
`RULES` op/or/cp/cr transition tree (including single-character rule
names and function-valued `<F>` targets), the `ALTS` `A`/`C`/`H`
presence flags, declarative-condition (`CD`) rendering, function-valued
push/replace (`p=<F>` / `r=<F>`), and per-position multi-token sets all
mirror `debug.ts`. Tracing is configurable
(`true | false | per-kind map | absent`, honouring `Defaults["trace"]`)
and capturable (`opts["out"]`).

The remaining differences are imposed by the Go engine's public API:

1. **Trace detail.** All six kinds are emitted, but Go `parse` lines
   carry `alt` / `no-alt` without the TS alt *index* (the engine does not
   expose which alternate matched), and `lex` lines omit the matcher
   name. The `parse`/`node` streams fire from after-open/after-close rule
   state actions installed at parse start, the closest hook to the TS
   engine's post-match log points.
2. **`print` requires `debug.Use`.** The Go engine's `(*Tabnas).Use` is a
   concrete method that cannot be reassigned, so the TS `use()` wrapping
   is exposed as the package function `debug.Use(j, plugin, opts...)`;
   plugins loaded directly via `j.Use` do not trigger the `USE:` log.
3. **`Describe` / `Model` / `Abnf` are package functions** in Go, methods
   in TypeScript, and return `(value, error)`: they uphold the engine's
   no-panic guarantee, surfacing any internal failure as an
   `"internal"`-code error instead of panicking. Malformed specs (nil
   config, nil rule spec, nil alternate) render defensively
   (`***INVALID***`).
4. **`LEXER` section is summarised; plugin names are symbol-derived.**
   The engine exposes only custom lexer matchers (built-in enable flags
   appear under `CONFIG`) and stores plugins as bare functions, so plugin
   names come from each function's symbol and per-plugin options appear
   only when registered via `Tabnas.SetPluginOptions`.
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
