# How to trace a parse

Goal: see what the parser does, event by event, while it parses a
specific input.

## Steps

1. Create a parser instance dedicated to the traced run. Tracing is
   wired up when the plugin is loaded, so use a fresh instance rather
   than toggling it on an existing one.

2. Load the plugin with tracing enabled and printing off (printing is a
   separate, noisier feature — see
   [Silence the per-`use` grammar dump](silence-use-output.md)).

   TypeScript:

   ```js
   const { Tabnas } = require('tabnas')
   const { Debug } = require('@tabnas/debug')

   const am = new Tabnas()
   am.use(Debug, { print: false, trace: true })
   ```

   Go:

   ```go
   am := tabnas.New()
   am.Use(debug.Debug, &debug.Options{Print: false, Trace: debug.Defaults.Trace})
   ```

3. Parse your input. The trace prints to the console as the parse runs.

   ```js
   am('{ "a": 1 }')
   ```

   ```go
   am.Parse(`{ "a": 1 }`)
   ```

4. Read the output under the `========= TRACE ==========` banner. Each
   line is one event; the leading tag (`lex`, `rule`, `parse`, `node`,
   `stack`) tells you what kind. See the
   [Reference](../reference.md#trace-kinds) for the fields on each line.

## Notes

- Trace output goes to the parser's configured console. To capture it,
  redirect or override that console rather than reading a return value —
  tracing has no return value.
- If you see no trace output, confirm `trace` is enabled and at least one
  trace kind is on; see [Choose which events to trace](select-trace-kinds.md).
