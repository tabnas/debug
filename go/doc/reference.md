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
| `Debug(j *tabnas.Tabnas, opts map[string]any) error` | func (a `tabnas.Plugin`) | The plugin. Load it with `j.Use(tabnasdebug.Debug, opts)`. |
| `Describe(j *tabnas.Tabnas) (string, error)` | func | A sectioned, human-readable dump of the instance's grammar. |
| `Model(j *tabnas.Tabnas) (*DebugModel, error)` | func | The same information as a typed, JSON-serialisable object. |
| `Abnf(j *tabnas.Tabnas) (string, error)` | func | The live grammar rendered as ABNF text. |
| `Use(j *tabnas.Tabnas, plugin tabnas.Plugin, opts ...map[string]any) error` | func | `(*Tabnas).Use` plus the `print` behaviour: logs `USE:` and a `Describe` dump. |
| `Defaults` | `map[string]any` | The default options (`{"print": true, "trace": true}`). |
| `Version` | `string` | The module version. |
| `DebugModel`, `DebugTokenInfo`, `DebugTokenSet`, `DebugAltInfo`, `DebugRuleInfo`, `DebugRuleEdges`, `DebugLexMatcher`, `DebugConfigInfo`, `DebugPluginInfo` | structs | The typed shape of `Model`'s result, mirroring the TS exported types. |

## `Debug` — the plugin

```go
func Debug(j *tabnas.Tabnas, opts map[string]any) error
```

Loading it with `j.Use(tabnasdebug.Debug, opts)` installs the parse-trace
streams when tracing is enabled and records the `print` setting for
`tabnasdebug.Use`. Loading itself never panics: a panic while wiring
subscribers is returned as an `"internal"`-code `*tabnas.TabnasError`.

### Options

| Key | Type | Default | Meaning |
|---|---|---|---|
| `"print"` | `true` / `false` / `*bool` / absent | `true` (from `Defaults`) | Log `USE:` plus a full `Describe` dump when a later plugin is loaded via `tabnasdebug.Use`. |
| `"trace"` | `true` / `false` / `*bool` / per-kind map / absent | `true` (from `Defaults`) | Which parse events to log. |
| `"out"` | `io.Writer` | `os.Stdout` | Where trace and print output is written. |

`trace` handling mirrors the TypeScript `true | false | object`:

- `true` — log every kind (`step`, `rule`, `lex`, `parse`, `node`, `stack`);
- an explicit `false` (or a `*bool` false) — off;
- a per-kind map (`map[string]any` or `map[string]bool` of kind →
  boolean) — on; the map is merged over the all-true defaults, so a
  partial map cannot turn other kinds off implicitly (set them `false`
  explicitly) — matching the engine-side deep-merge of `Debug.defaults`
  in TypeScript;
- absent (or `opts` is `nil`) — falls back to `Defaults["trace"]` (on).

## `Use(j, plugin, opts...) (error)`

The Go form of the TypeScript `print` behaviour. The TS plugin wraps the
instance's `use()` in place; the Go engine's `(*Tabnas).Use` is a
concrete method and cannot be reassigned, so the wrapped form is a
package function:

```go
j.Use(tabnasdebug.Debug, map[string]any{"print": true})
tabnasdebug.Use(j, myPlugin, myOpts) // loads myPlugin, then logs:
// USE: myPlugin
//
// ========= INSTANCE ========
// ...full Describe dump...
```

It delegates to `j.Use` and, when the instance's `print` option is
active, logs `USE: <plugin name>` plus the full `Describe` dump to the
configured writer. A plugin load error is returned unchanged and
suppresses the log (matching TS, where a throwing `use()` never reaches
the log). The plugin name is derived from the plugin function's symbol,
the Go analogue of the TS function `name` property.

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
| `========= PLUGIN =========` | The loaded plugins by name (derived from each plugin function's symbol), plus any options registered via `Tabnas.SetPluginOptions`. |
| `========= ABNF =========` | The grammar rendered as ABNF (same as `Abnf`). |

The headers are identical to the TypeScript port's, pinned by a shared
golden fixture, so dumps from either runtime are diffable.

The `error` return upholds the engine's no-panic guarantee: a malformed
grammar (nil config, nil rule spec, nil alternate) is rendered
defensively, and any remaining panic is recovered and returned as an
`"internal"`-code `*tabnas.TabnasError` with an empty string. On success
the error is `nil`. (A `nil` instance, for example, surfaces as an
internal error rather than a crash.)

## `Model(j) (*DebugModel, error)`

The structured counterpart to `Describe`, mirroring the canonical
TypeScript `tn.debug.model()`. The grammar-structure fields are
JSON-serialisable (all types carry JSON tags matching the TS field
names) and round-trip through `encoding/json`.

```go
type DebugModel struct {
	Tag       string            `json:"tag"`
	Tokens    []DebugTokenInfo  `json:"tokens"`
	TokenSets []DebugTokenSet   `json:"tokenSets"`
	Rules     []DebugRuleInfo   `json:"rules"`
	Graph     []DebugRuleEdges  `json:"graph"`
	Lexer     []DebugLexMatcher `json:"lexer"`
	Config    DebugConfigInfo   `json:"config"`
	Plugins   []DebugPluginInfo `json:"plugins"`
	Abnf      string            `json:"abnf"`
}
```

| Field | Shape | Notes |
|---|---|---|
| `Tag` | `string` | The instance tag (`""` when unset). |
| `Tokens` | `[]DebugTokenInfo` (`Tin int`, `Name string`, `Fixed string,omitempty`) | The token table; `Fixed` present only for fixed (literal) tokens. |
| `TokenSets` | `[]DebugTokenSet` (`Name string`, `Tins []int`) | Named token sets (`IGNORE`, `VAL`, `KEY`) and their member tins. |
| `Rules` | `[]DebugRuleInfo` | Each rule's name and its `Open` / `Close` alternates as `DebugAltInfo`. |
| `Graph` | `[]DebugRuleEdges` | Per-rule push/replace edges (`OpenPush`, `OpenReplace`, `ClosePush`, `CloseReplace`); function-valued targets recorded as `"<fn>"`. |
| `Lexer` | `[]DebugLexMatcher` (`Order`, `Matcher`, `Make`) | The custom lexer matchers, priority order; `Make` is the matcher function's name when recoverable. |
| `Config` | `DebugConfigInfo` (`Start`, `Finish`, `SafeKey`, `Lex map[string]bool`) | Start rule, finish flag, safe-key, per-lexer enable flags. |
| `Plugins` | `[]DebugPluginInfo` (`Name`, `Options,omitempty`) | The applied plugins by symbol name; options attached when registered via `SetPluginOptions`. |
| `Abnf` | `string` | Same text as `Abnf(j)`. |

`DebugAltInfo` mirrors the TS type:

```go
type DebugAltInfo struct {
	Seq      []any          `json:"seq"`                // token name(s) per lookahead position
	Push     string         `json:"push,omitempty"`     // `p` target rule (or "<fn>")
	Replace  string         `json:"replace,omitempty"`  // `r` target rule (or "<fn>")
	Back     int            `json:"back,omitempty"`     // `b` token push-back
	Counters map[string]int `json:"counters,omitempty"` // `n` counter ops
	Groups   []string       `json:"groups"`             // `g` group tags
	Action   bool           `json:"action"`             // `a` present
	Cond     bool           `json:"cond"`               // `c` (or declarative CD) present
	Modifier bool           `json:"modifier"`           // `h` present
}
```

`Seq` entries are token *names* (e.g. `"#NR"`); a multi-token lookahead
position is a nested `[]any` of names (so the field round-trips through
JSON unchanged), and a wildcard (unconstrained) position is the empty
string. A nil alternate — the Go counterpart of the TS null alt entry —
renders defensively as the single entry `"***INVALID***"`.

Like `Describe`, `Model` returns an error to uphold the no-panic
guarantee: any recovered panic surfaces as an `"internal"`-code
`*tabnas.TabnasError` with a nil model. Rules and tokens are ordered
deterministically (rules by name, tokens by tin) rather than in TS
insertion order.

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
var Defaults = map[string]any{"print": true, "trace": true}
```

Used when the plugin is loaded with no explicit `print` / `trace`
options. Mirrors the TypeScript `Debug.defaults`, where printing and
tracing are on by default (a bare `true` for trace enables every kind).

## Trace output

When tracing is on, each parse begins with a `========= TRACE ==========`
banner, then each enabled kind writes one line per event to `opts["out"]`
(default `os.Stdout`). Every TypeScript trace kind has a Go stream; most
lines lead with the parse state — the upcoming source, the token window
`[src0 src1]~[name0 name1]`, and the parse depth — mirroring the TS
`descParseState` prefix.

| Kind | Line prefix | Logs |
|---|---|---|
| `rule` | `  rule ` | A rule opening (`OPEN`) or closing (`CLOSE`): name, instance, depth, prev/parent/child ids, counters/props (`N<…> U<…> K<…>`). |
| `lex` | `  lex  ` | A token produced: name, source, index, row:col. |
| `parse` | `  parse` | An alternate matched (`alt`, or `no-alt`): the matched token sequence, any push (`p:`) / replace (`r:`) target, counters/props. |
| `node` | `  node ` | A node-build step: the `why` code and the node so far. |
| `stack` | `  stack` | The current rule stack and the partial nodes. |
| `step` | `  step ` | The parse-loop iteration counter. |

The `rule`, `stack` and `step` streams come from the engine's rule
subscriber (fired at the same point the TS engine logs them), `lex` from
the lex subscriber, and `parse` / `node` from after-open/after-close rule
state actions installed at parse start (the closest Go analogue of the
TS engine's post-match log points). Two shape differences remain: `parse`
lines carry `alt` / `no-alt` without the TS alt *index* (the engine does
not expose which alternate matched), and `lex` lines omit the matcher
name.

## Differences from the TypeScript surface

| Area | TypeScript | Go |
|---|---|---|
| `describe` form | `tn.debug.describe()` method, returns `string` | `Describe(j)` package func, returns `(string, error)` |
| `model` form | `tn.debug.model()` method, returns `DebugModel` | `Model(j)` package func, returns `(*DebugModel, error)` |
| `abnf` form | `tn.debug.abnf()` method | `Abnf(j)` package func, returns `(string, error)` |
| `print` option | wraps the instance's `use()` in place | `tabnasdebug.Use(j, plugin, opts...)` package func (the engine's `Use` method cannot be wrapped) |
| Trace kinds | `step`, `rule`, `lex`, `parse`, `node`, `stack` | same six kinds; `parse` lines lack the alt index, `lex` lines the matcher name |
| Trace destination | instance console (`get_console`) | `opts["out"]` / `os.Stdout` |
| `LEXER` section | every matcher | custom matchers only |
| `PLUGIN` section | each plugin + options | each plugin by symbol name + options when registered via `SetPluginOptions` |
| Token ordering | engine insertion order | sorted by tin (built-ins first), for determinism |

These divergences are imposed by the Go engine's public API; they are
explained in [concepts](concepts.md) and were carried over from the
project's combined `docs/reference.md`.
