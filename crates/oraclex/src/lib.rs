#![allow(clippy::all)]

//! Reference standards as claims about measurement processes, and the release program that decides
//! whether a mutated case may ship.
//!
//! Covers the checkable remainder of blueprint section 31 (Biological Oracles and Reference
//! Standards) and section 32 (Biological Mutation and Stress Program). `bioprism-oracle` already owns
//! §31's mesh — the ladder, set-valued combination, disagreement, expiry — and `bioprism-mutation`
//! and `bioprism-stress` own §32's generators. This crate is what neither of them is: the *reference
//! standards themselves*, and the *validation program* that stands between a generated case and a
//! released pack.
//!
//! # The two rules everything here is a restatement of
//!
//! **An oracle that cannot decide must abstain, and abstention is not a verdict of "valid".**
//! `crates/bioworlds` built a world that genuinely underdetermines its question and found the v0.1
//! oracle answering `valid` on it — a wrong answer rather than a missing one. [`Determination`] makes
//! the honest answer cheap and the dishonest one impossible to reach by accident: [`Unresolved`]
//! cannot be constructed without naming at least one [`Missing`] item, and there is no `is_valid`
//! that folds abstentions in with support.
//!
//! **A reference standard is a claim about a measurement process, not about truth.**
//! [`standard::ReferenceBasis`] enumerates the processes that produce references — readers, an
//! orthogonal assay, a versioned classifier, a simulator, a record, a later event — and has no
//! variant meaning "the biological fact". Two oracles disagreeing is a finding with a witness;
//! `crates/choreography`'s rule applies unchanged, so [`endpoint::reconcile`] answers unresolved and
//! names the unranked pair rather than defaulting to the higher-authority source.
//!
//! # Classification: what these two sections actually are
//!
//! Twenty-one modules were in scope, and the test applied to each was the one this workspace keeps
//! needing: *is the detailed design a set of predicates over an artifact, or a description of what
//! people do?* **Nineteen are predicates and are implemented. One is mostly programme and is
//! implemented only in the part that is not. One is half predicate and half search procedure, and the
//! search half is absent.**
//!
//! Section 31 is nearly all predicate. §31.05 through §31.12 give fingerprint concordance, reader
//! panels and consensus rules, modality independence, perturbation controls, frozen snapshots,
//! geometric reference levels, integrated-label separation, and date-source hierarchies — every one of
//! them a check over an artifact. **§31.17, "Oracle Audit, Independence and Quality Management", is
//! mostly programme**: convening reviewers, running inter-laboratory comparisons, publishing
//! limitations and conflicts, maintaining a quality-management system. Three of its clauses are
//! decidable and are in [`audit`] — separation of authoring from hidden grading, the worked case's two
//! prohibitions on a sponsor, and release gate 6's independent reproduction. The rest is not
//! implemented and is not claimed.
//!
//! Section 32 is eleven predicates and one boundary case. §32.05, §32.10, and §32.12 through §32.20
//! each name operators, a validation program, and failure risks that are predicates over a declaration
//! or over a pair of artifacts. **§32.21, "Mutation Composition, Interactions, and Failure
//! Minimization", is half predicate and half search**: its composition and minimality conditions are
//! checkable and are in [`compose`], but "delta debugging over artifacts and state" is a search over
//! candidate subsets driven by re-running a failing pipeline, and "human review for nonlocal effects"
//! is a meeting. Neither is here.
//!
//! Every module id this crate cites is one it checks something for. Two ids appear in prose only and
//! are deliberately outside the citation set: 31.13 (synthetic, mechanistic and semi-synthetic
//! oracles) and 31.16 (oracle versioning, drift and expiration), both already owned by
//! `bioprism-oracle` and `bioprism-worldfactory`.
//!
//! # What lives where
//!
//! | Module | Blueprint | The predicate that earns it |
//! |---|---|---|
//! | [`identity`] | 31.05, 32.05 | molecular evidence outranks a textual join; a mixture leaves pairwise identity underdetermined |
//! | [`panel`] | 31.06, 32.18 | changing the consensus rule changes the reference and never erases the reads |
//! | [`orthogonal`] | 31.07, 32.13 | channels sharing a failure mode cannot confirm each other; discordance lists what would explain it |
//! | [`perturbation`] | 31.08, 32.17 | a mechanism claim needs two modalities and a rescue; observational evidence reaches no interventional plane |
//! | [`longitudinal`] | 31.09 | a snapshot cannot grow; escrow opens only through a rule predeclared before the freeze |
//! | [`standard`] | 31.10, 31.11, 32.12 | a level nobody measured is unresolved; regrades append |
//! | [`endpoint`] | 31.12 | an unranked source pair is unresolved; unknown is neither censored nor ineligible |
//! | [`audit`] | 31.17, in part | no party both authors and hides the grading; no finding is withheld |
//! | [`missing`] | 32.10, 32.19 | absence is typed and has no route to a number; individual data does not cross an aggregate boundary |
//! | [`units`] | 32.16 | no conversion without a caller-supplied factor; a value inside its own precision of a cut is unresolved |
//! | [`citation`] | 32.15 | a citation with no passage supports nothing; untrusted text cannot become a directive |
//! | [`execution`] | 32.14, 32.20 | a partial write is not a result; a twin's calibration and test sets are disjoint |
//! | [`compose`] | 32.21, in part | two non-invariant transformations that compose to invariance cancelled |
//! | [`program`] | the §32 release gates | a disposition that names every gate it could not meet |
//!
//! # No invented biology
//!
//! Not one biological constant appears in this crate. Every threshold, tolerance, floor, precision and
//! conversion factor is a parameter with no default: [`units::ConversionTable`] starts empty and
//! [`units::convert`] fails rather than guess a molecular weight, [`missing::egress`] takes its
//! small-cell floor from the caller, and [`units::threshold_call`] takes the measurement's own
//! precision. 32.05's worked relation mentions a fifteen-percent mixture; that number is its
//! illustration and is not in the code. A test scans [`SOURCES`] and enforces this.
//!
//! # Determinism
//!
//! No clock, no randomness, no I/O. Every timestamp is caller-supplied and every collection is a
//! `BTree`, so serialisation order is stable across runs and machines.

pub mod audit;
pub mod citation;
pub mod compose;
pub mod context_compilation_research_copilot;
pub mod endpoint;
pub mod error;
pub mod execution;
pub mod identity;
pub mod interpretation_engine;
pub mod longitudinal;
pub mod missing;
pub mod orthogonal;
pub mod panel;
pub mod performance_reliability_interoperability_gateway;
pub mod perturbation;
pub mod program;
pub mod publication_release_contract_model;
pub mod standard;
pub mod statistical_analysis_research_workbench;
pub mod units;
pub mod verdict;

pub use error::OracleXError;
pub use interpretation_engine::{
    assure_interpretation, interpretation_inference_manifest, EvidenceBackedResult3,
    EvidenceState as InterpretationEvidenceState, InteractiveInterpretation1,
    InterpretationCandidate3, InterpretationInferenceError,
    CONTRACT_VERSION as INTERPRETATION_INFERENCE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_INFERENCE_FEATURE_ID,
};
pub use performance_reliability_interoperability_gateway::{
    negotiate_performance_reliability, performance_reliability_interoperability_gateway_manifest,
    CapabilityInvocation5, CapabilityWorkload4, PerformanceReliabilityInteroperabilityError,
    ReliableCapabilityResult6, WorkloadEvidenceState,
    CONTRACT_VERSION as PERFORMANCE_RELIABILITY_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as PERFORMANCE_RELIABILITY_INTEROPERABILITY_FEATURE_ID,
};
pub use statistical_analysis_research_workbench::{
    qualify_statistical_analysis, statistical_analysis_research_workbench_manifest,
    AnalysisCandidate5, AnalysisDisposition, AnalysisQuestion4, QualifiedAnalysisResult5,
    StatisticalAnalysisWorkbenchError,
    CONTRACT_VERSION as STATISTICAL_ANALYSIS_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as STATISTICAL_ANALYSIS_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use verdict::{
    Contradiction, Determination, Missing, NotEvaluable, Support, Unresolved, Witness,
};

/// This crate's own source, for the tests that check the code rather than its behaviour.
///
/// `crates/modalities` established the pattern: "no invented constants" is not expressible as a type,
/// so it is expressed as a test over the text. `include_str!` bakes the sources in at compile time, so
/// the test reads no files at runtime and stays deterministic.
pub const SOURCES: [(&str, &str); 21] = [
    ("audit.rs", include_str!("audit.rs")),
    ("citation.rs", include_str!("citation.rs")),
    ("compose.rs", include_str!("compose.rs")),
    (
        "context_compilation_research_copilot.rs",
        include_str!("context_compilation_research_copilot.rs"),
    ),
    ("endpoint.rs", include_str!("endpoint.rs")),
    ("error.rs", include_str!("error.rs")),
    ("execution.rs", include_str!("execution.rs")),
    ("identity.rs", include_str!("identity.rs")),
    ("lib.rs", include_str!("lib.rs")),
    ("longitudinal.rs", include_str!("longitudinal.rs")),
    ("missing.rs", include_str!("missing.rs")),
    ("orthogonal.rs", include_str!("orthogonal.rs")),
    ("panel.rs", include_str!("panel.rs")),
    (
        "performance_reliability_interoperability_gateway.rs",
        include_str!("performance_reliability_interoperability_gateway.rs"),
    ),
    (
        "statistical_analysis_research_workbench.rs",
        include_str!("statistical_analysis_research_workbench.rs"),
    ),
    ("perturbation.rs", include_str!("perturbation.rs")),
    ("program.rs", include_str!("program.rs")),
    (
        "publication_release_contract_model.rs",
        include_str!("publication_release_contract_model.rs"),
    ),
    ("standard.rs", include_str!("standard.rs")),
    ("units.rs", include_str!("units.rs")),
    ("verdict.rs", include_str!("verdict.rs")),
];
