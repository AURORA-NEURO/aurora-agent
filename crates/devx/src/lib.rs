//! Developer diagnostics and compile introspection, for a developer who is usually an agent.
//!
//! Implements the part of blueprint section 11 (Developer Platform) that `bioprism-sdk` does not,
//! together with 23.32 (SDK, CLI, debugger and developer experience). The framing this crate takes
//! from the repository: the developer here is frequently an agent rather than a person at a
//! terminal, and that changes what each §11 module has to be. A diagnostic is not a well-phrased
//! sentence, it is machine-actionable state. A debugger is not a UI, it is a queryable record of
//! what the compiler decided. A local dev loop is not a watch script, it is a contract about what
//! a change invalidates.
//!
//! `bioprism-docgraph` built the navigation half of that idea — a context-bundle compiler for
//! documentation, with an omission record. This crate is the diagnostic and introspection half.
//!
//! # The modules
//!
//! | blueprint | here |
//! |---|---|
//! | 11.01 platform overview, 23.32 diagnostics clause | [`diagnostic`] |
//! | 11.03 CLI specification ("stable exit codes"), 40.36 retry semantics | [`taxonomy`], [`exitaudit`] |
//! | 11.19 trace, fork and diff viewer; 11.25 diagnostics; 43.16 explain-plan | [`introspect`] |
//! | 11.24 scaffolding and conformance; 11.25 local development; 41.10 change impact | [`devloop`] |
//! | 11.02, 11.03 "fail with actionable diagnostics" | [`lint`](mod@lint), [`catalogue`](mod@catalogue) |
//! | 23.32 local debugger | [`debugger`] |
//!
//! # The four things this crate asserts
//!
//! **A diagnostic states what would have to change.** [`Diagnostic::remedies`] is not optional
//! decoration: [`lint`](mod@lint) fails any catalogue entry without one, and a [`Remedy`] that
//! does not say how you would know it worked fails too. `bioprism-examples` applies the same rule to blocked
//! property claims; this is that rule at the error-message layer.
//!
//! **The retryability taxonomy is total.** Nine classes, each mapping to exactly one of terminal,
//! retryable-after-change, retryable-as-is, and to exactly one *surface that must change*. The
//! second axis is this crate's addition: "retry after caller change" without naming the surface is
//! a shrug with a type.
//!
//! **Introspection does not weaken the certificate's distinctions.** `bioprism-section` separates
//! *provably cannot matter* from *nobody checked*; [`introspect`] carries both through unchanged
//! and adds a third state of its own — *the record does not say* — rather than folding an unknown
//! into a zero.
//!
//! **Unknown blast radius is not empty blast radius.** [`devloop::invalidated_by`] refuses on a
//! path no declared surface owns, instead of returning an empty invalidation set that reads as
//! safety.
//!
//! # The exit-code audit
//!
//! `bioprism-cli` ships ten exit codes; the taxonomy has nine classes. [`exitaudit::audit`]
//! reports **no defects, two imprecisions and one note**. The two defects it used to report — exit
//! 4 carrying five classes, and `Stale` advertised as terminal when it is safe to retry after a
//! re-read — were fixed in the registry by splitting exit 4 into codes 4, 6, 7 and 8 and giving
//! `Stale` code 9. Both were first found by `bioprism-services`; this crate reproduced them from an
//! independent table, and the rows that reported them are computed from that table rather than
//! written out, so they stopped firing when the registry changed rather than being deleted. What
//! survives is `Internal` sharing exit 5 with `Unavailable`, reported twice — once as a collision
//! on the code and once as a meaning too narrow for the class — because the two classes retry and
//! page alike, so the sharing costs attribution rather than a decision.
//!
//! The detector is still held against a registry that fails: [`exitaudit::registry_before_the_split`]
//! retains the pre-fix table, and auditing it must still produce both defects. A clean result on
//! the shipped registry means nothing unless a dirty one is still reachable. See [`exitaudit`] for
//! the surviving imprecision and for why the registry is transcribed rather than imported.
//!
//! # Section 11 measured
//!
//! §11's twenty-five modules are **67.4% shared scaffolding**.
//!
//! Method, stated precisely because the number moves a lot with the definition: strip trailing
//! whitespace, drop blank lines, and count each remaining line *instance* (not each distinct
//! text). §11 has 1,670 non-blank line instances across 25 files, mean 66.8 per module. An
//! instance counts as scaffolding when its exact trimmed text also appears in at least one other
//! module. 1,168 of 1,670 qualify: 69.9%. Raising the threshold from "at least 2 modules" to "all
//! 25 modules" drops it only to 1,125 instances, or 67.4% — the shared frame is not a gradient,
//! it is a block of **exactly 45 lines present verbatim in every one of the 25 files**: five
//! invariants, five failure/mitigation pairs, five testing-strategy bullets, five metrics, four
//! implementation-sequence steps, and the two-paragraph "why this module exists". Mean
//! distinguishing content is 20.1 lines per module.
//!
//! Two other definitions of the same measurement, so the figure can be compared against somebody
//! else's: by *distinct text*, the corpus compresses 1,670 instances into 560 unique lines, a
//! ratio of 2.98; by *per-module unique share*, the range is 22% (11.02) to 38% (11.07). The
//! pairwise check the brief cites reproduces exactly — `13_PLUGIN_SDK.md` is 94 raw lines of which
//! 18 non-blank lines appear nowhere in `12_PLUGIN_AND_EXTENSION_SYSTEM.md`, frontmatter title and
//! H1 included.
//!
//! The number that mattered most while building this crate: the sentence *"emit an actionable
//! diagnostic rather than silently repairing or discarding state"* appears **125 times** in §11 —
//! five times in each of the twenty-five modules — and §11 never defines "actionable".
//! [`lint`](mod@lint) is this crate's answer.
//!
//! # Where §11 assumes tooling it never specifies
//!
//! - **"stable exit codes"** (11.03) is the *only* occurrence of the phrase "exit code" anywhere
//!   in §11. No code values, no registry, no retry mapping, and no reference to 40.36, which is
//!   where the retry semantics actually live. [`exitaudit`] exists because that gap is load
//!   bearing.
//! - **`doctor`** appears in both 11.02's and 11.03's command lists, and 11.25 asks for
//!   "platform-specific doctor checks". What it checks, what it prints and what it exits with are
//!   never stated.
//! - **"typo suggestions"** (11.03) implies an edit-distance ranker over the command tree: no
//!   metric, no threshold, no tie-break rule.
//! - **`prism config explain` prints every resolved value and source** (11.03) implies a
//!   provenance-tracking configuration loader, and 11.02's "effective configuration can be printed
//!   and hashed" implies a canonical serialisation of that configuration. Neither is specified,
//!   and neither cites the canonicalisation rules §43 already has.
//! - **`prism operation resume`, checkpoints, and "Ctrl-C produces a consistent canceled state"**
//!   (11.03) imply a durable operation log and a cancellation protocol. Neither exists in §11.
//! - **11.25's diagnostics list** — "structured logs, trace IDs, effective config, environment
//!   report, provider health, bundle verifier, and reproducible bug-report command" — names seven
//!   tools in one sentence and specifies none. A "reproducible bug-report command" implies
//!   capturing and redacting an entire environment.
//! - **11.13's generated tests** cover "timeout" and "cancellation", which need a clock and a
//!   cancellation signal that no §11 module defines.
//! - **The quickstart is specified twice, differently**: 11.02 says `uvx prism-eval demo`, 23.32
//!   says `uvx aurora-prism weave demo`. Different package, different subcommand, same claim.
//! - **23.32's visual language** requires round-trip editing to "preserve semantics and stable
//!   identifiers" without defining an identifier scheme for the nodes being round-tripped.
//!
//! # Deliberately not implemented
//!
//! - **No terminal, no rendering, no colour.** `bioprism-cli` owns output. A [`Diagnostic`] is a
//!   record; nothing here formats one for a width-80 console.
//! - **No watch loop, no filesystem, no process execution.** [`devloop`] maps a *declared* change
//!   onto a *declared* set of artefacts. It never reads a file, never notices one changed, and
//!   never runs a test it names.
//! - **No LSP and no actual debugger.** [`debugger`] models 23.32's surface and reports, pane by
//!   pane, what this workspace could supply. All five development-mode actions report
//!   [`RequiresLiveSession`](debugger::ActionAvailability::RequiresLiveSession), because there is
//!   no session here to act on.
//! - **No second plugin registry.** `bioprism-sdk` owns 40.16 registration, capability declaration
//!   and version negotiation. This crate converts its typed errors into diagnostics
//!   ([`from_negotiation_error`], [`from_manifest_error`]) and nothing more.
//! - **No second command tree and no second exit-code registry.** `bioprism-cli` owns both.
//!   [`exitaudit`] describes what it ships and never replaces it.
//! - **No second graph walk.** [`devloop`] calls `bioprism-docgraph`'s change-impact analysis.
//! - **No trace store and no divergence analysis.** `bioprism-trace` owns those.
//! - **No clock, no randomness, no network.** Every function here is deterministic in its inputs.
//! - **No natural-language processing.** [`lint`](mod@lint) matches lowercase substrings and
//!   checks structural fields; that ceiling is why its prose rules are warnings and not errors.

pub mod catalogue;
pub mod analysis_control;
pub mod debugger;
pub mod devloop;
pub mod diagnostic;
pub mod error;
pub mod exitaudit;
pub mod introspect;
pub mod lint;
pub mod taxonomy;

pub use catalogue::{catalogue, from_manifest_error, from_negotiation_error, lookup};
pub use analysis_control::{
    operate_analysis_control, AnalysisCandidate, AnalysisControlError, AnalysisControlReceipt,
    AnalysisDisposition, AnalysisPortfolio, AnalysisRequest, AnalysisState,
    CONTRACT_VERSION as ANALYSIS_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as ANALYSIS_CONTROL_FEATURE_ID,
    PRECLINICAL_BOUNDARY as ANALYSIS_CONTROL_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as ANALYSIS_CONTROL_SCHEMA_VERSION,
};
pub use debugger::{
    debugger_surface, ActionAvailability, DevAction, Pane, PaneAvailability, PaneModel,
    SurfaceReport,
};
pub use devloop::{
    invalidated_by, validate_contract, workspace_contract, ArtifactKind, ChangeUnit, Invalidated,
    InvalidationSet, Surface, SURFACE_CANONICAL_SERIALISATION,
};
pub use diagnostic::{
    Certainty, Diagnostic, DiagnosticCode, Discrepancy, ExplanationRequirement, LineSpan, Remedy,
    Site,
};
pub use error::{CatalogueError, CodeError, DevxError, IntrospectError, LoopError};
pub use exitaudit::{
    audit, audit_registry, registry_before_the_split, shipped_registry, AuditSeverity, ClassRouting,
    Divergence, DivergenceKind, ExitCodeAudit, RegistryUnderAudit, ShippedExitCode,
    SHIPPED_REGISTRY_SOURCE,
};
pub use introspect::{
    CompileRecord, DeveloperQuestion, IntrospectionCoverage, OmissionAnswer, OmissionEntry,
    PassOutcome, PassRecord,
};
pub use lint::{
    lint, lint_catalogue, lint_diagnostic, LintFinding, LintReport, LintRule, LintSeverity,
};
pub use taxonomy::{taxonomy_table, ChangeRequired, DiagnosticClass, Retryability, TaxonomyRow};
