# Tutorial: your first trace

This walkthrough takes you from nothing to a working parser that prints
its own grammar and traces a parse. By the end you will have loaded the
plugin, read a grammar description, and watched the parser work step by
step.

You will need the `tabnas` parser installed alongside the debug plugin.
Pick the track for your language and follow it top to bottom.

## 1. Set up a project

### TypeScript / JavaScript

```bash
mkdir trace-demo && cd trace-demo
npm init -y
npm install tabnas @tabnas/debug
```

Create `demo.js`:

```js
const { Tabnas } = require('tabnas')
const { Debug } = require('@tabnas/debug')

const am = new Tabnas()
am.use(Debug, { print: false, trace: false })
```

### Go

```bash
mkdir trace-demo && cd trace-demo
go mod init trace-demo
go get github.com/rjrodger/tabnas/go
go get github.com/rjrodger/tabnas-debug/go
```

Create `main.go`:

```go
package main

import (
	tabnas "github.com/rjrodger/tabnas/go"
	debug "github.com/rjrodger/tabnas-debug/go"
)

func main() {
	am := tabnas.New()
	am.Use(debug.Debug, &debug.Options{Print: false, Trace: nil})
	_ = am
}
```

At this point the plugin is loaded but quiet: printing is off and
tracing is off.

## 2. Describe the grammar

The plugin attached a `describe` method to the instance. Ask it what the
parser knows.

TypeScript:

```js
console.log(am.debug.describe())
```

Go:

```go
fmt.Println(am.Debug.Describe())
```

Run it. You will see a report divided into labelled sections — `TOKENS`,
`RULES`, `ALTS`, `LEXER`, `PLUGIN` and more. Each section lists part of
the parser's active configuration. You did not write any grammar yet, so
this is the parser's built-in default. Skim it; you do not need to
understand every line. The point is that the grammar is now visible.

## 3. Turn on tracing

Tracing logs what the parser does as it parses. Load the plugin again,
this time with tracing on, then parse a small input.

TypeScript:

```js
const traced = new Tabnas()
traced.use(Debug, { print: false, trace: true })
traced('a:1')
```

Go:

```go
traced := tabnas.New()
traced.Use(debug.Debug, &debug.Options{Print: false, Trace: debug.Defaults.Trace})
traced.Parse("a:1")
```

Run it. Below a `========= TRACE ==========` banner you will see one line
per parse event: lexer matches (`lex`), rule open/close (`rule`),
alternate selection (`parse`), node construction (`node`), and the rule
stack (`stack`). Each line shows where in the source the parser is and
what it decided.

## 4. Read one trace line

Find a `rule` line. Reading left to right it shows the upcoming source
text, the current token window, the parse depth, the rule name with its
instance number and whether it is opening or closing, and its links to
neighbouring rules. The [reference](reference.md) lists every field; for
now, notice that you can follow the parser descending into the input and
coming back out.

## What you have learned

You loaded the plugin, printed a grammar with `describe()`, enabled
tracing, and read the parser's per-event log. From here:

- To trace a real parse in your own project, see
  [Trace a parse](how-to/trace-a-parse.md).
- To narrow the noise to the events you care about, see
  [Choose which events to trace](how-to/select-trace-kinds.md).
- To understand what the trace and `describe()` output mean, see the
  [Reference](reference.md) and [Explanation](explanation.md).
