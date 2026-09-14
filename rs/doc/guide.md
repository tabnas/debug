# How-to guide (Rust)

Focused recipes. Each assumes you have read the [tutorial](tutorial.md)
and have a `Tabnas` instance with a grammar installed. This is the Rust
port of the [TypeScript how-to guide](../../ts/doc/guide.md).

```rust
use tabnas::Tabnas;
use tabnas_debug::{abnf, apply, describe, model, use_plugin, DebugOptions, TraceKinds};
```

## Dump a grammar description

`describe` needs no plugin: it reads the instance:

```rust
println!("{}", describe(&parser));
```

## Assert on a section

The eight banners are exported as `SECTIONS`, so a test can check they
are all present and in order without hard-coding the strings:

```rust
use tabnas_debug::SECTIONS;

let text = describe(&parser);
let mut cursor = 0;
for section in SECTIONS {
    let found = text[cursor..]
        .find(section)
        .unwrap_or_else(|| panic!("{section} missing or out of order"));
    cursor += found + section.len();
}
```

## Consume the grammar as data

```rust
let built = model(&parser);

// The start rule lives under `config`, not at the top level.
assert_eq!(built.config.start, "val");

// Which rules push into which.
let edges = built.graph.iter().find(|rule| "val" == rule.name).unwrap();
assert_eq!(edges.open_push, ["add"]);

// Loaded plugins, by name.
assert!(built.plugins.iter().any(|plugin| "Debug" == plugin.name));
```

## Serialise the model

Every `Debug*` type derives `serde::Serialize`, with names matching the
TypeScript fields and the Go JSON tags, so a model serialised here is
comparable with one from another runtime:

```rust
let json = serde_json::to_value(model(&parser))?;
assert!(json["config"]["safeKey"].is_boolean());
assert!(json["tokenSets"].is_array());
# Ok::<(), serde_json::Error>(())
```

Absent optional fields are skipped rather than written as `null`, so an alt
with no `push` has no `push` key at all.

## Render a grammar as ABNF

```rust
println!("{}", abnf(&parser));
```

The emitter reads only the live engine. Rules become productions, open
alternates become `/`-separated alternatives, and each token becomes a
named terminal defined in a legend after the productions.

## Log a description on later plugin loads (the print option)

`Tabnas::use_plugin` is a concrete method, not a field you can reassign, so
the TypeScript `use()` wrapping is a free function here:

```rust
apply(&mut parser, DebugOptions::new())?;        // print: true by default
use_plugin(&mut parser, some_plugin(), None)?;   // logs "USE: <name>" + describe
# Ok::<(), tabnas::PluginError>(())
```

A plugin installed directly through `parser.use_plugin(...)` does **not**
trigger the dump. With `print: false` (or no debug plugin installed at
all) `use_plugin` still installs the plugin, just silently.

## Trace a parse

```rust
apply(&mut parser, DebugOptions::default())?;
parser.parse("1+2")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Output goes to the engine's debug sink, stderr by default.

## Capture trace output (for example, in a test)

There is no `out` option: the sink belongs to the engine, and both trace
lines and the `USE:` dump go through it.

```rust
use std::sync::{Arc, Mutex};

let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
let sink = lines.clone();
parser.options.debug.output = Some(Arc::new(move |line: &str| {
    sink.lock().unwrap().push(line.to_string());
}));

apply(&mut parser, DebugOptions::new().with_print(false))?;
parser.parse("1+2")?;

assert!(lines.lock().unwrap().iter().any(|line| line.starts_with("lex ")));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Select individual trace kinds

```rust
apply(
    &mut parser,
    DebugOptions::new().with_trace(TraceKinds {
        lex: true,
        rule: true,
        ..TraceKinds::none()
    }),
)?;
# Ok::<(), tabnas::PluginError>(())
```

`TraceKinds::none()` starts from everything off, `TraceKinds::all()` from
everything on, so a partial selection is explicit either way, rather
than depending on a merge rule.

Note that `step` has no Rust engine hook and logs nothing; selecting it
alone installs no tracing at all.

## Disable tracing

```rust
apply(&mut parser, DebugOptions::new().without_trace())?;
// or, for introspection only — no tracing AND no USE: dumps:
apply(&mut parser, DebugOptions::quiet())?;
# Ok::<(), tabnas::PluginError>(())
```

`DebugOptions::quiet()` is what a test suite wants: it installs the
plugin (so `model().plugins` reports it) without writing anything.

## Skip the plugin entirely

If you only want introspection, do not install anything:
`describe(&parser)`, `model(&parser)` and `abnf(&parser)` work on a bare
instance. That is why the shared fixture grammars do not load the plugin.
