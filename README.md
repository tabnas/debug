# @tabnas/debug

<!-- tabnas-badges -->
[![npm](https://tabnas.github.io/status/badges/debug-npm.svg)](https://www.npmjs.com/package/@tabnas/debug)
[![CI](https://github.com/tabnas/debug/actions/workflows/ci.yml/badge.svg)](https://github.com/tabnas/debug/actions/workflows/ci.yml)
[![go](https://tabnas.github.io/status/badges/debug-go.svg)](https://pkg.go.dev/github.com/tabnas/debug/go)
[![tabnas standard](https://tabnas.github.io/status/badges/debug-standard.svg)](https://tabnas.github.io/status/)
<!-- /tabnas-badges -->

Debug / introspection plugin for the
[tabnas](https://github.com/tabnas/parser) parser. It makes a grammar
*visible*: `model()` returns a structured description of an engine's
installed grammar (rules, tokens, plugins), `describe()` renders it as
text, `abnf()` re-expresses it as ABNF, and `trace` logs a parse step by
step. A dev/test aid for authoring and inspecting grammars — **never a
runtime dependency**.

Docs, guides, the error reference and the playground: **[tabnas.dev](https://tabnas.dev)**.

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ tag: 'demo' })
tn.use(Debug, { print: false, trace: false })

tn.debug.model().tag        // => 'demo'
typeof tn.debug.describe()  // => 'string'
```

## Three implementations

| Path | Description |
|---|---|
| [`ts/`](ts/) | TypeScript / JavaScript (`@tabnas/debug`). **Canonical.** |
| [`go/`](go/) | Go (`github.com/tabnas/debug/go`, package `tabnasdebug`). Tracks `ts/`. |
| [`rs/`](rs/) | Rust (the `tabnas-debug` crate, library `tabnas_debug`). Tracks `ts/`. |

The TypeScript implementation is the source of truth; the Go and Rust
ports mirror its behaviour — including the structured model, the granular
trace kinds, and the `print` option (as `tabnasdebug.Use` /
`tabnas_debug::use_plugin`) — as far as each engine API allows. The
remaining shape differences are documented in
[`docs/reference.md`](docs/reference.md), the authoritative divergence
register.

## Documentation

Four-quadrant [Diátaxis](https://diataxis.fr) docs, per language:

| | Tutorial | How-to | Reference | Concepts |
|---|---|---|---|---|
| **TypeScript** | [tutorial](ts/doc/tutorial.md) | [guide](ts/doc/guide.md) | [reference](ts/doc/reference.md) | [concepts](ts/doc/concepts.md) |
| **Go** | [tutorial](go/doc/tutorial.md) | [guide](go/doc/guide.md) | [reference](go/doc/reference.md) | [concepts](go/doc/concepts.md) |
| **Rust** | [tutorial](rs/doc/tutorial.md) | [guide](rs/doc/guide.md) | [reference](rs/doc/reference.md) | [concepts](rs/doc/concepts.md) |

Per-language quick starts: [`ts/README.md`](ts/README.md),
[`go/README.md`](go/README.md), [`rs/README.md`](rs/README.md).

## Build and test

All three implementations consume the
[`tabnas`](https://github.com/tabnas/parser) parser engine. The Go module
resolves it at a pinned published version; the TypeScript package and the
Rust crate resolve it from a sibling `../parser` checkout, so clone that
first — and build its TypeScript
(`cd parser/ts && npm install && npm run build`), which the Rust crate
does not need (cargo compiles the engine from source).

```bash
make build   # build all three implementations
make test    # build + test all three
```

Targeted: `make test-ts`, `make test-go`, `make test-rs`.

Contributors and AI agents: see [`AGENTS.md`](AGENTS.md).

## License

MIT. Copyright (c) Richard Rodger.
