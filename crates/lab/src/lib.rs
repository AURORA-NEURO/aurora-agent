//! The Inference Lab: hypothesis separation, architecture search, and the holdout policy that
//! makes a self-improvement claim mean anything.
//!
//! Implements blueprint section 09 (Inference Lab) — specifically 09.02 (context-value expansion),
//! 09.03 (hypothesis separation and evidence obligations), 09.04 (architecture search space),
//! 09.07 (risk-triggered branching and verification), 09.09 (Pareto optimization and architecture
//! search), 09.10 (benchmark-gated self-improvement and evolution cards), and 09.11 (holdout,
//! rollback and online learning policy).
//!
//! # The failure this crate exists to prevent
//!
//! A system that improves itself will, given the chance, improve its score by learning its own
//! holdout. 09.10 asks for evolution cards; 09.11 asks for query accounting and rollback. Neither
//! is worth much alone, and together they say one thing:
//!
//! > **An improvement measured on a holdout the system has already touched is not an improvement,
//! > and must not be representable as one.**
//!
//! That is a type-level fact here, not a warning in a doc comment:
//!
//! - [`holdout::CleanMeasurement`] has private fields, one constructor
//!   ([`holdout::Holdout::measure`]), and implements `Serialize` but **not** `Deserialize`. A score
//!   cannot be written by hand and cannot be reconstituted from JSON, so it cannot reach a report
//!   without the exposure ledger having vouched for it. This is `bioprism-scale`'s `NominalCount`
//!   move — a count that cannot be published without its effective size — pointed the other way.
//! - Contamination is a typed error, not a flag on a value. There is no `measure_anyway` and no
//!   `Option<CleanMeasurement>` whose `None` arm is easy to ignore.
//! - Contamination travels down configuration lineage, so renaming a configuration does not
//!   launder a burned holdout.
//! - [`evolution::EvolutionCard`] and [`evolution::ImprovementClaim`] inherit the absence of a
//!   deserializer. A contaminated card is *constructible* — you must be able to record that it
//!   happened — but it cannot yield an [`evolution::ImprovementClaim`].
//! - Exposure is append-only. [`rollback::Deployment::rollback`] restores a whole configuration
//!   bundle and reports, in [`rollback::RollbackReceipt`], exactly what it could not restore.
//!
//! One hole in that fence is real and is stated rather than papered over. *Selecting* on a holdout
//! burns it for every descendant of the chosen configuration; merely *measuring* a baseline does
//! not, because an evolution card needs a before and an after on the same surface and a rule that
//! burned the descendants would make a clean card impossible. What that leaves open is a person
//! reading the baseline's score, seeing what it got wrong, and writing the next configuration
//! accordingly — a use of the holdout that produces no event to record.
//! [`holdout::ExposureKind::propagates_to_descendants`] documents it, an evolution card's
//! `would_have_to_be_true` field is where it is supposed to be declared, and it is why
//! [`holdout::Partition::RotatingPrivateCertification`] is rotated rather than merely counted.
//!
//! # What lives here and what does not
//!
//! Three sibling crates already own parts of section 09 and this crate does not duplicate them:
//!
//! | Blueprint module | Owner |
//! |---|---|
//! | 09.01 overview, 09.08 evaluation-conditioned router | `bioprism-routing` |
//! | 09.06 component attribution and interactions | `bioprism-evalengine` |
//! | 09.05 counterfactual suffix replay | `bioprism-runtime` (fork/replay, blueprint 05.05) |
//!
//! `bioprism-routing`'s result is part of the argument rather than a detail: its router
//! **captures 0% of available gain** under regime holdout and abstains on every task. This crate
//! does not build a second router and does not improve that number. It is the measurement any
//! future routing improvement would have to be made against — and if that improvement were
//! measured on the same regime holdout that produced the zero, [`holdout`] is what would refuse it.
//!
//! # Shape
//!
//! - [`hypothesis`] — 09.03. Two hypotheses are separable and the separating evidence is named, or
//!   they are **not separable and saying so is the answer**. Evidence obligations come from
//!   `bioprism-obligation`'s 39.06 decision obligation graph rather than a second copy.
//! - [`holdout`] — 09.11. Partitions, exposure accounting, and [`holdout::CleanMeasurement`].
//! - [`rollback`] — 09.11. Immutable bundles, checkpoints, and a receipt for what a rollback
//!   cannot undo.
//! - [`pareto`] — 09.09. Dominated, dominating and **incomparable**, with no scalarizer and no
//!   tiebreak. Unmeasured axes follow `bioprism-atlas`: a hole is not a low score.
//! - [`evolution`] — 09.10. Evolution cards, protected surfaces, and improvement claims.
//! - [`risk`] — 09.07. Stated trigger predicates, hard ceilings, and a ledger that reports what
//!   branching cost before what it caught.
//! - [`context_value`] — 09.02. Acquisition ordering by declared value per unit cost, with privacy
//!   as an exclusion rather than a denominator.
//! - [`space`] — 09.04. Component kinds, protected surfaces, and the immutable bundle registry that
//!   [`holdout`] resolves lineage against.
//! - [`report`] — one report that states the unflattering things first.
//!
//! # Not implemented, deliberately
//!
//! No model, no prompt, no LLM call, and no agent. No training of any kind, no contextual bandit,
//! no exploration schedule, no surrogate model, and no threshold tuning: every number in a
//! [`risk::Trigger`], a [`context_value::AcquisitionAction`] or a [`pareto::Profile`] is written by
//! a caller. No search *execution* — [`space`] describes a search space and [`pareto`] maintains
//! the archive a search would write into, but nothing here proposes or runs a candidate. No clock
//! and no system RNG: exposure events are ordered by an integer sequence, and every ordering in the
//! crate is deterministic under identifier ties. No canary deployment, no shadow routing, no
//! traffic split, no automatic rollback trigger, and no significance testing or confidence
//! intervals.
//!
//! # Where the blueprint asserts a mechanism it does not specify
//!
//! Section 09 is `status: "Planned"` throughout, and five of its requirements name a mechanism
//! without defining one. Each is handled by refusing rather than by inventing:
//!
//! - **09.02's expected value.** "Expected reduction in harmful uncertainty ... divided by token,
//!   latency, monetary, and privacy cost" defines no estimator for the numerator and no exchange
//!   rate for the denominator. [`context_value::AcquisitionAction::value`] is declared by the
//!   caller and documented as uncalibrated.
//! - **09.03's diversity.** "Structural templates, independent samples, counterfactual prompting,
//!   or specialist components" all require a generator. Only the checkable half — deduplication by
//!   assumptions — is implemented.
//! - **09.07's risk estimate.** "Estimate decision risk" names nine features and no function over
//!   them. [`risk::Trigger`] is a stated predicate instead, and an unmeasured feature yields
//!   [`risk::TriggerOutcome::Undetermined`] rather than a default.
//! - **09.09's surrogates.** "Use component-effect models ... to predict promising candidates"
//!   describes a model this workspace does not have; the same section then says predictions never
//!   replace certification. Only the archive is implemented.
//! - **09.10 and 09.11's intervals and thresholds.** An evolution card is asked to record
//!   "before/after metrics, intervals", and rollback to fire on "safety, reliability, cost, or
//!   latency thresholds". No interval estimator and no threshold semantics are given, so
//!   [`evolution::EvolutionCard`] reports a point delta and says nothing about its precision, and
//!   rollback is a function a monitor calls rather than the monitor.
//!
//! # Example
//!
//! ```
//! use bioprism_lab::holdout::{Holdout, Partition};
//! use bioprism_lab::space::ConfigurationId;
//!
//! let mut holdout = Holdout::new("private-a", Partition::RotatingPrivateCertification, 8);
//! let candidate = ConfigurationId::new("v2");
//!
//! holdout.record_selection(&candidate, "won the architecture-development panel");
//! let refused = holdout.measure(std::slice::from_ref(&candidate), "admissible_rate", 0.94);
//!
//! assert!(refused.is_err());
//! ```

pub mod context_value;
pub mod error;
pub mod evolution;
pub mod holdout;
pub mod hypothesis;
pub mod pareto;
pub mod report;
pub mod risk;
pub mod rollback;
pub mod space;

pub use context_value::{
    expand, AcquisitionAction, AcquisitionCost, AcquisitionKind, ExclusionReason, Expansion,
    PlannedAcquisition, PrivacyBoundary, StopReason,
};
pub use error::{
    EvolutionError, HoldoutError, LabError, ParetoError, RollbackError, SeparationError, SpaceError,
};
pub use evolution::{
    ChangeProposal, ContaminationRecord, EvolutionArchive, EvolutionCard, ImprovementClaim,
    MeasurementSurface,
};
pub use holdout::{
    CleanMeasurement, ExposureEvent, ExposureKind, ExposureWatermark, Holdout, HoldoutId,
    HoldoutLedger, Partition,
};
pub use hypothesis::{
    separate, DisagreementPoint, DischargedSeparator, Hypothesis, HypothesisSet, Locus,
    NotSeparableReason, Observations, PendingSeparator, Retirement, SeparationVerdict, Stance,
};
pub use pareto::{
    compare, Admission, AxisValue, Direction, Dominance, Incomparability, Objective, ParetoFront,
    Profile, Selection, Unresolved,
};
pub use report::LabReport;
pub use risk::{
    BranchAction, BranchCeiling, BranchCost, BranchLedger, BranchOutcome, BranchPlan, BranchPolicy,
    BranchRule, BranchVerdict, BranchYield, Catch, PermissionLevel, Reversibility, RiskFeatures,
    Trigger, TriggerOutcome, UndeterminedPolicy, ValueBand,
};
pub use rollback::{
    Checkpoint, Deployment, DeploymentEvent, ExposureSinceCheckpoint, RollbackReceipt,
};
pub use space::{
    ArchitectureSpace, CandidateArchitecture, ComponentKind, ComponentSpec, ConfigurationId,
    ParameterValue, ProtectedSurface,
};
