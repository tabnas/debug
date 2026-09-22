# Reference

Exact behaviour of the `@tabnas/debug` plugin. The TypeScript
implementation (`ts/src/debug.ts`) is canonical. The Go implementation
(`go/debug.go`) and the Rust one (`rs/src/`) track it functionally, but
the three engines expose tracing and introspection through different
idioms, so the surfaces differ in shape. All three are documented below.

This file is the authoritative divergence register: a difference that is
real and intended is recorded here, not left for a reader to discover.

## Entry point

| Language | Symbol | Form |
|---|---|---|
| TypeScript | `Debug` | `Plugin` function: `(tabnas, options) => void` |
| Go | `Debug` | `tabnas.Plugin`: `func(j *tabnas.Tabnas, opts map[string]any) error` |
| Rust | `plugin(options)` / `apply(parser, options)` | builds a `tabnas::Plugin`; `apply` installs it |

Load it with the engine's `use` / `Use` method:

```js
tn.use(Debug, options)
```

```go
j.Use(debug.Debug, opts)   // opts is map[string]any
```

```rust
tabnas_debug::apply(&mut parser, DebugOptions::default())?;
// or build the Plugin and install it yourself:
parser.use_plugin(tabnas_debug::plugin(DebugOptions::default()), None)?;
```

Rust options are a typed `DebugOptions` struct rather than an option map:
a trace selection is a set of `bool` fields, and no `Value` could carry
one anyway.

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

### Rust

| Field | Type | Meaning |
|---|---|---|
| `print` | `bool` | Log `USE:` plus the full `describe` dump when a later plugin is loaded via `tabnas_debug::use_plugin`. |
| `trace` | `Option<TraceKinds>` | Which parse events to log; `None` traces nothing. |

`TraceKinds` is a struct of six `bool` fields — `step`, `rule`, `lex`,
`parse`, `node`, `stack` — with `TraceKinds::all()` and
`TraceKinds::none()` constructors. `DebugOptions::default()` is `print:
true` with every kind on, matching `Debug.defaults`;
`DebugOptions::quiet()` is the introspection-only setting (no `USE:`
dumps, no tracing) a test suite wants. Builders `with_print`,
`with_trace` and `without_trace` compose.

Rust has no `out` option: the ENGINE owns the output sink
(`parser.options.debug.output`, defaulting to stderr), and both the trace
lines and the `USE:` dump are written through it. Set that sink to
capture them.

Like Go, Rust exposes the `print` wrapper as a function —
`tabnas_debug::use_plugin(&mut parser, plugin, options)` — because
`Tabnas::use_plugin` is a concrete method, not a reassignable field.

## Defaults

| | TypeScript | Go | Rust |
|---|---|---|---|
| symbol | `Debug.defaults` | `debug.Defaults` (a `map[string]any`) | `DebugOptions::default()` |
| `print` | `true` | `true` | `true` |
| `trace` | all kinds `true` | `true` (all kinds) | `Some(TraceKinds::all())` |

## Describing a grammar

| Language | Form |
|---|---|
| TypeScript | `tn.debug.describe()` — method attached to the instance, returns `string` |
| Go | `debug.Describe(j)` — package function taking the instance, returns `(string, error)` |
| Rust | `tabnas_debug::describe(&parser)` — free function taking the instance, returns `String` |

The Go form returns an `error` alongside the report to uphold the
engine's no-panic guarantee: a malformed grammar spec (nil config, nil
rule spec, nil alternate) is rendered defensively, and any remaining
panic is recovered and returned as an `"internal"`-code
`*tabnas.TabnasError` with an empty report string. On success the error
is `nil`.

All three produce a snapshot of the instance's active configuration with
no side effects, organised into these sections, in this order, with these
exact headers:

| Header | Contents |
|---|---|
| `========= INSTANCE ========` | The instance tag (`tag:`). Every port prints the engine's tag verbatim, and all three engines default an unset tag to `-`, so an untagged instance renders `tag: -` everywhere. (See the engine-version note below: a Go engine older than the `tabnas.DefaultTag` alignment left an unset tag empty and rendered a bare `tag:`.) |
| `========= TOKENS ========` | Each token: name, tin, and fixed source text (if any). Followed by a token-set sub-block (`IGNORE`, `VAL`, `KEY`, plus any custom set) listing member token names. |
| `========= RULES =========` | Each rule's push/replace transition tree: the distinct rule-name targets reached by an open-push (`op`), open-replace (`or`), close-push (`cp`) and close-replace (`cr`) alternate. Empty categories are omitted; single-character rule names are valid targets. Function-valued (`PF`/`RF`) targets render as `<F>`. |
| `========= ALTS =========` | Each rule's open and close alternates: token sequence, push (`p`), replace (`r`), backtrack (`b`), counters (`n`), group (`g`), the action/condition/modifier presence flags (`A`/`C`/`H`), and the declarative condition (`CD`). Function-valued push/replace render as `p=<F>` / `r=<F>`. Per-position multi-token sets render as `[a,b]`, a single token bare. |
| `========= LEXER =========` | Lexer matchers. TS lists every matcher; Go and Rust list only the custom matchers their public APIs expose (the built-in enable flags are reported under `CONFIG`). |
| `========= CONFIG ========` | Key parser settings: rule `start`, `finish`, `safeKey`, and the built-in lex enable flags (`lex.fixed`, `lex.space`, `lex.line`, `lex.text`, `lex.number`, `lex.comment`, `lex.string`, `lex.value`). |
| `========= PLUGIN =========` | Loaded plugins. TS lists each plugin and its options; Go lists each plugin by its function symbol name, plus options registered via `Tabnas.SetPluginOptions`; Rust lists each plugin by its declared `Plugin` name, plus options from its plugin-options namespace. |

Section headers are identical across all three implementations so output
can be diffed. The eight of them are the parity contract that
`test/spec/sections.tsv` pins, byte-for-byte and in order.

## Structured model

| Language | Form |
|---|---|
| TypeScript | `tn.debug.model()` — returns `DebugModel` |
| Go | `debug.Model(j)` — returns `(*DebugModel, error)` |
| Rust | `tabnas_debug::model(&parser)` — returns `DebugModel` |

All three return the same information as `describe()` / `Describe` as a
typed, JSON-serialisable object: the token table (`tokens`), token sets
(`tokenSets`), rules and alternates as data (`rules`), the
rule-reference graph (`graph`), lexer matchers (`lexer`), key config
(`config`), plugins (`plugins`) and the ABNF text (`abnf`). All three
export the full type set: `DebugModel`, `DebugTokenInfo`,
`DebugTokenSet`, `DebugAltInfo`, `DebugRuleInfo`, `DebugRuleEdges`,
`DebugLexMatcher`, `DebugConfigInfo`, `DebugPluginInfo` — the Go structs
carry JSON tags matching the TS field names, so serialised output is
comparable across runtimes.

A nil/null alternate renders as the seq entry `***INVALID***` in the two
runtimes that can have one; Rust's `AltSpec` is a value and cannot be
null, so the case does not arise. A function-valued push/replace target
is `"<fn>"`. The Go and Rust `back` fields omit an explicit `b: 0` (both
have zero-value, not nullable, integers). Go rule/token ordering is
deterministic (rules by name, tokens by tin) rather than TS insertion
order; Rust keeps the engine's `IndexMap` order for rules, which IS the
TS insertion order, and sorts tokens by tin and token sets by name
because the Rust engine holds those unordered.

The Go model's slice fields are always initialised, never left nil, so an
empty section serialises as `[]` — matching TS — rather than `null`. Rust
`Vec` fields are likewise always present, and absent optionals are
skipped rather than serialised as `null`.

The Rust field names come from `serde` renames chosen to match the TS
field names and the Go JSON tags exactly (`tokenSets`, `openPush`,
`safeKey`, …), which is what makes `model.tsv` shareable three ways.

`test/spec/model.tsv` pins this cross-runtime comparability: it runs the
`rules` and `graph` sections of both models through JSON for every
grammar in the shared registry, sorted by name to absorb the documented
ordering difference. The *instance-level* sections (`lexer`, `plugins`,
`tag`) are outside that fixture: `lexer` is summarised in Go, the Go
fixtures need not load the debug plugin (in Go, `Describe`/`Model`/`Abnf`
are package functions), and `tag` depends on the engine version — see
below.

### Engine-version note: the unset instance `tag`

The TypeScript engine has always defaulted an unset `tag` to `-`
(`ts/src/defaults.ts`). The Go engine used to leave `Options().Tag`
empty, so an untagged instance rendered `tag: -` in TS and a bare
`tag:` in Go, and `Model(j).Tag` was `""` where TS gave `"-"`.

That is **fixed in the engine**: `github.com/tabnas/parser/go` now
exports `DefaultTag = "-"` and `Make` applies it to an unset
`Options.Tag`, so both runtimes report `-`. Verified against the sibling
engine checkout (`cd go && go test ./...` with the repo `go.work`
active).

The fix is not yet in a published engine release, so the Go suite's two
resolutions **disagree** on this one value:

| Resolution | Engine | Unset tag |
|---|---|---|
| `GOWORK=off` (`make test`, `make test-go`) | published `parser/go v0.6.1` | `""` |
| workspace on (plain `go test`; what CI generates) | sibling `parser/go` `main` | `"-"` |

A shared fixture has to pass under both, so `tag` is left out of
`model.tsv` until `go/go.mod` moves past the alignment. Once the engine
bump lands, both resolutions report `-`, `tag` can join `rules`/`graph`
in the fixture, and this note can go.

## Trace output

Under tracing, each event prints one line to the instance's console
(TypeScript), to `opts["out"]` / `os.Stdout` (Go), or to the engine's
own debug sink, `parser.options.debug.output` (Rust). Every runtime
begins each parse with a `========= TRACE ==========` banner and logs
the enabled kinds (`step`, `rule`, `lex`, `parse`, `node`, `stack`);
most lines lead with the parse state — upcoming source, the token window
`[t0 t1]~[tin0 tin1]`, and the parse depth. Rust logs five of the six:
`step` has no engine hook.

Go derives the streams from the engine's hooks (rule/lex subscribers, a
parse-prepare hook, and after-open/after-close rule state actions); the
`parse` lines say `alt` / `no-alt` without the TS alt index, and `lex`
lines omit the matcher name, because the Go engine does not expose them.

Rust derives them from the engine's typed subscribers: `lex` from
`subscribe_lex`, `rule` and `stack` from `subscribe_rules` (the latter
reading the context's rule stack), and `parse` and `node` from
`subscribe_rule_done` — which carries the matched alternate, so Rust
`parse` lines DO report the alternate's push/replace/back/groups, though
still not an alt index. The banner is written from a parse-prepare hook,
as in Go. Output goes to the engine's own debug sink.

**Rust `step` never fires.** In TypeScript the engine itself calls
`ctx.log('step', …)` once per parse step; the Rust engine emits no such
event and has no `ctx.log`. The option is kept so the kind names stay in
step across runtimes, and selecting it is accepted — but nothing is
logged for it, and a selection of `step` ALONE installs no tracing at
all, not even the per-parse banner.

## Parity and remaining differences: Go vs. canonical TypeScript

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


## Parity and remaining differences: Rust vs. canonical TypeScript

The Rust port covers `describe`, `model`, `abnf`, the `print` wrapper and
five of the six trace kinds, and passes the same shared
`test/spec/*.tsv` fixtures the other two runtimes do. The differences are
imposed by the Rust engine's public API and by Rust's type system:

1. **Free functions, not instance methods.** `describe(&parser)`,
   `model(&parser)` and `abnf(&parser)` take the instance, exactly as
   Go's package functions do — Rust cannot add methods to a type it does
   not own. Unlike Go they are **infallible**: the Go signatures return
   `(value, error)` because the Go engine's accessors can fail, while the
   Rust accessors cannot, so there is no error to surface.
2. **`print` requires `tabnas_debug::use_plugin`.** `Tabnas::use_plugin`
   is a concrete method, not a reassignable field, so the TS `use()`
   wrapping is a free function here too. A plugin installed directly
   through `Tabnas::use_plugin` does not trigger the `USE:` dump. The
   plugin records its `print` setting as an instance *decoration*
   (`debug.print`), which is how the wrapper knows what to do.
3. **`step` never fires.** The Rust engine has no `ctx.log` and emits no
   per-step event, so the `step` kind is accepted for option-name parity
   and logs nothing. See "Trace output" above.
4. **Trace line shape.** Lines are derived from the engine's typed
   subscribers, so they carry what those subscribers expose. Rust `parse`
   lines DO name the matched alternate's push/replace/back/groups (the
   `RuleDone` event carries them) where Go's cannot, but no runtime but
   TypeScript reports an alt *index*, and Rust `lex` lines omit the
   matcher name for the same reason Go's do.
5. **No `out` option.** The output sink belongs to the engine
   (`parser.options.debug.output`, stderr by default) and the plugin
   writes both trace lines and the `USE:` dump through it. Capture by
   setting that sink, rather than by passing a writer to the plugin.
6. **Ordering.** Rules keep the engine's `IndexMap` order, which IS the
   TypeScript insertion order — Rust matches TS here where Go cannot.
   Tokens are ordered by tin and token sets by name, because the Rust
   engine holds those in unordered maps; alt counter maps are sorted for
   the same reason.
7. **`LEXER` is summarised, and `make` is always empty.** As in Go, the
   engine enumerates only CUSTOM lexer matchers (built-in enable flags
   appear under `CONFIG`). The `make` field is the Go analogue of the TS
   factory *name*, and Rust function values carry no name at all, so it
   is always `""`. Rust's `order` is an `f64`, the engine's own type:
   matcher priorities are not necessarily whole, and the TS field is a
   plain JavaScript number, so this matches TS more closely than Go's
   `int` can.
8. **`ALTS` condition rendering.** The TS `CN=` (the normalised
   condition's counter map) has no Rust counterpart, as it has no Go one.
   Rust's declarative conditions are path/op/value comparisons rather
   than TS's `{n, d}` shape, and render as `CD=` entries; a callback
   condition shows only as the `C` flag, as everywhere.
9. **The shared-fixture loader is hand-written.** `@tabnas/support` has
   no Rust half, so `rs/tests/common/spec.rs` implements the loader. It
   is the one loader that CAN drift from the other two; `test/AGENTS.md`
   pins the codec it has to keep.
10. **Fixed token names carry a `#` prefix.** `Tabnas::token_with_source`
    normalises a fixed token's name to `#…`, so a token declared `Ta`
    reports as `#Ta` in `TOKENS`, in `ALTS` sequences and in
    `model().tokens`. TypeScript stores the name as given. This is an
    engine convention rather than a plugin choice: both ports print what
    `token_name` returns. It does not reach the shared fixtures, whose
    grammars name every token with the prefix already.
11. **Re-installing updates the trace selection rather than stacking it.**
    The Rust engine accumulates subscribers and parse-prepare hooks, so
    the plugin registers its callbacks once per instance and has later
    installs replace a shared selection — including with `trace: None`,
    which turns tracing off. TypeScript needs the same care for its own
    `use()` wrapper (`__debugUseWrapped`), and for the same reason:
    deriving a child re-runs the parent's plugins.


## Two ABNF defects, fixed in the canonical runtime

`abnf()` had two cases that emitted output no conforming ABNF tool
accepts. Both were surfaced by an automated review of the Rust port
(tabnas/debug#37), verified against `ts/src/debug.ts` as canonical
behaviour rather than port defects, and fixed in tabnas/debug#45 — in
TypeScript first, then Go and Rust, pinned by the shared `collide`
fixture that all three run. They are kept here because the old output
still turns up in grammars captured before the fix.

1. **A token whose bare name matched a rule name took the rule's name.**
   The emitter seeded one name map with the rule names and stripped the
   `#` from a token before mapping it, so a grammar with a rule `NR` and
   a token `#NR` gave both the symbol `NR`. The output carried a
   self-referential `NR = NR` plus a second `NR = …` definition (using
   `=`, not the incremental `=/`). Rules and tokens are now separate
   namespaces over one shared claim set, so the token suffixes to `NR-2`.

2. **An unrecognised match regex emitted a legend entry that was only a
   comment.** The fallback was `T = ; /…/`; `;` starts an ABNF comment
   that runs to end of line, so the rule was left with no elements — not
   merely non-round-tripping, but unparseable, and one such token
   invalidated the whole grammar rather than just that rule. It now emits
   an RFC 5234 §4 prose-val, `T = <regex /…/>`, which is a real element.

All three runtimes emit the `collide` fixture byte-for-byte identically.
