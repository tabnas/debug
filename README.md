# @tabnas/debug

Debug plugin for the [tabnas](https://github.com/rjrodger/tabnas)
parser — tracing hooks and a `describe()` method for `Tabnas` instances.

This repository contains two implementations of the same plugin:

| Path | Description |
|---|---|
| [`ts/`](ts/) | TypeScript / JavaScript implementation (`@tabnas/debug`). **Canonical.** |
| [`go/`](go/) | Go implementation (`github.com/rjrodger/tabnas-debug/go`). Kept at parity with `ts/`. |

The TypeScript implementation is the source of truth; the Go port mirrors
its behaviour, options, defaults and output format.

## Documentation

See [`docs/`](docs/) — a [tutorial](docs/tutorial.md), [how-to
guides](docs/README.md), a [reference](docs/reference.md), and an
[explanation](docs/explanation.md) of how the plugin works. Per-language
usage lives in [`ts/README.md`](ts/README.md) and
[`go/README.md`](go/README.md).

## Build and test

The `tabnas` parser dependency is pinned to its GitHub `main` branch and
is required to build or test either implementation.

```bash
make build   # build both implementations
make test    # test both implementations
```

Contributors and AI agents: see [`AGENTS.md`](AGENTS.md) for repository
conventions and the parity rules.

## License

MIT. Copyright (c) Richard Rodger.
