# Reference (Rust)

The exact Rust surface of the debug plugin. For the cross-language
contract and the authoritative divergence register, see
[`../../docs/reference.md`](../../docs/reference.md). TypeScript
(`ts/src/debug.ts`) is canonical.

## Import

```rust
use tabnas_debug::{
    abnf, apply, describe, model, plugin, use_plugin,
    DebugOptions, TraceKinds, SECTIONS, TRACE_BANNER, VERSION,
    DebugModel, DebugAltInfo, DebugConfigInfo, DebugLexMatcher,
    DebugPluginInfo, DebugRuleEdges, DebugRuleInfo, DebugSeqItem,
    DebugTokenInfo, DebugTokenSet,
};
```

## Exported symbols

| Symbol | Form |
|---|---|
| `plugin(options)` | `fn(DebugOptions) -> tabnas::Plugin` |
| `apply(parser, options)` | `fn(&mut Tabnas, DebugOptions) -> Result<(), PluginError>` |
| `use_plugin(parser, plugin, options)` | `fn(&mut Tabnas, Plugin, Option<Value>) -> Result<(), PluginError>` |
| `describe(parser)` | `fn(&Tabnas) -> String` |
| `model(parser)` | `fn(&Tabnas) -> DebugModel` |
| `abnf(parser)` | `fn(&Tabnas) -> String` |
| `SECTIONS` | `[&str; 8]`, the `describe` banners, in order |
| `TRACE_BANNER` | `&str`, written once per traced parse |
| `VERSION` | `&str` |

`describe`, `model` and `abnf` are **free functions and infallible**:
Rust cannot add methods to a foreign type (so they are not instance
methods as in TypeScript), and the Rust engine's accessors cannot fail
(so they do not return `Result` as in Go).

## `DebugOptions`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `print` | `bool` | `true` | Log `USE:` plus the full `describe` dump when a later plugin is loaded via `use_plugin`. |
| `trace` | `Option<TraceKinds>` | `Some(TraceKinds::all())` | Which parse events to log; `None` traces nothing. |

| Constructor / builder | Meaning |
|---|---|
| `DebugOptions::default()` / `::new()` | The canonical defaults: printing on, every trace kind on. |
| `DebugOptions::quiet()` | Introspection only: no `USE:` dumps, no tracing. |
| `.with_print(bool)` | Set `print`. |
| `.with_trace(TraceKinds)` | Trace exactly these kinds. |
| `.without_trace()` | Trace nothing. |

There is no `out` option. The output sink belongs to the ENGINE
(`parser.options.debug.output`, stderr by default) and the plugin writes
both trace lines and the `USE:` dump through it.

## `TraceKinds`

A struct of six `bool` fields (`step`, `rule`, `lex`, `parse`, `node`,
`stack`) with `TraceKinds::all()` and `TraceKinds::none()`
constructors, so a partial selection is explicit rather than depending on
a merge rule.

**`step` never fires.** The Rust engine has no `ctx.log` and emits no
per-step event. The kind is kept so the names stay in step across
runtimes; selecting it is accepted and logs nothing, and selecting it
ALONE installs no tracing at all, not even the per-parse banner.

## `describe(parser) -> String`

Eight sections, in this order, with these exact banners:

| Banner | Contents |
|---|---|
| `========= INSTANCE ========` | The instance tag. The engine defaults an unset tag to `-`. |
| `========= TOKENS ========` | Each token: tin, name, and fixed source text when it has one; then a token-set sub-block. |
| `========= RULES =========` | Each rule's push/replace transition tree: open-push (`op`), open-replace (`or`), close-push (`cp`), close-replace (`cr`). Empty categories are omitted. |
| `========= ALTS =========` | Each rule's open and close alternates: token sequence, `r=`/`p=` targets, `b=`, `n=`, the `A`/`C`/`H` presence flags, declarative conditions (`CD=`), and `g=`. |
| `========= LEXER =========` | Custom lexer matchers only. The built-in enable flags are under `CONFIG`. |
| `========= CONFIG ========` | `start`, `finish`, `safeKey`, and the eight `lex.*` enable flags. |
| `========= PLUGIN =========` | Each plugin by its declared name, plus its options when the instance holds a bag for it. |
| `========= ABNF =========` | The output of `abnf`. |

The banners are the cross-runtime parity contract, pinned by
`../../test/spec/sections.tsv`. The text between them is not pinned.

## `model(parser) -> DebugModel`

```rust
pub struct DebugModel {
    pub tag: String,
    pub tokens: Vec<DebugTokenInfo>,
    pub token_sets: Vec<DebugTokenSet>,   // serialises as "tokenSets"
    pub rules: Vec<DebugRuleInfo>,
    pub graph: Vec<DebugRuleEdges>,
    pub lexer: Vec<DebugLexMatcher>,
    pub config: DebugConfigInfo,
    pub plugins: Vec<DebugPluginInfo>,
    pub abnf: String,
}
```

Every type derives `serde::Serialize`, with names chosen to match the
TypeScript field names and the Go JSON tags exactly (`tokenSets`,
`openPush`, `openReplace`, `closePush`, `closeReplace`, `safeKey`), which
is what makes `../../test/spec/model.tsv` shareable across runtimes.

| Type | Fields |
|---|---|
| `DebugTokenInfo` | `tin`, `name`, `fixed` (skipped when absent) |
| `DebugTokenSet` | `name`, `tins` (ascending) |
| `DebugSeqItem` | untagged: a `String`, or a `Vec<String>` for a multi-token position |
| `DebugAltInfo` | `seq`, `push`, `replace`, `back`, `counters` (all skipped when absent), `groups`, `action`, `cond`, `modifier` |
| `DebugRuleInfo` | `name`, `open`, `close` |
| `DebugRuleEdges` | `name`, `openPush`, `openReplace`, `closePush`, `closeReplace` |
| `DebugLexMatcher` | `order` (`f64`, since priorities are not necessarily whole), `matcher`, `make` (always `""`, because Rust function values carry no name) |
| `DebugConfigInfo` | `start`, `finish`, `safeKey`, `lex` (the eight flags, in canonical order) |
| `DebugPluginInfo` | `name`, `options` (skipped when absent) |

The start rule is `model.config.start`, **not** `model.start`, as in
every runtime.

A function-valued push/replace target is `"<fn>"`. `back` omits an
explicit `0` and `counters` an empty map, because Rust integers and maps
are not nullable (the same reason Go's do).

## `abnf(parser) -> String`

A re-compilable ABNF rendering of the live grammar, read from the engine
alone, never from an ABNF compiler.

Rules become productions in start-rule-first order, open alternates
become `/`-separated alternatives, and tokens become named terminals
defined in a legend after the productions, with `=` aligned:

```abnf
val = add
add = NR [ PL add ]

NR = <number>
PL = "+"
```

| Token kind | Legend form |
|---|---|
| fixed literal containing a letter | `%s"hi"` (RFC 7405, case-sensitive) |
| fixed literal of punctuation | `"+"` |
| fixed literal a char-val cannot hold | `%x0D.0A` |
| match regex, single char range | `%x30-39` |
| match regex, case-insensitive literal | `"foo"` |
| any other match regex | `<regex /…/>` (an RFC 5234 prose-val) |
| function-backed match token | `<built-in NAME>`. No ABNF form exists, so it falls through to the description, as the canonical runtime does |
| built-in lexer token | `<number>`, `<string>`, `<text>`, … |

Every angle-bracket form here is an RFC 5234 §4 prose-val, the construct
the grammar provides for describing a rule in prose. A prose-val cannot
hold a `>` or anything outside printable ASCII, so those are escaped:
`^a>b` renders as `<regex /^a\u003Eb/>`. Such a token does not
round-trip, but the grammar still parses.

An instance with no rules emits the empty string.

## `use_plugin(parser, plugin, options)`

Installs `plugin`, then dumps `USE: <name>` plus `describe` when the
debug plugin was installed with `print` on. `Tabnas::use_plugin` is a
concrete method rather than a field you can reassign, so this wrapping is a
free function (as it is in Go). A plugin installed directly through
`Tabnas::use_plugin` does not trigger the dump, and a failing plugin's
error propagates unchanged.

The plugin records its `print` setting as an instance decoration
(`debug.print`), which is how the wrapper knows what to do, so calling
`use_plugin` on an instance with no debug plugin is simply silent, not an
error.

## Trace output

Each traced parse opens with `TRACE_BANNER`
(`\n========= TRACE ==========`), then one line per event, prefixed by
kind:

| Prefix | Carries |
|---|---|
| `lex` | token name, value, position, row:column, source text |
| `rule` | rule name, instance, state, depth, node, live counters |
| `stack` | the context's rule stack |
| `parse` | the matched alternate's push/replace/back/groups, or `no-alt` |
| `node` | the rule's node as it completes |

Lines are indented by rule depth. Output goes to
`parser.options.debug.output`.

**Re-applying the plugin updates the selection; it does not stack.** The
engine accumulates subscribers and parse-prepare hooks, so a second
install would otherwise double every banner and every event, and a
narrower selection could not switch the first set off. The callbacks are
registered exactly once per instance and read a shared selection that
later installs replace, including with `trace: None`, which turns
tracing off. This matters because `Tabnas::derive` re-runs a parent's
plugins on the child.

## `VERSION`

Must equal `rs/Cargo.toml`'s `version` and `ts/package.json`'s
`"version"`. `rs/tests/version_test.rs` fails the build if they drift.
