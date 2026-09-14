# tabnas-debug (Rust)

Debug / introspection plugin for the
[`tabnas`](https://github.com/tabnas/parser) parser engine, crate
`tabnas-debug` (library `tabnas_debug`).

It makes a grammar *visible*: `describe` dumps an instance's installed
grammar (tokens, rules, plugins) as labelled text, `model` returns the
same information as structured, serialisable data, `abnf` re-expresses it
as ABNF, and tracing logs a parse event by event. A dev/test aid, never
a runtime dependency.

This is the Rust port of the canonical TypeScript implementation in
[`../ts`](../ts); the TypeScript version is authoritative and this crate
tracks it. The Rust engine exposes tracing and introspection through
different idioms, so the surface differs in shape: free functions
instead of instance methods, typed options instead of an option map, and
a `use_plugin` wrapper for the `print` option. See
[the concepts doc](doc/concepts.md) and [reference](doc/reference.md) for
the details, and [`../docs/reference.md`](../docs/reference.md) for the
authoritative cross-runtime divergence register.

## Install

The `tabnas` crate is not published to a registry, so the engine is
consumed as a **sibling checkout**, the standard tabnas development
model. Clone `https://github.com/tabnas/parser` next to this repository
and point at it:

```toml
[dependencies]
tabnas = { path = "../parser/rs" }
tabnas-debug = { path = "../debug/rs" }
```

## Use

```rust
use tabnas::Tabnas;
use tabnas_debug::{abnf, apply, describe, model, DebugOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut parser = Tabnas::new();
    // ... install a grammar ...

    // Introspection needs no plugin at all: these are free functions.
    println!("{}", describe(&parser));
    println!("{}", abnf(&parser));
    println!("{}", model(&parser).config.start);

    // Tracing does need the plugin. Events go to the engine's debug
    // sink (stderr by default).
    apply(&mut parser, DebugOptions::default())?;
    parser.parse("a:1")?;
    Ok(())
}
```

`describe`, `model` and `abnf` read a live instance and have no side
effects, so they work on any `Tabnas` whether or not the plugin is
installed. Installing the plugin is what turns on tracing and the `USE:`
dump.

To capture output instead of writing it to stderr, set the engine's sink:

```rust
parser.options.debug.output = Some(std::sync::Arc::new(|line: &str| {
    // collect `line`
}));
```

## Documentation

- [Tutorial](doc/tutorial.md). Zero to a working inspection, step by step.
- [How-to guide](doc/guide.md). Focused recipes.
- [Reference](doc/reference.md). The exact exports, options and output.
- [Concepts](doc/concepts.md). How it works and how it differs from the
  TypeScript version.

## Build and test

The engine is a path dependency on the sibling checkout, so there is
nothing to fetch and nothing to build first:

```bash
cargo build --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Or, from the repository root, `make test-rs` runs the tests and clippy.
There is only one resolution here, always the sibling engine, unlike
the Go module's pinned/workspace pair.

The suite runs the shared `../test/spec/*.tsv` conformance fixtures (the
same files the TypeScript and Go suites run) over the same named
grammars. A row green in one runtime and red in another is a failure, not
a discrepancy.

## License

MIT.
