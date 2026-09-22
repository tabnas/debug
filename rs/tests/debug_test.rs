/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! In-language tests for what the shared fixtures cannot express:
//! instance-level model fields, the plugin's options, the `USE:` dump,
//! and parse tracing.
//!
//! Anything expressible as grammar → report belongs in
//! `test/spec/*.tsv` instead, so every runtime checks it.

mod common;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use common::fixture;
use tabnas::{AltSpec, Plugin, Tabnas};
use tabnas_debug::{
    abnf, apply, describe, model, plugin, use_plugin, DebugOptions, TraceKinds, SECTIONS,
    TRACE_BANNER, VERSION,
};

/// Collect everything the engine writes to its debug output.
fn capture(parser: &mut Tabnas) -> Arc<Mutex<Vec<String>>> {
    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    parser.options.debug.output = Some(Arc::new(move |message: &str| {
        sink.lock().unwrap().push(message.to_string());
    }));
    lines
}

/// A grammar installed as a plugin, so `derive` rebuilds it on the child.
fn derivable_parser() -> Tabnas {
    let grammar = Plugin::new("DerivableFixture", |parser, _options| {
        parser.options.rule.start = "top".into();
        let number = parser
            .options
            .token("#NR")
            .expect("the engine has a number token");
        let end = parser
            .options
            .token("#ZZ")
            .expect("the engine has an end token");
        parser.define_rule("top", move |spec| {
            spec.clear();
            spec.add_open(AltSpec {
                s: vec![vec![number]],
                ..Default::default()
            });
            spec.add_close(AltSpec {
                s: vec![vec![end]],
                ..Default::default()
            });
        });
        Ok(())
    });
    let mut parser = Tabnas::new();
    parser
        .use_plugin(grammar, None)
        .expect("the derivable fixture installs");
    parser
}

#[test]
fn describe_emits_every_section_in_order() {
    for name in ["bare", "add", "greet"] {
        let parser = fixture::build(name).expect("a known grammar");
        let text = describe(&parser);
        let mut cursor = 0;
        for section in SECTIONS {
            let found = text[cursor..]
                .find(section)
                .unwrap_or_else(|| panic!("{name}: {section} missing or out of order"));
            cursor += found + section.len();
        }
    }
}

/// A non-trivial grammar: a `top` rule that pushes to a single-character
/// rule name `x`, with a group tag on the open alternate. Mirrors
/// `makeTreeGrammar` in `ts/test/debug.test.js` and `treeGrammar` in
/// `go/debug_test.go`, so the three describe-body tests assert the same
/// shape.
///
/// The token names carry the `#` prefix where the TypeScript mirror
/// writes them bare. That is the ENGINE, not the plugin:
/// `Tabnas::token_with_source` normalises a fixed token's name to `#…`,
/// and `describe` prints whatever `token_name` reports. Writing them
/// bare here would still produce `#Ta` in the dump, so they are written
/// as the engine stores them.
fn tree_grammar() -> Tabnas {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "top".into();
    let ta = parser.token_with_source("#Ta", "a");
    let tx = parser.token_with_source("#Tx", "x");
    let end = parser
        .options
        .token("#ZZ")
        .expect("the engine has an end token");

    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![ta]],
            p: Some("x".into()),
            g: "topgrp".into(),
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
    parser.define_rule("x", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![tx]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
    parser
}

/// The slice of a `describe` dump between two section banners.
fn section<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let start = text.find(from).unwrap_or_else(|| panic!("no {from}"));
    let end = text.find(to).unwrap_or_else(|| panic!("no {to}"));
    &text[start..end]
}

#[test]
fn describe_lists_custom_tokens_with_their_fixed_source() {
    let text = describe(&tree_grammar());
    let tokens = section(&text, SECTIONS[1], SECTIONS[2]);
    assert!(tokens.contains("#Ta"), "TOKENS should list #Ta:\n{tokens}");
    assert!(tokens.contains("#Tx"), "TOKENS should list #Tx:\n{tokens}");
    assert!(
        tokens.contains("\"a\""),
        "TOKENS should show the fixed source of Ta:\n{tokens}"
    );
}

#[test]
fn describe_renders_alt_bodies_with_sequence_push_and_group() {
    let text = describe(&tree_grammar());
    let alts = section(&text, SECTIONS[3], SECTIONS[4]);
    for expected in ["top:", "OPEN:", "CLOSE:", "[#Ta]", "p=x", "g=topgrp"] {
        assert!(
            alts.contains(expected),
            "ALTS should contain {expected:?}:\n{alts}"
        );
    }
}

#[test]
fn describe_keeps_a_single_character_push_target_in_the_rules_tree() {
    // The open-push edge from `top` to the single-character rule `x` must
    // survive. A previous off-by-one in the canonical runtime dropped
    // single-character targets, so all three ports pin it.
    let text = describe(&tree_grammar());
    let rules = section(&text, SECTIONS[2], SECTIONS[3]);
    assert!(
        rules.contains("op: x"),
        "RULES tree should contain the single-char push edge op: x:\n{rules}"
    );
}

#[test]
fn describe_reports_the_instance_tag() {
    let mut parser = fixture::build("bare").expect("a known grammar");
    // The engine defaults an unset tag to "-".
    assert!(describe(&parser).contains("  tag: -"));

    parser.options.tag = "mine".into();
    assert!(describe(&parser).contains("  tag: mine"));
    assert_eq!(model(&parser).tag, "mine");
}

#[test]
fn model_reports_tokens_token_sets_and_config() {
    let parser = fixture::build("add").expect("a known grammar");
    let built = model(&parser);

    // The registered fixed token appears with its source text.
    let plus = built
        .tokens
        .iter()
        .find(|token| "#PL" == token.name)
        .expect("the #PL token is in the table");
    assert_eq!(plus.fixed.as_deref(), Some("+"));

    // A built-in token appears with no fixed source.
    let number = built
        .tokens
        .iter()
        .find(|token| "#NR" == token.name)
        .expect("the #NR token is in the table");
    assert_eq!(number.fixed, None);

    // Tokens are ascending by tin, so the table reads as a table.
    let tins: Vec<_> = built.tokens.iter().map(|token| token.tin).collect();
    let mut sorted = tins.clone();
    sorted.sort_unstable();
    assert_eq!(tins, sorted);

    // The engine's default token sets, by name, members ascending.
    let names: Vec<&str> = built
        .token_sets
        .iter()
        .map(|set| set.name.as_str())
        .collect();
    assert_eq!(names, ["IGNORE", "KEY", "VAL"]);
    for set in &built.token_sets {
        let mut sorted = set.tins.clone();
        sorted.sort_unstable();
        assert_eq!(set.tins, sorted, "{} members are ascending", set.name);
    }

    assert_eq!(built.config.start, "val");
    assert!(built.config.finish);
    assert!(built.config.safe_key);
    // The lex flags keep the canonical order.
    let flags: Vec<&str> = built.config.lex.keys().map(String::as_str).collect();
    assert_eq!(
        flags,
        ["fixed", "space", "line", "text", "number", "comment", "string", "value"]
    );

    // model() carries the same ABNF text abnf() returns.
    assert_eq!(built.abnf, abnf(&parser));
}

#[test]
fn model_records_applied_plugins() {
    let mut parser = fixture::build("bare").expect("a known grammar");
    assert!(model(&parser).plugins.is_empty());

    apply(&mut parser, DebugOptions::quiet()).expect("the debug plugin installs");

    let names: Vec<String> = model(&parser)
        .plugins
        .into_iter()
        .map(|info| info.name)
        .collect();
    assert_eq!(names, ["Debug"]);
}

#[test]
fn model_serialises_with_the_cross_runtime_field_names() {
    let parser = fixture::build("add").expect("a known grammar");
    let json = serde_json::to_value(model(&parser)).expect("the model serialises");
    let object = json.as_object().expect("the model is an object");

    // The camelCase names the TypeScript model and the Go JSON tags use.
    for key in [
        "tag",
        "tokens",
        "tokenSets",
        "rules",
        "graph",
        "lexer",
        "config",
        "plugins",
        "abnf",
    ] {
        assert!(object.contains_key(key), "the model has a {key} field");
    }
    assert!(object["config"]
        .as_object()
        .expect("config is an object")
        .contains_key("safeKey"));

    let val_rule = object["rules"]
        .as_array()
        .expect("rules is an array")
        .iter()
        .find(|rule| Some("val") == rule["name"].as_str())
        .expect("the val rule is present");
    let open = &val_rule["open"].as_array().expect("open is an array")[0];
    assert_eq!(open["push"].as_str(), Some("add"));
    // Absent optionals drop out rather than serialising as null.
    assert!(open.get("replace").is_none());
    assert!(open.get("back").is_none());
    assert!(open.get("counters").is_none());
}

#[test]
fn use_plugin_dumps_describe_when_printing() {
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);

    apply(&mut parser, DebugOptions::new().without_trace()).expect("the debug plugin installs");
    // Installing the debug plugin itself prints nothing.
    assert!(lines.lock().unwrap().is_empty());

    let noop = Plugin::new("Noop", |_parser, _options| Ok(()));
    use_plugin(&mut parser, noop, None).expect("the plugin installs");

    let captured = lines.lock().unwrap().join("\n");
    assert!(captured.starts_with("USE: Noop"), "got: {captured}");
    assert!(captured.contains(SECTIONS[0]));
    assert!(captured.contains(SECTIONS[7]));
}

#[test]
fn use_plugin_stays_quiet_when_printing_is_off() {
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);

    apply(&mut parser, DebugOptions::quiet()).expect("the debug plugin installs");
    let noop = Plugin::new("Noop", |_parser, _options| Ok(()));
    use_plugin(&mut parser, noop, None).expect("the plugin installs");

    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn use_plugin_without_the_debug_plugin_installed_is_silent() {
    // No debug plugin means no `print` decoration, so nothing is dumped —
    // the wrapper is inert rather than an error.
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);

    let noop = Plugin::new("Noop", |_parser, _options| Ok(()));
    use_plugin(&mut parser, noop, None).expect("the plugin installs");

    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn use_plugin_propagates_a_failing_plugin() {
    let mut parser = fixture::build("add").expect("a known grammar");
    apply(&mut parser, DebugOptions::quiet()).expect("the debug plugin installs");

    let failing = Plugin::new("Boom", |_parser, _options| {
        Err(tabnas::PluginError("boom".into()))
    });
    let error = use_plugin(&mut parser, failing, None).expect_err("the plugin fails");
    assert_eq!(error.0, "boom");
}

#[test]
fn tracing_logs_a_banner_and_events() {
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);
    apply(&mut parser, DebugOptions::new().with_print(false)).expect("the debug plugin installs");

    parser.parse("1+2").expect("the add grammar parses 1+2");

    let captured = lines.lock().unwrap().clone();
    assert!(
        captured.iter().any(|line| line == TRACE_BANNER),
        "the trace banner is written once per parse"
    );
    for prefix in ["lex ", "rule ", "stack ", "parse ", "node "] {
        assert!(
            captured.iter().any(|line| line.starts_with(prefix)),
            "a {prefix:?} line was logged; got {captured:#?}"
        );
    }
}

#[test]
fn trace_kinds_are_selectable() {
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);
    apply(
        &mut parser,
        DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                lex: true,
                ..TraceKinds::none()
            }),
    )
    .expect("the debug plugin installs");

    parser.parse("1+2").expect("the add grammar parses 1+2");

    let captured = lines.lock().unwrap().clone();
    assert!(captured.iter().any(|line| line.starts_with("lex ")));
    for prefix in ["rule ", "stack ", "parse ", "node "] {
        assert!(
            !captured.iter().any(|line| line.starts_with(prefix)),
            "no {prefix:?} line when that kind is off"
        );
    }
}

#[test]
fn tracing_off_writes_nothing() {
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);
    apply(&mut parser, DebugOptions::quiet()).expect("the debug plugin installs");

    parser.parse("1+2").expect("the add grammar parses 1+2");

    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn step_alone_installs_no_tracing() {
    // `step` has no Rust engine hook, so selecting only `step` is
    // accepted but logs nothing — not even the per-parse banner. See
    // docs/reference.md.
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);
    apply(
        &mut parser,
        DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                step: true,
                ..TraceKinds::none()
            }),
    )
    .expect("the debug plugin installs");

    parser.parse("1+2").expect("the add grammar parses 1+2");

    assert!(lines.lock().unwrap().is_empty());
}

// --- regressions: re-installing the plugin -------------------------------
//
// The engine ACCUMULATES subscribers and parse-prepare hooks, so a second
// install must reuse the first one's registration rather than stack a
// second set. `derive` re-runs a parent's plugins on the child, so this is
// not a hypothetical.

/// Lines written during one parse of `1+2`, after applying `options` in
/// order.
fn trace_of(applications: &[DebugOptions]) -> Vec<String> {
    let mut parser = fixture::build("add").expect("a known grammar");
    let lines = capture(&mut parser);
    for options in applications {
        apply(&mut parser, *options).expect("the debug plugin installs");
    }
    parser.parse("1+2").expect("the add grammar parses 1+2");
    let captured = lines.lock().unwrap().clone();
    captured
}

#[test]
fn reapplying_the_plugin_does_not_stack_trace_subscribers() {
    let once = trace_of(&[DebugOptions::new().with_print(false)]);
    let twice = trace_of(&[
        DebugOptions::new().with_print(false),
        DebugOptions::new().with_print(false),
    ]);

    let banners = |lines: &[String]| lines.iter().filter(|line| *line == TRACE_BANNER).count();
    assert_eq!(banners(&once), 1, "one banner per parse");
    assert_eq!(banners(&twice), 1, "still one banner after re-applying");
    assert_eq!(
        once.len(),
        twice.len(),
        "re-applying must not duplicate trace events"
    );
}

#[test]
fn reapplying_with_a_narrower_selection_disables_the_older_streams() {
    let lines = trace_of(&[
        DebugOptions::new().with_print(false),
        DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                lex: true,
                ..TraceKinds::none()
            }),
    ]);

    assert!(lines.iter().any(|line| line.starts_with("lex ")));
    for prefix in ["rule ", "stack ", "parse ", "node "] {
        assert!(
            !lines.iter().any(|line| line.starts_with(prefix)),
            "a narrower re-apply must silence {prefix:?}; got {lines:#?}"
        );
    }
}

#[test]
fn reapplying_without_trace_turns_tracing_off() {
    let lines = trace_of(&[
        DebugOptions::new().with_print(false),
        DebugOptions::new().with_print(false).without_trace(),
    ]);
    assert!(
        lines.is_empty(),
        "re-applying without trace must silence everything; got {lines:#?}"
    );
}

#[test]
fn a_failed_trace_install_can_be_retried() {
    let should_fail = Arc::new(AtomicBool::new(false));
    let fail = should_fail.clone();
    let mut parser = fixture::build("add").expect("a known grammar");
    parser.config_modifier_ref("@fail", move |_| {
        assert!(
            !fail.load(Ordering::SeqCst),
            "requested configuration failure"
        );
    });
    parser
        .grammar_json(r#"{"options":{"config":{"modify":{"fail":"@fail"}}}}"#)
        .expect("the failure modifier starts disabled");

    should_fail.store(true, Ordering::SeqCst);
    assert!(apply(&mut parser, DebugOptions::new().with_print(false)).is_err());

    should_fail.store(false, Ordering::SeqCst);
    apply(&mut parser, DebugOptions::new().with_print(false))
        .expect("a failed trace install can be retried");
    let lines = capture(&mut parser);
    parser
        .parse("1+2")
        .expect("the retried parser still parses");

    let captured = lines.lock().unwrap().clone();
    assert!(captured.iter().any(|line| line.starts_with("lex ")));
    assert!(captured.iter().any(|line| line.starts_with("rule ")));
}

#[test]
fn deriving_a_child_does_not_stack_trace_subscribers() {
    // `derive` re-runs the parent's plugins against the child's options.
    let mut parent = derivable_parser();
    apply(&mut parent, DebugOptions::new().with_print(false)).expect("the debug plugin installs");

    let mut child = parent.derive(|options| options.tag = "child".into()).ok();
    let Some(child) = child.as_mut() else {
        panic!("deriving a child must not fail");
    };
    assert_ne!(parent.id, child.id);
    assert_eq!(child.lex_subscribers.len(), 1);
    assert_eq!(child.rule_subscribers.len(), 1);
    assert_eq!(child.rule_done_subscribers.len(), 1);
    let lines = capture(child);
    child.parse("1").expect("the child parses a number");

    let captured = lines.lock().unwrap().clone();
    assert_eq!(
        captured.iter().filter(|line| *line == TRACE_BANNER).count(),
        1,
        "one banner per parse on a derived instance; got {captured:#?}"
    );
    for prefix in ["lex ", "rule ", "stack ", "parse ", "node "] {
        assert!(
            captured.iter().any(|line| line.starts_with(prefix)),
            "the derived instance keeps its {prefix:?} stream; got {captured:#?}"
        );
    }
}

#[test]
fn a_child_trace_selection_does_not_change_its_parent() {
    let mut parent = derivable_parser();
    let parent_lines = capture(&mut parent);
    apply(&mut parent, DebugOptions::new().with_print(false)).expect("debug installs on parent");

    let mut child = parent
        .derive(|options| options.tag = "child".into())
        .expect("derive a child");
    let child_lines = capture(&mut child);
    apply(
        &mut child,
        DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                lex: true,
                ..TraceKinds::none()
            }),
    )
    .expect("narrow tracing on child");

    parent.parse("1").expect("the parent still parses");
    child.parse("1").expect("the child still parses");

    let parent_lines = parent_lines.lock().unwrap().clone();
    let child_lines = child_lines.lock().unwrap().clone();
    assert!(parent_lines.iter().any(|line| line.starts_with("rule ")));
    assert!(child_lines.iter().any(|line| line.starts_with("lex ")));
    assert!(!child_lines.iter().any(|line| line.starts_with("rule ")));
}

// --- regressions: reporting fidelity -------------------------------------

#[test]
fn lex_lines_bound_the_source_text_they_quote() {
    // The `src=` field used to carry the whole token source, so one long
    // string token put its entire text on the trace line while the value
    // beside it was cut at `maxlen`. The canonical runtime bounds both
    // through `ctx.F`; so does this port.
    let mut parser = fixture::build("bare").expect("a known grammar");
    parser.options.rule.start = "top".into();
    let string = parser
        .options
        .token("#ST")
        .expect("the engine has a string token");
    let end = parser
        .options
        .token("#ZZ")
        .expect("the engine has an end token");
    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![string]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
    let lines = capture(&mut parser);
    apply(
        &mut parser,
        DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                lex: true,
                ..TraceKinds::none()
            }),
    )
    .expect("the debug plugin installs");

    let maxlen = parser.options.debug.maxlen;
    let source = format!("\"{}\"", "x".repeat(20 * maxlen));
    parser.parse(&source).expect("a long string parses");

    let captured = lines.lock().unwrap().clone();
    let lex = captured
        .iter()
        .find(|line| line.starts_with("lex ") && line.contains("#ST"))
        .unwrap_or_else(|| panic!("a lex line for the string token; got {captured:#?}"));
    let (_, quoted) = lex
        .split_once(" src=")
        .expect("the lex line carries a src field");
    assert!(
        quoted.chars().count() <= maxlen + "...".len(),
        "src is cut at maxlen ({maxlen}); got {} chars: {quoted}",
        quoted.chars().count()
    );
    assert!(quoted.ends_with("..."), "a cut src is marked; got {quoted}");
    assert!(
        lex.chars().count() < source.len(),
        "the trace line must not scale with the token source"
    );
}

#[test]
fn lex_lines_quote_control_characters_once() {
    // A line token's source IS a newline. The value beside it is rendered
    // by the engine as JSON (`"\n"`), and the canonical `ctx.F` renders
    // the source the same way; this port used to escape by hand and then
    // format, doubling the backslash to `"\\n"` so a real newline read
    // like the two characters `\` `n`.
    let mut parser = Tabnas::make_json();
    let lines = capture(&mut parser);
    apply(
        &mut parser,
        DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                lex: true,
                ..TraceKinds::none()
            }),
    )
    .expect("the debug plugin installs");

    parser
        .parse("[1,\n\t2]")
        .expect("json with whitespace parses");

    let captured = lines.lock().unwrap().clone();
    let src_of = |token: &str| -> String {
        let line = captured
            .iter()
            .find(|line| line.starts_with("lex ") && line.contains(token))
            .unwrap_or_else(|| panic!("a lex line for {token}; got {captured:#?}"));
        line.split_once(" src=")
            .expect("the lex line carries a src field")
            .1
            .to_string()
    };
    assert_eq!(src_of("#LN"), "\"\\n\"", "a newline is escaped once");
    assert_eq!(src_of("#SP"), "\"\\t\"", "a tab is escaped once");
}

#[test]
fn lexer_matcher_order_keeps_fractional_priorities() {
    // Truncating to an integer would report 1.2 and 1.8 as the same order.
    let mut parser = fixture::build("bare").expect("a known grammar");
    for (name, order) in [("early", 1.2_f64), ("late", 1.8_f64)] {
        parser.options.lex.matchers.insert(
            name.to_string(),
            tabnas::LexMatcher {
                name: name.to_string(),
                order,
                matcher: None,
                imperative: None,
                factory: None,
            },
        );
    }

    let orders: Vec<f64> = model(&parser)
        .lexer
        .into_iter()
        .map(|matcher| matcher.order)
        .collect();
    assert_eq!(orders, [1.2, 1.8]);
}

#[test]
fn a_function_backed_match_token_gets_a_valid_abnf_form() {
    // An ABNF comment starts at `;` and runs to end of line, so a legend
    // entry of `T = ; …` would define a rule with no elements at all. The
    // canonical runtime only special-cases a RegExp and otherwise falls
    // through to the built-in description; so does this port.
    let mut parser = fixture::build("bare").expect("a known grammar");
    let tin = parser.token("#FN");
    parser.options.match_tokens.insert(
        "#FN".to_string(),
        tabnas::MatchToken {
            name: "#FN".to_string(),
            tin,
            matcher: tabnas::MatchTokenMatcher::Callback(std::sync::Arc::new(|_source| None)),
            eager: false,
        },
    );
    parser.options.rule.start = "top".into();
    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(tabnas::AltSpec {
            s: vec![vec![tin]],
            ..Default::default()
        });
    });

    let emitted = abnf(&parser);
    let legend = emitted
        .lines()
        .find(|line| line.starts_with("FN "))
        .unwrap_or_else(|| panic!("the FN legend entry is present; got:\n{emitted}"));
    assert_eq!(legend, "FN = <built-in FN>");
    assert!(
        !emitted.contains(" = ;"),
        "no legend entry may be nothing but a comment; got:\n{emitted}"
    );
}

#[test]
fn the_plugin_is_named_debug() {
    let installed = plugin(DebugOptions::quiet());
    assert_eq!(installed.name, "Debug");
}

#[test]
fn version_is_exported() {
    assert!(!VERSION.is_empty());
}
