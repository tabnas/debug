# Concepts (Rust)

The *why* and *how* for the Rust port (`tabnas_debug`). Reach for it when
something in the output surprises you, or to understand the engine
relationship and the deliberate differences from the canonical
TypeScript implementation. The [reference](reference.md) lists *what*;
this explains the mechanism.

## What the crate is for

A grammar built from rules, alternates and tokens is hard to hold in your
head. This crate makes one *visible*, four ways:

- **`describe`** renders the live instance as labelled text.
- **`model`** returns the same information as typed, serialisable data,
  the surface other repositories' tests assert against.
- **`abnf`** re-expresses the grammar as ABNF.
- **tracing** logs the parse as it happens.

It is a developer tool, **not part of the parse path**. Nothing here is a
runtime dependency of a grammar.

## The engine relationship

The crate reads the engine's public surface and nothing else: rule specs
(`rule_specs`), token identities (`token_name`, `fixed_source`), options
(`options.rule`, `options.fixed`, `options.lex`, …), and the installed
plugin list. That is why `describe` / `model` / `abnf` need no plugin
installed: they are pure functions of an instance.

Tracing is the exception. It has to *observe* a parse, so it installs
subscribers, which means installing the plugin.

## Free functions, not instance methods

TypeScript attaches `describe` / `model` / `abnf` to the instance
(`tn.debug.describe()`). Rust cannot add methods to a type it does not
own, so (exactly as the Go port does) they are free functions taking
`&Tabnas`.

Unlike Go they are **infallible**. The Go signatures return
`(value, error)` to uphold the engine's no-panic guarantee, because the
Go engine's accessors can fail. The Rust accessors cannot, so there is no
error to surface and returning `Result` would be noise.

## The eight sections are the contract

`describe` emits eight banners, in order:

```
INSTANCE  TOKENS  RULES  ALTS  LEXER  CONFIG  PLUGIN  ABNF
```

`../../test/spec/sections.tsv` pins them byte-for-byte for every
grammar in the shared registry, in all three runtimes. That is what makes two
runtimes' dumps diffable. The text BETWEEN the banners is deliberately
not pinned: the engines expose different detail, and pinning the prose
would freeze an accident.

They live in one place, `describe::SECTIONS`, so a test can assert on
them without copying the strings.

## ABNF: the round-trip

The emitter is the empirical inverse of an ABNF compiler's forward
encoding. Rules become productions, open alternates become `/`-separated
alternatives, the token sequence plus any push/replace target becomes an
element list, and each token resolves to a terminal via the fixed-literal
or match-regex config.

Two properties of the emitter are essential:

**It must never depend on an ABNF compiler.** It reads only the live
engine. A dependency would make the round-trip claim circular, which is
also why the fixture grammars are hand-written against the engine rather
than compiled from ABNF source.

**Synthetic rules fold back.** A forward compiler synthesises helper
rules for `[...]`, `*(...)`, groups and chain steps, named `_gen<n>_…` or
carrying a `$`. Emitting those as productions would reproduce the
expanded internal form rather than the grammar someone wrote, so the
foldable ones are inlined back into the construct they encode.
Repetition helpers (`_star` / `_plus` and their `$alt…` partners) use a
probe-optimised subgraph that does not reconstruct reliably, so those are
emitted unchanged, still a valid, recognition-equivalent grammar.

Lookahead is not output. An alternate consumes `len(s) - b` tokens;
tokens beyond that were matched to choose the alternate and then pushed
back. Rendering them as ABNF elements would claim input the alternate
never eats.

## Where the trace kinds come from

TypeScript drives all six kinds through the engine's `ctx.log`. The Rust
engine has no `ctx.log`; it has typed subscribers, so each kind is wired
to the one that carries its information:

| Kind | Rust source |
|---|---|
| `lex` | `subscribe_lex` |
| `rule` | `subscribe_rules` |
| `stack` | `subscribe_rules` (the context's rule stack) |
| `parse` | `subscribe_rule_done` (the matched alternate) |
| `node` | `subscribe_rule_done` (the completed node) |
| `step` | no engine hook |

`RuleDone` carries the matched alternate's push/replace/back/groups, so
Rust `parse` lines say more than the Go port's can, though no runtime
but TypeScript reports an alt *index*.

## Ordering

Rules come out of the engine's `IndexMap`, so their order IS the
TypeScript insertion order: Rust matches TS here where Go cannot. Token
sets, the token table and alt counter maps come out of unordered maps, so
those are sorted (by name, by tin, by name) rather than left to iteration
order. An unstable dump is not a dump.

## Differences from the TS version

TypeScript is canonical; this Rust port mirrors its option names,
defaults, output format and section ordering, and passes the same shared
`../../test/spec/*.tsv` fixtures. The differences are **intentional**,
imposed by the Rust engine's API and by Rust's type system. The full list
is in [`../../docs/reference.md`](../../docs/reference.md); the ones that
change how you write code:

| Area | TypeScript | Rust |
|---|---|---|
| **Entry** | `tn.use(Debug, options)` | `apply(&mut parser, options)`, or `parser.use_plugin(plugin(options), None)` |
| **Options** | an object, `trace: true \| false \| per-kind` | a typed `DebugOptions` with `trace: Option<TraceKinds>` |
| **Introspection** | `tn.debug.describe()` | `describe(&parser)`, a free function, and infallible |
| **`print`** | wraps `tabnas.use` | the free function `use_plugin(&mut parser, plugin, options)` |
| **Output sink** | the instance console | the engine's `options.debug.output` (stderr by default); no `out` option |
| **`step` kind** | fires once per parse step | **never fires**: no engine hook |
| **Rule order** | insertion order | insertion order (matches TS) |
| **Token order** | insertion order | by tin, since the engine holds them unordered |

One more, about the test harness rather than the library:
`@tabnas/support` has no Rust half, so `rs/tests/common/spec.rs`
implements the shared-fixture loader. It is the one loader that can drift
from the other two, and `../../test/AGENTS.md` pins the format it has to
keep.
