#![allow(clippy::all)]

//! Two hub surfaces, and a register of everything their sections named and never defined.
//!
//! The scope was the remainder of two blueprint sections: ten modules of **BioCapability Atlas and
//! Metrics**, and twelve of **BioAtlas Public Hub and Ecosystem**. Two of the twenty-two are
//! implemented here — 34.08 (BioCapability Atlas) and 34.09 (Failure Atlas and First-Divergence
//! Browser) — and those two ids are the only ones written anywhere in this crate. The other twenty
//! are named by title, in prose, for the reason `bioprism-atlashub` gave when it did the same:
//! `tools/coverage.sh` counts any `NN.MM` token under `crates/` as a position taken, and citing a
//! module in order to move a percentage is the one thing this workspace must not do.
//!
//! # The classification, which is the headline
//!
//! The test, inherited from `bioprism-atlashub`: *is the detailed design a set of predicates over an
//! artifact, or a description of what people see and do?*
//!
//! | Section remainder | Code-bearing | Interface or process | Named, undefined |
//! |---|---|---|---|
//! | capability metrics, 10 modules | **0** | 10 | 0 |
//! | public hub, 12 modules | **2** | 10 | 0 |
//!
//! **Zero of the ten capability-metrics modules is code-bearing.** `bioprism-metrics` reported that
//! its section names 222 metrics across nineteen modules and that not one carries a formula, an
//! estimator, or a stated denominator. The ten left over inherit that exactly: each is the same
//! template — a one-sentence purpose, six measurement-dimension nouns, five or six metric names, one
//! worked-interpretation sentence — above four blocks (required stratification, statistical
//! reporting, failure-aware visualization, release gates) that are byte-identical across all
//! nineteen and that `bioprism-metrics` already implements. Strip the shared blocks and what is left
//! defines nothing. The honest response is to say so, which is what [`catalogue`] does with 56
//! entries and no estimators.
//!
//! Two of the ten came closest and are worth naming, because both were considered and both were
//! declined on the merits rather than for want of room:
//!
//! - *Translation Spine and Evidence Maturity Metrics* names "weakest-link maturity", and the name
//!   itself implies a reduction — the minimum over a path. But the spine is
//!   `bioprism-foundation`'s, which already refuses a path with a missing edge, refuses an edge whose
//!   evidence is ungraded, and declines to invent the penalty magnitudes for the same reason. A
//!   second spine here would be a second spine.
//! - *Multi-Agent Coordination and Molecule Value* defines "team gain" in its purpose sentence as
//!   value "beyond the best single participant", which is a stated baseline and one of very few in
//!   the section. It is also an aggregation rule, and this crate does not build a second one.
//!
//! **Two of the twelve public-hub modules are code-bearing, and this refines rather than overturns
//! `bioprism-atlashub`'s reading.** That crate classified all twelve as interface or process, and it
//! was right about what it was judging: the atlas *rendering* and the failure *browser* are
//! presentation, and there is no renderer here either. What survives is not the surface but the
//! **bound on the surface** — what an aggregation built to be looked at is permitted to answer — and
//! that is a predicate over an artifact.
//!
//! # The two invariants, in their native currency
//!
//! **A coverage debt is a claim about what was not measured.** `bioprism-metrics` made a bare
//! aggregate unrepresentable: `BareScore` implements neither `Serialize` nor `Deserialize`, so a
//! score cannot be published without its coverage. [`DebtStatement`] is the same fact from the other
//! side. It has private fields, one constructor that reads a `bioprism_metrics::CapabilityGrid`, and
//! a deserialization path that recomputes its own summary and refuses a stored statement its
//! contents contradict. There is no `DebtStatement::new(total, unmeasured)`, because a pair of
//! integers is precisely the artifact — a "94% covered" badge with nothing behind it — the type
//! exists to prevent. See [`debt`].
//!
//! **Browsing is not evidence.** A grouped chart of failures invites one question — *how often does
//! it do that?* — and a set of failure records cannot answer it: it says how many failures were
//! recorded and is silent on how many times the thing was attempted. [`FailureBrowse`] therefore
//! declares [`Question::RatePerAttempt`] and refuses it with
//! [`Unanswerable::NoAttemptDenominator`]; the rate becomes available only through
//! [`FailureBrowse::rate_against`], which must be handed a grid that is a reading of the *same*
//! subject and a cell that is not a hole. `bioprism-lens` made non-visual answerability a type-level
//! bound; the idea is reused here against a different failure, and none of the code is. See
//! [`browse`].
//!
//! # Shape
//!
//! - [`surface`] — [`SurfaceCell`], which has no blank; [`Question`], [`Answer`] and
//!   [`Unanswerable`]; the [`Surface`] trait and [`audit`], which checks a surface against its own
//!   declaration.
//! - [`debt`] — 34.08. [`DebtStatement`], [`Hole`], and [`Discharge`], which separates a hole that
//!   was measured from a hole that was declared away.
//! - [`browse`] — 34.09. [`FailureBrowse`], its facets, and the withheld-record rule.
//! - [`catalogue`] — the 118 metrics in scope, the 117 nothing defines, and the three numbers this
//!   crate emits with their denominators written down.
//!
//! # Vocabularies borrowed rather than minted
//!
//! Both sections share a paragraph requiring the surface to support unavailable, controlled, stale,
//! under-review, disputed, withdrawn, non-reproducible and not-comparable states, and forbidding
//! their replacement by "zero, empty, or hidden values". `bioprism_hub::PublicationState` already
//! enumerates all eight, so [`SurfaceCell::Withheld`] carries it. `bioprism_atlas::UnmeasuredReason`
//! already says why nobody measured, so [`SurfaceCell::Hole`] carries that. A ninth vocabulary would
//! have been the same failure one level up: two enums for one distinction is how a renderer ends up
//! drawing `Withdrawn` as a blank because it only learned about the other enum.
//!
//! # What is not built here
//!
//! - **No second atlas.** `bioprism-atlas` is the atlas. A [`DebtStatement`] holds no scores, no
//!   evidence, no ontology and no cells; it reads a grid and keeps identifiers.
//! - **No second aggregation rule, leaderboard or ranking.** `bioprism-metrics` owns aggregation,
//!   comparability and partial order; `bioprism-hub` owns boards and submissions.
//!   [`Question::ComparisonWith`] is refused by both surfaces here.
//! - **No second lens.** `bioprism-lens` answers *what question is this a view of, and did anyone
//!   check*, over compiled Decision Sections. This crate answers *may this aggregate be shown as a
//!   number*, over objects a hub already holds.
//! - **No estimators.** The one blueprint metric given a definition is *profile coverage*, and its
//!   documentation says the denominator is a choice this crate made rather than one the module
//!   states.
//! - **No renderer, no server, no clock, no randomness.** Ordering is `BTreeMap` throughout, so the
//!   same inputs produce the same buckets in the same order in any process.
//!
//! # Both sections, measured
//!
//! Method, matching the one `bioprism-atlashub` used so the numbers are comparable: over a section's
//! module files, count non-blank lines with surrounding whitespace trimmed; a line is *unique* if it
//! occurs in exactly one file across the section; boilerplate is the complement. The character basis
//! weights each line by its length instead of counting it once.
//!
//! | Section | Front matter | Unique / total | By line | By character |
//! |---|---|---|---|---|
//! | capability metrics, 19 modules | counted | 296 / 1210 | **75.5%** | **76.7%** |
//! | capability metrics, 19 modules | stripped | 277 / 1077 | 74.3% | 76.8% |
//! | public hub, 23 modules | counted | 348 / 1339 | **74.0%** | 82.2% |
//! | public hub, 23 modules | stripped | 325 / 1178 | **72.4%** | 82.6% |
//!
//! The public-hub figures reproduce `bioprism-atlashub`'s exactly: 74.0% counting front matter,
//! 72.4% stripped, a 1.6-point sensitivity. The capability-metrics figures come out 0.1 point above
//! the 75.4% / 76.6% previously reported — about two lines in 1210 — and the numerators and
//! denominators are printed above so the difference is checkable rather than arguable. Including
//! each section's index file moves both by a further 0.2 to 0.5 points and is the likelier source of
//! small disagreements, since an index is not a module and counting it inflates the unique share.
//!
//! One asymmetry is worth reading off the table. In the capability-metrics section the line and
//! character bases agree to within a point, because its shared blocks are long prose sentences and
//! its unique content is long prose sentences. In the public-hub section the character figure is
//! eight points *higher* than the line figure, because what varies between its modules is short
//! bullets — "findability", "cutoff correctness" — while what repeats is an eight-step user flow, a
//! seven-line trust list and a JSON object. Counting lines therefore flatters the public-hub section
//! considerably: by weight, it is 82% the same document twenty-three times.

pub mod browse;
pub mod catalogue;
pub mod debt;
pub mod error;
pub mod surface;

pub use browse::{
    browse, browse_with_visibility, Bucket, BucketKey, Facet, FailureBrowse, Visibility,
};
pub use catalogue::{
    by_module, named_in_scope, DefinedMetric, NamedMetric, Origin, DEFINED_HERE,
    METRICS_NAMED_IN_SCOPE, NAMED_NEVER_DEFINED,
};
pub use debt::{DebtStatement, DebtStatementFields, Discharge, Hole};
pub use error::AtlasxError;
pub use surface::{
    audit, Answer, Audit, Question, Share, ShareFields, Surface, SurfaceCell, Unanswerable,
};

/// The schema version of the surface objects this crate emits.
///
/// Bumped when a serialized shape changes, because a stored [`DebtStatement`] read under different
/// derivation rules is a debt claiming a coverage it does not have — the same reason
/// `bioprism-metrics` versions its own.
pub const ATLASX_SCHEMA_VERSION: &str = "bioprism-atlasx/0.1";
pub mod computational_execution_assurance;
pub mod context_compilation_assurance;
pub mod federated_execution_control_plane;
pub mod mechanism_contract_model;
pub use computational_execution_assurance::{
    assure_computational_execution, computational_execution_assurance_manifest,
    ComputationalExecutionError, ExecutionArtifact7, ExecutionEvidenceState, ExecutionNode3,
    ExecutionRun7, ResearchWorkflowSpec3,
    CONTRACT_VERSION as COMPUTATIONAL_EXECUTION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as COMPUTATIONAL_EXECUTION_ASSURANCE_FEATURE_ID,
};
pub use context_compilation_assurance::{
    compile_context, context_compilation_assurance_manifest, CompiledResearchContext6,
    ContextCompilationDisposition, ContextCompilationError, ContextCompilationQuestion4,
    ContextFragment5, CONTRACT_VERSION as CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
};
pub use federated_execution_control_plane::{
    federated_execution_control_plane_manifest, plan_federated_execution, ExecutionDisposition,
    ExecutionNode5, ExecutionRun8, FederatedExecutionError, PeerAttestation5,
    ResearchWorkflowSpec4, CONTRACT_VERSION as FEDERATED_EXECUTION_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_EXECUTION_CONTROL_FEATURE_ID,
};
pub use mechanism_contract_model::{
    admit_atlasx_mechanism_contract, mechanism_contract_model_manifest, AtlasxMechanismCandidate5,
    AtlasxMechanismPeer5, AtlasxMechanismPortfolio2, AtlasxMechanismPortfolio2Artifact,
    AtlasxMechanismQuestion4, MechanismContractModelError, MechanismEvidenceState,
    CONTRACT_VERSION as ATLASX_MECHANISM_CONTRACT_VERSION,
    FEATURE_ID as ATLASX_MECHANISM_FEATURE_ID,
};
