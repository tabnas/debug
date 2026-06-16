# @tabnas/debug

Debug plugin for the [`tabnas`](https://github.com/tabnas/parser) parser.

Adds parse tracing, a printable `describe()`, and a **structured** `model()`
to a `Tabnas` instance — so you can inspect the grammar and instance as text
*or* as data.

## Install

```bash
npm install @tabnas/parser @tabnas/debug
```

## Use

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ tag: 'demo' })
tn.use(Debug, { print: false, trace: false })

// describe() is the printable form; model() is the structured form.
typeof tn.debug.describe()        // => 'string'

const m = tn.debug.model()
m.tag                             // => 'demo'
m.config.start                    // => 'val'
m.plugins.map((p) => p.name)      // => ['Debug']
Array.isArray(m.rules)            // => true
```

## Structured output — `model()`

`describe()` renders the instance as text; `model()` returns the same
information as a typed, JSON-serialisable object (`DebugModel`):

| field | what it holds |
|---|---|
| `tag` | the instance tag |
| `tokens` | `{ tin, name, fixed? }[]` — the token table |
| `tokenSets` | named token sets → member tins |
| `rules` | each rule's `open` / `close` alternates as structured `{ seq, push, replace, back, counters, groups, action, cond, modifier }` |
| `graph` | per-rule push/replace reference edges (`openPush`, `openReplace`, `closePush`, `closeReplace`) |
| `lexer` | the lexer matchers (`order`, `matcher`, `make`) |
| `config` | start rule, finish flag, safe-key, and the per-lexer enable flags |
| `plugins` | the applied plugins (name + options) |
| `abnf` | the live grammar rendered as ABNF text |

The grammar-structure fields round-trip through `JSON.stringify`. The
printable helpers — `describe()` (full dump), `abnf()` (ABNF text) — and
per-kind parse `trace` logging remain available.

## License

MIT.
