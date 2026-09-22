# Tutorial: your first trace

This walkthrough takes you from nothing to a working parser that prints
its own grammar and traces a parse. By the end you will have loaded the
plugin, read a grammar description, and watched the parser work.

You will need the [`tabnas`](https://github.com/tabnas/parser) parser
engine alongside the debug plugin. Pick the track for your language.

## 1. Set up a project

### TypeScript / JavaScript

The plugin resolves the engine from a sibling `../parser` checkout. Build
that first, then build this package:

```bash
(cd ../parser/ts && npm install && npm run build)
cd ts && npm install && npm run build
```

Create `demo.js`:

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas()
tn.use(Debug, { print: false, trace: false })
```

### Go

```bash
go get github.com/tabnas/parser/go
go get github.com/tabnas/debug/go
```

Create `main.go`:

```go
package main

import (
	"fmt"

	tabnas "github.com/tabnas/parser/go"
	debug "github.com/tabnas/debug/go"
)

func main() {
	j := tabnas.Make()
	fmt.Println("ready")
	_ = j
	_ = debug.Debug
}
```

### Rust

The engine crate is unpublished, so it is consumed as a sibling
checkout. Clone `https://github.com/tabnas/parser` next to this
repository and declare both by path:

```toml
[dependencies]
tabnas = { path = "../parser/rs" }
tabnas-debug = { path = "../debug/rs" }
```

Create `src/main.rs`:

```rust
use tabnas::Tabnas;
use tabnas_debug::{apply, describe, DebugOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = Tabnas::new();
    apply(&mut parser, DebugOptions::quiet())?;
    println!("ready");
    let _ = describe(&parser);
    Ok(())
}
```

At this point the plugin is available but quiet.

## 2. Describe the grammar

Ask the plugin what the parser knows.

TypeScript — the plugin attached a `describe` method to the instance:

```js
console.log(tn.debug.describe())
```

Go — `Describe` is a package function you pass the instance to. It
returns `(string, error)`; the error is `nil` for a well-formed instance:

```go
report, err := debug.Describe(j)
if err != nil {
	panic(err)
}
fmt.Println(report)
```

Rust — `describe` is a free function too, and an infallible one, because
the Rust engine's accessors cannot fail:

```rust
println!("{}", tabnas_debug::describe(&parser));
```

Run it. You will see a report divided into eight labelled sections:
`INSTANCE`, `TOKENS`, `RULES`, `ALTS`, `LEXER`, `CONFIG`, `PLUGIN` and
`ABNF`. Each lists part of the parser's
active configuration. The engine ships no grammar of its own, so a bare
instance shows little; add tokens and rules (or load a grammar plugin)
and they appear here. Skim it — the point is that the grammar is visible.

## 3. Turn on tracing

Tracing logs what the parser does as it parses.

TypeScript:

```js
const traced = new Tabnas()
traced.use(Debug, { print: false, trace: true })
traced('a:1')
```

Go:

```go
j.Use(debug.Debug, map[string]any{"trace": true})
j.Parse("a:1")
```

Rust:

```rust
let mut traced = Tabnas::new();
apply(&mut traced, DebugOptions::default().with_print(false))?;
traced.parse("a:1")?;
```

Run it. You will see one line per parse event, tagged by kind:
`step`, `stack`, `rule` (each rule opening and closing), `lex`
(each token produced), `parse` (the alternate match result) and `node`
(the node built so far). Each line shows where in the source the parser
is and what it decided. Rust emits five of the six: the Rust engine has
no per-step event, so `step` is accepted as an option name and logs
nothing. Rust trace lines go to the engine's own debug sink, stderr by
default, rather than to a console the plugin owns.

## 4. Read one trace line

Find a `rule` line. It shows the rule name with its instance number,
whether it is opening (`o`) or closing (`c`), the parse depth, and the
node built so far. Follow the lines top to bottom and you can watch the
parser descend into the input and come back out.

## What you have learned

You loaded the plugin, printed a grammar, enabled tracing, and read the
parser's per-event log. From here:

- [Trace a parse](how-to/trace-a-parse.md) in your own project.
- [Choose which events to trace](how-to/select-trace-kinds.md), in any
  of the three runtimes.
- The [Reference](reference.md) and [Explanation](explanation.md) cover
  what the output means and how the plugin works.
