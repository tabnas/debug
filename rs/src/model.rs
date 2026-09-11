/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! The structured counterpart to [`crate::describe`]: an instance and its
//! grammar as typed, JSON-serialisable data, so tools and tests can
//! consume the grammar programmatically.
//!
//! Mirrors the canonical TypeScript `tabnas.debug.model()` and its
//! exported types in `ts/src/debug.ts` (`DebugModel`, `DebugTokenInfo`,
//! `DebugTokenSet`, `DebugAltInfo`, `DebugRuleInfo`, `DebugRuleEdges`,
//! `DebugLexMatcher`, `DebugConfigInfo`, `DebugPluginInfo`). The serde
//! field names match the TS field names and the Go JSON tags, so a model
//! decoded from any runtime is comparable with the others — the claim
//! `test/spec/model.tsv` pins.

use std::collections::BTreeMap;

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use tabnas::{AltSpec, RuleSpec, Tabnas, Tin, TIN_MAX};

use crate::abnf::abnf;

/// One row of the token table: the token's tin (token identification
/// number), its name, and — for fixed (literal) tokens — the source text
/// it matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugTokenInfo {
    /// Token identification number.
    pub tin: Tin,
    /// Token name (e.g. `#NR`).
    pub name: String,
    /// Fixed source text, when a literal token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed: Option<String>,
}

/// A named token set and its member tins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugTokenSet {
    /// Set name (`IGNORE`, `VAL`, `KEY`, …).
    pub name: String,
    /// Member tins, ascending.
    pub tins: Vec<Tin>,
}

/// One lookahead position of an alternate's token sequence: a single
/// token name, or the several names a multi-token position accepts.
///
/// Serialises untagged — a bare string or an array of strings — matching
/// the TypeScript `(string | string[])[]` and the Go `[]any`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DebugSeqItem {
    /// A single-token position.
    One(String),
    /// A position accepting any of several tokens.
    Any(Vec<String>),
}

/// The structured form of a single rule alternate — the data behind the
/// `ALTS` text of [`crate::describe`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugAltInfo {
    /// Token name(s) per lookahead position.
    pub seq: Vec<DebugSeqItem>,
    /// `p` target rule (or `<fn>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<String>,
    /// `r` target rule (or `<fn>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace: Option<String>,
    /// `b` token push-back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back: Option<usize>,
    /// `n` counter ops.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counters: Option<BTreeMap<String, i32>>,
    /// `g` group tags.
    pub groups: Vec<String>,
    /// `a` present.
    pub action: bool,
    /// `c` present.
    pub cond: bool,
    /// `h` present.
    pub modifier: bool,
}

/// One rule: its name and its open/close alternates as structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugRuleInfo {
    /// Rule name.
    pub name: String,
    /// Open-phase alternates, in order.
    pub open: Vec<DebugAltInfo>,
    /// Close-phase alternates, in order.
    pub close: Vec<DebugAltInfo>,
}

/// One rule's outgoing edges in the rule-reference graph: the distinct
/// push/replace rule-name targets of its open and close alternates
/// (function-valued targets recorded as `<fn>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugRuleEdges {
    /// Rule name.
    pub name: String,
    /// Distinct `p` targets of open alts.
    #[serde(rename = "openPush")]
    pub open_push: Vec<String>,
    /// Distinct `r` targets of open alts.
    #[serde(rename = "openReplace")]
    pub open_replace: Vec<String>,
    /// Distinct `p` targets of close alts.
    #[serde(rename = "closePush")]
    pub close_push: Vec<String>,
    /// Distinct `r` targets of close alts.
    #[serde(rename = "closeReplace")]
    pub close_replace: Vec<String>,
}

/// One lexer matcher, in priority order.
///
/// The Rust engine enumerates only CUSTOM matchers; the built-in
/// matchers appear as enable flags under [`DebugConfigInfo::lex`]
/// instead — the same limit the Go port documents. `make` is the Go
/// analogue of the TypeScript factory name and has no Rust counterpart
/// (function values carry no name), so it is always empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugLexMatcher {
    /// Priority (lower runs first).
    pub order: i64,
    /// Registered matcher name.
    pub matcher: String,
    /// Matcher function name, when recoverable.
    pub make: String,
}

/// The key parser settings: start rule, finish flag, safe-key, and the
/// built-in per-lexer enable flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DebugConfigInfo {
    /// Starting rule name.
    pub start: String,
    /// Auto-close unclosed structures at end of source.
    pub finish: bool,
    /// Prevent `__proto__` keys.
    #[serde(rename = "safeKey")]
    pub safe_key: bool,
    /// Built-in lexer enable flags, in the canonical order.
    pub lex: IndexMap<String, bool>,
}

/// One applied plugin: its name, and its options when the instance holds
/// a bag for it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugPluginInfo {
    /// Plugin name.
    pub name: String,
    /// Plugin options, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

/// The full structured description of an instance: the token table, token
/// sets, rules and alternates as data, the rule-reference graph, lexer
/// matchers, config, plugins, and the ABNF text.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugModel {
    /// Instance tag; the engine's default (`-`) when unset.
    pub tag: String,
    /// The token table.
    pub tokens: Vec<DebugTokenInfo>,
    /// Named token sets.
    #[serde(rename = "tokenSets")]
    pub token_sets: Vec<DebugTokenSet>,
    /// Rules and their alternates.
    pub rules: Vec<DebugRuleInfo>,
    /// Per-rule push/replace edges.
    pub graph: Vec<DebugRuleEdges>,
    /// Lexer matchers, priority order.
    pub lexer: Vec<DebugLexMatcher>,
    /// Key parser settings.
    pub config: DebugConfigInfo,
    /// Applied plugins.
    pub plugins: Vec<DebugPluginInfo>,
    /// The re-compilable ABNF rendering of the live grammar.
    pub abnf: String,
}

/// Build the structured model of `parser`.
pub fn model(parser: &Tabnas) -> DebugModel {
    let specs = parser.rule_specs();

    DebugModel {
        tag: parser.options.tag.clone(),
        tokens: tokens(parser),
        token_sets: token_sets(parser),
        rules: specs
            .iter()
            .map(|spec| DebugRuleInfo {
                name: spec.name.clone(),
                open: spec.open.iter().map(|alt| alt_info(parser, alt)).collect(),
                close: spec.close.iter().map(|alt| alt_info(parser, alt)).collect(),
            })
            .collect(),
        graph: specs.iter().map(|spec| rule_edges(spec)).collect(),
        lexer: lexer(parser),
        config: config(parser),
        plugins: plugins(parser),
        abnf: abnf(parser),
    }
}

/// Every token identity the instance knows, ascending by tin.
///
/// The TypeScript engine keeps one bidirectional token map to read this
/// from. The Rust engine spreads the same information over the built-in
/// range plus the fixed, match and named-identity tables, so they are
/// merged here.
pub(crate) fn tokens(parser: &Tabnas) -> Vec<DebugTokenInfo> {
    let mut tins: IndexSet<Tin> = (1..TIN_MAX).collect();
    tins.extend(parser.options.fixed.tokens.values().map(|token| token.tin));
    tins.extend(parser.options.match_tokens.values().map(|token| token.tin));
    tins.extend(parser.options.tokens.values().copied());

    let mut tins: Vec<Tin> = tins.into_iter().collect();
    tins.sort_unstable();
    tins.into_iter()
        .map(|tin| DebugTokenInfo {
            tin,
            name: parser.token_name(tin),
            fixed: parser.fixed_source(tin).map(str::to_string),
        })
        .collect()
}

/// The named token sets, by name, each with its members ascending.
///
/// The Rust engine holds token sets in an unordered map, so both levels
/// are sorted — as the Go port sorts — rather than left to iteration
/// order.
pub(crate) fn token_sets(parser: &Tabnas) -> Vec<DebugTokenSet> {
    let mut names: Vec<&String> = parser.options.token_set.keys().collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let mut tins = parser.options.token_set[name].clone();
            tins.sort_unstable();
            DebugTokenSet {
                name: name.clone(),
                tins,
            }
        })
        .collect()
}

/// The structured form of one alternate.
pub(crate) fn alt_info(parser: &Tabnas, alt: &AltSpec) -> DebugAltInfo {
    let seq = alt
        .s
        .iter()
        .map(|position| match position.len() {
            // A position with no token constraint accepts anything; the
            // canonical runtimes render it as the empty string.
            0 => DebugSeqItem::One(String::new()),
            1 => DebugSeqItem::One(parser.token_name(position[0])),
            _ => DebugSeqItem::Any(position.iter().map(|tin| parser.token_name(*tin)).collect()),
        })
        .collect();

    DebugAltInfo {
        seq,
        push: target_name(
            alt.p.as_deref(),
            alt.p_fn.is_some() || alt.p_match.is_some(),
        ),
        replace: target_name(
            alt.r.as_deref(),
            alt.r_fn.is_some() || alt.r_match.is_some(),
        ),
        // The TypeScript alt distinguishes "no `b`" from `b: 0`; the Rust
        // and Go alts do not, so a zero push-back is omitted in both.
        back: (0 != alt.b).then_some(alt.b),
        counters: (!alt.n.is_empty()).then(|| {
            alt.n
                .iter()
                .map(|(name, value)| (name.clone(), *value))
                .collect()
        }),
        groups: groups(alt),
        action: !alt.a.is_empty()
            || !alt.action_fns.is_empty()
            || !alt.matched_action_fns.is_empty(),
        cond: !alt.c.is_empty()
            || alt.c_ref.is_some()
            || alt.c_fn.is_some()
            || alt.c_match.is_some()
            || alt.c_lex.is_some()
            || alt.c_lex_match.is_some(),
        modifier: alt.h.is_some() || alt.h_match.is_some(),
    }
}

/// A named push/replace target, or `<fn>` for a function-valued one.
fn target_name(name: Option<&str>, has_fn: bool) -> Option<String> {
    match name {
        Some(name) => Some(name.to_string()),
        None if has_fn => Some("<fn>".to_string()),
        None => None,
    }
}

/// An alternate's group tags. The engine stores them as one comma string.
pub(crate) fn groups(alt: &AltSpec) -> Vec<String> {
    alt.g
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

/// The distinct push/replace rule targets of a rule's open/close alts.
pub(crate) fn rule_edges(spec: &RuleSpec) -> DebugRuleEdges {
    DebugRuleEdges {
        name: spec.name.clone(),
        open_push: edges(&spec.open, Step::Push),
        open_replace: edges(&spec.open, Step::Replace),
        close_push: edges(&spec.close, Step::Push),
        close_replace: edges(&spec.close, Step::Replace),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Step {
    Push,
    Replace,
}

/// The distinct targets of one step across a list of alternates, in
/// first-seen order.
pub(crate) fn edges(alts: &[AltSpec], step: Step) -> Vec<String> {
    let mut seen: IndexSet<String> = IndexSet::new();
    for alt in alts {
        let (name, has_fn) = match step {
            Step::Push => (
                alt.p.as_deref(),
                alt.p_fn.is_some() || alt.p_match.is_some(),
            ),
            Step::Replace => (
                alt.r.as_deref(),
                alt.r_fn.is_some() || alt.r_match.is_some(),
            ),
        };
        if let Some(target) = target_name(name, has_fn) {
            seen.insert(target);
        }
    }
    seen.into_iter().collect()
}

/// The custom lexer matchers, in priority order.
pub(crate) fn lexer(parser: &Tabnas) -> Vec<DebugLexMatcher> {
    let mut matchers: Vec<&tabnas::LexMatcher> = parser.options.lex.matchers.values().collect();
    matchers.sort_by(|left, right| {
        left.order
            .partial_cmp(&right.order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
    });
    matchers
        .into_iter()
        .map(|matcher| DebugLexMatcher {
            order: matcher.order as i64,
            matcher: matcher.name.clone(),
            make: String::new(),
        })
        .collect()
}

/// The key parser settings.
pub(crate) fn config(parser: &Tabnas) -> DebugConfigInfo {
    let options = &parser.options;
    DebugConfigInfo {
        start: options.rule.start.clone(),
        finish: options.rule.finish,
        safe_key: options.safe.key,
        lex: lex_flags(parser),
    }
}

/// The built-in lexer enable flags, in the canonical order the
/// TypeScript `describe()` prints them.
pub(crate) fn lex_flags(parser: &Tabnas) -> IndexMap<String, bool> {
    let options = &parser.options;
    [
        ("fixed", options.fixed.lex),
        ("space", options.space.lex),
        ("line", options.line.lex),
        ("text", options.text.lex),
        ("number", options.number.lex),
        ("comment", options.comment.lex),
        ("string", options.string.lex),
        ("value", options.value.lex),
    ]
    .into_iter()
    .map(|(name, enabled)| (name.to_string(), enabled))
    .collect()
}

/// The applied plugins, in application order, each with its option bag
/// when the instance holds one.
pub(crate) fn plugins(parser: &Tabnas) -> Vec<DebugPluginInfo> {
    parser
        .installed_plugins()
        .into_iter()
        .map(|plugin| {
            let options = parser
                .plugin_options(&plugin.name)
                .filter(|value| !matches!(value, tabnas::Value::Undefined))
                .map(|value| value.to_json());
            DebugPluginInfo {
                name: plugin.name,
                options,
            }
        })
        .collect()
}
