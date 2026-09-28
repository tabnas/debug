/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! Emit an ABNF representation of an instance's *live* grammar.
//!
//! This reads ONLY the running engine (options + installed rule specs); it
//! never depends on `@tabnas/abnf`. That independence is a hard constraint
//! — the emitter is the inverse of abnf's forward encoding, and proving it
//! with abnf as a dependency would be circular.
//!
//! The mapping: tabnas rules become ABNF productions, OPEN alts become
//! `/`-separated alternatives, the token sequence (`s`) plus any
//! push/replace target (`p`/`r`) becomes a space-separated element list,
//! and each token resolves to an ABNF terminal via the fixed-literal /
//! match-regex config.
//!
//! Actions carry no ABNF meaning and are ignored. Constructs that cannot
//! be represented (an arbitrary match regex, say) are emitted as ABNF
//! comments, so the output stays valid and self-documenting even though
//! such rules will not round-trip.
//!
//! A repetition is read by SHAPE, not by name. Since tabnas/bnf#80 the
//! BNF compiler (which abnf, ebnf and gbnf compile through) emits every
//! `*A` as a replace loop: a helper `H` whose first open alternative is
//! the entry `{ c: [n.rep == 0], n: {rep: 1}, r: H }` — it consumes
//! nothing, pushes nothing and replaces the rule with itself under the
//! guard, allocating the node and counting the iteration — followed by
//! the continue alternatives that take one item and come back to `H`,
//! and by the exits (a FOLLOW peek `{ s: FOLLOW, b: 1 }` and `{ }`).
//! The guard is part of the shape: `s`, `b`, `p` and `r` alone also
//! describe a user rule's own non-consuming self-replace, a guarded or
//! counted state transition, which is no repetition. Such a rule is
//! rendered wherever it is referenced as `*A` / `*( a b )`, and neither
//! it nor its `H$alt0` / `H$alt0$step1` iteration helpers — the rules
//! named `H$…`, and only those — is emitted as a production. The older
//! push chain (`H = A H / ε`, one frame per item) carries no such entry
//! and renders as before, as a kept production, wherever it sits, inside
//! a loop's iteration included.
//!
//! Ported from `ts/src/debug.ts` (`emitAbnf` and friends), which is
//! canonical — except for the repeat loop above, where THIS PORT LEADS:
//! the canonical TypeScript and the Go port still list the loop's entry
//! as one of the rule's own alternatives and follow later. See
//! `docs/reference.md`, "The repeat loop: the Rust port leads".

use std::collections::BTreeSet;

use indexmap::{IndexMap, IndexSet};
use tabnas::{AltSpec, CompareOp, Condition, RuleSpec, Tabnas, Tin, Value};

/// Map engine rule and token names onto legal ABNF rule names.
///
/// RFC 5234: `rulename = ALPHA *(ALPHA / DIGIT / "-")`. Engine names are
/// not so constrained — the abnf compiler synthesises `_gen1_star_term`
/// and `…$alt0`, and a regex token arrives as `RX___U0030__U0039`.
/// Emitted verbatim those are rejected by every conforming ABNF tool, so
/// each is mapped to a legal name ONCE and reused for the production head
/// and every reference.
///
/// Distinct source names can sanitise to the same string (`a_b` and `a-b`
/// both give `a-b`), so collisions get a numeric suffix — without it the
/// grammar would silently merge two rules.
///
/// RULES AND TOKENS ARE SEPARATE NAMESPACES, which is what the two caches
/// are for. A token's bare name and a rule's name come from different
/// sources and can coincide: `#NR` beside a rule called `NR` is ordinary.
/// One shared cache merged them — the token looked up `NR`, hit the entry
/// `new` had made for the RULE, and the emitter wrote the rule's own name
/// for a terminal. A grammar whose rule `NR` referenced token `#NR` came
/// out as a self-referential `NR = NR` plus a second `NR = <number>`
/// definition: two definitions of one rule with `=` rather than
/// incremental `=/`, and a production that recognises nothing.
///
/// Keeping the caches apart while sharing `taken` is the fix. A token
/// still sanitises the same way, but allocates against everything already
/// claimed, so it suffixes to `NR-2` instead of borrowing the rule's name.
struct AbnfNamer {
    rule_cache: IndexMap<String, String>,
    token_cache: IndexMap<String, String>,
    /// Claimed names, lowercased. RFC 5234 §2.1: "ABNF rule names are
    /// case-insensitive", so `Foo-Bar` and `foo-bar` ARE the same rule and
    /// the collision check has to fold case — comparing exact spellings
    /// would let a sanitised `foo_bar` land beside a reserved `Foo-Bar`
    /// and emit two definitions of one rule.
    taken: BTreeSet<String>,
}

fn is_legal_abnf_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

impl AbnfNamer {
    /// Seed with names that are already legal, claiming each up front:
    /// otherwise a sanitised synthetic could take `foo-bar` first and
    /// rename the user's real `foo-bar` rule out from under them.
    fn new(reserve: impl IntoIterator<Item = String>) -> Self {
        let mut namer = Self {
            rule_cache: IndexMap::new(),
            token_cache: IndexMap::new(),
            taken: BTreeSet::new(),
        };
        for name in reserve {
            if is_legal_abnf_name(&name) && !namer.is_taken(&name) {
                namer.claim(&name);
                namer.rule_cache.insert(name.clone(), name);
            }
        }
        namer
    }

    fn claim(&mut self, name: &str) {
        self.taken.insert(name.to_lowercase());
    }

    fn is_taken(&self, name: &str) -> bool {
        self.taken.contains(&name.to_lowercase())
    }

    /// A rule name. Already-legal names were claimed by `new`, so this is
    /// a cache hit for every user-authored rule.
    fn rule(&mut self, name: &str) -> String {
        if let Some(hit) = self.rule_cache.get(name) {
            return hit.clone();
        }
        let out = self.allocate(name);
        self.rule_cache.insert(name.to_string(), out.clone());
        out
    }

    /// A token's bare name. A rule may already hold this spelling, and the
    /// token must not borrow it, so this allocates rather than reading the
    /// rule cache.
    fn token(&mut self, bare: &str) -> String {
        if let Some(hit) = self.token_cache.get(bare) {
            return hit.clone();
        }
        let out = self.allocate(bare);
        self.token_cache.insert(bare.to_string(), out.clone());
        out
    }

    /// Sanitise to a legal rulename, then make it unique against every
    /// name claimed so far — reserved rule names and previously allocated
    /// names alike, since `taken` is shared by both namespaces.
    fn allocate(&mut self, name: &str) -> String {
        let mut out: String = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '-'
                }
            })
            .collect();
        if !out
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            out = format!("r{out}");
        }
        if self.is_taken(&out) {
            let mut suffix = 2;
            while self.is_taken(&format!("{out}-{suffix}")) {
                suffix += 1;
            }
            out = format!("{out}-{suffix}");
        }
        self.claim(&out);
        out
    }
}

/// Emit the ABNF for `parser`'s live grammar.
pub fn abnf(parser: &Tabnas) -> String {
    Emitter::new(parser).emit()
}

struct Emitter<'a> {
    parser: &'a Tabnas,
    /// Rule name -> spec, in installation order (the engine's `IndexMap`),
    /// which is the TypeScript insertion order the emitter's rule ordering
    /// depends on.
    rules: IndexMap<String, &'a RuleSpec>,
    namer: AbnfNamer,
    /// Token legend, in first-reference order.
    used: IndexMap<String, String>,
    end_tin: Option<Tin>,
    /// `bnf` wraps grammars in a synthetic `__start__` rule; when present
    /// it is skipped and the real start leads.
    synth_wrapper: Option<String>,
    /// The repeat loops: every rule with a loop entry (see
    /// [`is_loop_entry`]). Decided by shape, the whole of it, guard
    /// included, so a hand-built loop and a compiled one read the same
    /// and a user rule's own self-replace does not.
    loops: BTreeSet<String>,
    /// The synthetic rules a loop's iteration runs through (`H$alt0`,
    /// `H$alt0$step1`): the rules named `H$…`, and only those. Inlined
    /// into the repetition wherever the loop is referenced, never
    /// productions.
    loop_helpers: BTreeSet<String>,
}

impl<'a> Emitter<'a> {
    fn new(parser: &'a Tabnas) -> Self {
        let rules: IndexMap<String, &RuleSpec> = parser
            .rule_specs()
            .into_iter()
            .map(|spec| (spec.name.clone(), spec))
            .collect();
        let synth_wrapper =
            ("__start__" == parser.options.rule.start).then(|| parser.options.rule.start.clone());
        let loops: BTreeSet<String> = rules
            .iter()
            .filter(|(name, spec)| spec.open.iter().any(|alt| is_loop_entry(alt, name)))
            .map(|(name, _)| name.clone())
            .collect();
        let mut emitter = Self {
            namer: AbnfNamer::new(rules.keys().cloned()),
            rules,
            used: IndexMap::new(),
            end_tin: parser.options.token("#ZZ"),
            synth_wrapper,
            loops,
            loop_helpers: BTreeSet::new(),
            parser,
        };
        emitter.loop_helpers = emitter.find_loop_helpers();
        emitter
    }

    /// The iteration helpers of every loop `H`: the rules named `H$…`
    /// (`H$alt0`, pushing the item, and `H$alt0$step1`, replacing with
    /// `H`), and nothing else. They were once found by reachability from
    /// the continue alternatives, stopping at user rules and other loops,
    /// which reached through a foldable group the loop repeats to any
    /// synthetic production it references: the old push-chain star of
    /// `*( B *A C )` was folded away with its epsilon branch and its back
    /// edge, and `*( B A C )` came out. A group inside the iteration is
    /// inlined by [`Emitter::is_foldable`] on its own account; a kept
    /// production stays a bareword reference.
    fn find_loop_helpers(&self) -> BTreeSet<String> {
        let mut helpers = BTreeSet::new();
        for name in &self.loops {
            let prefix = format!("{name}$");
            helpers.extend(
                self.rules
                    .keys()
                    .filter(|rule| rule.starts_with(&prefix) && !self.loops.contains(*rule))
                    .cloned(),
            );
        }
        helpers
    }

    /// A rule the abnf forward-compiler synthesised for a `[...]` /
    /// `*(...)` / `1*(...)` / group / chain-step: named `_gen<n>_…` or
    /// carrying a `$`. These are never user-authored, so instead of
    /// emitting them as their own productions each is folded back into the
    /// ABNF construct it encodes — a reasonable round-trip (`tn.abnf(G)`
    /// then `abnf()` reproduces `G`, not the expanded internal form).
    fn is_synthetic(&self, name: &str) -> bool {
        if Some(name) == self.synth_wrapper.as_deref() {
            return false;
        }
        name.contains('$') || is_gen_name(name)
    }

    /// Only the clean cases fold — `[…]` optionals plus the group / chain
    /// helpers they inline through. A repetition in the old push-chain
    /// shape (`_star` / `_plus` and their `$alt…` helpers) does not
    /// reconstruct reliably, so those rules are emitted as productions
    /// unchanged — still a valid, recognition-equivalent grammar. A
    /// repetition in the loop shape is not decided here: see
    /// [`Emitter::is_folded`]. The kind is read from the rule's own
    /// segment ([`gen_kind`]), so a helper named after a repetition it
    /// merely contains is judged by what it is.
    fn is_foldable(&self, name: &str) -> bool {
        if !self.is_synthetic(name) || gen_kind(name) == Some("star") || name.contains("$alt") {
            return false;
        }
        gen_kind(name) != Some("plus") || self.plus_folds(name)
    }

    /// `1*A` compiles to a `_plus` helper: `A` followed by the star of `A`.
    /// The helper folds to `A *A` when that star is a loop and nothing
    /// else it reaches is a kept production; with an old-shape star, a
    /// kept production, the helper stays a production too, as it always
    /// has. Reaches through the chain steps (`_plus$step1`) a non-terminal
    /// item puts between the helper and its star.
    fn plus_folds(&self, name: &str) -> bool {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut pending: Vec<String> = vec![name.to_owned()];
        while let Some(current) = pending.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            let Some(spec) = self.rules.get(&current) else {
                continue;
            };
            for alt in spec.open.iter().chain(spec.close.iter()) {
                let Some(target) = alt.p.as_deref().or(alt.r.as_deref()) else {
                    continue;
                };
                if !self.rules.contains_key(target) || !self.is_synthetic(target) {
                    continue;
                }
                if self.loops.contains(target) || self.loop_helpers.contains(target) {
                    continue;
                }
                if gen_kind(target) == Some("star") || target.contains("$alt") {
                    return false;
                }
                pending.push(target.to_owned());
            }
        }
        true
    }

    /// A rule rendered where it is referenced and never as a production
    /// of its own: a foldable synthetic, a synthetic loop, or a loop's
    /// iteration helper. A loop that is a USER rule keeps its production
    /// (its body is the repetition) and is referenced by name, so a
    /// grammar whose start rule is a loop still has a start production.
    fn is_folded(&self, name: &str) -> bool {
        self.is_foldable(name)
            || (self.loops.contains(name) && self.is_synthetic(name))
            || self.loop_helpers.contains(name)
    }

    fn emit(mut self) -> String {
        let start_rule = self.start_rule();

        // Real start first, then the remaining USER (non-synthetic) rules.
        let user_rules: Vec<String> = self
            .rules
            .keys()
            .filter(|name| {
                Some(name.as_str()) != self.synth_wrapper.as_deref() && !self.is_folded(name)
            })
            .cloned()
            .collect();

        let mut ordered: Vec<String> = Vec::new();
        let mut seen_rules: BTreeSet<String> = BTreeSet::new();
        if let Some(start) =
            start_rule.filter(|start| self.rules.contains_key(start) && !self.is_folded(start))
        {
            seen_rules.insert(start.clone());
            ordered.push(start);
        }
        for name in user_rules {
            if seen_rules.insert(name.clone()) {
                ordered.push(name);
            }
        }

        let mut lines: Vec<String> = Vec::new();
        for name in ordered {
            let seen = BTreeSet::from([name.clone()]);
            let body = self.emit_body(&name, &seen);
            let head = self.namer.rule(&name);
            lines.push(format!("{head} = {body}"));
        }

        // Define each token as its own ABNF rule (named terminals), after
        // the productions, with `=` aligned for readability.
        if !self.used.is_empty() {
            let pad = self.used.keys().map(String::len).max().unwrap_or(0);
            lines.push(String::new());
            for (name, form) in &self.used {
                lines.push(format!("{name:pad$} = {form}"));
            }
        }
        lines.join("\n")
    }

    /// The grammar's real start rule, seeing through a `__start__` wrapper.
    fn start_rule(&self) -> Option<String> {
        let Some(wrapper) = self.synth_wrapper.clone() else {
            return Some(self.parser.options.rule.start.clone());
        };
        let spec = self.rules.get(&wrapper)?;
        for alt in &spec.open {
            if let Some(target) = alt.p.as_ref().or(alt.r.as_ref()) {
                return Some(target.clone());
            }
        }
        None
    }

    /// Full production body: open alternatives joined by `/`, then any
    /// close continuation — but PRESERVING an empty open alternative,
    /// which is essential for kept `*(…)` repetition rules, whose empty
    /// alt is what makes them zero-or-more.
    ///
    /// The empty alternative is rendered by wrapping the rest in `[ … ]`,
    /// NOT as a trailing `/`. ABNF's grammar is
    ///   alternation = concatenation *(*c-wsp "/" *c-wsp concatenation)
    /// so every `/` must be followed by a concatenation: `x = A x /` is a
    /// syntax error that every conforming ABNF tool rejects. `[ A x ]`
    /// says the same thing and is valid.
    fn emit_body(&mut self, name: &str, seen: &BTreeSet<String>) -> String {
        // A user rule that is a loop: its production IS the repetition.
        if self.loops.contains(name) {
            return self.repetition(name, seen);
        }
        let opens: Vec<AltSpec> = self
            .rules
            .get(name)
            .map(|spec| spec.open.clone())
            .unwrap_or_default();
        let raw: Vec<String> = opens.iter().map(|alt| self.seq_of_alt(alt, seen)).collect();
        let optional = raw.iter().any(String::is_empty);
        let non_empty: IndexSet<String> = raw.into_iter().filter(|item| !item.is_empty()).collect();
        let cont = self.close_cont(name, seen);

        // Nothing but an empty alternative. `option = "[" *c-wsp
        // alternation *c-wsp "]"` and `alternation` needs at least one
        // concatenation, so `[ ]` is not a legal option — there is nothing
        // to make optional.
        //
        // With a continuation, the open contributes nothing and the rule
        // IS its continuation. Without one, `x = ` is not a production at
        // all, and ABNF has no epsilon terminal — but an empty char-val is
        // legal (`char-val = DQUOTE *(%x20-21 / %x23-7E) DQUOTE` permits
        // zero chars) and matches exactly the empty string, which is what
        // this rule does.
        if optional && non_empty.is_empty() {
            return if cont.is_empty() {
                "\"\"".to_string()
            } else {
                cont
            };
        }

        let joined = non_empty.into_iter().collect::<Vec<_>>().join(" / ");
        let body = if optional {
            format!("[ {joined} ]")
        } else {
            joined
        };
        format!("{body} {cont}").trim().to_string()
    }

    /// Open alternatives joined by `/`, then any close continuation. Unlike
    /// [`Emitter::emit_body`] an empty open alternative is dropped, because
    /// an inlined construct contributes only its content.
    fn rule_seq(&mut self, name: &str, seen: &BTreeSet<String>) -> String {
        let opens = self.content_opens(name);
        let alts: IndexSet<String> = opens
            .iter()
            .map(|alt| self.seq_of_alt(alt, seen))
            .filter(|item| !item.is_empty())
            .collect();
        let joined = alts.into_iter().collect::<Vec<_>>().join(" / ");
        let cont = self.close_cont(name, seen);
        format!("{joined} {cont}").trim().to_string()
    }

    /// The close-alt continuation of a rule: its trailing element
    /// sequence, wrapped in `[ … ]` when an epsilon (empty) close alt
    /// makes it optional.
    fn close_cont(&mut self, name: &str, seen: &BTreeSet<String>) -> String {
        let closes: Vec<AltSpec> = self
            .rules
            .get(name)
            .map(|spec| spec.close.clone())
            .unwrap_or_default();
        let has_epsilon = closes
            .iter()
            .any(|alt| !self.is_end_alt(alt) && !has_content(alt, name));
        for alt in &closes {
            if self.is_end_alt(alt) || !has_content(alt, name) {
                continue;
            }
            let cont = self.seq_of_alt(alt, seen);
            if cont.is_empty() {
                continue;
            }
            return if has_epsilon {
                format!("[ {cont} ]")
            } else {
                cont
            };
        }
        String::new()
    }

    /// An alt whose whole sequence is the single end-of-source token.
    fn is_end_alt(&self, alt: &AltSpec) -> bool {
        let Some(end) = self.end_tin else {
            return false;
        };
        1 == alt.s.len() && 1 == alt.s[0].len() && end == alt.s[0][0]
    }

    fn content_opens(&self, name: &str) -> Vec<AltSpec> {
        self.rules
            .get(name)
            .map(|spec| {
                spec.open
                    .iter()
                    .filter(|alt| has_content(alt, name))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A loop, as a repetition of its iteration: `*A` when the iteration
    /// is one element, `*( a b )` otherwise. The iteration is the ` / `-
    /// joined rendering of the continue alternatives — the open
    /// alternatives with content — and its back edges render nothing: the
    /// loop is in `seen`, so `r: H` (from a terminal continue, or from
    /// `H$alt0$step1` at the end of a ref continue) terminates like any
    /// other loop-back. The entry, `{ r: H }` consuming nothing, and the
    /// exits, `{ s: FOLLOW, b: 1 }` and `{ }`, have no content and are
    /// skipped: they are bookkeeping, not syntax. Any close continuation
    /// of the loop rule runs once, after the last item, and follows the
    /// repetition.
    fn repetition(&mut self, name: &str, seen: &BTreeSet<String>) -> String {
        let opens = self.content_opens(name);
        let parts: IndexSet<String> = opens
            .iter()
            .map(|alt| self.seq_of_alt(alt, seen))
            .filter(|item| !item.is_empty())
            .collect();
        let iteration = repeat_of(&parts.into_iter().collect::<Vec<_>>());
        let cont = self.close_cont(name, seen);
        format!("{iteration} {cont}").trim().to_string()
    }

    /// Render one alt as an ABNF element sequence: its `s` tokens then its
    /// `p`/`r` target (synthetic targets are inlined).
    ///
    /// An alt consumes `len(s) - b` tokens — the engine records "matched
    /// minus backtrack". Tokens beyond that are LOOKAHEAD only: matched to
    /// choose the alt, then pushed back. Rendering them as ABNF elements
    /// would claim input the alt never eats.
    ///
    /// Deriving the count this way also covers the FIRST-set-guarded
    /// epsilon — `{ s: '#Y', b: 1 }` with no target, which the abnf
    /// compiler emits for the skip branch of an optional, where `#Y` is
    /// the FOLLOW token. Rendered as a consuming alternative,
    /// `top = [ X "@" ] Y` came back as `top = [ X T / Y ] Y`: the
    /// optional could swallow the follow.
    fn seq_of_alt(&mut self, alt: &AltSpec, seen: &BTreeSet<String>) -> String {
        let mut elements: Vec<String> = Vec::new();
        let keep = alt.s.len().saturating_sub(alt.b);
        for position in alt.s.iter().take(keep) {
            match position.len() {
                0 => continue,
                1 => {
                    let tin = position[0];
                    if Some(tin) == self.end_tin {
                        continue;
                    }
                    let terminal = self.terminal(tin);
                    elements.push(terminal);
                }
                _ => {
                    let inner: Vec<String> = position
                        .iter()
                        .filter(|tin| Some(**tin) != self.end_tin)
                        .copied()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .map(|tin| self.terminal(tin))
                        .collect();
                    if !inner.is_empty() {
                        elements.push(format!("( {} )", inner.join(" / ")));
                    }
                }
            }
        }
        let target = alt.p.clone().or_else(|| alt.r.clone());
        if let Some(target) = target {
            let reference = self.inline_ref(&target, seen);
            if !reference.is_empty() {
                elements.push(reference);
            }
        }
        elements.join(" ")
    }

    /// Inline a reference: a user rule stays a bareword; a synthetic rule
    /// folds back into the ABNF construct it encodes.
    fn inline_ref(&mut self, name: &str, seen: &BTreeSet<String>) -> String {
        if self.loops.contains(name) {
            if seen.contains(name) {
                // The back edge out of the loop's own iteration.
                return String::new();
            }
            if !self.is_synthetic(name) {
                // A user rule that is a loop keeps its production.
                return self.namer.rule(name);
            }
            let mut inner = seen.clone();
            inner.insert(name.to_string());
            return self.repetition(name, &inner);
        }
        // A user rule, or a kept (non-foldable, e.g. old-shape repetition)
        // synthetic rule, stays a bareword reference; only foldable
        // synthetics and a loop's iteration helpers inline.
        if !self.is_foldable(name) && !self.loop_helpers.contains(name) {
            return self.namer.rule(name);
        }
        if seen.contains(name) {
            // A foldable loop-back — returning empty terminates the loop.
            return String::new();
        }
        if !self.rules.contains_key(name) {
            return self.namer.rule(name);
        }
        let mut inner = seen.clone();
        inner.insert(name.to_string());
        // The optional's own helper, and only that: a star over an
        // optional is named after it (`_gen3_star__gen2_opt__gen1_group`),
        // and so are its iteration helpers, and a substring test for
        // `_opt` wrapped each of those in `[ … ]` too — `*[ [ T ] [  ] ]`,
        // with the step, whose only content is the back edge, as an empty
        // option, which RFC 5234 has no room for. (The canonical
        // TypeScript still tests the substring; it never inlined those
        // names, so it never met them. It follows with the loop.)
        if is_helper(name, "opt") {
            let body = self.rule_seq(name, &inner);
            return format!("[ {body} ]");
        }
        let body = self.rule_seq(name, &inner);
        if is_helper(name, "plus") || is_helper(name, "rep") {
            if let Some(counted) = self.counted_repetition(name, &body, &inner) {
                return counted;
            }
        }
        // group / chain-step: inline the body, parenthesising a bare
        // multi-way alternation that will sit inside a larger sequence.
        let multi = 1 < self.content_opens(name).len();
        if multi && self.close_cont(name, &inner).is_empty() {
            format!("( {body} )")
        } else {
            body
        }
    }

    /// `1*A` compiles to a `_plus` helper that is `A` followed by the star
    /// of `A`, and `n*A` to a `_rep` helper that is `A` `n` times followed
    /// by it. Rendered element by element those read `A *A` and `A A *A`:
    /// the same language as `1*A` and `2*A`, but not the same recogniser
    /// once recompiled. The abnf crate compiles `A *A` and `1*A`
    /// differently, and where `A` is nullable, or its FIRST meets its
    /// FOLLOW, the recompiled `A *A` rejects inputs the original accepts:
    /// `1*( [ "+" "e" ] )` on `+e`, `1*item` with `item = "]" "e" / [ "d" ]`
    /// on `]e`. So a helper whose body is the item of the loop it ends in,
    /// `n` times, then that loop's repetition is written back as the
    /// repetition it was compiled from: `1*A`, `1*[ A ]`, `2*( a b )`. Any
    /// other body renders as it is: a bounded `2*4A` compiles to nested
    /// optionals and ends in no loop, and a hand-built rule that merely
    /// carries the name is whatever it says.
    fn counted_repetition(
        &mut self,
        name: &str,
        body: &str,
        seen: &BTreeSet<String>,
    ) -> Option<String> {
        let loop_name = self.loop_after(name)?;
        let mut inner = seen.clone();
        inner.insert(loop_name.clone());
        let repetition = self.repetition(&loop_name, &inner);
        // The item as the repetition wrote it: `A`, `[ A ]`, `( A / B )`,
        // and for `*( a b )` the bare sequence `a b` the group inlined to.
        let one = repetition.strip_prefix('*')?;
        let mut items = vec![one];
        if let Some(inside) = one.strip_prefix("( ").and_then(|s| s.strip_suffix(" )")) {
            items.push(inside);
        }
        for item in items {
            let prefix = format!("{item} ");
            let mut count = 0usize;
            let mut rest = body;
            while let Some(next) = rest.strip_prefix(&prefix) {
                count += 1;
                rest = next;
            }
            if 0 < count && rest == repetition {
                return Some(format!("{count}{repetition}"));
            }
        }
        None
    }

    /// The loop a `_plus` / `_rep` helper ends in: the open target of the
    /// last rule of its chain (`H`, `H$step1`, … linked by their close
    /// replaces), when that target is a loop. The chain is followed by its
    /// close edges only, never into the pushed item, which may hold a
    /// loop of its own.
    fn loop_after(&self, name: &str) -> Option<String> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut current = name.to_string();
        loop {
            if !visited.insert(current.clone()) {
                return None;
            }
            let spec = self.rules.get(&current)?;
            match spec.close.iter().find_map(|alt| alt.r.clone()) {
                Some(next) if self.rules.contains_key(&next) => current = next,
                _ => break,
            }
        }
        let spec = self.rules.get(&current)?;
        spec.open
            .iter()
            .find_map(|alt| alt.p.clone().or_else(|| alt.r.clone()))
            .filter(|target| self.loops.contains(target))
    }

    /// Render a token reference: every token appears by its bare NAME
    /// (`#PL` -> `PL`), and its definition is recorded in the legend. A
    /// token name that is actually a rule name is a nonterminal reference
    /// and is returned as-is, with no legend entry.
    fn terminal(&mut self, tin: Tin) -> String {
        let full_name = self.parser.token_name(tin);
        if self.rules.contains_key(&full_name) {
            return self.namer.rule(&full_name);
        }
        let bare = full_name
            .strip_prefix('#')
            .unwrap_or(&full_name)
            .to_string();
        // Strip the '#' sigil first so '#NR' asks for 'NR' rather than
        // being sanitised to '-NR' and then prefixed.
        //
        // This goes through the TOKEN namespace. A rule may already hold
        // this spelling — a grammar with a rule `NR` and the `#NR` number
        // token is perfectly ordinary — and the token must not borrow it:
        // that emitted a self-referential `NR = NR` plus a duplicate
        // definition. The token namespace suffixes to `NR-2` instead.
        let name = self.namer.token(&bare);
        if !self.used.contains_key(&name) {
            let form = token_form(self.parser, tin, &full_name);
            self.used.insert(name.clone(), form);
        }
        name
    }
}

/// A rule the abnf forward-compiler synthesised, named `_gen<n>_…`.
fn is_gen_name(name: &str) -> bool {
    name.strip_prefix("_gen")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| ch.is_ascii_digit())
}

/// The construct a synthesised name encodes — `opt`, `group`, `star`,
/// `plus`, `rep` — read from the rule's OWN segment: the word after
/// `_gen<n>_` in the part before any `$`. A repetition's helper is named
/// after its item, so `_gen3_star__gen2_opt__gen1_group` is the star over
/// the optional over the group, and its iteration helpers
/// `…$alt0` / `…$alt0$step1` carry the whole of that name; a substring
/// test for `_opt` reached all of them. A chain step (`_gen1_group$step1`)
/// answers with the kind of the rule it continues.
fn gen_kind(name: &str) -> Option<&str> {
    let own = name.split('$').next().unwrap_or(name);
    let rest = own.strip_prefix("_gen")?;
    let digits = rest.len()
        - rest
            .trim_start_matches(|ch: char| ch.is_ascii_digit())
            .len();
    if digits == 0 {
        return None;
    }
    let rest = rest[digits..].strip_prefix('_')?;
    let kind = rest.split('_').next().unwrap_or(rest);
    (!kind.is_empty()).then_some(kind)
}

/// `name` is the helper of `kind` itself — not a chain step or an
/// iteration helper of it, which carry a `$`.
fn is_helper(name: &str, kind: &str) -> bool {
    !name.contains('$') && gen_kind(name) == Some(kind)
}

/// An alt that contributes something to the emitted sequence of `rule`,
/// decided by what it CONSUMES: it eats a token (`len(s) - b > 0`), or
/// pushes a rule, or replaces with a rule — unless it is `rule`'s own
/// loop entry ([`is_loop_entry`]). `{ }`, the FOLLOW peek
/// `{ s: FOLLOW, b: 1 }` and the entry `{ c: [n.rep == 0], n: {rep: 1},
/// r: rule }` are all epsilon: none of them moves the parse past any
/// input. Counting the peek as content rendered a follow-guarded exit
/// as a consuming alternative, and counting the entry rendered a loop
/// as one of its own alternatives. Every other replace with `rule`
/// itself is content, as it always was: the close `{ s: A, b: 1, r: rule }`
/// after an open that consumed `A` is the `[ rule ]` of
/// `rule = A [ rule ]`, and calling it empty for the self-replace alone
/// emitted `rule = A`, exactly one where the rule takes one or more.
fn has_content(alt: &AltSpec, rule: &str) -> bool {
    !is_loop_entry(alt, rule) && (alt.s.len() > alt.b || alt.p.is_some() || alt.r.is_some())
}

/// A repeat loop's entry, the whole of the compiler's shape: the
/// alternative consumes nothing, pushes nothing, replaces `rule` with
/// itself, is guarded by `n.rep == 0` and sets that counter to 1 —
/// allocating the node and counting the iteration on the way in. It is
/// what marks a rule as a loop. `s`, `b`, `p` and `r` alone are not
/// enough: a user rule's own non-consuming self-replace, a guarded or
/// counted state transition, has the same four and is no repetition, and
/// reading it as one rewrote the whole rule as `*…`, accepting empty and
/// repeated inputs the original need not.
fn is_loop_entry(alt: &AltSpec, rule: &str) -> bool {
    alt.s.len() <= alt.b
        && alt.p.is_none()
        && alt.r.as_deref() == Some(rule)
        && alt.n.get("rep") == Some(&1)
        && alt.c.iter().any(is_rep_guard)
}

/// The entry's guard, `n.rep == 0`: the counter the entry sets is still
/// at its start, so this is the first time through.
fn is_rep_guard(condition: &Condition) -> bool {
    condition.path == ["n", "rep"]
        && condition.op == CompareOp::Eq
        && matches!(condition.value, Value::Number(count) if count == 0.0)
}

/// A repetition over the ` / `-joined alternatives of an iteration:
/// `*A` and `*"a"` when the iteration is one element, `*( a b )` and
/// `*( a / b )` otherwise. A single element that is already a group or an
/// option (`( A / B )`, the rendering of an inlined multi-way group) is
/// not wrapped again. An empty iteration is an empty repetition: nothing.
fn repeat_of(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] if is_one_element(only) => format!("*{only}"),
        _ => format!("*( {} )", parts.join(" / ")),
    }
}

/// `text` is one ABNF element: a bare name or terminal, or one bracket
/// pair enclosing the whole of it. A repetition is not an element
/// (`repetition = [repeat] element`), so a nested `*I` has to be grouped:
/// `*( *I )`, never `**I`. A production body holds only names, `( … )`,
/// `[ … ]`, `*…` and `""`, so counting brackets is exact.
fn is_one_element(text: &str) -> bool {
    if text.starts_with(|ch: char| ch == '*' || ch.is_ascii_digit()) {
        return false;
    }
    if !text.contains(' ') {
        return true;
    }
    let mut chars = text.chars();
    let (Some(open), Some(close)) = (chars.next(), chars.next_back()) else {
        return false;
    };
    if !matches!((open, close), ('(', ')') | ('[', ']')) {
        return false;
    }
    // The opening bracket must be the one the last character closes.
    let mut depth = 0usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && index + ch.len_utf8() < text.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// The legend definition for a token — what it matches:
///   - fixed literal       -> `%s"<lit>"` (letters) / `"<lit>"` (punctuation)
///   - match regex         -> a char range, a literal, or an ABNF comment
///   - built-in matcher    -> `<number>` / `<string>` / …
///
/// The `%s` prefix is RFC 7405, which updates RFC 5234 — the only
/// construct emitted here that RFC 5234 alone does not define. It is kept
/// because it is what every current ABNF tool implements and it stays
/// readable; `%x48.69` would be pure RFC 5234 but unreadable, and a bare
/// char-val would silently lose case-sensitivity.
fn token_form(parser: &Tabnas, tin: Tin, full_name: &str) -> String {
    if let Some(literal) = parser.fixed_source(tin) {
        // RFC 5234: `char-val = DQUOTE *(%x20-21 / %x23-7E) DQUOTE` — so a
        // literal holding a control character, a `"`, or anything above
        // %x7E CANNOT go inside quotes. A token fixed to CRLF used to emit
        // `CRLF = "<CR>"`, an unterminated char-val. The numeric form has
        // no such restriction (and is case-sensitive already, so it
        // carries the `%s` meaning too).
        return if is_abnf_quotable_body(literal) {
            if literal.chars().any(|ch| ch.is_ascii_alphabetic()) {
                format!("%s\"{literal}\"")
            } else {
                format!("\"{literal}\"")
            }
        } else {
            numeric_val(literal)
        };
    }

    if let Some(matcher) = parser
        .options
        .match_tokens
        .values()
        .find(|matcher| matcher.tin == tin)
    {
        if let tabnas::MatchTokenMatcher::Regex(regex) = &matcher.matcher {
            return regex_to_abnf(regex.as_str());
        }
        // A function-backed matcher has no ABNF form to recover. The
        // canonical runtime only special-cases a RegExp and otherwise
        // falls through to the description below, so do the same rather
        // than inventing a form this port alone would emit.
    }

    // Built-in lexer token: describe it. It is lexer-provided, so a
    // grammar using it does not round-trip through bnf.
    let bare = full_name.strip_prefix('#').unwrap_or(full_name);
    let described = match bare {
        "NR" => "number",
        "ST" => "string",
        "TX" => "text",
        "VL" => "value",
        "SP" => "space",
        "LN" => "line",
        "CM" => "comment",
        "AA" => "any",
        "UK" => "unknown",
        "BD" => "bad",
        "ZZ" => "end-of-source",
        _ => return prose_val(&format!("built-in {bare}")),
    };
    prose_val(described)
}

/// A literal as an ABNF num-val: `%x0D`, or dot-concatenated for several
/// characters (`%x0D.0A`). RFC 5234 gives this no character restriction,
/// so it is the safe rendering for anything char-val cannot hold.
fn numeric_val(literal: &str) -> String {
    let parts: Vec<String> = literal
        .chars()
        .map(|ch| {
            let hex = format!("{:X}", ch as u32);
            if hex.len() % 2 == 1 {
                format!("0{hex}")
            } else {
                hex
            }
        })
        .collect();
    format!("%x{}", parts.join("."))
}

/// Every character of `text` fits inside an ABNF char-val: %x20-21 /
/// %x23-7E (printable ASCII except the double quote). An empty string
/// qualifies — `char-val` permits zero characters.
fn is_abnf_quotable_body(text: &str) -> bool {
    text.chars().all(|ch| {
        let code = ch as u32;
        (0x20..=0x21).contains(&code) || (0x23..=0x7E).contains(&code)
    })
}

/// An ABNF char-val holding at least one character.
fn is_abnf_quotable(text: &str) -> bool {
    !text.is_empty() && is_abnf_quotable_body(text)
}

/// Translate the anchored regex the abnf compiler installs for a match
/// token back to ABNF, covering the shapes it actually emits.
///
/// The Rust engine's matchers are `regex::Regex`, whose source carries
/// inline flags (`(?i)`) rather than the separate flag set a JavaScript
/// `RegExp` has; the case-insensitive branch reads that prefix instead.
fn regex_to_abnf(source: &str) -> String {
    let original = source;
    // Inline flags come first, then the anchor the compiler prepends.
    let (case_insensitive, source) = match source.strip_prefix("(?i)") {
        Some(rest) => (true, rest),
        None => (false, source),
    };
    let source = source.strip_prefix('^').unwrap_or(source);

    // Single char-class range: [\uXXXX-\uYYYY] -> %xXX-YY
    if let Some(range) = char_class_range(source, "\\u", 4) {
        return range;
    }
    // Single char-class range with bare hex escapes: [\xXX-\xYY].
    if let Some(range) = char_class_range(source, "\\x", 2) {
        return range;
    }

    // Case-insensitive literal: the abnf compiler encodes a bare ABNF
    // string `"foo"` containing at least one letter as an anchored,
    // case-insensitive regex over the escaped literal. Recover the literal
    // by unescaping, then verify the round-trip so a genuine regex is
    // never misread as a literal.
    if case_insensitive {
        let literal = unescape_regex_literal(source);
        if escape_regex_like(&literal) == source && is_abnf_quotable(&literal) {
            return format!("\"{literal}\"");
        }
    }

    // Anything else: no ABNF construct expresses this regex, so say so in
    // the one the grammar provides for exactly that — RFC 5234 §4
    // prose-val, "a last resort" for describing a rule in prose. It does
    // not round-trip, and is not meant to; it is a legal element naming
    // what the token matches.
    //
    // This returned `; /…/` before: a bare comment. `;` runs to end of
    // line, so the legend entry it produced (`T = ; /…/`) held no elements
    // at all — not merely non-round-tripping but unparseable, and one such
    // token made the WHOLE emitted grammar invalid rather than just that
    // rule.
    prose_val(&format!("regex /{original}/"))
}

/// Render `text` as an RFC 5234 §4 prose-val:
/// `prose-val = "<" *(%x20-3D / %x3F-7E) ">"`. A `>` would close the value
/// early and anything outside printable ASCII is not permitted, so both are
/// escaped rather than dropped — the text is here to say what the token
/// matches, and silently losing characters from it would defeat that.
fn prose_val(text: &str) -> String {
    let mut out = String::from("<");
    for ch in text.chars() {
        let cp = ch as u32;
        if (0x20..=0x3d).contains(&cp) || (0x3f..=0x7e).contains(&cp) {
            out.push(ch);
        } else {
            out.push_str(&format!("\\u{cp:04X}"));
        }
    }
    out.push('>');
    out
}

/// `[\uXXXX-\uYYYY]` or `[\xXX-\xYY]` as `%xLO-HI`.
fn char_class_range(source: &str, prefix: &str, digits: usize) -> Option<String> {
    let inner = source.strip_prefix('[')?.strip_suffix(']')?;
    let (low, rest) = inner.strip_prefix(prefix)?.split_at_checked(digits)?;
    let high = rest.strip_prefix('-')?.strip_prefix(prefix)?;
    if high.len() != digits {
        return None;
    }
    let low = u32::from_str_radix(low, 16).ok()?;
    let high = u32::from_str_radix(high, 16).ok()?;
    Some(format!("%x{low:X}-{high:X}"))
}

/// Mirror of the abnf compiler's `escapeRegExp`, used only to validate
/// that an unescaped candidate literal re-escapes to exactly the observed
/// regex source.
fn escape_regex_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if "\\^$.*+?()[]{}|".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Undo [`escape_regex_like`], plus the forward slash a JavaScript
/// `RegExp` source escapes automatically.
fn unescape_regex_literal(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.peek().copied() {
                if "\\^$.*+?()[]{}|/".contains(next) {
                    out.push(next);
                    chars.next();
                    continue;
                }
            }
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn namer_sanitises_and_disambiguates() {
        let mut namer = AbnfNamer::new(["keep-me".to_string()]);
        assert_eq!(namer.rule("keep-me"), "keep-me");
        assert_eq!(namer.rule("a_b"), "a-b");
        // `a-b` sanitises to the same string, so it gets a suffix.
        assert_eq!(namer.rule("a.b"), "a-b-2");
        // Not starting with a letter gets an `r` prefix.
        assert_eq!(namer.rule("1st"), "r1st");
        // Case-insensitive collision: `KEEP-ME` folds onto the claim.
        assert_eq!(namer.rule("KEEP-ME"), "KEEP-ME-2");
        // Memoised: the same source name always maps to the same output.
        assert_eq!(namer.rule("a_b"), "a-b");
    }

    #[test]
    fn namer_keeps_rules_and_tokens_apart() {
        // A rule `NR` and the built-in `#NR` token is an ordinary grammar.
        // The token must NOT be handed the rule's name: doing so emitted a
        // self-referential `NR = NR` plus a duplicate `NR = <number>`.
        let mut namer = AbnfNamer::new(["NR".to_string()]);
        assert_eq!(namer.rule("NR"), "NR");
        assert_eq!(namer.token("NR"), "NR-2");
        // Each namespace is memoised on its own, and neither drifts.
        assert_eq!(namer.token("NR"), "NR-2");
        assert_eq!(namer.rule("NR"), "NR");
        // A token whose name no rule claims is unaffected.
        assert_eq!(namer.token("PL"), "PL");
    }

    fn alt(s: &[Tin], b: usize, p: Option<&str>, r: Option<&str>) -> AltSpec {
        AltSpec {
            s: s.iter().map(|tin| vec![*tin]).collect(),
            b,
            p: p.map(str::to_owned),
            r: r.map(str::to_owned),
            ..Default::default()
        }
    }

    /// The compiler's loop entry: `{ c: [n.rep == 0], n: {rep: 1}, r: rule }`.
    fn entry(rule: &str) -> AltSpec {
        AltSpec {
            c: vec![Condition {
                path: vec!["n".into(), "rep".into()],
                op: CompareOp::Eq,
                value: Value::Number(0.0),
            }],
            n: HashMap::from([("rep".to_string(), 1)]),
            r: Some(rule.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn content_is_what_an_alt_consumes() {
        // Eats a token.
        assert!(has_content(&alt(&[7], 0, None, None), "H"));
        // Peeks one and gives it back: the follow-guarded exit.
        assert!(!has_content(&alt(&[7], 1, None, None), "H"));
        // Nothing at all.
        assert!(!has_content(&alt(&[], 0, None, None), "H"));
        // Pushes a rule.
        assert!(has_content(&alt(&[], 0, Some("item"), None), "H"));
        // Replaces with another rule, after a peek.
        assert!(has_content(&alt(&[7], 1, None, Some("H$alt0")), "H"));
        // The loop entry: replaces with the rule being rendered, guarded.
        assert!(!has_content(&entry("H"), "H"));
        // The same alt is content from any OTHER rule's point of view.
        assert!(has_content(&entry("H"), "H$alt0$step1"));
        // A self-replace WITHOUT the guard is not the entry and keeps its
        // content: a user rule's own state transition.
        assert!(has_content(&alt(&[], 0, None, Some("H")), "H"));
        // So does the guarded one-or-more continuation, a close alt
        // `{ s: A, b: 1, r: H }`: the `[ H ]` of `H = A [ H ]`.
        assert!(has_content(&alt(&[7], 1, None, Some("H")), "H"));
    }

    #[test]
    fn a_loop_entry_is_the_whole_compiler_shape() {
        assert!(is_loop_entry(&entry("H"), "H"));
        // A terminal continue consumes its token first.
        let terminal = AltSpec {
            s: vec![vec![7]],
            ..entry("H")
        };
        assert!(!is_loop_entry(&terminal, "H"));
        // A peek that is given back still consumes nothing.
        let peek = AltSpec {
            s: vec![vec![7]],
            b: 1,
            ..entry("H")
        };
        assert!(is_loop_entry(&peek, "H"));
        // Replacing with another rule, or pushing, is not the entry.
        let other = AltSpec {
            r: Some("H$alt0".into()),
            ..entry("H")
        };
        assert!(!is_loop_entry(&other, "H"));
        let push = AltSpec {
            p: Some("H".into()),
            ..entry("H")
        };
        assert!(!is_loop_entry(&push, "H"));
        // Nor is the same self-replace without the guard, with another
        // guard, with the guard on another value, or without the counter:
        // `s`, `b`, `p` and `r` alone do not make a loop.
        assert!(!is_loop_entry(&alt(&[], 0, None, Some("H")), "H"));
        let unguarded = AltSpec {
            c: vec![],
            ..entry("H")
        };
        assert!(!is_loop_entry(&unguarded, "H"));
        let other_guard = AltSpec {
            c: vec![Condition {
                path: vec!["n".into(), "mode".into()],
                op: CompareOp::Eq,
                value: Value::Number(0.0),
            }],
            ..entry("H")
        };
        assert!(!is_loop_entry(&other_guard, "H"));
        let second_time = AltSpec {
            c: vec![Condition {
                path: vec!["n".into(), "rep".into()],
                op: CompareOp::Eq,
                value: Value::Number(1.0),
            }],
            ..entry("H")
        };
        assert!(!is_loop_entry(&second_time, "H"));
        let uncounted = AltSpec {
            n: HashMap::new(),
            ..entry("H")
        };
        assert!(!is_loop_entry(&uncounted, "H"));
    }

    #[test]
    fn a_synthetic_kind_is_read_from_its_own_segment() {
        assert_eq!(gen_kind("_gen1_star_term"), Some("star"));
        assert_eq!(gen_kind("_gen2_opt__gen1_group"), Some("opt"));
        assert_eq!(gen_kind("_gen1_group"), Some("group"));
        // A star over an optional is named after it, and is a star.
        assert_eq!(gen_kind("_gen3_star__gen2_opt__gen1_group"), Some("star"));
        assert_eq!(gen_kind("_gen3_plus__gen2_opt__gen1_group"), Some("plus"));
        // The iteration helpers and chain steps answer for their rule.
        assert_eq!(
            gen_kind("_gen3_star__gen2_opt__gen1_group$alt0"),
            Some("star")
        );
        assert_eq!(
            gen_kind("_gen3_star__gen2_opt__gen1_group$alt0$step1"),
            Some("star")
        );
        assert_eq!(gen_kind("_gen1_group$step1"), Some("group"));
        // Not synthesised: a user rule, whatever it embeds.
        assert_eq!(gen_kind("my_opt"), None);
        assert_eq!(gen_kind("top$step1"), None);
        assert_eq!(gen_kind("_genx_opt"), None);
        assert_eq!(gen_kind("_gen1"), None);
        assert_eq!(gen_kind("_gen1__x"), None);
        // The helper itself, and only the helper, is wrapped as one.
        assert!(is_helper("_gen2_opt__gen1_group", "opt"));
        assert!(!is_helper("_gen3_star__gen2_opt__gen1_group", "opt"));
        assert!(!is_helper("_gen3_star__gen2_opt__gen1_group$alt0", "opt"));
        assert!(!is_helper(
            "_gen3_star__gen2_opt__gen1_group$alt0$step1",
            "opt"
        ));
        assert!(!is_helper("_gen3_plus__gen2_opt__gen1_group$step1", "plus"));
        assert!(is_helper("_gen3_plus__gen2_opt__gen1_group", "plus"));
    }

    #[test]
    fn a_repetition_groups_all_but_one_element() {
        let parts = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(repeat_of(&parts(&[])), "");
        assert_eq!(repeat_of(&parts(&["A"])), "*A");
        assert_eq!(repeat_of(&parts(&["a b"])), "*( a b )");
        assert_eq!(repeat_of(&parts(&["A", "B"])), "*( A / B )");
        // An inlined multi-way group is already one element.
        assert_eq!(repeat_of(&parts(&["( A / B )"])), "*( A / B )");
        assert_eq!(repeat_of(&parts(&["[ A ]"])), "*[ A ]");
        // Two groups are two elements, whatever the ends look like.
        assert_eq!(repeat_of(&parts(&["( A ) ( B )"])), "*( ( A ) ( B ) )");
        // A repetition is not an element: `**I` is not ABNF.
        assert_eq!(repeat_of(&parts(&["*I"])), "*( *I )");
    }

    #[test]
    fn numeric_val_pads_to_whole_bytes() {
        assert_eq!(numeric_val("\r\n"), "%x0D.0A");
        assert_eq!(numeric_val("A"), "%x41");
    }

    #[test]
    fn regex_ranges_and_literals_translate() {
        assert_eq!(regex_to_abnf("^[\\u0030-\\u0039]"), "%x30-39");
        assert_eq!(regex_to_abnf("^[\\x30-\\x39]"), "%x30-39");
        assert_eq!(regex_to_abnf("(?i)^foo"), "\"foo\"");
        // An escaped literal round-trips through the validation.
        assert_eq!(regex_to_abnf("(?i)^a\\.b"), "\"a.b\"");
        // A genuine regex is never misread as a literal. With no ABNF
        // form it becomes a prose-val, which is an ELEMENT: the bare
        // `; /…/` comment this used to emit left the legend entry with
        // nothing in it, so the whole grammar failed to parse.
        assert_eq!(regex_to_abnf("(?i)^a.b"), "<regex /(?i)^a.b/>");
        assert_eq!(regex_to_abnf("^\\d+"), "<regex /^\\d+/>");
    }

    #[test]
    fn prose_val_escapes_what_it_cannot_hold() {
        // RFC 5234 §4 allows %x20-3D / %x3F-7E only, so a '>' would close
        // the value early and must be escaped rather than dropped.
        assert_eq!(prose_val("plain"), "<plain>");
        assert_eq!(regex_to_abnf("^a>b"), "<regex /^a\\u003Eb/>");
        // Every angle-bracket form the emitter produces is a prose-val,
        // the built-in descriptions included.
        assert_eq!(prose_val("number"), "<number>");
    }

    #[test]
    fn quotable_rejects_control_and_quote_characters() {
        assert!(is_abnf_quotable("hi"));
        assert!(!is_abnf_quotable(""));
        assert!(!is_abnf_quotable("a\"b"));
        assert!(!is_abnf_quotable("\r\n"));
    }
}
