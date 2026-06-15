# @tabnas/debug

Debug plugin for the [tabnas](https://github.com/tabnas/parser)
parser — tracing hooks and a `describe()` method for `Tabnas` instances.

This repository contains two implementations of the same plugin:

| Path | Description |
|---|---|
| [`ts/`](ts/) | TypeScript / JavaScript implementation (`@tabnas/debug`). **Canonical.** |
| [`go/`](go/) | Go implementation (`github.com/tabnas/debug/go`). Kept at parity with `ts/`. |

The TypeScript implementation is the source of truth; the Go port mirrors
its behaviour, options, defaults and output format.

## Documentation

See [`docs/`](docs/) — a [tutorial](docs/tutorial.md), [how-to
guides](docs/README.md), a [reference](docs/reference.md), and an
[explanation](docs/explanation.md) of how the plugin works. Per-language
usage lives in [`ts/README.md`](ts/README.md) and
[`go/README.md`](go/README.md).

## Build and test

Both implementations consume the [`tabnas`](https://github.com/tabnas/parser)
parser engine. It is not published to a registry, so it is fetched from
its GitHub `main` branch and built into `vendor/` (git-ignored) by
`scripts/fetch-parser.sh`. The Makefile runs this for you:

```bash
make build   # fetch engine, build both implementations
make test    # fetch engine, build + test both
```

Contributors and AI agents: see [`AGENTS.md`](AGENTS.md) for repository
conventions and the parity rules.

## License

MIT. Copyright (c) Richard Rodger.
