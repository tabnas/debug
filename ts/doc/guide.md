# How-to guide

Focused recipes for real tasks. Each is self-contained. For the full API,
see the [reference](reference.md); for the why, see [concepts](concepts.md).

All recipes assume:

```js ignore
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')
```

## Attach the plugin without side effects

By default the plugin prints a description on every `use()` and turns
tracing on. To attach it quietly and call its methods yourself, disable
both:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas()
tn.use(Debug, { print: false, trace: false })

typeof tn.debug.describe   // => 'function'
typeof tn.debug.model      // => 'function'
typeof tn.debug.abnf       // => 'function'
```

## Print a human-readable grammar dump

`describe()` returns a single string with labelled sections (`INSTANCE`,
`TOKENS`, `RULES`, `ALTS`, `LEXER`, `CONFIG`, `PLUGIN`, `ABNF`). Print it
to read a grammar at a glance:

```js ignore
console.log(tn.debug.describe())
```

To assert on its content, check for a section header or a known row:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ tag: 'demo' })
tn.use(Debug, { print: false, trace: false })

const out = tn.debug.describe()
out.includes('========= INSTANCE ========')   // => true
out.includes('tag: demo')                       // => true
out.includes('  start: ')                       // => true
```

## Inspect a grammar as data

When you want to *assert* on a grammar (in a test) or feed it to another
tool, use `model()` instead of parsing the `describe()` text. It returns a
typed object.

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

// Which rules exist?
m.rules.map((r) => r.name)                       // => ['val', 'add']

// What does `val`'s first open alternate do?
m.rules.find((r) => r.name === 'val').open[0].push   // => 'add'

// The model agrees with the engine's own rule map.
m.rules.map((r) => r.name).sort().join(',') === Object.keys(tn.rule()).sort().join(',')   // => true
```

## Find the push/replace edges of a rule

The `graph` field is the rule-reference graph: for each rule, the
distinct rule targets reached by open-push, open-replace, close-push and
close-replace alternates. Use it to see how rules connect.

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
m.graph.find((g) => g.name === 'val').openPush        // => ['add']
m.graph.find((g) => g.name === 'add').closeReplace     // => ['add']
m.graph.find((g) => g.name === 'add').openPush         // => []
```

## Diff two grammars

Because `model()` is JSON-serialisable, you can snapshot a grammar and
compare it to another — a quick way to see what a refactor or a plugin
change did. Compare the grammar-structure fields (skip `lexer`, whose
`matcher`/`make` carry runtime names):

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

function grammarOf(build) {
  const tn = new Tabnas({ rule: { start: 'val' } })
  tn.token('#ZZ'); tn.token('#NR'); tn.token('#PL')
  build(tn)
  tn.use(Debug, { print: false, trace: false })
  const m = tn.debug.model()
  return JSON.parse(JSON.stringify({ rules: m.rules, graph: m.graph, abnf: m.abnf }))
}

const a = grammarOf((tn) => {
  tn.rule('val', (rs) => rs.open([{ p: 'add' }]))
  tn.rule('add', (rs) => rs.open([{ s: ['#NR'] }]).close([{ s: ['#ZZ'] }]))
})
const b = grammarOf((tn) => {
  tn.rule('val', (rs) => rs.open([{ p: 'add' }]))
  tn.rule('add', (rs) => rs.open([{ s: ['#NR'] }]).close([{ s: ['#ZZ'] }]))
})

JSON.stringify(a) === JSON.stringify(b)   // => true
```

## Render a grammar as ABNF

`abnf()` emits the live grammar as ABNF text. It reads only the running
engine — it never imports an ABNF library — so it works on any grammar,
hand-written or plugin-supplied.

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

Constructs that ABNF cannot express (an arbitrary match regex) are
emitted as ABNF comments (`; /.../`) so the output stays valid text; such
a grammar will not round-trip. See [concepts](concepts.md) for the
round-trip contract.

## Capture trace output (instead of printing to the console)

Tracing logs to the instance's console provider. Supply your own
`get_console()` to capture the lines — useful in tests, or to write
trace output somewhere other than `console`:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const lines = []
const fakeConsole = { log: (...a) => lines.push(a.join(' ')), dir() {}, error() {} }

const tn = new Tabnas({ debug: { get_console: () => fakeConsole } })
  .make({ fixed: { token: { Ta: 'a' } }, rule: { start: 'top' } })
Object.keys(tn.rule()).forEach((rn) => tn.rule(rn, null))
tn.rule('top', (rs) => rs.open([{ s: ['Ta'] }]).close([{ s: '#ZZ' }]))
tn.use(Debug, { print: false, trace: true })

tn.parse('a')

lines.some((l) => l.includes('========= TRACE'))   // => true
lines.filter((l) => l.includes('  parse')).length > 0   // => true
```

## Trace only some event kinds

`trace` may be an object of `kind -> boolean`. The kinds are `step`,
`rule`, `lex`, `parse`, `node`, `stack`. Because the engine deep-merges
`Debug.defaults` (all kinds `true`) with your options, set the kinds you
want off explicitly:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const lines = []
const fakeConsole = { log: (...a) => lines.push(a.join(' ')), dir() {}, error() {} }

const tn = new Tabnas({ debug: { get_console: () => fakeConsole } })
  .make({ fixed: { token: { Ta: 'a' } }, rule: { start: 'top' } })
Object.keys(tn.rule()).forEach((rn) => tn.rule(rn, null))
tn.rule('top', (rs) => rs.open([{ s: ['Ta'] }]).close([{ s: '#ZZ' }]))

tn.use(Debug, {
  print: false,
  trace: { rule: true, lex: false, parse: false, node: false, stack: false, step: false },
})
tn.parse('a')

lines.filter((l) => l.includes('  rule')).length > 0    // => true
lines.filter((l) => l.includes('  lex')).length          // => 0
```

## Print a description automatically after each `use()`

Leave `print: true` (the default) and the plugin logs `USE:` plus a full
`describe()` dump every time a *later* plugin is applied — handy while
authoring a stack of grammar plugins. (The first `use()` is the one that
installs the print wrapper, so the dump appears from the second `use()`
onward.)

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const lines = []
const fakeConsole = { log: (...a) => lines.push(a.join(' ')), dir() {}, error() {} }

const tn = new Tabnas({ debug: { get_console: () => fakeConsole } })
tn.use(Debug, { print: true, trace: false })
tn.use(function myplugin(_tn, _opts) {}, {})

const useLog = lines.find((l) => l.startsWith('USE:'))
useLog.includes('myplugin')                          // => true
useLog.includes('========= INSTANCE ========')        // => true
```
