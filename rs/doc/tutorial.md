# Tutorial: inspect your first grammar (Rust)

This tutorial takes you from nothing to a working inspection of a live
parser: the text dump, the structured model, the ABNF rendering, and a
traced parse. It is the Rust port of the
[TypeScript tutorial](../../ts/doc/tutorial.md); the TypeScript
implementation is canonical and this crate (`tabnas_debug`) tracks it.

The debug plugin reports on a *running* engine, so you need a grammar to
look at. Step 2 builds a small one by hand, the same `add` grammar the
shared fixtures use.

## 1. Add the crates

The `tabnas` crate is not published to a registry, so clone
`https://github.com/tabnas/parser` next to this repository and point at
it:

```toml
[dependencies]
tabnas = { path = "../parser/rs" }
tabnas-debug = { path = "../debug/rs" }
```

Nothing needs building first: cargo compiles the engine from source.

## 2. Build a small grammar by hand

`val` pushes `add`; `add` matches a number, then optionally a `+` that
replaces back into `add`, with an epsilon close and an end-of-source
close.

```rust
use tabnas::{AltSpec, Tabnas};

fn add_grammar() -> Tabnas {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "val".into();

    let plus = parser.token_with_source("#PL", "+");
    let number = parser.options.token("#NR").expect("the #NR token");
    let end = parser.options.token("#ZZ").expect("the #ZZ token");

    parser.define_rule("val", |spec| {
        spec.clear();
        spec.add_open(AltSpec { p: Some("add".into()), ..Default::default() });
    });
    parser.define_rule("add", move |spec| {
        spec.clear();
        spec.add_open(AltSpec { s: vec![vec![number]], ..Default::default() });
        spec.add_close(AltSpec {
            s: vec![vec![plus]],
            r: Some("add".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
        spec.add_close(AltSpec { s: vec![vec![end]], ..Default::default() });
    });
    parser
}
```

## 3. Read the description

`describe` is a free function: it takes the instance, and needs no
plugin installed:

```rust
use tabnas_debug::describe;

let parser = add_grammar();
println!("{}", describe(&parser));
```

You get eight labelled sections, in this order:

```
========= INSTANCE ========
========= TOKENS ========
========= RULES =========
========= ALTS =========
========= LEXER =========
========= CONFIG ========
========= PLUGIN =========
========= ABNF =========
```

Those banners are the cross-runtime parity contract: every runtime emits
them byte-for-byte and in order, so two runtimes' dumps can be diffed.

## 4. Consume the grammar as data

`model` returns the same information as a typed value, which is what you
want in a test:

```rust
use tabnas_debug::model;

let built = model(&add_grammar());
assert_eq!(built.config.start, "val");

// The rule-reference graph: which rules push or replace into which.
let val = built.graph.iter().find(|edges| "val" == edges.name).unwrap();
assert_eq!(val.open_push, ["add"]);
```

Note `model.config.start`, **not** `model.start`. The start rule lives
under `config`, as it does in every runtime.

## 5. Render the grammar as ABNF

```rust
use tabnas_debug::abnf;

println!("{}", abnf(&add_grammar()));
```

```abnf
val = add
add = NR [ PL add ]

NR = <number>
PL = "+"
```

The emitter reads only the live engine: it never depends on an ABNF
compiler. Tokens become named terminals defined in a legend after the
productions; a built-in lexer token renders as `<number>` (a description,
not a re-compilable rule), while a fixed literal renders as `"+"` or, for
a literal containing letters, `%s"hi"`.

## 6. Trace a parse

Tracing is the one thing that DOES need the plugin, because it installs
subscribers on the instance:

```rust
use tabnas_debug::{apply, DebugOptions};

let mut parser = add_grammar();
apply(&mut parser, DebugOptions::default())?;
parser.parse("1+2")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Each parse opens with a `========= TRACE ==========` banner, then one
line per event. Output goes to the engine's debug sink, stderr by
default. To capture it instead:

```rust
use std::sync::{Arc, Mutex};

let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
let sink = lines.clone();
parser.options.debug.output = Some(Arc::new(move |line: &str| {
    sink.lock().unwrap().push(line.to_string());
}));
```

Select just the kinds you want:

```rust
use tabnas_debug::TraceKinds;

apply(
    &mut parser,
    DebugOptions::new()
        .with_print(false)
        .with_trace(TraceKinds { lex: true, ..TraceKinds::none() }),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

One kind is inert here: `step` has no Rust engine hook and logs nothing.
See [concepts](concepts.md#differences-from-the-ts-version).

## What you have learned

- `describe`, `model` and `abnf` are free functions over a live instance
  and need no plugin.
- The eight section banners are the parity contract.
- `model.config.start` is the start rule.
- Tracing needs the plugin, writes to the engine's debug sink, and is
  selectable per kind.

Next: the [how-to guide](guide.md) for focused recipes, the
[reference](reference.md) for the exact surface, and
[concepts](concepts.md) for how it works.
