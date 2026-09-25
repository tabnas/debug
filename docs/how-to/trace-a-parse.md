# How to trace a parse

Goal: see what the parser does, event by event, while it parses a
specific input.

The traced instance needs a grammar. The engine ships none, so a parse
on a bare instance has no events to log. Each example below gives its
instance the grammar from the [tutorial](../tutorial.md): one token,
`#TA`, and one rule, `top`, that matches it. In your own project, define
your grammar or load a grammar plugin in its place, and parse your own
input.

## TypeScript

1. Use a fresh instance for the traced run: tracing is wired up when the
   plugin loads.

2. Give the instance its grammar, then load the plugin with tracing on
   and printing off:

   ```js
   const { Tabnas } = require('@tabnas/parser')
   const { Debug } = require('@tabnas/debug')

   const tn = new Tabnas({ fixed: { token: { '#TA': 'a' } }, rule: { start: 'top' } })
   tn.rule('top', (rs) => rs.open([{ s: ['#TA'] }]))
   tn.use(Debug, { print: false, trace: true })
   ```

3. Parse your input; the trace prints as it runs:

   ```js
   tn.parse('a')
   ```

4. Read the lines. After the `========= TRACE ==========` banner, the
   leading tag (`lex`, `rule`, `parse`, `node`, `stack`) names the event
   kind. A `step` line has no tag: it starts with the bare step number,
   such as `0:`. See the [Reference](../reference.md#trace-output) for
   the fields.

## Go

1. Give the instance its grammar, load the plugin with `"trace": true`,
   then parse:

   ```go
   j := tabnas.Make()
   ta := j.Token("#TA", "a")
   j.Rule("top", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
   	rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{ta}}})
   })
   j.SetOptions(tabnas.Options{Rule: &tabnas.RuleOptions{Start: "top"}})
   j.Use(debug.Debug, map[string]any{"trace": true})
   j.Parse("a")
   ```

2. Trace lines go to stdout (pass an `io.Writer` under `"out"` to
   capture them). You get the same kinds as TypeScript (`step`, `stack`,
   `rule`, `lex`, `parse`, `node`) with matching line shapes, each tagged
   by kind, `step` included; see the
   [trace output reference](../reference.md#trace-output) for the small
   remaining differences (no alt index on `parse` lines, no matcher name
   on `lex` lines).

## Rust

1. Give the instance its grammar, install the plugin with tracing on and
   printing off, then parse:

   ```rust
   use tabnas::Tabnas;
   use tabnas_debug::{apply, DebugOptions};

   fn main() -> Result<(), Box<dyn std::error::Error>> {
       let mut parser = Tabnas::new();
       parser.options.rule.start = "top".into();
       let a = parser.token_with_source("#TA", "a");
       parser.define_rule("top", move |spec| {
           spec.add_open(tabnas::AltSpec { s: vec![vec![a]], ..Default::default() });
       });
       apply(&mut parser, DebugOptions::default().with_print(false))?;
       parser.parse("a")?;
       Ok(())
   }
   ```

2. Trace lines go to the engine's own debug sink, stderr by default.
   There is no `out` option: set `parser.options.debug.output` to capture
   them instead.

3. You get five of the six kinds: `stack`, `rule`, `lex`, `parse` and
   `node`. `step` never fires, because the Rust engine emits no per-step
   event. Rust `parse` lines also name the matched alternate's
   push/replace/back/groups, when it has any, which Go's cannot; see the
   [trace output reference](../reference.md#trace-output).

## Notes

- TypeScript trace output goes to the parser's configured console; to
  capture it, override that console. Go trace output goes to stdout by
  default, or to the `io.Writer` passed as `opts["out"]`. Rust trace
  output goes to `parser.options.debug.output`, stderr by default.
- If you see no output, or only the banner, confirm tracing is enabled
  and that at least one kind that the runtime can emit is on; see
  [Choose which events to trace](select-trace-kinds.md). Confirm too that
  the instance has a grammar: the engine ships none, so a bare instance
  prints no events. In Rust a selection of `step` alone installs no
  tracing at all, not even the per-parse banner.
