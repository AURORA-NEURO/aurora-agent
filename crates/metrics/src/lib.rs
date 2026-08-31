//! Metric semantics for the BioCapability Atlas: what a capability number may and may not claim.
//!
//! Implements the part of blueprint section 33 (BioCapability Atlas and Metrics) that sits *above*
//! the atlas — aggregation, comparability, uncertainty, ranking and release-gate arithmetic — under
//! the decision recorded in ADR-006, **Reject a universal scalar agent-intelligence score**:
//!
//! > Agent performance depends on capability, task distribution, cost, latency, permissions,
//! > architecture, model, and evaluator composition. A single number hides tradeoffs and invites
//! > benchmark gaming. [...] Pack-specific aggregates are allowed when their construction is
//! > transparent; no universal "agent IQ" will be canonical.
//!
//! That ADR is this crate's constitution, and every module below is one of its consequences made
//! mechanical. The ADR also lists what it costs — "Leaderboards are less superficially simple" —
//! and the API is correspondingly inconvenient in exactly the places where convenience would be a
//! lie.
//!
//! # The abuse this crate exists to prevent
//!
//! A capability atlas invites one specific misuse: collapse the grid into a single number so two
//! systems can be ranked. `bioprism-atlas` holds the line at the cell — a capability with no
//! evidence is `Unmeasured`, categorically distinct from measured-and-poor, and there is no
//! `score_or_zero`. This crate holds it at every operation above the cell, with three rules encoded
//! as type-level facts rather than warnings:
//!
//! 1. **An aggregate over a grid containing an unmeasured cell is not an aggregate over the grid.**
//!    [`aggregate::BareScore`] implements neither `Serialize` nor `Deserialize`, so an aggregate
//!    reaches a report only as a [`aggregate::CoveredAggregate`], which carries the contributing
//!    cells and the unmeasured cells in the same object and re-derives its coverage on
//!    deserialization. This is `bioprism-scale`'s `NominalCount` / `EffectiveSize` mechanism,
//!    transposed from instance counts to scores.
//! 2. **Two scores measured under different conditions are not comparable.**
//!    [`comparability::comparable`] returns the first blocking dimension, in the shape
//!    `bioprism-standards` established. Conditions are modelled explicitly and
//!    [`conditions::Condition::Unrecorded`] blocks: **silence does not match silence**.
//! 3. **Incomparable is a real answer.** [`ranking::Dominance::Incomparable`] is a first-class
//!    state that no arbitrary tiebreak collapses, following `bioprism-lab`'s Pareto front. Where a
//!    total order is demanded, [`ranking::PartialRanking::totalise`] requires a predeclared,
//!    digested weighting and records every refusal the weighting overwrote.
//!
//! # Which section 33 modules `bioprism-atlas` already discharges
//!
//! `bioprism-atlas` owns the atlas object itself and does not stop at 33. Specifically it
//! discharges:
//!
//! - **33.00 and 33.01's atlas half** — the capability × evidence object, the coverage report, the
//!   holes list, coverage debt, and `composite`, the eligibility gate that usually refuses.
//! - **33.18** — the failure atlas, first-divergence stage and the causal chain, via its `failure`
//!   module and the first-divergence histogram in its coverage report.
//! - **33.19's clustering half** — independent parents separated from generated instances, and
//!   `Measurement::effective_size` reporting the clustering unit rather than the instance count.
//! - **03.09** (capability ontology, its families, dimensions and `confounds_with` relation),
//!   **03.10** (the closed failure taxonomy), and **43.40**'s claim ladder with `permitted_claim`.
//!
//! This crate covers the remaining fifteen modules — 33.02 through 33.17 — not by implementing
//! their estimators, which section 33 never defines, but by implementing the arithmetic that
//! governs all of them: 33.01's eligibility and stratification rules, 33.04's intervals and
//! abstention accounting, 33.11's worst-domain reporting, 33.16's cost-conditioned comparability,
//! and 33.17's requirement that a paired comparison state what was held fixed.
//!
//! # Every place section 33 names a metric it never defines
//!
//! All of them. Section 33's nineteen modules follow one template, of which "Primary metrics" is a
//! bulleted list of names. Counted across modules 01–19 that list holds **108 metric names**, above
//! a "Measurement dimensions" list holding a further **114** — "correct-for-wrong-reason rate",
//! "posterior entropy reduction", "oracle regret", "claim-flip rate", "weakest-link maturity",
//! "coordination overhead", "minimization ratio", "rank instability", "weight sensitivity" — and
//! **not one of the 222 carries a formula, an estimator, or a stated denominator**. Seven of the
//! 108 name a denominator in passing ("obligations closed per 1,000 tokens", "cost per successful
//! parent world", "compute-to-information ratio"); the rest are bare noun phrases. What the section
//! does define, repeatedly and identically, is the *discipline* around those numbers:
//! stratification, clustering, exclusions, intervals, worst-domain reporting, predeclared weights.
//!
//! This crate therefore implements the discipline and refuses to invent the estimators. Two metrics
//! are given explicit definitions anyway, because they are about ranking rather than about biology
//! and this crate is where ranking lives: [`ranking::RankInstability`] defines "rank instability"
//! and "weight sensitivity" as leave-one-capability-out perturbation of the declared weighting, and
//! says in its own docs that this is *a* definition rather than *the* definition.
//!
//! # Shape
//!
//! - [`conditions`] — 33.01's stratification key as data, and the three-state condition.
//! - [`interval`] — intervals, interval arithmetic, and the estimate that cannot drop its interval.
//! - [`grid`] — the capability grid and the bridge from `bioprism_atlas::Atlas`.
//! - [`weighting`] — predeclared weights with a content digest.
//! - [`aggregate`] — coverage-carrying aggregation and the unserializable bare score.
//! - [`comparability`] — first-blocking-dimension refusal, and the comparison report.
//! - [`ranking`] — partial order, refusal, recorded totalisation, rank instability.
//! - [`gate`] — release predicates with met / violated / unevaluable outcomes.
//! - [`analytics`] — bounded domain-neutral descriptive summaries, paired contrasts, replicate
//!   spread, cost/latency, and calibration arithmetic over caller-supplied observations.
//!
//! # What this crate does not do
//!
//! - **No evaluation execution.** Nothing here runs a trial, calls a system or scores an output.
//!   Every number arrives from a caller who measured it.
//! - **No real measurements.** There is no dataset, no fixture pack and no shipped grid. The tests
//!   construct grids by hand precisely so that no number in this crate is mistaken for evidence.
//! - **No statistical inference library.** No estimator, no bootstrap, no significance test, no
//!   inferential variance model. [`interval::Interval`] carries an interval a caller computed
//!   elsewhere, together with the method and clustering unit that make it interpretable.
//!   [`analytics`] reports descriptive population variance and equal-width calibration summaries;
//!   neither is an uncertainty interval or a fitted scientific model.
//!   `bioprism-atlas` declines intervals for the same reason and this crate does not paper over it:
//!   a grid built by [`grid::CapabilityGrid::from_atlas`] reports uncertainty gates as
//!   *unevaluable*, which is the truthful verdict.
//! - **No rendering.** 33.01 lists ten public-card panels; this crate emits their input.
//! - **No reconciliation between evaluation regimes.** Unlike `bioprism-standards`, which can
//!   convert millimetres to centimetres, there is no act that makes a pack-3 score comparable to a
//!   pack-4 score. Rescoring is a re-evaluation, not a conversion.

pub mod aggregate;
pub mod analytics;
pub mod comparability;
pub mod conditions;
pub mod error;
pub mod gate;
pub mod grid;
pub mod interval;
pub mod ranking;
pub mod weighting;
pub mod discovery_rate_integrity_support;
pub mod local_discovery_rate_integrity_inference;
pub mod multimodal_discovery_rate_integrity_inference;
pub mod throughput_discovery_rate_integrity_inference;
pub mod federated_continual_discovery_rate_integrity_inference;
pub mod local_discovery_rate_integrity_contract_model;
pub mod multimodal_discovery_rate_integrity_contract_model;
pub mod throughput_discovery_rate_integrity_contract_model;
pub mod federated_continual_discovery_rate_integrity_contract_model;
pub mod local_discovery_rate_integrity_research_copilot;
pub mod multimodal_discovery_rate_integrity_research_copilot;
pub mod throughput_discovery_rate_integrity_research_copilot;
pub mod federated_continual_discovery_rate_integrity_research_copilot;
pub mod local_discovery_rate_integrity_workflow_fabric;
pub mod multimodal_discovery_rate_integrity_workflow_fabric;
pub mod throughput_discovery_rate_integrity_workflow_fabric;
pub mod federated_continual_discovery_rate_integrity_workflow_fabric;

pub use aggregate::{
    AggregationRule, BareScore, Coverage, CoveredAggregate, CoveredAggregateFields, UnmeasuredCell,
    WorstCell,
};
pub use analytics::{
    analyse as analyse_analytics, AnalyticsCoverage, AnalyticsError, AnalyticsInput,
    AnalyticsReport, CalibrationBin, CalibrationObservation, CalibrationSummary, DescriptiveStats,
    DimensionSummary, Direction as AnalyticsDirection, EvidenceState as AnalyticsEvidenceState,
    MetricObservation, PairSummary, PairedObservation, ANALYTICS_SCHEMA_VERSION,
    MAX_ANALYTICS_ROWS,
};
pub use comparability::{
    aggregates_comparable, comparable, comparable_under, grids_comparable, ComparabilityPolicy,
    ComparisonReport, Verdict, CHECK_ORDER,
};
pub use conditions::{
    Budget, Condition, Direction, MeasurementConditions, ScoringRule, Stratum, Subject,
    STRATIFICATION_KEY,
};
pub use error::{MetricsError, ScoreIncomparability, UnrecordedSide};
pub use gate::{
    EvaluabilityGap, GateOutcome, GatePredicate, GateReport, GateVerdict, PredicateOutcome,
    ReleaseGate,
};
pub use grid::{CapabilityGrid, GridCell};
pub use interval::{
    weighted_mean, ClusteringUnit, ConfidenceLevel, Estimate, Interval, IntervalBasis,
    IntervalEstimate, NoIntervalReason, PointEstimate,
};
pub use ranking::{
    breakdown, compare, compare_under, CapabilityBreakdown, CapabilityVector,
    CollapsedIncomparability, Dominance, Instability, PairRelation, PartialRanking,
    RankInstability, RankedSystem, SystemId, TotalRanking, Unorderable,
};
pub use weighting::{DeclaredWeighting, DeclaredWeightingFields};
pub use discovery_rate_integrity_support::*;
pub use local_discovery_rate_integrity_inference::*;
pub use multimodal_discovery_rate_integrity_inference::*;
pub use throughput_discovery_rate_integrity_inference::*;
pub use federated_continual_discovery_rate_integrity_inference::*;
pub use local_discovery_rate_integrity_contract_model::*;
pub use multimodal_discovery_rate_integrity_contract_model::*;
pub use throughput_discovery_rate_integrity_contract_model::*;
pub use federated_continual_discovery_rate_integrity_contract_model::*;
pub use local_discovery_rate_integrity_research_copilot::*;
pub use multimodal_discovery_rate_integrity_research_copilot::*;
pub use throughput_discovery_rate_integrity_research_copilot::*;
pub use federated_continual_discovery_rate_integrity_research_copilot::*;
pub use local_discovery_rate_integrity_workflow_fabric::*;
pub use multimodal_discovery_rate_integrity_workflow_fabric::*;
pub use throughput_discovery_rate_integrity_workflow_fabric::*;
pub use federated_continual_discovery_rate_integrity_workflow_fabric::*;

/// The schema version of the metric objects this crate emits.
///
/// Bumped when a serialized shape changes, because a stored [`CoveredAggregate`] whose coverage
/// fields are read under different rules is a number claiming a coverage it does not have.
pub const METRICS_SCHEMA_VERSION: &str = "bioprism-metrics/0.1";
