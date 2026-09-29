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
defaults, so a partial map cannot turn other kinds off implicitly (set
them `false` explicitly), matching the TS engine-side deep-merge of
`Debug.defaults`; and when the key is **absent** (or `opts` is `nil`) the
value falls back to `Defaults["trace"]`, which is on. The kinds are the
TypeScript six: `step`, `rule`, `lex`, `parse`, `node`, `stack`.

The `print` behaviour is exposed as the package function
`debug.Use(j, plugin, opts...)`: the Go engine's `(*Tabnas).Use` is a
concrete method that cannot be wrapped in place (the TS plugin reassigns
`tabnas.use`), so later plugin loads must go through `debug.Use` to get
the `USE:` log.

Trace output can be captured: pass any `io.Writer` under `opts["out"]` and
the trace streams write there instead of `os.Stdout`.

### Rust

| Field | Type | Meaning |
|---|---|---|
| `print` | `bool` | Log `USE:` plus the full `describe` dump when a later plugin is loaded via `tabnas_debug::use_plugin`. |
| `trace` | `Option<TraceKinds>` | Which parse events to log; `None` traces nothing. |

`TraceKinds` is a struct of six `bool` fields (`step`, `rule`, `lex`,
`parse`, `node`, `stack`) with `TraceKinds::all()` and
`TraceKinds::none()` constructors. `DebugOptions::default()` is `print:
true` with every kind on, matching `Debug.defaults`;
`DebugOptions::quiet()` is the introspection-only setting (no `USE:`
dumps, no tracing) a test suite wants. Builders `with_print`,
`with_trace` and `without_trace` compose.

Rust has no `out` option: the ENGINE owns the output sink
(`parser.options.debug.output`, defaulting to stderr), and both the trace
lines and the `USE:` dump are written through it. Set that sink to
capture them.

Like Go, Rust exposes the `print` wrapper as a function,
`tabnas_debug::use_plugin(&mut parser, plugin, options)`, because
`Tabnas::use_plugin` is a concrete method, not a field that can be
reassigned.

## Defaults

| | TypeScript | Go | Rust |
|---|---|---|---|
| symbol | `Debug.defaults` | `debug.Defaults` (a `map[string]any`) | `DebugOptions::default()` |
| `print` | `true` | `true` | `true` |
| `trace` | all kinds `true` | `true` (all kinds) | `Some(TraceKinds::all())` |

## Describing a grammar

| Language | Form |
|---|---|
| TypeScript | `tn.debug.describe()`: method attached to the instance, returns `string` |
| Go | `debug.Describe(j)`: package function taking the instance, returns `(string, error)` |
| Rust | `tabnas_debug::describe(&parser)`: free function taking the instance, returns `String` |

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
| TypeScript | `tn.debug.model()`: returns `DebugModel` |
| Go | `debug.Model(j)`: returns `(*DebugModel, error)` |
| Rust | `tabnas_debug::model(&parser)`: returns `DebugModel` |

All three return the same information as `describe()` / `Describe` as a
typed, JSON-serialisable object: the token table (`tokens`), token sets
(`tokenSets`), rules and alternates as data (`rules`), the
rule-reference graph (`graph`), lexer matchers (`lexer`), key config
(`config`), plugins (`plugins`) and the ABNF text (`abnf`). All three
export the full type set: `DebugModel`, `DebugTokenInfo`,
`DebugTokenSet`, `DebugAltInfo`, `DebugRuleInfo`, `DebugRuleEdges`,
`DebugLexMatcher`, `DebugConfigInfo`, `DebugPluginInfo`. The Go structs
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
empty section serialises as `[]`, matching TS, rather than `null`. Rust
`Vec` fields are likewise always present, and absent `Option` fields are
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
are package functions), and `tag` has not been added; see below.

### Engine-version note: the unset instance `tag`

The TypeScript engine has always defaulted an unset `tag` to `-`
(`ts/src/defaults.ts`). The Go engine used to leave `Options().Tag`
empty, so an untagged instance rendered `tag: -` in TS and a bare
`tag:` in Go, and `Model(j).Tag` was `""` where TS gave `"-"`.

That is **fixed in the engine**: `github.com/tabnas/parser/go` now
exports `DefaultTag = "-"` and `Make` applies it to an unset
`Options.Tag`, so both runtimes report `-`. The engine release that
`go/go.mod` requires carries the fix, so `Model(j).Tag` is `-` under
`GOWORK=off` and under a workspace alike.

`tag` is still left out of `model.tsv`, although nothing now stops it
joining `rules`/`graph` in the fixture. Once it does, this note can go.

## Trace output

Under tracing, each event prints one line to the instance's console
(TypeScript), to `opts["out"]` / `os.Stdout` (Go), or to the engine's
own debug sink, `parser.options.debug.output` (Rust). Every runtime
begins each parse with a `========= TRACE ==========` banner and logs
the enabled kinds (`step`, `rule`, `lex`, `parse`, `node`, `stack`);
most lines lead with the parse state: upcoming source, the token window
`[t0 t1]~[tin0 tin1]`, and the parse depth. Rust logs five of the six:
`step` has no engine hook.

Go derives the streams from the engine's hooks (rule/lex subscribers, a
parse-prepare hook, and after-open/after-close rule state actions); the
`parse` lines say `alt` / `no-alt` without the TS alt index, and `lex`
lines omit the matcher name, because the Go engine does not expose them.

Rust derives them from the engine's typed subscribers: `lex` from
`subscribe_lex`, `rule` and `stack` from `subscribe_rules` (the latter
reading the context's rule stack), and `parse` and `node` from
`subscribe_rule_done`, which carries the matched alternate, so Rust
`parse` lines DO report the alternate's push/replace/back/groups, though
still not an alt index. The banner is written from a parse-prepare hook,
as in Go. Output goes to the engine's own debug sink.

**Rust `step` never fires.** In TypeScript the engine itself calls
`ctx.log('step', …)` once per parse step; the Rust engine emits no such
event and has no `ctx.log`. The option is kept so the kind names stay in
step across runtimes, and selecting it is accepted, but nothing is
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
(`true | false | per-kind map | absent`, honouring `Defaults["trace"]`),
and its output can be captured (`opts["out"]`).

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
   engine has no equivalent `AltSpec` field: it folds counter conditions
   into the `C` function rather than retaining a separate map, so `CN=`
   is not emitted. The presence of a condition is still flagged by `C`,
   and declarative conditions are rendered via `CD=`.
6. **Token ordering.** The Go engine exposes token sets through Go maps
   (for example, `IGNORE` is a `map[Tin]bool`) and custom token names
   through `cfg.TinNames` (a `map[Tin]string`), neither of which
   preserves insertion order. Exact TS insertion-order parity is
   therefore not possible without engine changes; the Go port instead
   orders tokens and token-set members by tin (built-in tins in their
   canonical `TinBD..TinCA` order, then custom tins ascending) so the
   output is deterministic and diffable.


## Parity and remaining differences: Rust vs. canonical TypeScript

The Rust port covers `describe`, `model`, `abnf`, the `print` wrapper and
five of the six trace kinds, and passes the same shared
`test/spec/*.tsv` fixtures the other two runtimes do. The differences are
imposed by the Rust engine's public API and by Rust's type system:

1. **Free functions, not instance methods.** `describe(&parser)`,
   `model(&parser)` and `abnf(&parser)` take the instance, exactly as
   Go's package functions do: Rust cannot add methods to a type it does
   not own. Unlike Go they are **infallible**: the Go signatures return
   `(value, error)` because the Go engine's accessors can fail, while the
   Rust accessors cannot, so there is no error to surface.
2. **`print` requires `tabnas_debug::use_plugin`.** `Tabnas::use_plugin`
   is a concrete method, not a field that can be reassigned, so the TS
   `use()` wrapping is a free function here too. A plugin installed
   directly through `Tabnas::use_plugin` does not trigger the `USE:`
   dump. The plugin records its `print` setting as an instance
   *decoration* (`debug.print`), which is how the wrapper knows what to
   do.
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
   TypeScript insertion order, so Rust matches TS here where Go cannot.
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
   than the TypeScript `{n, d}` shape, and render as `CD=` entries; a
   callback condition shows only as the `C` flag, as everywhere.
9. **The shared-fixture loader is hand-written.** `@tabnas/support` has
   no Rust half, so `rs/tests/common/spec.rs` implements the loader. It
   is the one loader that CAN drift from the other two; `test/AGENTS.md`
   pins the format it has to keep.
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
    installs replace a shared selection, including with `trace: None`,
    which turns tracing off. TypeScript needs the same care for its own
    `use()` wrapper (`__debugUseWrapped`), and for the same reason:
    deriving a child re-runs the parent's plugins.
12. **The repeat loop renders as a repetition.** Here the Rust port
    LEADS the canonical: it reads the loop shape tabnas/bnf#80 compiles
    every repetition to and emits `*A` / `*( a b )`, where TypeScript
    and Go still list the loop as one of its own alternatives. See "The
    repeat loop: the Rust port leads" below.


## Two ABNF defects, fixed in the canonical runtime

`abnf()` had two cases that emitted output no conforming ABNF tool
accepts. Both were surfaced by an automated review of the Rust port
(tabnas/debug#37), verified against `ts/src/debug.ts` as canonical
behaviour rather than port defects, and fixed in tabnas/debug#45: in
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
   that runs to end of line, so the rule was left with no elements. That
   made it not merely non-round-tripping but impossible to parse, and one
   such token invalidated the whole grammar rather than just that rule. It
   now emits an RFC 5234 §4 prose-val, `T = <regex /…/>`, which is a real
   element.

All three runtimes emit the `collide` fixture byte-for-byte identically.


## The repeat loop: the Rust port leads

tabnas/bnf#80 changed how the BNF compiler (which the `abnf`, `ebnf` and
`gbnf` grammars compile through) emits every repetition. Before it, `*A`
compiled to a right-recursive helper `H = A H / ε`, each item pushing a
new `H`: a push chain, one frame per item. Since it, `*A` is a same-depth replace
loop. For `*A` with helper `H` (named as before, `_gen1_star_A`; `1*A`
is `A` followed by the star of `A`; a repetition inside a group nests as
`_gen2_star__gen1_group`):

```text
H             open   { c: {n.rep: 0}, n: {rep: 1}, r: H }   the entry: allocate, count
                     { s: FIRST(A), b: 1, r: H$alt0 } …     continue (a rule item)
                     { s: A, r: H }                         continue (a terminal item)
                     { s: FOLLOW(H), b: 1 }  { }            the exits
H$alt0        open   { p: A, n: {rep: 0} }                  push the item
              close  { r: H$alt0$step1, n: {rep: 1} }       capture it
H$alt0$step1  open   { r: H }                               back to the loop
```

The entry consumes nothing and names the loop itself: for what the
grammar recognises it does nothing. An emitter that counts it as an
alternative renders the loop as one of its own alternatives,
`r-gen1-star-A = [ r-gen1-star-A / r-gen1-star-A-alt0 ]`, and the
recompiled grammar rejects inputs the original accepts. That is what
the canonical TypeScript (`emitAbnf` in `ts/src/debug.ts`) and the Go
port emit today for a grammar in the new shape.

**The Rust emitter (`rs/src/abnf.rs`) implements the fix first; the
canonical and Go follow later.** This is the one place a port leads,
recorded here as the authority rules require, and it is a divergence
only until the other two catch up. What the Rust emitter does:

1. **Content is what an alternative consumes.** `has_content` is
   `len(s) - b > 0`, or a `p` target, or an `r` target other than the
   rule being rendered. `{ }` and the FOLLOW peek `{ s: FOLLOW, b: 1 }`
   are both epsilon. (The canonical still tests "`s` non-empty, or `p`,
   or `r`", which is how the entry and the peek were counted.)
2. **The self-replace entry is skipped** when rendering its own rule: it
   is bookkeeping, not syntax. Every other replace with the rule itself
   is content, as before: the close `{ s: A, b: 1, r: rule }` after an
   open that consumed `A` is the `[ rule ]` of `rule = A [ rule ]`. Only
   a loop has an entry to skip: a rule that is not one (its open
   alternatives hold no entry) keeps every alternative, a close in the
   entry's shape included, so `odd = A [ odd ]` renders as it always did.
3. **A rule whose open alternatives are the loop's whole scaffold,
   exactly as the compiler writes it, is a loop**, decided by shape
   rather than by name. Anything else renders as it did before. The
   scaffold, in order:

   - **The entry comes first, and only once.** It matches no token, not even a
     peeked one, pushes nothing, replaces the rule with itself, carries
     the guard `n.rep == 0` and no other condition, and sets that
     counter to 1 and no other. A user rule's own non-consuming self-replace, a
     guarded or counted state transition without that guard, is no
     loop and renders as a reference to the rule, as it always did. A
     second entry is neither a continue nor an exit: the first has set
     the counter, so its guard never holds.
   - **At least one continue**, each coming back to the rule having
     consumed or pushed on the way: directly (`{ s: A, r: H }`), or
     through a synthetic helper every way through which replaces
     onward to it (`H$alt0`, whose close replaces with `H$alt0$step1`,
     which replaces with `H`). A push is no back edge, even with an
     `r` beside it, which the engine does not follow. A continue
     carries no guard but the compiler's suffix-debt counter
     (`n.debt_… == 0`), since any other may contradict the state the
     entry leaves, as `n.rep == 0` does, and sets no counter. A
     helper's way back carries no condition at all and sets no counter
     but `rep`. A continue that a FOLLOW peek before it covers never
     runs: the compiler writes such dead continues where a token can
     both start the item and follow the loop, and they render as the
     source's alternatives, but a rule is a loop only when some
     continue is live.
   - **The empty exit `{ }`, with no condition**, and no continue after
     it, since it takes whatever comes. FOLLOW peeks may stand among
     the exits, before the continues too: the compiler puts one there
     where a keyword must end the loop rather than be taken as an item
     (`*word "end"` with `word = 1*ALPHA`). A peek shadows only what it
     peeks and stops the rule only where its token comes next, so no
     peek stands in for the empty exit. The compiler never guards an
     exit.
   - **Nothing a function decides.** An alternative whose route or
     backtrack a function decides, the entry's included, makes the
     rule no loop.

   So a rule with the entry and not the rest is no loop: a continue
   that never comes back (with the entry, `{ s: A }` and `{ }` take one
   `A` or nothing), one of several that never comes back, one that comes back
   having taken nothing, no exit or only a guarded or peeking one, an
   empty exit before a continue, the entry anywhere but first, a second
   entry, or an entry that peeks or carries a further condition.

   **Helpers.** The helpers of a loop `H` are the synthetic rules its
   iteration reaches short of a kept production, bounded by those
   productions and not by name: `H$alt0` and `H$alt0$step1`, the
   foldable group the iteration pushes, and the `$alt` / `$step` chain
   the compiler gives a group whose alternative starts with a rule
   (`( *A B / C )` has one). A kept production inside the iteration, a
   user rule, a nested loop, an old push-chain star or a synthetic rule
   that repeats by a cycle of its own, not through the loop, stays a
   reference by name with its own production, and nothing beyond it is
   reached. A `_plus` is judged for itself.

   **Rendering.** The loop is rendered wherever it is referenced as a
   repetition of its iteration: `*A` and `*"a"` when the iteration is
   one element, `*( a b )` otherwise, where the iteration is the
   ` / `-joined rendering of the continue alternatives. A terminal
   continue renders the token it consumes; a ref continue renders
   `H$alt0`'s pushed item (inlined when foldable) followed by its close
   continuation, and the back edges (`r: H$alt0$step1`, then `r: H`)
   render nothing. `H`, `H$alt0` and `H$alt0$step1` are never
   productions of their own.

   **Counted repetitions.** A `_plus` helper over a loop folds too when
   it is the compiler's construction: its chain (the helper and its
   `$step` helpers) consumes or pushes exactly the loop's own item, the
   token its continue consumes or the rule its iteration helper pushes
   when the way back takes nothing more, then pushes the loop, and its
   walk meets no cycle. Such a helper is
   written back as the `1*A` it was compiled from (a `_rep` helper as `2*A`):
   element by element it is `A *A`, the same language, but the `abnf`
   crate compiles `A *A` and `1*A` to different recognisers, and where
   `A` is nullable the recompiled `A *A` rejects inputs the original
   accepts. A chain over an item that merely renders the same, or ending
   in an old push-chain star, keeps its production. An old push-chain
   star inside the item is the item's, referenced by name, and never
   stops the fold.

   **Closes and the start.** A loop that is a USER rule keeps its
   production, whose body is the repetition followed by its close
   alternatives, as any rule's are. A close that replaces with the rule
   re-enters it, which is no back edge of the iteration, and renders as
   the rule's name: `H = *A [ B H ]`. The compiler's loops have no
   closes, and a synthetic rule is a loop only when any closes it has
   do nothing at all. A synthetic loop renders inline, wherever a rule
   refers to it, with no name of its own to render there, and the
   rendering would drop a close that could run it again: one that
   re-enters it, directly or through helpers, one that pushes, which
   comes back to the close phase when the pushed rule ends and runs the
   closes again, or one a function routes. The start rule is always a
   production, a synthetic loop included, since no rule encloses it to
   render it inline.
4. **A synthesised helper's kind is read from its own name segment**,
   the word after `_gen<n>_` in the part before any `$`, never from a
   name it embeds. A repetition's helper is named after its item: a star
   over an optional is `_gen3_star__gen2_opt__gen1_group`, and its
   iteration helpers carry the whole of that name. The canonical decides
   the `[ … ]` wrap by a test for `_opt` anywhere in the name, which is
   harmless there (those names are never inlined) and, once the loop
   renders them inline, wrapped the loop and its step as options too:
   `*[ [ T ] [  ] ]`,
   with an empty option RFC 5234 does not allow, where `*[ T ]` was
   meant.
5. **The old shape renders exactly as before.** A push-chain `_star`,
   its `_plus` and their `$alt` helpers carry no self-replace entry and
   stay kept productions (`r-gen1-star-A = [ A r-gen1-star-A ]`), as
   `TestAbnfKeepsRepetitionProduction`, the TypeScript "keeps repetition
   as a production" case and `abnf_keeps_a_repetition_production` pin.

The shapes are pinned by `rs/tests/abnf_test.rs` (`rep = *"a"`,
`rep = 1*"a"`, `rep = 2*"a"`, `n = 2*4"z"`, `doc = *item`,
`list = "[" *( "," item ) "]"`, `s = *( "a" / "b" ) ";"`,
`outer = *( "<" *"i" ">" )`, `top = *[ "," ]`, `top = 1*[ "a" ]`,
`top = 1*( "a" "b" )`, `top = 1*( "a" / "b" )`, hand-built from
the compiler's output because the emitter must never gain an ABNF
dependency, even in a test), which check the emitted text is RFC 5234
(no dangling `/`, legal rule names). More pin what the shape excludes,
each rendering as it always did:

- a user rule's unguarded self-replace, `st = st / A / B`
- a guarded close continuation, `one = A [ one ]`
- a close in the entry's whole shape on a rule without the scaffold,
  `odd = A [ odd ]`
- a rule with the entry and not the rest of the scaffold, `once = [
  once / A ]` for the entry, `{ s: A }` and `{ }`, and eighteen more,
  one for each way the scaffold can fail
- a rule item's loop whose step never comes back, and one whose step
  comes back only under a condition
- a synthetic rule whose close re-enters it, pushes, or reaches a
  helper a function routes or whose close pushes

And what it keeps:

- an old push-chain star inside a loop's group stays a kept
  production, `top = *( B r-gen1-star-A C )`
- a user loop whose close re-enters it renders `once = *A [ B once ]`
- a synthetic loop as the start rule renders `r-gen1-star-A = *A`
- a plus over a different item that renders the same keeps its
  production, as does one whose loop takes more than its item,
  `item *( item B )`

Four more pin the bound: a group with a `$alt` chain of its own inside
a loop renders as `top = *( *A B / C ) D`, the plus over it as
`top = 1*( *A B / C ) D`, a plus whose item holds an old push-chain
star as `top = 1*( B r-gen1-star-A C )`, and a loop over a helper that
repeats by its own cycle as `top = *r-gen2-group-alt0` with
`r-gen2-group-alt0 = A r-gen2-group-alt0 / B`. No shared `test/spec` fixture pins
them yet: all three runtimes run those, and two do not render the loop
yet. When TypeScript and Go follow, the shapes move to `test/spec` and
this section becomes history like the one above it.
