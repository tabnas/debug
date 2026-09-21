/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! Parse tracing: log events as the parser runs.
//!
//! The canonical TypeScript runtime defines SIX selectable kinds —
//! `step`, `rule`, `lex`, `parse`, `node`, `stack` — driven by the
//! engine's `ctx.log` callback. The Rust engine has no `ctx.log`; it
//! exposes typed subscribers instead, so each kind is wired to the
//! subscriber that carries its information:
//!
//! | Kind    | Rust source                                   |
//! |---------|-----------------------------------------------|
//! | `lex`   | `subscribe_lex`                               |
//! | `rule`  | `subscribe_rules`                             |
//! | `stack` | `subscribe_rules` (the context's rule stack)  |
//! | `parse` | `subscribe_rule_done` (the matched alternate) |
//! | `node`  | `subscribe_rule_done` (the completed node)    |
//! | `step`  | **no engine hook** — see below                |
//!
//! `step` is the raw passthrough kind: in TypeScript the engine itself
//! calls `ctx.log('step', …)` once per parse step. The Rust engine emits
//! no such event, so selecting `step` here is accepted (the option name
//! stays in step with the other runtimes) but nothing is ever logged for
//! it. That is an engine-API limit, recorded in `docs/reference.md`.

use std::sync::{Arc, Mutex};

use tabnas::{ParsePrepare, PluginError, Tabnas};

/// Which trace kinds are logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceKinds {
    /// Raw per-step passthrough. **Never fires in Rust** — the engine has
    /// no step event. Kept for option-name parity.
    pub step: bool,
    /// Each rule as it opens and closes.
    pub rule: bool,
    /// Each token the lexer produces.
    pub lex: bool,
    /// The alternate a rule matched.
    pub parse: bool,
    /// A rule's node as the rule completes.
    pub node: bool,
    /// The rule stack at each rule event.
    pub stack: bool,
}

impl TraceKinds {
    /// Every kind on — the canonical default.
    pub fn all() -> Self {
        Self {
            step: true,
            rule: true,
            lex: true,
            parse: true,
            node: true,
            stack: true,
        }
    }

    /// Every kind off. Build a selection with the field setters.
    pub fn none() -> Self {
        Self {
            step: false,
            rule: false,
            lex: false,
            parse: false,
            node: false,
            stack: false,
        }
    }

    /// True when at least one kind that Rust can actually emit is on.
    fn any_live(&self) -> bool {
        self.rule || self.lex || self.parse || self.node || self.stack
    }
}

impl Default for TraceKinds {
    fn default() -> Self {
        Self::all()
    }
}

/// The banner written once at the start of each traced parse.
pub const TRACE_BANNER: &str = "\n========= TRACE ==========";

/// The decoration key under which the plugin keeps its LIVE trace
/// selection.
const TRACE_DECORATION: &str = "debug.trace";

/// Per-instance trace state shared with that instance's callbacks.
struct TraceRuntime {
    instance_id: String,
    kinds: Mutex<Option<TraceKinds>>,
}

/// The live selection, shared with the registered callbacks.
type TraceState = Arc<TraceRuntime>;

/// Read the current selection. A poisoned lock traces nothing rather than
/// panicking: debugging must not make a parse less reliable.
fn current(state: &TraceState, instance_id: &str) -> Option<TraceKinds> {
    if state.instance_id != instance_id {
        return None;
    }
    state.kinds.lock().ok().and_then(|kinds| *kinds)
}

/// Install the trace subscribers on `parser`, or update the selection if
/// they are already installed.
///
/// The engine ACCUMULATES subscribers and parse-prepare hooks, so
/// registering a second set would double every banner and every event —
/// and a later, narrower selection could not switch the first set off.
/// Applying the plugin twice is not hypothetical: `derive` re-runs a
/// parent's plugins on the child. So the callbacks are registered exactly
/// once per instance and read a shared selection that later installs
/// update in place, which is how the canonical runtime's
/// `__debugUseWrapped` guard behaves for its own wrapper.
pub(crate) fn install(parser: &mut Tabnas, kinds: Option<TraceKinds>) -> Result<(), PluginError> {
    if let Some(state) = parser.decoration::<TraceState>(TRACE_DECORATION).cloned() {
        if state.instance_id == parser.id {
            if let Ok(mut live) = state.kinds.lock() {
                *live = kinds;
            }
            return Ok(());
        }
    }

    // Derived parsers inherit decorations and option callbacks, but not
    // subscribers. Replace the inherited state and named prepare hook with
    // child-owned versions, then register the child's subscribers below.
    let state: TraceState = Arc::new(TraceRuntime {
        instance_id: parser.id.clone(),
        kinds: Mutex::new(kinds),
    });

    // One banner per parse, as the canonical runtime emits it.
    let live = state.clone();
    parser.set_options(move |options| {
        options.parse.named_prepare.insert(
            "debug".to_string(),
            ParsePrepare::Context(Arc::new(move |context| {
                if current(&live, &context.instance.id).is_some_and(|kinds| kinds.any_live()) {
                    context.options.debug.write(TRACE_BANNER);
                }
            })),
        );
    })?;

    // Publish the initialized state only after the fallible options rebuild
    // succeeds. Otherwise a retry would mistake a partial install for a
    // complete one and skip registering the subscribers below.
    parser.decorate_opaque(TRACE_DECORATION, state.clone());

    {
        let live = state.clone();
        parser.subscribe_lex(move |token, rule, context| {
            if !current(&live, &context.instance.id).is_some_and(|kinds| kinds.lex) {
                return;
            }
            context.options.debug.write(&format!(
                "lex   {}{} {} pos={} {}:{} src={}",
                indent(rule.d),
                token.name,
                context.options.debug.format_source(&token.val),
                token.site.pos,
                token.site.ri,
                token.site.ci,
                quote(&token.src, context.options.debug.maxlen),
            ));
        });
    }

    {
        let live = state.clone();
        parser.subscribe_rules(move |rule, context| {
            let Some(kinds) = current(&live, &context.instance.id) else {
                return;
            };
            if kinds.rule {
                context.options.debug.write(&format!(
                    "rule  {}{}~{}/{:?} d={} node={}{}",
                    indent(rule.d),
                    rule.name,
                    rule.i,
                    rule.state,
                    rule.d,
                    context.options.debug.format_source(&rule.node.borrow()),
                    counters(&rule.n),
                ));
            }
            if kinds.stack {
                let stack = context
                    .rule_stack
                    .iter()
                    .map(|frame| format!("{}~{}", frame.name, frame.i))
                    .collect::<Vec<_>>()
                    .join("/");
                context
                    .options
                    .debug
                    .write(&format!("stack {}/{}", indent(rule.d), stack));
            }
        });
    }

    {
        let live = state.clone();
        parser.subscribe_rule_done(move |rule, context, done| {
            let Some(kinds) = current(&live, &context.instance.id) else {
                return;
            };
            if kinds.parse {
                let matched = match &done.alt {
                    Some(alt) => {
                        let mut parts = Vec::new();
                        if !alt.p.is_empty() {
                            parts.push(format!("p:{}", alt.p));
                        }
                        if !alt.r.is_empty() {
                            parts.push(format!("r:{}", alt.r));
                        }
                        if 0 != alt.b {
                            parts.push(format!("b:{}", alt.b));
                        }
                        if !alt.g.is_empty() {
                            parts.push(format!("g:{}", alt.g.join(",")));
                        }
                        if let Some(error) = &alt.err {
                            parts.push(format!("err:{}", error.err));
                        }
                        format!("alt {}", parts.join(" "))
                    }
                    None => "no-alt".to_string(),
                };
                context.options.debug.write(&format!(
                    "parse {}{}~{}/{:?} {}{}",
                    indent(rule.d),
                    rule.name,
                    rule.i,
                    done.state,
                    matched,
                    if done.forced { " forced" } else { "" },
                ));
            }
            if kinds.node {
                context.options.debug.write(&format!(
                    "node  {}{}~{} <{}>",
                    indent(rule.d),
                    rule.name,
                    rule.i,
                    context.options.debug.format_source(&rule.node.borrow()),
                ));
            }
        });
    }

    Ok(())
}

/// Depth indent, matching the canonical runtime's nesting cue.
fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

/// A rule's live counters, rendered only when it has any. The engine
/// holds them unordered, so they are sorted for a stable trace.
fn counters(values: &std::collections::HashMap<String, i32>) -> String {
    if values.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<String> = values
        .iter()
        .filter(|(_, value)| 0 != **value)
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    if pairs.is_empty() {
        return String::new();
    }
    pairs.sort();
    format!(" N<{}>", pairs.join(";"))
}

/// Source text for a trace line, kept on one line and bounded.
///
/// Quoted the way the canonical `ctx.F` (a JSON stringifier) quotes this
/// very field: `{:?}` escapes a newline, tab or carriage return ONCE, as
/// `\n` / `\t` / `\r`, which is what the engine's `format_source` shows
/// for the value beside it. Escaping them by hand first and then
/// formatting doubled the backslash, so a line token's `src=` read `"\\n"`
/// next to a `val` of `"\n"`, and a source holding a real newline was
/// indistinguishable from one holding the two characters `\` `n`.
///
/// Bounded the way `format_source` bounds the value and `ctx.F` bounds
/// this field: the rendered form is cut at `maxlen` characters and marked
/// with `...`. Without the cut a single long string token put its whole
/// source on the line, so a traced parse of untrusted input cost as much
/// output as the input itself, per token.
fn quote(source: &str, maxlen: usize) -> String {
    let rendered = format!("{source:?}");
    let mut chars = rendered.chars();
    let prefix: String = chars.by_ref().take(maxlen).collect();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_presets() {
        assert!(TraceKinds::all().any_live());
        assert!(!TraceKinds::none().any_live());
        // `step` alone has no Rust source, so it is not "live".
        let step_only = TraceKinds {
            step: true,
            ..TraceKinds::none()
        };
        assert!(!step_only.any_live());
    }

    #[test]
    fn counters_are_sorted_and_skip_zeroes() {
        let mut values = std::collections::HashMap::new();
        values.insert("zed".to_string(), 1);
        values.insert("abc".to_string(), 2);
        values.insert("nil".to_string(), 0);
        assert_eq!(counters(&values), " N<abc=2;zed=1>");
        assert_eq!(counters(&std::collections::HashMap::new()), "");
    }

    #[test]
    fn quote_escapes_control_characters() {
        // Escaped ONCE, as JSON (and the canonical `ctx.F`) renders them:
        // the six characters `"a\nb"`, on one line.
        assert_eq!(quote("a\nb", 99), "\"a\\nb\"");
        assert_eq!(quote("\r\t", 99), "\"\\r\\t\"");
        // A real newline and a literal backslash-n must not render alike.
        assert_eq!(quote("a\\nb", 99), "\"a\\\\nb\"");
        assert_ne!(quote("a\nb", 99), quote("a\\nb", 99));
    }

    #[test]
    fn quote_is_bounded_by_maxlen() {
        // Cut at `maxlen` characters of the rendered form and marked, as
        // the engine's `format_source` does for the value beside it.
        assert_eq!(quote("abcdef", 4), "\"abc...");
        assert_eq!(quote("ab", 4), "\"ab\"");
        // Multi-byte characters count as one each, never split.
        assert_eq!(
            quote("\u{1F600}\u{1F600}\u{1F600}", 3),
            "\"\u{1F600}\u{1F600}..."
        );
    }
}
