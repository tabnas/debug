/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! `describe()` — a human-readable dump of a live instance: its tag,
//! tokens, token sets, rules, alternates, lexer matchers, config,
//! plugins, and an ABNF rendering of the grammar.
//!
//! The EIGHT section banners are the cross-runtime parity contract
//! (`test/spec/sections.tsv`): `INSTANCE`, `TOKENS`, `RULES`, `ALTS`,
//! `LEXER`, `CONFIG`, `PLUGIN`, `ABNF`, emitted byte-for-byte and in
//! order by every runtime. The text BETWEEN them is not pinned, and
//! differs where the engines expose different detail.
//!
//! Ported from `ts/src/debug.ts`, which is canonical.

use std::fmt::Write as _;

use tabnas::{AltSpec, RuleSpec, Tabnas};

use crate::abnf::abnf;
use crate::model;

/// The eight section banners, in order. Pinned by
/// `test/spec/sections.tsv`.
pub const SECTIONS: [&str; 8] = [
    "========= INSTANCE ========",
    "========= TOKENS ========",
    "========= RULES =========",
    "========= ALTS =========",
    "========= LEXER =========",
    "========= CONFIG ========",
    "========= PLUGIN =========",
    "========= ABNF =========",
];

/// Render `parser` as printable text.
pub fn describe(parser: &Tabnas) -> String {
    let mut parts: Vec<String> = Vec::new();

    parts.push(SECTIONS[0].to_string());
    parts.push(format!("  tag: {}", parser.options.tag));
    parts.push("\n".to_string());

    parts.push(SECTIONS[1].to_string());
    parts.push(
        model::tokens(parser)
            .into_iter()
            .map(|token| {
                let fixed = token
                    .fixed
                    .map(|source| format!("\"{source}\""))
                    .unwrap_or_default();
                format!("  {}\t{}\t{}", token.tin, token.name, fixed)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    parts.push("\n".to_string());

    parts.push(
        model::token_sets(parser)
            .into_iter()
            .map(|set| {
                let tins = set
                    .tins
                    .iter()
                    .map(|tin| tin.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!("    {}\t{}", set.name, tins)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    parts.push("\n".to_string());

    let specs = parser.rule_specs();

    parts.push(SECTIONS[2].to_string());
    parts.push(rule_tree(&specs));
    parts.push("\n".to_string());

    parts.push(SECTIONS[3].to_string());
    parts.push(
        specs
            .iter()
            .map(|spec| {
                format!(
                    "  {}:\n{}{}",
                    spec.name,
                    describe_alts(parser, &spec.open, "OPEN"),
                    describe_alts(parser, &spec.close, "CLOSE"),
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    parts.push("\n".to_string());

    parts.push(SECTIONS[4].to_string());
    parts.push(format!(
        "  {}",
        model::lexer(parser)
            .into_iter()
            .map(|matcher| format!("{}: {} ({})", matcher.order, matcher.matcher, matcher.make))
            .collect::<Vec<_>>()
            .join("\n  ")
    ));
    parts.push("\n".to_string());

    parts.push(SECTIONS[5].to_string());
    let config = model::config(parser);
    let mut config_lines = vec![
        format!("  start: {}", config.start),
        format!("  finish: {}", config.finish),
        format!("  safeKey: {}", config.safe_key),
    ];
    for (name, enabled) in &config.lex {
        config_lines.push(format!("  lex.{name}: {enabled}"));
    }
    parts.push(config_lines.join("\n"));
    parts.push("\n".to_string());

    parts.push("\n".to_string());
    parts.push(SECTIONS[6].to_string());
    parts.push(format!(
        "  {}",
        model::plugins(parser)
            .into_iter()
            .map(|plugin| {
                let mut line = plugin.name;
                if let Some(serde_json::Value::Object(options)) = plugin.options {
                    for (key, value) in options {
                        let _ = write!(line, "\n    {key}: {value}");
                    }
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n  ")
    ));
    parts.push("\n".to_string());

    parts.push(SECTIONS[7].to_string());
    parts.push(abnf(parser));
    parts.push("\n".to_string());

    parts.join("\n")
}

/// The `RULES` section: each rule's push/replace targets by phase and
/// step, skipping the categories it has none of.
fn rule_tree(specs: &[&RuleSpec]) -> String {
    let mut out = String::new();
    for spec in specs {
        let categories = [
            ("op", model::edges(&spec.open, model::Step::Push)),
            ("or", model::edges(&spec.open, model::Step::Replace)),
            ("cp", model::edges(&spec.close, model::Step::Push)),
            ("cr", model::edges(&spec.close, model::Step::Replace)),
        ];
        let lines: Vec<String> = categories
            .into_iter()
            .filter(|(_, targets)| !targets.is_empty())
            .map(|(label, targets)| format!("{label}: {}", targets.join(" ")))
            .collect();
        let _ = write!(out, "  {}:\n    {}\n", spec.name, lines.join("\n    "));
    }
    out
}

/// One phase's alternates, as the `ALTS` section renders them.
fn describe_alts(parser: &Tabnas, alts: &[AltSpec], kind: &str) -> String {
    if alts.is_empty() {
        return String::new();
    }
    let mut out = format!("    {kind}:\n");
    let rendered: Vec<String> = alts
        .iter()
        .enumerate()
        .map(|(index, alt)| describe_alt(parser, alt, index))
        .collect();
    out.push_str(&rendered.join("\n"));
    out.push('\n');
    out
}

fn describe_alt(parser: &Tabnas, alt: &AltSpec, index: usize) -> String {
    let sequence = alt
        .s
        .iter()
        .map(|position| match position.len() {
            0 => String::new(),
            1 => parser.token_name(position[0]),
            _ => format!(
                "[{}]",
                position
                    .iter()
                    .map(|tin| parser.token_name(*tin))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        })
        .collect::<Vec<_>>()
        .join(" ");
    let sequence = format!("[{sequence}] ");

    let replace = match (&alt.r, alt.r_fn.is_some() || alt.r_match.is_some()) {
        (Some(name), _) => format!(" r={name}"),
        (None, true) => " r=<F>".to_string(),
        (None, false) => String::new(),
    };
    let push = match (&alt.p, alt.p_fn.is_some() || alt.p_match.is_some()) {
        (Some(name), _) => format!(" p={name}"),
        (None, true) => " p=<F>".to_string(),
        (None, false) => String::new(),
    };
    let no_target = if replace.is_empty() && push.is_empty() {
        "\t"
    } else {
        ""
    };

    let back = if 0 == alt.b {
        String::new()
    } else {
        format!("b={}", alt.b)
    };

    // The engine holds counters in an unordered map, so they are sorted
    // for a stable dump — the TypeScript runtime prints insertion order.
    let counters = if alt.n.is_empty() {
        String::new()
    } else {
        let mut pairs: Vec<String> = alt
            .n
            .iter()
            .map(|(name, value)| format!("{name}:{value}"))
            .collect();
        pairs.sort();
        format!("n={}", pairs.join(","))
    };

    let info = model::alt_info(parser, alt);
    let mut flags = String::new();
    if info.action {
        flags.push('A');
    }
    if info.cond {
        flags.push('C');
    }
    if info.modifier {
        flags.push('H');
    }

    // The TypeScript alt carries a declarative condition as `c.n` / `c.d`
    // and prints them as `CN=` / `CD=`. The Rust engine's declarative form
    // is a list of path/op/value comparisons instead, rendered here as
    // `CD=` entries; a callback condition shows only as the `C` flag, in
    // both runtimes.
    let declarative = if alt.c.is_empty() {
        "\t".to_string()
    } else {
        let rendered: Vec<String> = alt
            .c
            .iter()
            .map(|condition| {
                format!(
                    "{}{:?}{}",
                    condition.path.join("."),
                    condition.op,
                    condition.value
                )
            })
            .collect();
        format!(" CD={}", rendered.join(","))
    };

    let groups = model::groups(alt);
    let groups = if groups.is_empty() {
        String::new()
    } else {
        format!("\tg={}", groups.join(","))
    };

    format!(
        "      {index:>5} {sequence:<32}{replace}{push}{no_target}\t{back}\t{counters}\t{flags}\t{declarative}{groups}"
    )
}
