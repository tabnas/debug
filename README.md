# @tabnas/debug

Debug / introspection plugin for the
[tabnas](https://github.com/tabnas/parser) parser. It makes a grammar
*visible*: `model()` returns a structured description of an engine's
installed grammar (rules, tokens, plugins), `describe()` renders it as
text, `abnf()` re-expresses it as ABNF, and `trace` logs a parse step by
step. A dev/test aid for authoring and inspecting grammars — **never a
runtime dependency**.

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ tag: 'demo' })
tn.use(Debug, { print: false, trace: false })

tn.debug.model().tag        // => 'demo'
typeof tn.debug.describe()  // => 'string'
```

## Two implementations

| Path | Description |
|---|---|
| [`ts/`](ts/) | TypeScript / JavaScript (`@tabnas/debug`). **Canonical.** |
| [`go/`](go/) | Go (`github.com/tabnas/debug/go`, package `tabnasdebug`). Tracks `ts/`. |

The TypeScript implementation is the source of truth; the Go port mirrors
its behaviour — including the structured `Model`, the granular trace
kinds, and the `print` option (as `tabnasdebug.Use`) — as far as the Go
engine API allows. The remaining shape differences are documented in
[`docs/reference.md`](docs/reference.md).

## Documentation

Four-quadrant [Diátaxis](https://diataxis.fr) docs, per language:

| | Tutorial | How-to | Reference | Concepts |
|---|---|---|---|---|
| **TypeScript** | [tutorial](ts/doc/tutorial.md) | [guide](ts/doc/guide.md) | [reference](ts/doc/reference.md) | [concepts](ts/doc/concepts.md) |
| **Go** | [tutorial](go/doc/tutorial.md) | [guide](go/doc/guide.md) | [reference](go/doc/reference.md) | [concepts](go/doc/concepts.md) |

Per-language quick starts: [`ts/README.md`](ts/README.md),
[`go/README.md`](go/README.md).

## Build and test

Both implementations consume the
[`tabnas`](https://github.com/tabnas/parser) parser engine, which is not
published to a registry: it is fetched from its GitHub `main` branch and
built into `vendor/` (git-ignored) by `scripts/fetch-parser.sh`. The
Makefile runs this for you:

```bash
make build   # fetch engine, build both implementations
make test    # fetch engine, build + test both
```

Contributors and AI agents: see [`AGENTS.md`](AGENTS.md).

## License

MIT. Copyright (c) Richard Rodger.
