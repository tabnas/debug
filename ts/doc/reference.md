# Reference

The exact API surface of `@tabnas/debug` (TypeScript / JavaScript). This
implementation (`ts/src/debug.ts`) is canonical; the Go port tracks it
(see [`go/doc/reference.md`](../../go/doc/reference.md)).

## Exports

```js
const { Debug } = require('@tabnas/debug')
```

| Export | Type | Description |
|---|---|---|
| `Debug` | `Plugin` | The plugin function. Load it with `tn.use(Debug, options)`. |
| `Debug.defaults` | `DebugOptions` | The default options (see below). |

The module also exports the TypeScript types that describe `model()`'s
return value: `DebugModel`, `DebugTokenInfo`, `DebugTokenSet`,
`DebugAltInfo`, `DebugRuleInfo`, `DebugRuleEdges`, `DebugLexMatcher`,
`DebugConfigInfo`, `DebugPluginInfo`.

## Loading

```js
tn.use(Debug, options)
```

Applying the plugin attaches a `debug` object to the instance and, unless
disabled, wraps `use()` to print a description and installs parse-trace
logging.

## Options — `DebugOptions`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `print` | `boolean` | `true` | After every later `use()`, log `USE:` plus the full `describe()` dump to the instance console. |
| `trace` | `true` \| `false` \| per-kind flags | `true` (all kinds) | Which parse events to log. |

`trace` accepts:

- `true` — log every kind.
- `false` — log nothing.
- an object of `kind -> boolean` — log only the kinds set to `true`. The
  kinds are `step`, `rule`, `lex`, `parse`, `node`, `stack`. The engine
  deep-merges `Debug.defaults` (all kinds `true`) with your object, so a
  partial object cannot turn other kinds off implicitly — set them
  `false` explicitly.

`Debug.defaults`:

```js
{
  print: true,
  trace: { step: true, rule: true, lex: true, parse: true, node: true, stack: true },
}
```

## Instance methods (`tn.debug`)

Once loaded, the instance carries `tn.debug` with these methods. None
mutate the instance; all read the live engine state.

| Method | Returns | Description |
|---|---|---|
| `describe()` | `string` | A human-readable, sectioned dump of the instance's active grammar. |
| `model()` | `DebugModel` | The same information as a typed, JSON-serialisable object. |
| `abnf()` | `string` | The live grammar rendered as ABNF text. |

### `describe(): string`

Returns one string organised into these sections, in this order, with
these exact headers:

| Header | Contents |
|---|---|
| `========= INSTANCE ========` | The instance tag (`tag:`), empty when unset. |
| `========= TOKENS ========` | Each token: name, tin, fixed source text (if any). Then a token-set sub-block listing each set's member token names. |
| `========= RULES =========` | Each rule's push/replace transition tree: distinct rule-name targets reached by open-push (`op`), open-replace (`or`), close-push (`cp`), close-replace (`cr`). Empty categories omitted; function-valued targets render as `<F>`. |
| `========= ALTS =========` | Each rule's open/close alternates: token sequence, push (`p`), replace (`r`), backtrack (`b`), counters (`n`), the action/condition/modifier flags (`A`/`C`/`H`), declarative condition (`CN`/`CD`), group (`g`). A null entry in a sequence renders as `***INVALID***`. |
| `========= LEXER =========` | Each lexer matcher: `order: matcher (make)`. |
| `========= CONFIG ========` | `start`, `finish`, `safeKey`, and the built-in lex enable flags. |
| `========= PLUGIN =========` | Each loaded plugin and its options. |
| `========= ABNF =========` | The grammar rendered as ABNF (same as `abnf()`). |

### `model(): DebugModel`

The structured counterpart to `describe()`. The grammar-structure fields
are JSON-serialisable and round-trip through `JSON.stringify`.

```ts
type DebugModel = {
  tag: string
  tokens: DebugTokenInfo[]
  tokenSets: DebugTokenSet[]
  rules: DebugRuleInfo[]
  graph: DebugRuleEdges[]
  lexer: DebugLexMatcher[]
  config: DebugConfigInfo
  plugins: DebugPluginInfo[]
  abnf: string
}
```

| Field | Shape | Notes |
|---|---|---|
| `tag` | `string` | The instance tag (`''` when unset). |
| `tokens` | `{ tin: number; name: string; fixed?: string }[]` | The token table; `fixed` present only for fixed (literal) tokens. |
| `tokenSets` | `{ name: string; tins: number[] }[]` | Named token sets and their member tins. |
| `rules` | `DebugRuleInfo[]` | Each rule's name and its `open` / `close` alternates. |
| `graph` | `DebugRuleEdges[]` | Per-rule push/replace edges. |
| `lexer` | `{ order: number; matcher: string; make: string }[]` | The lexer matchers, in priority order. |
| `config` | `DebugConfigInfo` | Start rule, finish flag, safe-key, per-lexer enable flags. |
| `plugins` | `{ name: string; options?: object }[]` | The applied plugins. |
| `abnf` | `string` | Same text as `abnf()`. |

#### `DebugRuleInfo` and `DebugAltInfo`

```ts
type DebugRuleInfo = { name: string; open: DebugAltInfo[]; close: DebugAltInfo[] }

type DebugAltInfo = {
  seq: (string | string[])[]          // token name(s) per lookahead position
  push?: string                       // `p` target rule (or '<fn>')
  replace?: string                    // `r` target rule (or '<fn>')
  back?: number                       // `b` token push-back
  counters?: Record<string, number>   // `n` counter ops
  groups: string[]                    // `g` group tags
  action: boolean                     // `a` present
  cond: boolean                       // `c` present
  modifier: boolean                   // `h` present
}
```

`seq` entries are token *names* (e.g. `'#NR'`); a multi-token lookahead
position is a nested `string[]`. A null entry renders as the literal
string `'***INVALID***'`. `push`/`replace` are the literal target rule
name, or `'<fn>'` for a function-valued target. `back`, `counters`,
`push`, `replace` are present only when set.

#### `DebugRuleEdges`

```ts
type DebugRuleEdges = {
  name: string
  openPush: string[]      // distinct `p` targets of open alts
  openReplace: string[]   // distinct `r` targets of open alts
  closePush: string[]     // distinct `p` targets of close alts
  closeReplace: string[]  // distinct `r` targets of close alts
}
```

Each array holds the distinct rule-name targets; a function-valued target
is recorded as `'<fn>'`.

#### `DebugConfigInfo`

```ts
type DebugConfigInfo = {
  start: string
  finish: boolean
  safeKey: boolean
  lex: Record<string, boolean>   // fixed, space, line, text, number, comment, string, value
}
```

### `abnf(): string`

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
| Anything else | `; /<regex>/<flags>` (an ABNF comment) |

The emitter reads only the running engine; it never imports an ABNF
library. See [concepts](concepts.md) for the round-trip contract.

## Trace output

When `trace` is on, each parse event logs one line to the instance's
console provider — `config.debug.get_console()`, which defaults to the
global `console`. Supply a custom `get_console()` (via the instance
options `{ debug: { get_console } }`) to capture or redirect the lines.

Output begins with a `========= TRACE ==========` banner. Each kind has
its own line format; most lines lead with the upcoming source, the token
window `[t0 t1]~[tin0 tin1]`, and the parse depth. The kinds:

| Kind | Logs |
|---|---|
| `rule` | A rule opening (`OPEN`) or closing (`CLOSE`): name, instance, depth, prev/parent/child. |
| `lex` | A token produced: name, source, index, row:col, matcher. |
| `parse` | An alternate matched (or `no-alt`): alt index, token sequence, push/replace/back, groups. |
| `node` | A node-build step: the `why` code and the node so far. |
| `stack` | The current rule stack and the partial nodes. |
| `step` | A raw step dump. |

## No-side-effects guarantee

`describe()`, `model()` and `abnf()` are read-only snapshots of the
instance — they never alter the grammar or the parse. A malformed grammar
(e.g. a null entry in an alternate's token sequence) is rendered
defensively (`***INVALID***`) rather than throwing.

## Dependency note

`@tabnas/debug` is a development/test aid, not a runtime dependency. It is
never required to *run* a grammar — only to inspect or author one. The
ABNF emitter deliberately does not depend on `@tabnas/abnf`; it reads only
the live engine.
