/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! In-language tests for what the shared fixtures cannot express:
//! instance-level model fields, the plugin's options, the `USE:` dump,
//! and parse tracing.
//!
//! Anything expressible as grammar → report belongs in
//! `test/spec/*.tsv` instead, so every runtime checks it.

mod common;

use std::sync::{Arc, Mutex};

use common::fixture;
use tabnas::{Plugin, Tabnas};
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
fn deriving_a_child_does_not_stack_trace_subscribers() {
    // `derive` re-runs the parent's plugins against the child's options.
    let mut parent = fixture::build("add").expect("a known grammar");
    apply(&mut parent, DebugOptions::new().with_print(false)).expect("the debug plugin installs");

    let mut child = parent.derive(|options| options.tag = "child".into()).ok();
    let Some(child) = child.as_mut() else {
        panic!("deriving a child must not fail");
    };
    let lines = capture(child);
    child.parse("1+2").expect("the child parses 1+2");

    let captured = lines.lock().unwrap().clone();
    assert_eq!(
        captured.iter().filter(|line| *line == TRACE_BANNER).count(),
        1,
        "one banner per parse on a derived instance; got {captured:#?}"
    );
}

// --- regressions: reporting fidelity -------------------------------------

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
