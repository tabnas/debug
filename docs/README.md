# @tabnas/debug documentation

Debug plugin for the [`tabnas`](https://github.com/rjrodger/tabnas)
parser. It adds a grammar `describe()` method and optional parse tracing
to a parser instance.

These docs apply to both implementations — the canonical TypeScript
package (`ts/`) and the Go port (`go/`). The two engines expose tracing
and introspection differently, so the examples show each language's real
API; the [reference](reference.md) lists where they diverge.

Start where your goal fits:

- **[Tutorial](tutorial.md)** — new to the plugin? Follow this end to end
  to load it, describe a grammar, and read a trace.
- **How-to guides** — already set up and trying to get one thing done:
  - [Trace a parse](how-to/trace-a-parse.md)
  - [Choose which events to trace](how-to/select-trace-kinds.md)
  - [Describe a grammar without tracing](how-to/describe-a-grammar.md)
  - [Silence the per-`use` grammar dump](how-to/silence-use-output.md)
- **[Reference](reference.md)** — exact options, defaults, methods, trace
  kinds, and output sections.
- **[Explanation](explanation.md)** — how the plugin hooks into the
  parser and why it is built this way.
