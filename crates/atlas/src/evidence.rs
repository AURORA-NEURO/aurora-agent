//! Evidence records, measurements, and the unmeasured/measured-poor distinction.
//!
//! This module carries the property blueprint 43.40 makes non-negotiable and that 33.01 restates
//! as a release gate — "missing and abstained cases cannot disappear silently". A capability with
//! no evaluable trial is [`CapabilityCell::Unmeasured`]. A capability whose every evaluable trial
//! failed is [`CapabilityCell::Measured`] with a score of zero. These are categorically different
//! states and the type system keeps them apart:
//!
//! - [`Measurement`] cannot be constructed, or deserialized, with an empty denominator.
//! - [`CapabilityCell::score`] returns `Option<f64>`; there is no accessor that defaults a hole
//!   to zero.
//! - The serde tag is on the *state*, so an unmeasured cell serializes with no numeric field at
//!   all. A renderer cannot read a score it was never given.
//!
//! This mirrors [`bioprism_section::InfluenceClass`], where `Zero` means "provably cannot move
//! the decision" and `Unknown` means "nobody checked". [`UnmeasuredReason::influence_class`] maps
//! each hole onto that same vocabulary so the atlas and the omission manifest agree.
//!
//! NOT implemented: confidence intervals. 33.01 requires intervals clustered at the highest
//! dependency level. [`Measurement::effective_size`] reports that clustering unit honestly, but
//! the interval estimator needs per-trial outcomes retained past aggregation and belongs with the
//! statistics layer, not the atlas index.

use crate::error::AtlasError;
use crate::ontology::CapabilityId;
use bioprism_ids::{RunId, WorldId};
use bioprism_section::InfluenceClass;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The release tiers of 43.40, in ascending order. Derived `Ord` is the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    UnitConformance,
    SyntheticStructural,
    PublicObservedWorld,
    CrossDomainPublic,
    ControlledHiddenMultiSite,
    ProspectiveWorkflow,
}

impl EvidenceTier {
    pub const LADDER: [EvidenceTier; 6] = [
        EvidenceTier::UnitConformance,
        EvidenceTier::SyntheticStructural,
        EvidenceTier::PublicObservedWorld,
        EvidenceTier::CrossDomainPublic,
        EvidenceTier::ControlledHiddenMultiSite,
        EvidenceTier::ProspectiveWorkflow,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceTier::UnitConformance => "unit_conformance",
            EvidenceTier::SyntheticStructural => "synthetic_structural",
            EvidenceTier::PublicObservedWorld => "public_observed_world",
            EvidenceTier::CrossDomainPublic => "cross_domain_public",
            EvidenceTier::ControlledHiddenMultiSite => "controlled_hidden_multi_site",
            EvidenceTier::ProspectiveWorkflow => "prospective_workflow",
        }
    }
}

impl fmt::Display for EvidenceTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a trial's outcome was decided, in ascending authority.
///
/// The order is the enforcement mechanism for the 03.09/03.10 invariant that "nondeterministic
/// judgments never silently override deterministic or execution-grounded evidence": when several
/// oracles judged the same trial, the highest-authority record wins and the losers are counted as
/// superseded rather than dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleTier {
    ModelJudge,
    ExpertAdjudicated,
    Executable,
    Deterministic,
}

impl OracleTier {
    pub fn as_str(self) -> &'static str {
        match self {
            OracleTier::ModelJudge => "model_judge",
            OracleTier::ExpertAdjudicated => "expert_adjudicated",
            OracleTier::Executable => "executable",
            OracleTier::Deterministic => "deterministic",
        }
    }

    /// Whether the judgment is reproducible from the artifact alone.
    pub fn is_deterministic(self) -> bool {
        matches!(self, OracleTier::Deterministic | OracleTier::Executable)
    }
}

impl fmt::Display for OracleTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What happened on one trial.
///
/// Only [`TrialOutcome::Pass`] and [`TrialOutcome::Fail`] enter the denominator.
/// [`TrialOutcome::NonEvaluable`] means the trial could not be scored — a harness crash, a
/// missing artifact — and [`TrialOutcome::Abstained`] means the system declined, which 33.04
/// treats as a legitimate act rather than a failure. Both are counted and reported; neither is
/// quietly converted into a failure to keep a denominator alive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialOutcome {
    Pass,
    Fail,
    NonEvaluable,
    Abstained,
}

impl TrialOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            TrialOutcome::Pass => "pass",
            TrialOutcome::Fail => "fail",
            TrialOutcome::NonEvaluable => "non_evaluable",
            TrialOutcome::Abstained => "abstained",
        }
    }

    /// Whether the outcome contributes to a denominator.
    pub fn is_evaluable(self) -> bool {
        matches!(self, TrialOutcome::Pass | TrialOutcome::Fail)
    }
}

/// One scored trial, attributed to exactly one capability.
///
/// `ontology_version` is carried per record rather than assumed, because 03.09 versions the
/// hierarchy and a record compiled under an older vocabulary means something different.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    /// Identifies the execution trial. Several records may share it when several oracles judged
    /// the same run; they are then resolved by [`OracleTier`] authority.
    pub trial: String,
    pub capability: CapabilityId,
    pub ontology_version: String,
    pub tier: EvidenceTier,
    pub oracle: OracleTier,
    pub outcome: TrialOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunId>,
    /// The clustering unit. 33.19 requires generated scale to be reported separately from
    /// independent parents, so uncertainty is never inflated by descendants of one parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_world: Option<WorldId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// True when this trial is a generated instance rather than an independent parent.
    #[serde(default)]
    pub generated: bool,
}

impl EvidenceRecord {
    pub fn new(
        trial: impl Into<String>,
        capability: CapabilityId,
        ontology_version: impl Into<String>,
        tier: EvidenceTier,
        oracle: OracleTier,
        outcome: TrialOutcome,
    ) -> Self {
        EvidenceRecord {
            trial: trial.into(),
            capability,
            ontology_version: ontology_version.into(),
            tier,
            oracle,
            outcome,
            run: None,
            parent_world: None,
            site: None,
            domain: None,
            generated: false,
        }
    }

    pub fn with_run(mut self, run: RunId) -> Self {
        self.run = Some(run);
        self
    }

    pub fn with_parent_world(mut self, world: WorldId) -> Self {
        self.parent_world = Some(world);
        self
    }

    pub fn with_site(mut self, site: impl Into<String>) -> Self {
        self.site = Some(site.into());
        self
    }

    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn generated_instance(mut self) -> Self {
        self.generated = true;
        self
    }
}

/// The wire form of a [`Measurement`]. Public because it is the only route in: both
/// deserialization and construction go through [`Measurement::try_from`], so the empty-denominator
/// refusal cannot be bypassed by handing the atlas a JSON document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementFields {
    pub capability: CapabilityId,
    pub passes: usize,
    pub failures: usize,
    #[serde(default)]
    pub non_evaluable: usize,
    #[serde(default)]
    pub abstained: usize,
    /// Lower-authority judgments that disagreed with the retained one. Kept as a count so a
    /// reader can see that an override happened.
    #[serde(default)]
    pub superseded: usize,
    #[serde(default)]
    pub independent_parents: usize,
    #[serde(default)]
    pub generated_instances: usize,
    #[serde(default)]
    pub independent_sites: usize,
    #[serde(default)]
    pub domains: usize,
    pub highest_tier: EvidenceTier,
    pub strongest_oracle: OracleTier,
    #[serde(default)]
    pub deterministic_failures: usize,
}

/// A capability that was actually measured.
///
/// Fields are private and the only constructor validates, because the whole point of this crate
/// is that `Measurement { passes: 0, failures: 0, .. }` must not exist. That value would render
/// as a score of zero and be indistinguishable from a capability that was measured and failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MeasurementFields", into = "MeasurementFields")]
pub struct Measurement {
    capability: CapabilityId,
    passes: usize,
    failures: usize,
    non_evaluable: usize,
    abstained: usize,
    superseded: usize,
    independent_parents: usize,
    generated_instances: usize,
    independent_sites: usize,
    domains: usize,
    highest_tier: EvidenceTier,
    strongest_oracle: OracleTier,
    deterministic_failures: usize,
}

impl TryFrom<MeasurementFields> for Measurement {
    type Error = AtlasError;

    fn try_from(fields: MeasurementFields) -> Result<Self, Self::Error> {
        let evaluable = fields.passes + fields.failures;
        if evaluable == 0 {
            return Err(AtlasError::EmptyDenominator {
                capability: fields.capability.to_string(),
            });
        }
        let trials = evaluable + fields.non_evaluable + fields.abstained;
        for (unit, claimed) in [
            ("independent parents", fields.independent_parents),
            ("independent sites", fields.independent_sites),
            ("domains", fields.domains),
            ("generated instances", fields.generated_instances),
        ] {
            if claimed > trials {
                return Err(AtlasError::ImpossibleClusterCount {
                    capability: fields.capability.to_string(),
                    unit,
                    claimed,
                    trials,
                });
            }
        }
        if fields.deterministic_failures > fields.failures {
            return Err(AtlasError::ImpossibleClusterCount {
                capability: fields.capability.to_string(),
                unit: "deterministic failures",
                claimed: fields.deterministic_failures,
                trials: fields.failures,
            });
        }
        Ok(Measurement {
            capability: fields.capability,
            passes: fields.passes,
            failures: fields.failures,
            non_evaluable: fields.non_evaluable,
            abstained: fields.abstained,
            superseded: fields.superseded,
            independent_parents: fields.independent_parents,
            generated_instances: fields.generated_instances,
            independent_sites: fields.independent_sites,
            domains: fields.domains,
            highest_tier: fields.highest_tier,
            strongest_oracle: fields.strongest_oracle,
            deterministic_failures: fields.deterministic_failures,
        })
    }
}

impl From<Measurement> for MeasurementFields {
    fn from(m: Measurement) -> Self {
        MeasurementFields {
            capability: m.capability,
            passes: m.passes,
            failures: m.failures,
            non_evaluable: m.non_evaluable,
            abstained: m.abstained,
            superseded: m.superseded,
            independent_parents: m.independent_parents,
            generated_instances: m.generated_instances,
            independent_sites: m.independent_sites,
            domains: m.domains,
            highest_tier: m.highest_tier,
            strongest_oracle: m.strongest_oracle,
            deterministic_failures: m.deterministic_failures,
        }
    }
}

impl Measurement {
    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn passes(&self) -> usize {
        self.passes
    }

    pub fn failures(&self) -> usize {
        self.failures
    }

    pub fn non_evaluable(&self) -> usize {
        self.non_evaluable
    }

    pub fn abstained(&self) -> usize {
        self.abstained
    }

    pub fn superseded(&self) -> usize {
        self.superseded
    }

    pub fn independent_parents(&self) -> usize {
        self.independent_parents
    }

    pub fn generated_instances(&self) -> usize {
        self.generated_instances
    }

    pub fn independent_sites(&self) -> usize {
        self.independent_sites
    }

    pub fn domains(&self) -> usize {
        self.domains
    }

    pub fn highest_tier(&self) -> EvidenceTier {
        self.highest_tier
    }

    pub fn strongest_oracle(&self) -> OracleTier {
        self.strongest_oracle
    }

    pub fn deterministic_failures(&self) -> usize {
        self.deterministic_failures
    }

    /// The denominator. Guaranteed non-zero by construction.
    pub fn evaluable(&self) -> usize {
        self.passes + self.failures
    }

    /// Trials that were run but produced no score. Reported so they cannot vanish.
    pub fn excluded(&self) -> usize {
        self.non_evaluable + self.abstained
    }

    /// Pass rate over the evaluable denominator. Total division safety: [`Measurement`] cannot
    /// exist with an empty denominator.
    pub fn score(&self) -> f64 {
        self.passes as f64 / self.evaluable() as f64
    }

    /// The number of independent clustering units, not the number of instances.
    ///
    /// 33.19's worked example is exactly this: "A million-instance pack card discloses that it
    /// derives from 300 parent worlds." A million generated descendants of one parent buy one
    /// unit of independent evidence, not a million.
    pub fn effective_size(&self) -> usize {
        self.independent_parents.max(1)
    }

    pub fn depth(&self) -> MeasurementDepth {
        if self.independent_sites >= 2 && self.domains >= 2 {
            MeasurementDepth::MultiSiteCrossDomain
        } else if self.independent_sites >= 2 {
            MeasurementDepth::MultiSite
        } else if self.independent_parents >= 2 {
            MeasurementDepth::MultiParent
        } else if self.evaluable() >= 2 {
            MeasurementDepth::Clustered
        } else {
            MeasurementDepth::Single
        }
    }
}

/// How much independent structure stands behind a measurement.
///
/// Depth is not quality. A [`MeasurementDepth::Single`] capability may score 1.0; the depth says
/// that the 1.0 rests on one trial and licenses nothing above the conformance tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementDepth {
    Single,
    Clustered,
    MultiParent,
    MultiSite,
    MultiSiteCrossDomain,
}

impl MeasurementDepth {
    pub const ALL: [MeasurementDepth; 5] = [
        MeasurementDepth::Single,
        MeasurementDepth::Clustered,
        MeasurementDepth::MultiParent,
        MeasurementDepth::MultiSite,
        MeasurementDepth::MultiSiteCrossDomain,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            MeasurementDepth::Single => "single",
            MeasurementDepth::Clustered => "clustered",
            MeasurementDepth::MultiParent => "multi_parent",
            MeasurementDepth::MultiSite => "multi_site",
            MeasurementDepth::MultiSiteCrossDomain => "multi_site_cross_domain",
        }
    }
}

/// Why a capability has no measurement.
///
/// A hole is not a number, but it is not featureless either. The reason determines whether the
/// gap can participate in a sufficiency claim, by the same rule the omission manifest uses in
/// `bioprism-section`: only a gap that provably cannot move the conclusion counts as closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnmeasuredReason {
    /// No evidence record names this capability. The commonest and most honest hole.
    NotAttempted,
    /// Records exist but none was evaluable — a mix of harness failures and abstentions.
    NoEligibleEvidence,
    /// Every trial crashed or lost its artifact. The system was never actually scored.
    AllTrialsNonEvaluable,
    /// Every trial abstained. Under 33.04 abstention is a legitimate act, so this is a hole in
    /// coverage, not a run of failures.
    AllTrialsAbstained,
    /// Consent or policy forbids the measurement. The gap is permanent under current terms.
    InaccessibleByPolicy,
    /// Not measurable at this temporal cut; may become measurable later.
    DeferredAcquisition,
    /// The declared intended use puts this capability out of scope. The only reason that closes
    /// a hole, because a capability outside the declared use cannot change the declared claim.
    OutOfScopeByDeclaredUse,
}

impl UnmeasuredReason {
    pub fn as_str(self) -> &'static str {
        match self {
            UnmeasuredReason::NotAttempted => "not_attempted",
            UnmeasuredReason::NoEligibleEvidence => "no_eligible_evidence",
            UnmeasuredReason::AllTrialsNonEvaluable => "all_trials_non_evaluable",
            UnmeasuredReason::AllTrialsAbstained => "all_trials_abstained",
            UnmeasuredReason::InaccessibleByPolicy => "inaccessible_by_policy",
            UnmeasuredReason::DeferredAcquisition => "deferred_acquisition",
            UnmeasuredReason::OutOfScopeByDeclaredUse => "out_of_scope_by_declared_use",
        }
    }

    /// Projects the hole onto the omission-manifest vocabulary of blueprint 43.26.
    pub fn influence_class(self) -> InfluenceClass {
        match self {
            UnmeasuredReason::OutOfScopeByDeclaredUse => InfluenceClass::Zero,
            UnmeasuredReason::InaccessibleByPolicy => InfluenceClass::InaccessibleByPolicy,
            UnmeasuredReason::DeferredAcquisition => InfluenceClass::DeferredAcquisition,
            UnmeasuredReason::NotAttempted
            | UnmeasuredReason::NoEligibleEvidence
            | UnmeasuredReason::AllTrialsNonEvaluable
            | UnmeasuredReason::AllTrialsAbstained => InfluenceClass::Unknown,
        }
    }

    /// Whether a claim may be made about a subtree containing this hole.
    pub fn supports_claim(self) -> bool {
        self.influence_class().supports_sufficiency()
    }
}

impl fmt::Display for UnmeasuredReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One cell of the atlas: what is known about one capability.
///
/// Internally tagged on `state`, so an unmeasured cell serializes as
/// `{"state":"unmeasured","reason":"not_attempted"}` — with no score key for a downstream
/// renderer to coerce, default, or average.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CapabilityCell {
    Unmeasured { reason: UnmeasuredReason },
    Measured { measurement: Measurement },
}

impl CapabilityCell {
    pub fn unmeasured(reason: UnmeasuredReason) -> Self {
        CapabilityCell::Unmeasured { reason }
    }

    pub fn measured(measurement: Measurement) -> Self {
        CapabilityCell::Measured { measurement }
    }

    pub fn is_measured(&self) -> bool {
        matches!(self, CapabilityCell::Measured { .. })
    }

    /// `None` for a hole. There is no `score_or_zero`, by design.
    pub fn score(&self) -> Option<f64> {
        match self {
            CapabilityCell::Measured { measurement } => Some(measurement.score()),
            CapabilityCell::Unmeasured { .. } => None,
        }
    }

    pub fn measurement(&self) -> Option<&Measurement> {
        match self {
            CapabilityCell::Measured { measurement } => Some(measurement),
            CapabilityCell::Unmeasured { .. } => None,
        }
    }

    pub fn unmeasured_reason(&self) -> Option<UnmeasuredReason> {
        match self {
            CapabilityCell::Unmeasured { reason } => Some(*reason),
            CapabilityCell::Measured { .. } => None,
        }
    }

    pub fn depth(&self) -> Option<MeasurementDepth> {
        self.measurement().map(Measurement::depth)
    }

    /// The only supported way to put a cell on a page.
    ///
    /// A caller that wants to draw a heatmap must match on this, and the hole arm has no numeric
    /// payload to draw as a low bar. That is the whole mechanism: it is not a lint, it is the
    /// absence of the value.
    pub fn render(&self) -> CellRendering {
        match self {
            CapabilityCell::Measured { measurement } => CellRendering::Score {
                value: measurement.score(),
                depth: measurement.depth(),
                effective_size: measurement.effective_size(),
            },
            CapabilityCell::Unmeasured { reason } => CellRendering::Hole { reason: *reason },
        }
    }
}

/// What a renderer is allowed to draw for one cell.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellRendering {
    Score {
        value: f64,
        depth: MeasurementDepth,
        effective_size: usize,
    },
    Hole {
        reason: UnmeasuredReason,
    },
}

impl CellRendering {
    pub fn is_hole(&self) -> bool {
        matches!(self, CellRendering::Hole { .. })
    }
}
