# Tutorial: inspect your first grammar

This walkthrough takes you from nothing to a working result: you will
load the debug plugin onto a parser instance, read a structured
description of its grammar, render it as ABNF, and trace a parse. One
happy path, step by step.

You need the [`tabnas`](https://github.com/tabnas/parser) parser engine
and the `@tabnas/debug` plugin. In this repository they are consumed from
source; from a checkout, fetch the engine and install the TypeScript
package:

```bash
./scripts/fetch-parser.sh
cd ts && npm install
```

## 1. Load the plugin

The debug plugin attaches a `debug` object to a `Tabnas` instance. Pass
`{ print: false, trace: false }` for now so it stays quiet — you will
call its methods explicitly.

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ tag: 'demo' })
tn.use(Debug, { print: false, trace: false })

typeof tn.debug.describe        // => 'function'
typeof tn.debug.model           // => 'function'
```

A bare instance carries no grammar of its own — the engine ships none —
so there is nothing interesting to inspect yet. Load a grammar so the
description has content.

## 2. Load a grammar to inspect

Any grammar plugin works. This tutorial uses the engine's bundled `json`
grammar (it lives in the engine's test build), but you could equally use
your own rules. With a grammar loaded, `model()` returns the whole
grammar as data:

```js ignore
const path = require('path')
const PARSER_MAIN = require.resolve('@tabnas/parser')
const { json } = require(
  path.resolve(path.dirname(PARSER_MAIN), '..', 'dist-test', 'json-plugin.js'),
)

const tn = new Tabnas({ tag: 'demo' })
tn.use(json)
tn.use(Debug, { print: false, trace: false })

const m = tn.debug.model()
m.config.start                       // 'val'
m.plugins.map((p) => p.name)         // ['json', 'Debug']
m.rules.map((r) => r.name).sort()    // ['elem', 'list', 'map', 'pair', 'val']
```

The rest of this tutorial builds a tiny grammar by hand so every example
is self-contained and runnable.

## 3. Build a small grammar by hand

Here is a recogniser for sums like `1+2+3`: a `val` rule that pushes into
an `add` rule, where `add` matches a number and then optionally a `+`
followed by another `add`.

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ fixed: { token: { '#PL': '+' } }, rule: { start: 'val' } })
tn.token('#ZZ'); tn.token('#NR'); tn.token('#PL')
tn.rule('val', (rs) => rs.open([{ p: 'add' }]))
tn.rule('add', (rs) => rs
  .open([{ s: ['#NR'] }])
  .close([{ s: ['#PL'], r: 'add' }, {}, { s: ['#ZZ'] }]))

tn.use(Debug, { print: false, trace: false })

tn.debug.model().config.start                  // => 'val'
tn.debug.model().rules.map((r) => r.name)      // => ['val', 'add']
```

## 4. Read the grammar as data with `model()`

`model()` gives you a typed, JSON-serialisable object. Look at one rule
and its alternates:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ fixed: { token: { '#PL': '+' } }, rule: { start: 'val' } })
tn.token('#ZZ'); tn.token('#NR'); tn.token('#PL')
tn.rule('val', (rs) => rs.open([{ p: 'add' }]))
tn.rule('add', (rs) => rs
  .open([{ s: ['#NR'] }])
  .close([{ s: ['#PL'], r: 'add' }, {}, { s: ['#ZZ'] }]))
tn.use(Debug, { print: false, trace: false })

const m = tn.debug.model()

// `val` has one open alternate that pushes into `add`.
m.rules.find((r) => r.name === 'val').open[0].push   // => 'add'

// The push/replace edges of every rule live in the graph.
m.graph.find((g) => g.name === 'val').openPush        // => ['add']
m.graph.find((g) => g.name === 'add').closeReplace     // => ['add']
```

Every field of the model is described in the [reference](reference.md).

## 5. Render the grammar as ABNF

`abnf()` re-expresses the live grammar as [ABNF](https://www.rfc-editor.org/rfc/rfc5234)
text — productions, then a legend defining each token:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ fixed: { token: { '#PL': '+' } }, rule: { start: 'val' } })
tn.token('#ZZ'); tn.token('#NR'); tn.token('#PL')
tn.rule('val', (rs) => rs.open([{ p: 'add' }]))
tn.rule('add', (rs) => rs
  .open([{ s: ['#NR'] }])
  .close([{ s: ['#PL'], r: 'add' }, {}, { s: ['#ZZ'] }]))
tn.use(Debug, { print: false, trace: false })

tn.debug.abnf()
// => 'val = add\nadd = NR [ PL add ]\n\nNR = <number>\nPL = "+"'
```

The optional `+ add` continuation became `[ PL add ]`, and `PL` is
defined as the literal `"+"`. The number token has no ABNF literal, so it
is shown as the prose value `<number>`.

## 6. Trace a parse

Turn `trace: true` on and the plugin logs one line per parse event.
Capture the log by giving the instance a console:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const lines = []
const fakeConsole = { log: (...a) => lines.push(a.join(' ')), dir() {}, error() {} }

const tn = new Tabnas({ debug: { get_console: () => fakeConsole } })
  .make({ fixed: { token: { Ta: 'a' } }, rule: { start: 'top' } })
const rules = tn.rule()
Object.keys(rules).forEach((rn) => tn.rule(rn, null))
tn.rule('top', (rs) => rs.open([{ s: ['Ta'] }]).close([{ s: '#ZZ' }]))
tn.use(Debug, { print: false, trace: true })

tn.parse('a')

lines.some((l) => l.includes('========= TRACE'))   // => true
lines.some((l) => l.includes('  rule'))             // => true
```

Each line is tagged `rule`, `lex`, `parse`, `node` or `stack`, shows the
upcoming source and the token window, and the parse depth. Read them top
to bottom to watch the parser descend into the input and come back out.

## What you have learned

You loaded the plugin, read a grammar as structured data (`model()`),
rendered it as ABNF (`abnf()`), and traced a parse (`trace: true`). From
here:

- The [how-to guide](guide.md) has focused recipes (diff two grammars,
  capture trace output, round-trip ABNF).
- The [reference](reference.md) is the exact API surface.
- [Concepts](concepts.md) explains what the model captures and why.
