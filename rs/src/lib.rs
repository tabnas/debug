/* Copyright (c) 2021-2026 Richard Rodger and other contributors, MIT License */

//! Tracing and introspection plugin for the
//! [tabnas](https://github.com/tabnas/parser) parser engine.
//!
//! This is the developer tool the rest of the tabnas fleet's test suites
//! consume. It provides four things:
//!
//! - [`describe()`] — a human-readable dump of a live [`Tabnas`] instance:
//!   its tag, tokens, token sets, rules, alternates, lexer matchers,
//!   config, plugins, and an ABNF rendering of the grammar.
//! - [`model()`] — the *structured* counterpart: the same information as a
//!   typed, serialisable [`DebugModel`], so tools and tests can consume
//!   the grammar programmatically.
//! - [`abnf()`] — a re-compilable ABNF rendering of the instance's live
//!   grammar.
//! - **parse tracing** that logs events as the parser runs (see
//!   [`TraceKinds`]).
//!
//! The plugin is a developer tool, **not part of the parse path**.
//!
//! ```no_run
//! use tabnas::Tabnas;
//! use tabnas_debug::{apply, describe, DebugOptions};
//!
//! let mut parser = Tabnas::new();
//! // ... install a grammar ...
//! apply(&mut parser, DebugOptions::quiet())?;
//! println!("{}", describe(&parser));
//! # Ok::<(), tabnas::PluginError>(())
//! ```
//!
//! TypeScript is canonical: `ts/src/debug.ts` is the source of truth for
//! behaviour, option names, defaults, output format and section ordering.
//! The shared fixtures in `test/spec/*.tsv` are the parity contract.
//! Intentional differences are recorded in `docs/reference.md`.
//!
//! ## Free functions, not instance methods
//!
//! TypeScript attaches these as instance methods (`tn.debug.describe()`).
//! Rust cannot add methods to a type it does not own, so — exactly as the
//! Go port does — they are free functions taking the instance. They are
//! also infallible here: the Go port returns `(value, error)` because its
//! engine accessors can fail, while the Rust accessors cannot.

pub mod abnf;
pub mod describe;
pub mod model;
pub mod trace;

use tabnas::{Plugin, PluginError, Tabnas, Value};

pub use abnf::abnf;
pub use describe::{describe, SECTIONS};
pub use model::{
    model, DebugAltInfo, DebugConfigInfo, DebugLexMatcher, DebugModel, DebugPluginInfo,
    DebugRuleEdges, DebugRuleInfo, DebugSeqItem, DebugTokenInfo, DebugTokenSet,
};
pub use trace::{TraceKinds, TRACE_BANNER};

/// The README's Rust examples run as doctests, so a stale one fails the
/// gate rather than misleading the reader. Its `toml` and `bash` fences
/// are skipped; rustdoc runs only the `rust` ones.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}

/// VERSION is this crate's version. It MUST equal `ts/package.json`
/// "version" and the `version` field in `rs/Cargo.toml`: the release
/// orchestrator rewrites them, and `tests/version_test.rs` fails the
/// build if they drift. Mirrors `VERSION` in `ts/src/debug.ts` and
/// `const VERSION` in `go/debug.go`.
pub const VERSION: &str = "0.3.7";

/// The decoration key under which the plugin records its `print` setting,
/// so [`use_plugin`] can honour it later.
const PRINT_DECORATION: &str = "debug.print";

/// How the debug plugin behaves once installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugOptions {
    /// Dump [`describe()`] after each plugin installed through
    /// [`use_plugin`]. Default `true`.
    pub print: bool,
    /// Which trace kinds to log, or `None` to trace nothing. Default: all
    /// kinds.
    pub trace: Option<TraceKinds>,
}

impl Default for DebugOptions {
    fn default() -> Self {
        Self {
            print: true,
            trace: Some(TraceKinds::all()),
        }
    }
}

impl DebugOptions {
    /// The canonical defaults: printing on, every trace kind on.
    pub fn new() -> Self {
        Self::default()
    }

    /// Introspection only — no `USE:` dumps and no tracing. This is what
    /// a test suite that only wants [`describe()`] / [`model()`] / [`abnf()`]
    /// should install.
    pub fn quiet() -> Self {
        Self {
            print: false,
            trace: None,
        }
    }

    /// Set the `print` flag.
    pub fn with_print(mut self, print: bool) -> Self {
        self.print = print;
        self
    }

    /// Trace exactly these kinds.
    pub fn with_trace(mut self, kinds: TraceKinds) -> Self {
        self.trace = Some(kinds);
        self
    }

    /// Trace nothing.
    pub fn without_trace(mut self) -> Self {
        self.trace = None;
        self
    }
}

/// Build the debug plugin for `options`.
///
/// The returned [`Plugin`] can be installed with
/// [`Tabnas::use_plugin`], and — like every native plugin — re-runs
/// against a derived instance's options. Most callers want [`apply`].
pub fn plugin(options: DebugOptions) -> Plugin {
    Plugin::new("Debug", move |parser, _plugin_options| {
        install(parser, options)
    })
}

/// Install the debug plugin on `parser`.
///
/// The convenience constructor mirroring the TypeScript
/// `j.use(Debug, options)` call.
pub fn apply(parser: &mut Tabnas, options: DebugOptions) -> Result<(), PluginError> {
    parser.use_plugin(plugin(options), None).map(|_| ())
}

fn install(parser: &mut Tabnas, options: DebugOptions) -> Result<(), PluginError> {
    parser.decorate(PRINT_DECORATION, options.print);
    // Always call through, including for `None`: re-applying the plugin
    // must be able to turn a previously installed trace OFF, not just
    // widen it. `trace::install` registers its callbacks once per
    // instance and updates the live selection thereafter.
    trace::install(parser, options.trace)
}

/// Install `plugin` on `parser`, dumping [`describe()`] afterwards when the
/// debug plugin was installed with `print` on.
///
/// TypeScript reassigns `tabnas.use` to wrap it; neither Go's `(*Tabnas).Use`
/// nor Rust's [`Tabnas::use_plugin`] is a reassignable field, so both ports
/// expose the wrapper as a function instead. A plugin installed directly
/// through [`Tabnas::use_plugin`] therefore does not trigger the `USE:`
/// dump.
pub fn use_plugin(
    parser: &mut Tabnas,
    plugin: Plugin,
    options: Option<Value>,
) -> Result<(), PluginError> {
    let name = plugin.name.clone();
    parser.use_plugin(plugin, options)?;
    if parser.decoration::<bool>(PRINT_DECORATION).copied() == Some(true) {
        let dump = describe(parser);
        parser
            .options
            .debug
            .write(&format!("USE: {name}\n\n{dump}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_canonical_runtime() {
        let options = DebugOptions::default();
        assert!(options.print);
        assert_eq!(options.trace, Some(TraceKinds::all()));
    }

    #[test]
    fn quiet_turns_everything_off() {
        let options = DebugOptions::quiet();
        assert!(!options.print);
        assert_eq!(options.trace, None);
    }

    #[test]
    fn builders_compose() {
        let options = DebugOptions::new()
            .with_print(false)
            .with_trace(TraceKinds {
                lex: true,
                ..TraceKinds::none()
            });
        assert!(!options.print);
        assert_eq!(options.trace.map(|kinds| kinds.lex), Some(true));
        assert!(DebugOptions::new().without_trace().trace.is_none());
    }
}
