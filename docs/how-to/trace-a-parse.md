# How to trace a parse

Goal: see what the parser does, event by event, while it parses a
specific input.

## TypeScript

1. Use a fresh instance for the traced run — tracing is wired up when the
   plugin loads.

2. Load the plugin with tracing on and printing off:

   ```js
   const { Tabnas } = require('@tabnas/parser')
   const { Debug } = require('@tabnas/debug')

   const tn = new Tabnas()
   tn.use(Debug, { print: false, trace: true })
   ```

3. Parse your input; the trace prints as it runs:

   ```js
   tn('{ "a": 1 }')
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

2. Trace lines go to stdout (pass an `io.Writer` under `"out"` to
   capture them). You get the same kinds as TypeScript — `step`, `stack`,
   `rule`, `lex`, `parse`, `node` — with matching line shapes; see the
   [trace output reference](../reference.md#trace-output) for the small
   remaining differences (no alt index on `parse` lines, no matcher name
   on `lex` lines).

## Notes

- TypeScript trace output goes to the parser's configured console; to
  capture it, override that console. Go trace output goes to stdout by
  default, or to the `io.Writer` passed as `opts["out"]`.
- If you see no output, confirm tracing is enabled (and, in TypeScript,
  that at least one kind is on — see
  [Choose which events to trace](select-trace-kinds.md)).
