//! The bio evaluation engine remainder: scoring planes, evaluator independence, and the parts of
//! the evaluation contract that are predicates rather than procedures.
//!
//! Implements the uncovered remainder of blueprint section 26 (Biological Evaluation Engine) and
//! section 07 (Evaluation Engine), on top of what `bioprism-bioeval`, `bioprism-evalengine`,
//! `bioprism-atlas` and `bioprism-metrics` already hold. Nothing here is a second scorer, a second
//! capability posterior or a second aggregation rule; where a sibling owns a question, this crate
//! either feeds it or stays out of it, and each such boundary is named at the module that would
//! otherwise have crossed it.
//!
//! # The classification
//!
//! Sixteen modules were in scope. The test applied was the one this workspace has settled on: *is
//! the detailed design a set of predicates over an artifact, or a description of what people do?*
//! The answer splits differently from the way it split for `crates/stewardship` (twelve of §14's
//! eighteen were whole process modules) or `crates/ops` (§40's thirteen split three ways across
//! modules). Here the split runs **inside** the modules rather than between them:
//!
//! | Class | Count | Modules |
//! |---|---|---|
//! | Predicates over an artifact — implemented here | 13 | 26.01, 26.03, 26.05, 26.06, 26.07, 26.09, 26.11, 26.12, 26.16, 26.17, 26.18, 07.03, 07.09 |
//! | Part predicate, part infrastructure or process — the predicate half implemented, the rest named | 2 | 07.02, 07.13 |
//! | Already discharged by a sibling crate — deliberately **not** cited | 1 | *BioCapability Atlas and Posterior Profiles* |
//!
//! Not one of the sixteen was a *pure* process module. That is the finding, and it is the opposite
//! of §14's. Section 26's process content is real but it is not distributed by module: every one
//! of its twenty-four files ends with the same four blocks — "Diagnostic outputs", "Required
//! baselines", "Statistical analysis", "Release gates" — describing what a study author must do,
//! and every one of them opens with a purpose, an evaluation target, a numbered protocol and a
//! failure-mode list that are checkable. So the honest classification is per *block*, not per
//! module, and the numbers below say how much of each section is which.
//!
//! ## The one module deliberately not cited
//!
//! **BioCapability Atlas and Posterior Profiles** (§26, module 19). Its protocol is "map cells to
//! taxonomy, fit hierarchical difficulty and capability models, retain benchmark-family effects,
//! update posteriors, detect regressions and drift, publish uncertainty and sample support". Every
//! clause of that already exists: `bioprism-atlas` owns the capability × evidence object, the
//! coverage report and the holes list; `bioprism-metrics` owns clustering, intervals, worst-domain
//! reporting and the rule that an aggregate over a grid containing an unmeasured cell is not an
//! aggregate over the grid; `bioprism-evalengine::posterior` owns the capability posterior and its
//! coverage floors. Implementing it here would produce a second atlas with its own opinion about
//! what a hole is, which is worse than an uncovered id. Its module number appears nowhere in this
//! crate, so `tools/coverage.sh` will keep reporting it uncovered. That is correct: it is covered
//! by crates that do not name it, and the fix is a citation in one of those, not a reimplementation
//! in this one.
//!
//! ## What the two split modules leave behind
//!
//! - **07.02, Deterministic, Property, and Execution Evaluators.** The evaluable half — telling a
//!   broken harness from a failing system — is [`evaluator`]. The rest is a sandbox: a separate
//!   evaluator image, grader files mounted after agent execution, read-only artifact access, no
//!   ambient credentials. `bioprism-safety` states this workspace's position on such controls — a
//!   library of plain Rust types may model one and may not claim one — and this crate spawns no
//!   process, so those are named and not built.
//! - **07.13, Release Gates and CI Policy.** The gate arithmetic is `bioprism-metrics::gate` and
//!   the coverage floors are `bioprism-evalengine::posterior`; the change-aware panel is
//!   `bioprism-adaptive`'s. What was genuinely unowned is the override paragraph — rationale,
//!   expiry, affected versions, required follow-up — and that is [`waiver`]. The CI comment's
//!   content and the review cadence are process and stay process.
//!
//! # How much of these two sections is template
//!
//! Method: normalise each line by trimming whitespace, drop blank lines, and count a line as shared
//! when the identical line occurs in at least *k* of the section's module files. Section 26 has 24
//! module files, section 07 has 13. Section indexes are excluded.
//!
//! | Section | Shared lines | Share | Mean lines/module |
//! |---|---|---|---|
//! | 26 | 696 / 1365 | **51.0%** | 56.9 |
//! | 07 | 585 / 824 | **71.0%** | 63.4 |
//!
//! ## Sensitivity
//!
//! **Threshold: no effect at all.** *k* = 50%, 80% and 100% give identical answers in both
//! sections. Every shared line in these two corpora is shared by *every* module — there is no
//! middle stratum of lines common to some files and not others. This is the pure skeleton regime,
//! the opposite of the one §39 reported, where a corpus split between skeleton files and free prose
//! made the threshold dominate. Nothing about the numbers above depends on where the threshold is
//! put, which is worth knowing because it is the parameter most often argued about.
//!
//! **Definition: the dominant parameter for §26, nearly irrelevant for §07.** Including or
//! excluding YAML front matter and markdown headings moves §26 across a 17.6-point range
//! (33.4% with both stripped, 41.4% front-matter only, 46.1% headings only, 51.0% with both kept)
//! and §07 across 5.2 points (69.2%–74.4%). The asymmetry has a cause: §07's headings differ
//! between modules — its `### Channels`, `### Gate types`, `### Bounded suffix` are real,
//! module-specific structure — while §26's headings are identical across all twenty-four files, so
//! counting them inflates §26 and slightly deflates §07. This matches the settled finding that YAML
//! front matter alone moved §23 by 4.3 points and §43 by 15.1.
//!
//! **Unit: comparable to definition.** Counting whole `##` blocks instead of lines gives §26 40.0%
//! (96/240 blocks) and §07 55.6% (65/117); counting body *bytes* gives §26 56.5% and §07 64.9%.
//! Across the three units §26 spans 16.5 points and §07 15.4.
//!
//! So: **§26 is in the regime where definition dominates and unit is close behind; §07 is in the
//! regime where unit dominates and definition barely matters; neither is sensitive to threshold at
//! all.** The blocks identical in every file are, for §26, "Diagnostic outputs", "Required
//! baselines", "Statistical analysis" and "Release gates"; for §07, "Invariants", "Failure modes
//! and mitigations", "Testing strategy", "Metrics" and "Implementation sequence".
//!
//! # Every place these two sections name a metric or a rule they never define
//!
//! Counted over the sixteen in-scope modules. Section 26's per-module "Metrics" list is six bare
//! noun phrases; across the twelve in scope that is **72 metric names, none carrying a formula, an
//! estimator or a denominator**. Section 07's identical "Metrics" block contributes five more per
//! module, of which "false-positive and false-negative rates for any diagnostic or safety decision"
//! is the only one with even an implied denominator. The specific gaps that bit while implementing:
//!
//! - *oracle disagreement rate*, *ungradable rate*, *adjudication stability* (26.01) — no
//!   denominator. [`mesh::Mesh::disagreements`] returns the pairs; the ratio is the caller's.
//! - *grounded-claim precision*, *evidence recall*, *stale-evidence rate* (26.03) — no denominator,
//!   and "recall" has no defined ground-truth evidence set at all. [`grounding::Census`] is a
//!   partition of five states instead.
//! - *information gain per token/cost*, *regret versus oracle acquisition policy* (26.05) — no
//!   posterior and no named oracle policy; 26.05 lists five candidate baselines and privileges
//!   none. [`acquisition::Trace::regret_against`] refuses without one.
//! - *avoidable tissue consumption*, *decision utility per specimen unit*, *resource-regret versus
//!   optimal policy* (26.06) — "avoidable" and "optimal" both need a counterfactual policy the
//!   section never supplies. [`burden::Ledger::wasted_nonrenewable`] reports the one subset that
//!   needs no counterfactual: material destroyed for an action that then failed.
//! - *forecast calibration*, *censoring-aware metric*, *revision quality* (26.07) — need a survival
//!   model and a belief trajectory.
//! - *bias under simulation*, *identification validity*, *decision regret* (26.09) — no graph
//!   format and no identification criterion, so [`estimand::Identification`] records what was
//!   claimed and checked and infers nothing.
//! - *robustness slope*, *mutation-family coverage* (26.12) — no perturbation magnitude axis, and
//!   no admission rule that is not already `bioprism-mutation`'s.
//! - *prospective success*, *replication rate* (26.16) — both need a scoring rule over the
//!   prediction's own state space, which `bioprism-bioeval` owns.
//! - *time to detect injection*, *minimum necessary disclosure* (07.09) — need a detector and a
//!   counterfactual task.
//! - 26.11's *discrepancy localization time* and 07.02's *contributor usability: time to first
//!   successful extension* are stopwatch readings about people, not properties of an artifact.
//!
//! Two *rules* are named and left undefined in a way that matters more than the metrics. 26.01 says
//! "a mesh is not a majority vote" and that "the verdict policy is task-specific", without saying
//! what a policy is or how one is written; this crate therefore produces
//! [`bioprism_evalengine::Contribution`]s and lets that crate's ladder decide, rather than inventing
//! a policy language. 07.13 requires that a statistical gate use "intervals, posterior
//! probabilities, or sequential tests" and specifies none of the three; `bioprism-metrics` reports
//! such a gate as *unevaluable*, and [`waiver`] consumes that verdict rather than guessing.
//!
//! # Shape
//!
//! - [`plane`] — the spine. Three cell states, and no path from unscored to zero (26.17).
//! - [`mesh`] — evaluator independence and what disagreement is evidence *of* (26.01).
//! - [`grounding`] — claims, typed evidence edges, and contested as its own state (26.03).
//! - [`acquisition`] — obligations closed, redundancy, stopping, and refused regret (26.05).
//! - [`burden`] — a nonrenewable ledger where two forks cannot spend one aliquot (26.06).
//! - [`worldline`] — four clocks per observation and leakage as a witness (26.07).
//! - [`estimand`] — five required elements and a label a simulator cannot shed (26.09).
//! - [`repro`] — a certificate that refuses to be a validity claim (26.11).
//! - [`metamorphic`] — false invariance and false sensitivity, never summed (26.12).
//! - [`reveal`] — seal, commit, reveal, and a rubric digest that cannot move (26.16).
//! - [`design`] — the factorial design that makes an attribution askable (26.18).
//! - [`evaluator`] — telling a broken harness from a failing system (07.02).
//! - [`trajectory`] — path properties without a reference path (07.03).
//! - [`boundary`] — contextual integrity, and the composite that is refused (07.09).
//! - [`waiver`] — the release-gate override, and the veto that cannot be waived (07.13).
//!
//! # What this crate is not
//!
//! No scoring rule, no reference distribution, no partial-credit arithmetic — `bioprism-bioeval`
//! owns all three, including the rule that reference dispersion selects the scoring rule.
//! [`mesh::Mesh::independent_ratings`] *feeds* its reader panel and corrects the one thing that
//! panel cannot see — that several of its raters read one source — and stops there. No evidence
//! ladder and no component attribution: `bioprism-evalengine` owns those, and [`mesh`] and
//! [`design`] emit its input types rather than reimplementing its refusals. No capability atlas,
//! no coverage report, no
//! aggregation across systems, no ranking. No panel selection or scheduling (`bioprism-adaptive`).
//! No execution of anything: no process, no clock, no randomness, no network. Every number in this
//! crate arrived from a caller who measured it, and every fixture in the tests is constructed by
//! hand so that nothing here can be mistaken for evidence about biology.

pub mod acquisition;
pub mod boundary;
pub mod burden;
pub mod design;
pub mod error;
pub mod estimand;
pub mod evaluator;
pub mod federation_release;
pub mod grounding;
pub mod mesh;
pub mod metamorphic;
pub mod plane;
pub mod quality_assurance;
pub mod repro;
pub mod reveal;
pub mod trajectory;
pub mod waiver;
pub mod worldline;

pub use acquisition::{AcquisitionKind, Action, Obligation, ReferencePolicy, Regret, Trace};
pub use boundary::{Assessment, Channel, Effect, Flow, FlowVerdict, Policy};
pub use burden::{BranchLedger, Draw, DrawOutcome, Ledger, Resource, ResourceClass};
pub use design::{Arm, Contrast, FactorialDesign};
pub use error::{
    AcquisitionError, BoundaryError, BurdenError, DesignError, EstimandError, EvaluatorError,
    GroundingError, MeshError, MetamorphicError, PlaneError, ReproError, RevealError,
    TrajectoryError, WaiverError, WorldlineError,
};
pub use estimand::{
    ClaimKind, Corroboration, Estimand, Evidentiary, Finding, Identification, IdentificationCheck,
};
pub use evaluator::{Diagnostic, EvaluatorRun, Health, Panel, TaskOutcome};
pub use federation_release::{
    federation_gateway_manifest, prepare_federation_release, FederationGatewayError,
    FederationGatewayReceipt, FederationGatewayRequest, GatewayDisposition,
    RunState as FederationRunState, SignedResearchObject as FederationSignedResearchObject,
    ValidatedResearchRun as FederationValidatedResearchRun,
    CONTRACT_VERSION as FEDERATION_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as FEDERATION_GATEWAY_FEATURE_ID,
};
pub use grounding::{ClaimState, EdgeKind, Evidence, Grounding, LocatorStatus, SupportEdge};
pub use mesh::{
    Census, Disagreement, EvaluatorDecl, EvaluatorKind, EvaluatorVerdict, Mesh, Witness,
};
pub use metamorphic::{
    verdict, Direction, Family, FamilyReport, Relation, Response, Suite, Trial, TrialVerdict,
};
pub use plane::{
    CapabilityTier, Cell, Dimension, ExcludedDimension, Fold, FoldPolicy, Score, ScorePlane,
    UnscoredReason,
};
pub use quality_assurance::{
    assure_quality, QualityAssuranceError, QualityAssuranceReceipt, QualityAssuranceRequest,
    QualityDisposition, QualityMetric, QualityState, QualityVerdict, ResearchObject,
    CONTRACT_VERSION as QUALITY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as QUALITY_ASSURANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY as QUALITY_ASSURANCE_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as QUALITY_ASSURANCE_SCHEMA_VERSION,
};
pub use repro::{Certificate, Observed, OutputKind, OutputSpec, OutputVerdict, Reexecution};
pub use reveal::{Commitment, Outcome, Registration, Revealed, Scoring, Sealed};
pub use trajectory::{BoundedSuffix, PathProperty, PropertyOutcome, Step, Trajectory};
pub use waiver::{Gate, GateKind, GateVerdict, ReleaseDecision, WaivedGate, Waiver};
pub use worldline::{Clock, Decision, LeakWitness, Observation, Worldline};

/// The blueprint modules this crate implements, as a checkable list.
///
/// Present so the citation set is data rather than a claim in prose. `tests/citations.rs` reads
/// every source file, extracts every `NN.MM` token, and fails if the set of section-26 and
/// section-07 ids found is not exactly this one. The rule it enforces is the workspace's: citing a
/// module moves the coverage number, so a citation not backed by an implementation moves the number
/// without moving capability.
pub const IMPLEMENTED_MODULES: [&str; 15] = [
    "07.02", "07.03", "07.09", "07.13", "26.01", "26.03", "26.05", "26.06", "26.07", "26.09",
    "26.11", "26.12", "26.16", "26.17", "26.18",
];

/// Section-26 and section-07 ids this crate may mention without implementing them.
///
/// One entry. 07.05 (score model, partial credit and aggregation) is `bioprism-evalengine`'s and is
/// already covered there; [`error::PlaneError`] names it because the refusal it documents is that
/// crate's rule seen from this side. Mentioning it moves no coverage number, and the allowance is
/// enumerated here rather than left implicit so that the next such mention has to be argued for.
pub const CITED_BUT_OWNED_ELSEWHERE: [&str; 1] = ["07.05"];

/// Schema version for everything this crate serializes.
///
/// Separate from `bioprism-evalengine`'s schema version because these are different objects; a
/// consumer merging a stored [`plane::Fold`] across a bump is reading a number whose denominator
/// rule may have changed.
pub const BIOEVALX_SCHEMA_VERSION: &str = "bioprism-bioevalx/0.1";
