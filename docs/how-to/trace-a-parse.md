# How to trace a parse

Goal: see what the parser does, event by event, while it parses a
specific input.

## TypeScript

1. Use a fresh instance for the traced run — tracing is wired up when the
   plugin loads.

2. Load the plugin with tracing on and printing off:

   ```js
   const { Tabnas } = require('tabnas')
   const { Debug } = require('@tabnas/debug')

   const am = new Tabnas()
   am.use(Debug, { print: false, trace: true })
   ```

3. Parse your input; the trace prints as it runs:

   ```js
   am('{ "a": 1 }')
   ```

4. Read the lines: the leading tag (`lex`, `rule`, `parse`, `node`,
   `stack`) tells you the event kind. See the
   [Reference](../reference.md#trace-output) for the fields.

## Go

1. Load the plugin with `"trace": true`, then parse:

   ```go
   j := tabnas.Make()
   j.Use(debug.Debug, map[string]any{"trace": true})
   j.Parse(`{ "a": 1 }`)
   ```

2. Trace lines go to stdout. You get `[lex]` lines (one per token) and
   `[rule]` lines (one per rule open/close). The Go engine exposes these
   two streams; the finer TypeScript kinds are not available — see the
   [documented differences](../reference.md#documented-differences-go-vs-canonical-typescript).

## Notes

- TypeScript trace output goes to the parser's configured console; to
  capture it, override that console. Go trace output goes to stdout.
- If you see no output, confirm tracing is enabled (and, in TypeScript,
  that at least one kind is on — see
  [Choose which events to trace](select-trace-kinds.md)).
