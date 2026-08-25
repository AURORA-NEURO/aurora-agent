//! The benchmark compiler.
//!
//! Implements blueprint section 06. Section 06 is a compiler in the ordinary sense: the input is an
//! observed research trajectory, the passes are semantic analyses over it, and the output is a set
//! of Decision Cells any architecture can be resumed from. 06.01 states the pipeline; this crate is
//! the stages of it that no other crate owns.
//!
//! ```text
//! segment     trace -> episodes, repetition, typed decision boundaries        06.02  06.03
//!    |        boundaries are ranked by bioprism_trace::segment, not re-ranked here
//! reconstruct decision -> the options that existed, behind a hindsight firewall  06.04
//!    |        a future-sourced option validates the set; it is never shown to an agent
//! diverge     failing x reference -> the earliest step the agent actually caused  06.05
//!    |        a divergence on an observation is refused, not relocated to a nearby action
//! attribute   analysis + constraint ledger -> a failure card with typed blame     06.06
//!    |        a contradiction inside the task outranks every other explanation
//! minimize    context + preserved property -> a proven 1-minimal context          06.07
//!    |        losing the property is a typed error, never a smaller cell
//! synthesise  minimization -> ProposedOracle -> review(reviewer) -> ReviewedOracle 06.08
//!    |        only a reviewed oracle has a grade method
//! contrast    two cells + one intervention -> a matched counterfactual pair       06.09
//!    |        an undeclared field that moved is a typed error
//! calibrate   panel runs -> difficulty, and equivalence classes not instance count 06.13
//! dedup       instances -> rename-proof duplicate groups, contamination risk       06.14
//! ```
//!
//! ## Which §06 modules are discharged elsewhere
//!
//! Three sibling crates already own parts of section 06, and this crate depends on them rather than
//! reimplementing them. Nothing below is duplicated here.
//!
//! | Blueprint module | Owner | What this crate does instead |
//! |---|---|---|
//! | 06.02–06.03 ranking | `bioprism_trace::segment` | calls it; carries its `CandidateScore` through unchanged and adds decision *type*, reversibility and no-op filtering |
//! | 06.05 textual divergence | `bioprism_trace::first_divergence`, `is_actionable` | calls both; adds the backward dependency graph, the necessity ranking and the refusal to blame an environment-produced step |
//! | 06.07 world minimization | `bioprism_prism::minimize` | that one minimizes a `World` against the fiber oracle and states a single-pass 1-minimal guarantee; this one minimizes an arbitrary hierarchical context against a **caller-supplied** preserved property, runs to a fixpoint, and ships the proof |
//! | 06.10 mutation engine | `bioprism_mutation::apply`, `lineage` | nothing. Generating families here would create lineages the mutation crate's graph does not know about |
//! | 06.11 semantics-preserving mutations | `bioprism_mutation::relation` | nothing |
//! | 06.12 controlled semantic and fault mutations | `bioprism_mutation::relation` | nothing. 06.09's counterfactual pairs are the same operation seen from the cell side, and [`counterfactual::pair`] validates a pair the mutation engine produced rather than producing one |
//! | 06.13 effective diversity | `bioprism_mutation::diversity` | restates the identical `(parent, family, oracle signature)` definition because this crate cannot depend on the mutation engine. **If they ever disagree, `bioprism_mutation` is authoritative** |
//! | 06.14 content digest rule | `bioprism_mutation::lineage::content_digest` | follows it: [`dedup::content_fingerprint`] hashes what an instance tests and never its id or description, so a rename does not mint a new benchmark |
//! | 06.15 lineage and million scale | not implemented anywhere yet | out of scope: it needs a store and a pack registry |
//! | Decision Cell and its approval gate | `bioprism_prism::cell` | produces cells; does not redefine them |
//!
//! ## What is deliberately not implemented
//!
//! Each module names its own omissions; these are the ones that shape the whole crate.
//!
//! - **No model anywhere.** 06.02 permits a sequence model for phase labels, 06.03 a model for
//!   boundary proposal, 06.06 a model for taxonomy assignment, 06.08 a calibrated model judge.
//!   Every classifier here is transparent arithmetic or a declared field, and where no rule fires
//!   the answer is [`boundary::DecisionType::Unclassified`] rather than the most common label.
//! - **No execution.** Nothing replays a world, forks an architecture, or runs a checker. 06.05's
//!   bounded counterfactual forks, 06.07's probabilistic preservation over repeated trials, and
//!   06.08's execution-test rung all need a runtime or a sandbox this workspace does not carry
//!   here. The consequence is visible rather than hidden:
//!   [`pipeline::CompilerConfidence::state_reconstruction`] is always
//!   [`pipeline::StageConfidence::Unmeasured`].
//! - **No clock, no RNG.** Holdout assignment is a hash bucket; exposure dates are opaque strings
//!   the crate never parses; 06.02's "long idle gaps" signal is unavailable because the Trace IR
//!   deliberately carries no wall-clock time.
//! - **No semantic similarity.** 06.14 lists embedding and entailment layers for near-duplicate
//!   detection. [`dedup::structural_fingerprint`] defeats renaming exactly, and two instances that
//!   mean the same thing in different words will not be caught.
//! - **Nothing here is validated.** The ranking weights, [`causal::FIRST_CAUSAL_THRESHOLD`] and the
//!   no-op filter are stated cutoffs. No study of reviewer agreement on real trajectories has been
//!   run in this workspace, so a compilation orders candidates for a human to read; it is not
//!   evidence that the top candidate is the cause.
//!
//! ## The invariants that are types rather than tests
//!
//! | Rule | How it is enforced |
//! |---|---|
//! | An unreviewed oracle cannot grade | [`oracle::ProposedOracle`] has no `grade` method; [`oracle::ReviewedOracle`] has private fields, one constructor, and `Serialize` without `Deserialize` |
//! | A failure is never pinned to a step the agent did not control | [`causal::CausalVerdict::cell_step`] returns `None` for [`causal::CausalVerdict::EnvironmentDivergence`], and [`causal::CausalAnalysis::localise_to`] refuses one |
//! | A reduction that lost its reason is not a smaller cell | [`minimize::minimize`] returns [`error::MinimizeError::PropertyLost`] |
//! | An uncited claim cannot carry blame | [`attribute::assert_with`] has no path to [`attribute::Assertion::Evidenced`] with an empty citation list |
//! | An unmeasured stage is not a low score | [`pipeline::StageConfidence`] has no `value_or_zero`, and [`pipeline::CompilerConfidence`] has no method that averages its five stages |
//! | A hindsight-sourced option cannot be shown to an agent | [`actions::CandidateActionSet::visible_to_agent`] filters on [`actions::Provenance`], and a false provenance is [`error::ActionError::HindsightLeak`] |

pub mod actions;
pub mod attribute;
pub mod boundary;
pub mod calibrate;
pub mod causal;
pub mod counterfactual;
pub mod dedup;
pub mod error;
pub mod minimize;
pub mod mechanism_control;
pub mod oracle;
pub mod pipeline;

pub use actions::{
    why_not_decision_bearing, CandidateAction, CandidateActionSet, Coverage, Feasibility,
    Provenance,
};
pub use attribute::{
    assert_with, failure_card, Assertion, AttributionLayer, Blame, Citation, ConstraintOutcome,
    ConstraintRecord, ConstraintSource, FailureCard,
};
pub use boundary::{
    boundaries, episodes, repetitions, Boundary, DecisionType, Episode, RepeatedAction, Repetition,
    Reversibility,
};
pub use calibrate::{
    calibrate, effective_diversity, BenchInstance, Calibration, CalibrationVerdict, CapabilityTier,
    DifficultyEstimate, EffectiveDiversity, InstanceCalibration, PanelRun,
};
pub use causal::{analyse, CausalAnalysis, CausalCandidate, CausalScore, CausalVerdict};
pub use counterfactual::{
    contrast, ContrastOutcome, CounterfactualPair, ExpectedResponse, Intervention,
    InterventionTarget, NoRealismReview, RealismCheck, CELL_FIELDS,
};
pub use dedup::{
    assess_contamination, assign_holdout, content_fingerprint, deduplicate, structural_fingerprint,
    ContaminationReport, ContaminationRisk, DedupReport, DuplicateGroup, DuplicateLayer,
    ExposureLedger, Holdout, Instance, LeakChannel, LeakProbe,
};
pub use error::{
    ActionError, CausalError, CompileError, CounterfactualError, MinimizeError, OracleError,
};
pub use minimize::{
    minimize, minimize_preserving, ContextItem, Guard, InterestProbe, InterestSignature,
    MinimalityWitness, MinimizeBudget, Minimization, Tier,
};
pub use mechanism_control::{
    operate_mechanism_control, MechanismCandidate, MechanismControlError,
    MechanismControlReceipt, MechanismDisposition, MechanismPortfolio, MechanismQuestion,
    MechanismState, StudyContext,
    CONTRACT_VERSION as MECHANISM_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_CONTROL_FEATURE_ID,
    PRECLINICAL_BOUNDARY as MECHANISM_CONTROL_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as MECHANISM_CONTROL_SCHEMA_VERSION,
};
pub use oracle::{
    synthesise, ExploitAttempt, OracleStrength, ProposedOracle, ReviewedOracle, SYNTHESIS_ORDER,
};
pub use pipeline::{
    compile, Compilation, CompilerConfidence, OutputClass, ProvenanceEntry, StageConfidence,
};
