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
//! it nor its iteration helpers — `H$alt0` and `H$alt0$step1`, and the
//! `$alt` / `$step` chains of the foldable groups the iteration pushes,
//! everything the iteration reaches short of a kept production — is
//! emitted as a production. The older push chain (`H = A H / ε`, one
//! frame per item) carries no such entry and renders as before, as a
//! kept production, wherever it sits, inside a loop's iteration included.
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
    /// The repeat loops: every rule whose open alternatives are the
    /// compiler's whole scaffold, the entry, the continues that come back
    /// and an exit (see [`Emitter::is_loop`]). Decided by shape, so a
    /// hand-built loop and a compiled one read the same, and neither a
    /// user rule's own self-replace nor an entry without its back edge
    /// does.
    loops: BTreeSet<String>,
    /// The synthetic rules a loop's iteration runs through (`H$alt0`,
    /// `H$alt0$step1`, and the `$alt` / `$step` chains of the groups it
    /// pushes), bounded by the kept productions (see
    /// [`Emitter::find_loop_helpers`]). Inlined into the repetition
    /// wherever the loop is referenced, never productions.
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
        let mut emitter = Self {
            namer: AbnfNamer::new(rules.keys().cloned()),
            rules,
            used: IndexMap::new(),
            end_tin: parser.options.token("#ZZ"),
            synth_wrapper,
            loops: BTreeSet::new(),
            loop_helpers: BTreeSet::new(),
            parser,
        };
        emitter.loops = emitter
            .rules
            .iter()
            .filter(|(name, spec)| emitter.is_loop(name, spec))
            .map(|(name, _)| name.clone())
            .collect();
        emitter.loop_helpers = emitter.find_loop_helpers();
        emitter
    }

    /// `rule` is a repeat loop: its open alternatives are the compiler's
    /// whole scaffold, in the compiler's order, not its entry alone. The
    /// entry comes first ([`is_loop_entry`]); then at least one continue,
    /// an alternative that consumes, pushes or replaces, and every
    /// continue comes back to `rule` having made progress
    /// ([`Emitter::continues_loop`]); and the empty exit `{ }`, an
    /// alternative that consumes, pushes and replaces nothing and carries
    /// no condition, since a repetition can stop whatever comes next. No
    /// continue may follow it: it takes whatever comes and so shadows
    /// every alternative after it. The FOLLOW peek `{ s: FOLLOW, b: 1 }`
    /// may stand among the exits too, and shadows only what it peeks: the
    /// compiler puts one before the continues where a keyword must end
    /// the loop rather than be taken as an item (`*word "end"` with
    /// `word = 1*ALPHA`). The compiler never guards an exit, and a rule
    /// with a contentless alternative under a condition is no loop: the
    /// condition may never hold, leaving the rule no way to stop, and
    /// before a continue it shadows it whenever it holds. The
    /// entry alone read a rule as a loop whose continue never came back:
    /// the entry, `{ s: A }` and `{ }` take `A` at most once, and were
    /// emitted as `*A`. A continue that does not come back ends the rule
    /// after one item, and one that comes back having taken nothing
    /// repeats nothing, so a rule with either is no repetition. An
    /// alternative whose route or backtrack a function decides (`p_fn`,
    /// `r_fn`, `b_fn` and their `_match` forms, a modifier `h`) cannot be
    /// read, and a rule with one is no loop either. An exit gives back
    /// exactly what it peeks, and a synthetic loop's closes, if it has
    /// any, do nothing at all ([`is_idle`]), as the compiler's never do.
    fn is_loop(&self, rule: &str, spec: &RuleSpec) -> bool {
        let Some((first, rest)) = spec.open.split_first() else {
            return false;
        };
        if !is_loop_entry(first, rule) || is_dynamic(first) {
            return false;
        }
        let (mut continues, mut exit, mut shadowed) = (false, false, false);
        let dead = shadowed_by_peeks(&spec.open, rule);
        for (index, alt) in rest.iter().enumerate() {
            // The compiler writes one entry; a second can never be taken
            // (the first set its counter) and is neither continue nor exit.
            if is_dynamic(alt) || is_loop_entry(alt, rule) {
                return false;
            }
            if !has_content(alt, rule, true) {
                // The compiler never guards an exit: one with a condition
                // may never stop the rule, and one before a continue
                // shadows it whenever the condition holds.
                // The compiler's exits give back exactly what they peek:
                // the empty exit nothing, a FOLLOW peek its tokens.
                if !is_plain_way(alt) || alt.b != alt.s.len() || alt.s.iter().any(Vec::is_empty) {
                    return false;
                }
                // Only the empty exit stops the rule whatever comes next,
                // and shadows every alternative after it.
                if alt.s.is_empty() {
                    exit = true;
                    shadowed = true;
                }
            } else if !shadowed && self.continues_loop(alt, rule) {
                // A continue a FOLLOW peek before it covers never runs, and
                // does not make the rule a loop, though it still renders:
                // it is the source's own alternative, as `*( "a" / "b" )`
                // with `b` in FOLLOW keeps its `B`.
                continues |= !dead[index + 1];
            } else {
                return false;
            }
        }
        continues && exit && (!self.is_synthetic(rule) || spec.close.iter().all(is_idle))
    }

    /// A continue in one of the compiler's two shapes, coming back to its
    /// loop `rule` having taken one item. A terminal continue consumes
    /// its token and replaces with `rule`, giving back any it peeked
    /// after it (`{ s: [A], r: H }`, `{ s: [A, X], b: 1, r: H }`). A rule
    /// continue peeks the item's first tokens, gives them all back and
    /// replaces with the loop's iteration helper
    /// ([`Emitter::is_iteration_helper`]), which pushes the item and comes
    /// back. Either sets no counter, carries no guard but the
    /// suffix-debt counter, and matches no empty token slot, which takes
    /// any token and renders as nothing. Any other shape, a push beside
    /// the replace (which the engine takes instead), a helper of another
    /// shape, one with an empty way through, or one that pushes the loop
    /// itself, is no continue the compiler writes, and a rule with one is
    /// no loop.
    fn continues_loop(&self, alt: &AltSpec, rule: &str) -> bool {
        if alt.p.is_some()
            || alt.s.is_empty()
            || alt.s.iter().any(Vec::is_empty)
            || !alt.n.is_empty()
            || !is_guarded_as_compiled(alt)
        {
            return false;
        }
        match alt.r.as_deref() {
            Some(target) if target == rule => alt.s.len() == alt.b + 1,
            Some(target) => alt.s.len() == alt.b && self.is_iteration_helper(target, rule),
            None => false,
        }
    }

    /// `helper` is the iteration helper the compiler gives the loop
    /// `rule` over a rule item, and nothing else: its one open pushes the
    /// item (a rule other than `rule`, which reaches `rule` by no
    /// synthetic way of its own), its one close replaces with its step,
    /// and the step's one open replaces with `rule`, all matching no
    /// token and setting no counter but `rep` (`H$alt0` → `H$alt0$step1`
    /// → `H`).
    fn is_iteration_helper(&self, helper: &str, rule: &str) -> bool {
        let bare =
            |alt: &AltSpec| alt.s.is_empty() && alt.b == 0 && alt.p.is_none() && is_helper_way(alt);
        let Some(spec) = self.rules.get(helper) else {
            return false;
        };
        let ([open], [close]) = (spec.open.as_slice(), spec.close.as_slice()) else {
            return false;
        };
        let item = match open.p.as_deref() {
            Some(item) if item != rule => item,
            _ => return false,
        };
        if !open.s.is_empty() || open.b != 0 || open.r.is_some() || !is_helper_way(open) {
            return false;
        }
        if !self.is_synthetic(helper) || !bare(close) || self.reaches(item, rule) {
            return false;
        }
        let Some(step) = close.r.as_deref() else {
            return false;
        };
        let Some(step_spec) = self.rules.get(step) else {
            return false;
        };
        let [step_open] = step_spec.open.as_slice() else {
            return false;
        };
        self.is_synthetic(step)
            && bare(step_open)
            && step_open.r.as_deref() == Some(rule)
            && step_spec.close.iter().all(is_idle)
    }

    /// `from` reaches `rule`, by a push or a replace, itself or through
    /// synthetic rules: an item that does repeats `rule` inside its own
    /// iteration, which the repetition cannot render (its references to
    /// `rule` read as the iteration's back edge). The compiler's items
    /// reach their loop, when they do, through a user rule.
    fn reaches(&self, from: &str, rule: &str) -> bool {
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut pending = vec![from];
        while let Some(name) = pending.pop() {
            if name == rule {
                return true;
            }
            if (name != from && !self.is_synthetic(name)) || !visited.insert(name) {
                continue;
            }
            if let Some(spec) = self.rules.get(name) {
                pending.extend(
                    spec.open
                        .iter()
                        .chain(spec.close.iter())
                        .filter_map(|alt| alt.p.as_deref().or(alt.r.as_deref())),
                );
            }
        }
        false
    }

    /// The iteration helpers of every loop `H`: the synthetic rules
    /// reachable from its continue alternatives without passing through
    /// a KEPT PRODUCTION, which the walk neither enters nor adds
    /// ([`Emitter::bounds_loop_helpers`]): `H$alt0`, pushing the item,
    /// `H$alt0$step1`, replacing with `H`, and the `$alt` / `$step` chain
    /// the compiler gives a group whose alternative starts with a rule
    /// (`_gen2_group$alt0`, `_gen2_group$alt0$step1`, `_gen2_group$alt1`
    /// for `( *A B / C )`). A user rule the iteration pushes keeps its
    /// production; a nested loop is a loop of its own; an old push-chain
    /// star stays a kept production, as does a `_plus`, which
    /// [`Emitter::is_foldable`] judges for itself, a synthetic rule that
    /// repeats by a cycle of its own ([`Emitter::cycles`]), and one with
    /// an empty way through ([`Emitter::is_nullable_unfolded`]).
    ///
    /// The bound is by kept productions, not by name. A walk that
    /// stopped only at user rules and other loops reached through a
    /// foldable group to the old star inside `*( B *A C )` and folded it
    /// away with its epsilon branch and its back edge: `*( B A C )`. A
    /// scan for the rules named `H$…` never reached the group's own
    /// chain, whose `$alt` names [`Emitter::is_foldable`] refuses, so
    /// `*( *A B / C )` came out as `*( r-gen2-group-alt0 /
    /// r-gen2-group-alt1 )` over three kept productions, and the `_plus`
    /// over the same group, refused by [`Emitter::plus_folds`] for the
    /// same names, as `X *X` in place of `1*X`.
    fn find_loop_helpers(&self) -> BTreeSet<String> {
        let mut helpers = BTreeSet::new();
        let mut pending: Vec<String> = self
            .loops
            .iter()
            .filter_map(|name| self.rules.get(name).map(|spec| (name, spec)))
            .flat_map(|(name, spec)| {
                spec.open
                    .iter()
                    .filter_map(|alt| alt.p.as_deref().or(alt.r.as_deref()))
                    .filter(|target| *target != name.as_str())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        while let Some(name) = pending.pop() {
            if self.bounds_loop_helpers(&name)
                || helpers.contains(&name)
                || self.cycles(&name)
                || self.is_nullable_unfolded(&name)
            {
                continue;
            }
            let Some(spec) = self.rules.get(&name) else {
                continue;
            };
            helpers.insert(name.clone());
            for alt in spec.open.iter().chain(spec.close.iter()) {
                if let Some(target) = alt.p.as_deref().or(alt.r.as_deref()) {
                    pending.push(target.to_owned());
                }
            }
        }
        helpers
    }

    /// A synthetic rule with an empty way through its opens, one that
    /// takes nothing (`{ }`, or a peek it gives back), which nothing but
    /// a loop's inlining would inline: not an optional's own helper,
    /// which inlines as `[ … ]`, nor a foldable rule, which inlines
    /// wherever it is referenced. Inlined as a loop's helper its empty
    /// way was dropped, `*( A B )` for an iteration that also takes `B`
    /// alone, so it stays a production of its own, referenced by name,
    /// as it always was.
    fn is_nullable_unfolded(&self, name: &str) -> bool {
        !is_helper(name, "opt")
            && !self.is_foldable(name)
            && self
                .rules
                .get(name)
                .is_some_and(|spec| spec.open.iter().any(|alt| !has_content(alt, name, false)))
    }

    /// A synthetic rule that reaches itself again through synthetic rules
    /// none of which bounds a loop's helpers ([`Emitter::bounds_loop_helpers`]):
    /// a repetition of its own, which no loop's iteration accounts for.
    /// Inlined as a loop's helper, the way back to itself rendered as
    /// nothing, through `seen`, so the helper `A H / B` pushed from a loop
    /// came out as `*( A / B )` where the loop takes `*( *A B )`. It stays
    /// what it was before the loop was read: a production of its own,
    /// referenced by name. A loop's own `H$alt0` reaches itself only
    /// through `H`, a loop, and is no cycle.
    fn cycles(&self, name: &str) -> bool {
        let targets = |rule: &str| -> Vec<&str> {
            self.rules
                .get(rule)
                .map(|spec| {
                    spec.open
                        .iter()
                        .chain(spec.close.iter())
                        .filter_map(|alt| alt.p.as_deref().or(alt.r.as_deref()))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut pending = targets(name);
        while let Some(target) = pending.pop() {
            if target == name {
                return true;
            }
            if self.bounds_loop_helpers(target) || !visited.insert(target) {
                continue;
            }
            pending.extend(targets(target));
        }
        false
    }

    /// A rule that is a production of its own, or decides that for
    /// itself, and so bounds the iteration helpers of every loop: a user
    /// rule, a loop, or a repetition helper that is no loop's own — an old
    /// push-chain `_star` with its `$alt` helpers
    /// ([`Emitter::is_kept_repetition`]), and a `_plus` in either shape,
    /// which [`Emitter::is_foldable`] judges through
    /// [`Emitter::plus_folds`]. A loop's own `H$alt0` and `H$alt0$step1`
    /// carry the loop's whole name, star and all, and are read by their
    /// own segment: the loop.
    fn bounds_loop_helpers(&self, name: &str) -> bool {
        if !self.is_synthetic(name) || self.loops.contains(name) {
            return true;
        }
        matches!(gen_kind(name), Some("star" | "plus")) && !self.loops.contains(own_segment(name))
    }

    /// A repetition helper in the old push-chain shape, kept as a
    /// production as it always was: a `_star` that is not a loop, or one
    /// of its `$alt0` / `$alt0$step1` iteration helpers, which carry its
    /// name. A loop's own `H$alt0` is not one (its own segment is the
    /// loop), nor is the `$alt` chain of a group or the `$step` chain of
    /// a `_plus`.
    fn is_kept_repetition(&self, name: &str) -> bool {
        gen_kind(name) == Some("star") && !self.loops.contains(own_segment(name))
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
    /// The helper folds to `A *A` when that star is a loop; with an
    /// old-shape star, a kept production, the helper stays a production
    /// too, as it always has. Its own chain is the helper and its `$step`
    /// helpers, which share its own segment, and only the repetition that
    /// chain reaches is its trailing star: an old-shape star inside the
    /// item is the item's, a production of its own referenced by name,
    /// and neither folds nor stops the fold. It folds only when its own
    /// chain reaches a loop, and meets no cycle on the way: a helper that
    /// re-enters itself, or a chain of helpers that comes back round
    /// without passing through a loop, repeats by that cycle, and folded,
    /// the cycle's back edge rendered as nothing, so `A*` came out as one
    /// `A`. Reaches through the chain steps (`_plus$step1`) a non-terminal
    /// item puts between the helper and its star, and through the group
    /// it pushes and that group's own `$alt` / `$step` chain, which fold:
    /// the `_plus` over `( *A B / C )` is `1*( *A B / C )`. Refusing
    /// every `$alt` name here refused that chain, and the helper came out
    /// as `X *X`, which does not round-trip on a nullable item.
    fn plus_folds(&self, name: &str) -> bool {
        // A depth-first walk with an explicit stack: `open` holds the
        // rules on the current path, so reaching one again is a cycle;
        // `done` those whose walk has finished, which a second path may
        // reach without one. A step is judged by the plus it belongs to,
        // through the segment they share.
        let mut open: BTreeSet<String> = BTreeSet::new();
        let mut done: BTreeSet<String> = BTreeSet::new();
        let mut reached_loop = false;
        let mut stack: Vec<(String, Vec<String>)> = Vec::new();
        let targets = |current: &str| -> Vec<String> {
            self.rules
                .get(current)
                .map(|spec| {
                    spec.open
                        .iter()
                        .chain(spec.close.iter())
                        .filter_map(|alt| alt.p.as_deref().or(alt.r.as_deref()))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        open.insert(name.to_owned());
        stack.push((name.to_owned(), targets(name)));
        while let Some((current, pending)) = stack.last_mut() {
            let Some(target) = pending.pop() else {
                open.remove(current.as_str());
                done.insert(current.clone());
                stack.pop();
                continue;
            };
            let own = own_segment(current) == own_segment(name);
            if !self.rules.contains_key(&target) || !self.is_synthetic(&target) {
                continue;
            }
            if self.loops.contains(&target) || self.loop_helpers.contains(&target) {
                reached_loop |= own && self.loops.contains(&target);
                continue;
            }
            if self.is_kept_repetition(&target) {
                // The plus's own trailing star in the old shape keeps the
                // plus a production; one inside the item stays the item's
                // production, referenced by name.
                if own {
                    return false;
                }
                continue;
            }
            if open.contains(&target) {
                return false;
            }
            if done.contains(&target) {
                continue;
            }
            open.insert(target.clone());
            let next = targets(&target);
            stack.push((target, next));
        }
        let plus = own_segment(name);
        reached_loop
            && self
                .loop_after(plus)
                .is_some_and(|tail| self.counted_by_construction(plus, &tail).is_some())
    }

    /// A rule rendered where it is referenced and never as a production
    /// of its own, unless it is the start rule: a foldable synthetic, a
    /// synthetic loop, or a loop's iteration helper. A loop that is a
    /// USER rule keeps its production (its body is the repetition) and is
    /// referenced by name.
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

        // The start rule is always a production, folded or not: nothing
        // encloses it to render it where it is referenced, and a grammar
        // whose start is a synthetic loop (a standalone `*A`) otherwise
        // came out with no production at all.
        let mut ordered: Vec<String> = Vec::new();
        let mut seen_rules: BTreeSet<String> = BTreeSet::new();
        if let Some(start) = start_rule.filter(|start| self.rules.contains_key(start)) {
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
        // One that repeats nothing matches exactly the empty string, which
        // `""` says and an empty production cannot (see below).
        if self.loops.contains(name) {
            let body = self.repetition(name, seen);
            return if body.is_empty() {
                "\"\"".to_string()
            } else {
                body
            };
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
            .any(|alt| !self.is_end_alt(alt) && !self.has_content(alt, name));
        for alt in &closes {
            if self.is_end_alt(alt) || !self.has_content(alt, name) {
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
                    .filter(|alt| self.has_content(alt, name))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// [`has_content`] for an alt of `name`, which is a loop or is not:
    /// only a loop has an entry to skip.
    fn has_content(&self, alt: &AltSpec, name: &str) -> bool {
        has_content(alt, name, self.loops.contains(name))
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
        // The loop's own name is seen so that the iteration's back edges
        // render nothing; a close that re-enters a user loop is no back
        // edge but the rule again, and renders as its name. (A synthetic
        // loop's closes do nothing at all: see [`is_idle`].)
        let cont = if self.is_synthetic(name) {
            self.close_cont(name, seen)
        } else {
            let mut outer = seen.clone();
            outer.remove(name);
            self.close_cont(name, &outer)
        };
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
            if let Some(counted) = self.counted_repetition(name, &inner) {
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
    fn counted_repetition(&mut self, name: &str, seen: &BTreeSet<String>) -> Option<String> {
        let loop_name = self.loop_after(name)?;
        let count = self.counted_by_construction(name, &loop_name)?;
        let mut inner = seen.clone();
        inner.insert(loop_name.clone());
        let repetition = self.repetition(&loop_name, &inner);
        repetition
            .starts_with('*')
            .then(|| format!("{count}{repetition}"))
    }

    /// How many times the chain of the `_plus` / `_rep` helper `name`
    /// takes the item of the loop `tail` it ends in, when it is the
    /// compiler's construction and nothing else: each rule of the chain
    /// has one plain open alternative, consuming the loop's item token
    /// (`{ s: [A], p: tail }` for `1*"a"`, `{ s: [A A], … }` for `2*"a"`)
    /// or pushing the loop's item rule (`{ p: item }`, then a close
    /// replace to the next step), and the last pushes `tail` and ends.
    /// A chain that takes anything else, or a different rule that
    /// merely renders the same, is no counted repetition: `X *X` and
    /// `1*X` compile to different recognisers, and only the compiler's
    /// own construction is the `1*X` it was compiled from.
    fn counted_by_construction(&self, name: &str, tail: &str) -> Option<usize> {
        let item = self.loop_item(tail)?;
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut current = name;
        let mut count = 0usize;
        loop {
            if !visited.insert(current) {
                return None;
            }
            let spec = self.rules.get(current)?;
            let [open] = spec.open.as_slice() else {
                return None;
            };
            if !is_plain_way(open) || open.b != 0 || open.r.is_some() {
                return None;
            }
            for slot in &open.s {
                if item != Item::Token(slot.clone()) {
                    return None;
                }
                count += 1;
            }
            match open.p.as_deref() {
                Some(target) if target == tail => {
                    let ends = spec.close.iter().all(|alt| {
                        is_plain_way(alt) && alt.s.is_empty() && alt.p.is_none() && alt.r.is_none()
                    });
                    return (ends && 0 < count).then_some(count);
                }
                Some(target) if item == Item::Rule(target.to_owned()) => count += 1,
                Some(_) => return None,
                None => {}
            }
            let [close] = spec.close.as_slice() else {
                return None;
            };
            if !is_plain_way(close) || !close.s.is_empty() || close.p.is_some() {
                return None;
            }
            current = close.r.as_deref()?;
        }
    }

    /// The one item the loop `rule` repeats, as the compiler builds it:
    /// the token its terminal continues consume (`{ s: [A], r: rule }`,
    /// or `{ s: [A, X], b: 1, r: rule }` with a token of lookahead),
    /// or the rule its iteration helper pushes, after a continue that
    /// peeks one of that rule's FIRST tokens (`{ s: [A], b: 1, r:
    /// rule$alt0 }`, `rule$alt0` opening `{ p: item }`). None for a loop
    /// over anything else.
    fn loop_item(&self, rule: &str) -> Option<Item> {
        let spec = self.rules.get(rule)?;
        let mut item: Option<Item> = None;
        for alt in spec.open.iter().skip(1) {
            if !has_content(alt, rule, true) {
                continue;
            }
            let this = if alt.r.as_deref() == Some(rule) && alt.s.len() == alt.b + 1 {
                // It consumes its one token and gives back any it peeked
                // after it (`{ s: [A, X], b: 1 }`, where the compiler looks
                // two tokens ahead).
                Item::Token(alt.s[0].clone())
            } else if alt.s.len() == alt.b {
                let helper = self.rules.get(alt.r.as_deref()?)?;
                let [open] = helper.open.as_slice() else {
                    return None;
                };
                if !open.s.is_empty() || open.r.is_some() || !is_plain_way(open) {
                    return None;
                }
                Item::Rule(open.p.clone()?)
            } else {
                return None;
            };
            match &item {
                None => item = Some(this),
                Some(seen) if *seen == this => {}
                Some(_) => return None,
            }
        }
        item
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
    let rest = own_segment(name).strip_prefix("_gen")?;
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

/// The rule a synthesised name belongs to: the part before any `$`. A
/// chain step (`_gen1_group$step1`) and an iteration helper (`H$alt0`,
/// `H$alt0$step1`) answer with the rule they continue.
fn own_segment(name: &str) -> &str {
    name.split('$').next().unwrap_or(name)
}

/// `name` is the helper of `kind` itself — not a chain step or an
/// iteration helper of it, which carry a `$`.
fn is_helper(name: &str, kind: &str) -> bool {
    !name.contains('$') && gen_kind(name) == Some(kind)
}

/// An alt that contributes something to the emitted sequence of `rule`,
/// decided by what it CONSUMES: it eats a token (`len(s) - b > 0`), or
/// pushes a rule, or replaces with a rule — unless `rule` is a loop and
/// this is its entry ([`is_loop_entry`]). `{ }`, the FOLLOW peek
/// `{ s: FOLLOW, b: 1 }` and the entry `{ c: [n.rep == 0], n: {rep: 1},
/// r: rule }` are all epsilon: none of them moves the parse past any
/// input. Counting the peek as content rendered a follow-guarded exit
/// as a consuming alternative, and counting the entry rendered a loop
/// as one of its own alternatives. Every other replace with `rule`
/// itself is content, as it always was: the close `{ s: A, b: 1, r: rule }`
/// after an open that consumed `A` is the `[ rule ]` of
/// `rule = A [ rule ]`, and calling it empty for the self-replace alone
/// emitted `rule = A`, exactly one where the rule takes one or more. The
/// same close carrying the entry's guard and counter is content too when
/// `rule` is no loop (`rule_is_loop`, decided by its OPEN alternatives):
/// a rule that is not a loop has no entry, only alternatives, and
/// `odd = A [ odd ]` renders as it always did.
fn has_content(alt: &AltSpec, rule: &str, rule_is_loop: bool) -> bool {
    !(rule_is_loop && is_loop_entry(alt, rule))
        && (alt.s.len() > alt.b || alt.p.is_some() || alt.r.is_some())
}

/// The one item a loop repeats ([`Emitter::loop_item`]): a token slot its
/// continue consumes, or the rule its iteration helper pushes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Item {
    Token(Vec<Tin>),
    Rule(String),
}

/// An alternative whose route, backtrack or whole shape a function
/// decides when it matches, so what it pushes, replaces or consumes
/// cannot be read from the spec.
fn is_dynamic(alt: &AltSpec) -> bool {
    alt.p_fn.is_some()
        || alt.p_match.is_some()
        || alt.r_fn.is_some()
        || alt.r_match.is_some()
        || alt.b_fn.is_some()
        || alt.b_match.is_some()
        || alt.h.is_some()
        || alt.h_match.is_some()
}

/// A continue guarded as the compiler guards one: by nothing, or by the
/// suffix-debt counter alone (`n.debt_… == 0`, which tabnas-bnf puts on
/// a left-recursion tail loop's contested branches). Any other
/// condition may contradict the state the entry leaves (`n.rep == 0`
/// after the entry has set it to 1) and keep the continue from ever
/// running, so a rule with one is no loop.
fn is_guarded_as_compiled(alt: &AltSpec) -> bool {
    !is_dynamic(alt)
        && matches!(alt.c.as_slice(), [] | [_])
        && alt.c.iter().all(is_debt_guard)
        && alt.c_ref.is_none()
        && alt.c_fn.is_none()
        && alt.c_match.is_none()
        && alt.c_lex.is_none()
        && alt.c_lex_match.is_none()
}

/// The suffix-debt guard, `n.debt_… == 0`.
fn is_debt_guard(condition: &Condition) -> bool {
    matches!(condition.path.as_slice(), [bag, counter] if bag == "n" && counter.starts_with("debt_"))
        && condition.op == CompareOp::Eq
        && matches!(condition.value, Value::Number(count) if count == 0.0)
}

/// An alternative that does nothing: it matches no token, pushes and
/// replaces nothing, and carries no condition. The compiler's loops have
/// no closes, and a synthetic rule is a loop only when its closes, if it
/// has any, are all idle. A synthetic loop renders inline, wherever a
/// rule refers to it, with no name of its own to render there, so a
/// close that could run it again would be lost: one that re-enters it,
/// directly or through helpers, one that pushes, which comes back to the
/// close phase when the pushed rule ends and runs the closes again, or
/// one a function routes. Each was found in turn, and the compiler writes
/// none of them.
fn is_idle(alt: &AltSpec) -> bool {
    alt.s.is_empty() && alt.b == 0 && alt.p.is_none() && alt.r.is_none() && is_plain_way(alt)
}

/// An alternative on a loop's way back, as the compiler writes one: a
/// plain way ([`is_plain_way`]) that sets no counter but the loop's own
/// `rep` (the iteration helper counts it). Another counter could turn a
/// guarded continue off for the next iteration.
fn is_helper_way(alt: &AltSpec) -> bool {
    is_plain_way(alt) && alt.n.keys().all(|counter| counter == "rep")
}

/// For each open alternative of the loop `rule`, whether it is a continue
/// that a FOLLOW peek before it covers, and so never runs: the peek
/// matches wherever the continue would. The compiler writes such dead
/// continues where FIRST meets FOLLOW (`{ s: [C], b: 1 }`, then
/// `{ s: [C], b: 1, r: H$alt0 }`, with the live continues that look
/// further ahead before both); a rule is a loop only when some continue
/// is live, and a hand-built one with nothing but dead ones (`{ s: A,
/// b: 1 }` before `{ s: A, r: H }`) takes no item at all.
fn shadowed_by_peeks(open: &[AltSpec], rule: &str) -> Vec<bool> {
    let mut peeks: Vec<&[Vec<Tin>]> = Vec::new();
    open.iter()
        .map(|alt| {
            if !has_content(alt, rule, true) {
                if !alt.s.is_empty() {
                    peeks.push(&alt.s);
                }
                false
            } else {
                peeks.iter().any(|peek| covers(peek, &alt.s))
            }
        })
        .collect()
}

/// The token sequence `peek` matches wherever `item` does: it is no
/// longer, and each of its slots holds every token `item`'s does.
fn covers(peek: &[Vec<Tin>], item: &[Vec<Tin>]) -> bool {
    peek.len() <= item.len()
        && peek
            .iter()
            .zip(item)
            .all(|(slot, other)| other.iter().all(|tin| slot.contains(tin)))
}

/// An alternative a loop's helper can come back through: read from the
/// spec alone, and taken whatever the rule's state, with no condition of
/// any kind.
fn is_plain_way(alt: &AltSpec) -> bool {
    !is_dynamic(alt)
        && alt.c.is_empty()
        && alt.c_ref.is_none()
        && alt.c_fn.is_none()
        && alt.c_match.is_none()
        && alt.c_lex.is_none()
        && alt.c_lex_match.is_none()
}

/// A repeat loop's entry, the whole of the compiler's shape: the
/// alternative matches no token, not even a peeked one (which would
/// leave the counter unset wherever that token is not next), pushes
/// nothing, replaces `rule` with
/// itself, is guarded by `n.rep == 0` and by nothing else, and sets that
/// counter to 1 and no other — allocating the node and counting the iteration on the
/// way in. A further condition, in `c` or any other channel, may keep
/// the entry from running, and with it the counter it sets, which the
/// continues may be guarded on. It is
/// what marks a rule as a loop. `s`, `b`, `p` and `r` alone are not
/// enough: a user rule's own non-consuming self-replace, a guarded or
/// counted state transition, has the same four and is no repetition, and
/// reading it as one rewrote the whole rule as `*…`, accepting empty and
/// repeated inputs the original need not.
fn is_loop_entry(alt: &AltSpec, rule: &str) -> bool {
    alt.s.is_empty()
        && alt.b == 0
        && alt.p.is_none()
        && alt.r.as_deref() == Some(rule)
        && alt.n.len() == 1
        && alt.n.get("rep") == Some(&1)
        && matches!(alt.c.as_slice(), [guard] if is_rep_guard(guard))
        && alt.c_ref.is_none()
        && alt.c_fn.is_none()
        && alt.c_match.is_none()
        && alt.c_lex.is_none()
        && alt.c_lex_match.is_none()
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
        // `H` is a loop here; the last case renders a rule that is not.
        let in_loop = |alt: &AltSpec| has_content(alt, "H", true);
        // Eats a token.
        assert!(in_loop(&alt(&[7], 0, None, None)));
        // Peeks one and gives it back: the follow-guarded exit.
        assert!(!in_loop(&alt(&[7], 1, None, None)));
        // Nothing at all.
        assert!(!in_loop(&alt(&[], 0, None, None)));
        // Pushes a rule.
        assert!(in_loop(&alt(&[], 0, Some("item"), None)));
        // Replaces with another rule, after a peek.
        assert!(in_loop(&alt(&[7], 1, None, Some("H$alt0"))));
        // The loop entry: replaces with the rule being rendered, guarded.
        assert!(!in_loop(&entry("H")));
        // The same alt is content from any OTHER rule's point of view.
        assert!(has_content(&entry("H"), "H$alt0$step1", false));
        // A self-replace WITHOUT the guard is not the entry and keeps its
        // content: a user rule's own state transition.
        assert!(in_loop(&alt(&[], 0, None, Some("H"))));
        // So does the guarded one-or-more continuation, a close alt
        // `{ s: A, b: 1, r: H }`: the `[ H ]` of `H = A [ H ]`.
        assert!(in_loop(&alt(&[7], 1, None, Some("H"))));
        // And the entry's own shape, guard and counter included, when the
        // rule being rendered is NOT a loop: it has no entry to skip.
        assert!(has_content(&entry("odd"), "odd", false));
        assert!(!has_content(&entry("odd"), "odd", true));
        // With a peek it is no entry at all, whatever the rule: the close
        // `{ c: [n.rep == 0], n: {rep: 1}, s: A, b: 1, r: odd }` is the
        // `[ odd ]` of `odd = A [ odd ]`.
        let guarded_close = AltSpec {
            s: vec![vec![7]],
            b: 1,
            ..entry("odd")
        };
        assert!(has_content(&guarded_close, "odd", false));
        assert!(has_content(&guarded_close, "odd", true));
    }

    #[test]
    fn a_synthesised_name_belongs_to_its_own_segment() {
        assert_eq!(own_segment("_gen2_group"), "_gen2_group");
        assert_eq!(own_segment("_gen2_group$alt0$step1"), "_gen2_group");
        assert_eq!(
            own_segment("_gen3_star__gen2_group$alt0"),
            "_gen3_star__gen2_group"
        );
        assert_eq!(own_segment("top"), "top");
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
        // A peek, even one given back, is not the entry either: it sets
        // the counter only where that token comes next.
        let peek = AltSpec {
            s: vec![vec![7]],
            b: 1,
            ..entry("H")
        };
        assert!(!is_loop_entry(&peek, "H"));
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
