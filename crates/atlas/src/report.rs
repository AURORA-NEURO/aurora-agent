//! Coverage reporting, and the refusal to rank.
//!
//! Implements the reporting side of blueprint 33.01 and 33.19. The design rule is one sentence:
//! an atlas that shows only what was measured is a marketing artifact, so [`CoverageReport::holes`]
//! is never elided — it has no `skip_serializing_if`, and an empty holes list is itself a claim
//! that had to be earned.
//!
//! Holes are also projected onto the omission-manifest vocabulary of blueprint 43.26 via
//! [`CoverageReport::omission_manifest`]. That is not decoration: it means the atlas and the
//! context compiler use one definition of "this gap cannot move the conclusion", so a coverage
//! hole and an evidence omission are gated by the same predicate,
//! [`bioprism_section::OmissionManifest::supports_sufficiency_claim`].
//!
//! [`composite`] implements 33.01's no-single-score policy the only honest way — as a gate that
//! usually says no. The blueprint's own worked interpretation is a refusal, and the metric that
//! matters on the way there is coverage debt.
//!
//! NOT implemented: the ten panels of 33.01's public card (heatmap, risk–coverage curve, Pareto
//! frontier, first-divergence histogram, and the rest). This module emits the data those panels
//! consume; drawing them is a presentation concern and would pin the atlas to one renderer.

use crate::atlas::{Atlas, Inconsistency};
use crate::claim::{permitted_claim, ClaimTier};
use crate::error::AtlasError;
use crate::evidence::{MeasurementDepth, UnmeasuredReason};
use crate::failure::FailureStage;
use crate::ontology::{CapabilityFamily, CapabilityId};
use bioprism_section::{OmissionGroup, OmissionManifest};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// One measured capability, as a reader needs to see it: the score never travels without the
/// depth and the effective size that qualify it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredEntry {
    pub capability: CapabilityId,
    pub family: CapabilityFamily,
    pub score: f64,
    pub depth: MeasurementDepth,
    pub evaluable: usize,
    /// Trials that were run but produced no score. Present so they cannot vanish.
    pub excluded: usize,
    /// The clustering unit, not the instance count.
    pub effective_size: usize,
    pub generated_instances: usize,
    pub permitted_claim: ClaimTier,
}

/// One hole, with what it costs.
///
/// `blocks_claims_for` is the part that makes a hole actionable rather than decorative: an
/// unmeasured leaf silently caps the permitted claim of every ancestor above it, and naming those
/// ancestors turns "we did not measure this" into "we cannot say this".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hole {
    pub capability: CapabilityId,
    pub family: CapabilityFamily,
    pub reason: UnmeasuredReason,
    pub influence: bioprism_section::InfluenceClass,
    /// True when the capability has children. An interior capability with no direct evidence is
    /// still listed — it is genuinely unmeasured — but it is not a gap in coverage, because the
    /// measurable granularity is below it. Aggregate holes block nothing and are excluded from
    /// [`CoverageReport::omission_manifest`], where they would double-count their own leaves.
    pub aggregate: bool,
    pub blocks_claims_for: Vec<CapabilityId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyCoverage {
    pub family: CapabilityFamily,
    pub total: usize,
    pub measured: usize,
    pub holes: usize,
}

impl FamilyCoverage {
    /// A family with no measured capability at all. 33.01's worked interpretation turns on
    /// exactly this shape.
    pub fn is_dark(&self) -> bool {
        self.total > 0 && self.measured == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepthCount {
    pub depth: MeasurementDepth,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageCount {
    pub stage: FailureStage,
    pub count: usize,
}

/// 33.19's coverage debt, made concrete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageDebt {
    pub total_capabilities: usize,
    pub measured: usize,
    pub unmeasured: usize,
    /// Holes that a declared out-of-scope statement closes. Counted separately because they are
    /// the only ones that do not block a claim.
    pub closed_by_declaration: usize,
    pub dark_families: Vec<CapabilityFamily>,
    /// Failures the closed mechanism set of 03.10 did not cover. Debt against the taxonomy, not
    /// against the system under test.
    pub unclassified_failures: usize,
    /// Failures recorded but not localised — contested, flattened, or with lost evidence.
    pub undiagnosed_failures: usize,
}

impl CoverageDebt {
    /// Fraction of capabilities with no measurement. `0.0` for an empty ontology, which is a
    /// vacuous atlas rather than a complete one — read it with [`CoverageDebt::total_capabilities`].
    pub fn ratio(&self) -> f64 {
        if self.total_capabilities == 0 {
            return 0.0;
        }
        self.unmeasured as f64 / self.total_capabilities as f64
    }
}

/// What was measured, at what depth, and where the atlas has holes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub ontology_version: String,
    pub measured: Vec<MeasuredEntry>,
    /// Never skipped, never summarised away.
    pub holes: Vec<Hole>,
    pub depth_histogram: Vec<DepthCount>,
    pub family_coverage: Vec<FamilyCoverage>,
    /// 33.18's first-divergence histogram, as counts. `Unclassified` failures have no stage and
    /// are counted in [`CoverageDebt::unclassified_failures`] instead of being placed somewhere
    /// convenient.
    pub first_divergence_histogram: Vec<StageCount>,
    pub debt: CoverageDebt,
    pub inconsistencies: Vec<Inconsistency>,
}

impl CoverageReport {
    pub fn of(atlas: &Atlas) -> Self {
        let ontology = atlas.ontology();

        let mut measured = Vec::new();
        let mut depths: BTreeMap<MeasurementDepth, usize> = BTreeMap::new();
        let mut family_totals: BTreeMap<CapabilityFamily, (usize, usize)> = BTreeMap::new();

        for (capability, cell) in atlas.cells() {
            let family = ontology
                .node(capability)
                .map_or(CapabilityFamily::DomainReasoning, |n| n.family);
            let slot = family_totals.entry(family).or_insert((0, 0));
            slot.0 += 1;
            if let Some(m) = cell.measurement() {
                slot.1 += 1;
                *depths.entry(m.depth()).or_insert(0) += 1;
                measured.push(MeasuredEntry {
                    capability: capability.clone(),
                    family,
                    score: m.score(),
                    depth: m.depth(),
                    evaluable: m.evaluable(),
                    excluded: m.excluded(),
                    effective_size: m.effective_size(),
                    generated_instances: m.generated_instances(),
                    permitted_claim: permitted_claim(atlas, capability),
                });
            }
        }

        let mut holes = Vec::new();
        let mut closed_by_declaration = 0usize;
        for (capability, reason) in atlas.holes() {
            if reason.supports_claim() {
                closed_by_declaration += 1;
            }
            let family = ontology
                .node(capability)
                .map_or(CapabilityFamily::DomainReasoning, |n| n.family);
            let aggregate = ontology
                .children(capability)
                .is_ok_and(|children| !children.is_empty());
            let blocks_claims_for = if reason.supports_claim() || aggregate {
                Vec::new()
            } else {
                ontology
                    .ancestors(capability)
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            };
            holes.push(Hole {
                capability: capability.clone(),
                family,
                reason,
                influence: reason.influence_class(),
                aggregate,
                blocks_claims_for,
            });
        }

        let mut stages: BTreeMap<FailureStage, usize> = BTreeMap::new();
        let mut unclassified_failures = 0usize;
        let mut undiagnosed_failures = 0usize;
        for failure in atlas.failures() {
            if let Some(stage) = failure.first_divergence_stage() {
                *stages.entry(stage).or_insert(0) += 1;
            }
            if failure.labels.has_unclassified() || failure.first_divergence_stage().is_none() {
                unclassified_failures += 1;
            }
            if !failure.is_diagnosed() {
                undiagnosed_failures += 1;
            }
        }

        let family_coverage: Vec<FamilyCoverage> = family_totals
            .into_iter()
            .map(|(family, (total, measured))| FamilyCoverage {
                family,
                total,
                measured,
                holes: total - measured,
            })
            .collect();
        let dark_families: Vec<CapabilityFamily> = family_coverage
            .iter()
            .filter(|f| f.is_dark())
            .map(|f| f.family)
            .collect();

        let debt = CoverageDebt {
            total_capabilities: ontology.len(),
            measured: measured.len(),
            unmeasured: holes.len(),
            closed_by_declaration,
            dark_families,
            unclassified_failures,
            undiagnosed_failures,
        };

        CoverageReport {
            ontology_version: ontology.version().to_string(),
            measured,
            holes,
            depth_histogram: depths
                .into_iter()
                .map(|(depth, count)| DepthCount { depth, count })
                .collect(),
            family_coverage,
            first_divergence_histogram: stages
                .into_iter()
                .map(|(stage, count)| StageCount { stage, count })
                .collect(),
            debt,
            inconsistencies: atlas.inconsistencies(),
        }
    }

    pub fn has_holes(&self) -> bool {
        !self.holes.is_empty()
    }

    /// The holes, grouped by structural reason, in the vocabulary of blueprint 43.26.
    ///
    /// Groups are keyed by [`UnmeasuredReason`] because that is the reason a hole exists, and the
    /// influence class follows from it. Examples are capped at five: a manifest that inlines every
    /// capability stops being readable, and the atlas is the full list.
    pub fn omission_manifest(&self) -> OmissionManifest {
        let mut grouped: BTreeMap<UnmeasuredReason, Vec<&CapabilityId>> = BTreeMap::new();
        for hole in self.holes.iter().filter(|h| !h.aggregate) {
            grouped
                .entry(hole.reason)
                .or_default()
                .push(&hole.capability);
        }
        let mut manifest = OmissionManifest::default();
        for (reason, capabilities) in grouped {
            manifest.push(OmissionGroup {
                reason: format!("capability unmeasured: {reason}"),
                influence: reason.influence_class(),
                count: capabilities.len(),
                bound: None,
                examples: capabilities.iter().take(5).map(|c| c.to_string()).collect(),
            });
        }
        manifest
    }

    /// Whether the coverage is complete enough for aggregation to be defensible at all.
    ///
    /// Delegates to the omission manifest so that "sufficient" means the same thing here as it
    /// does in the context compiler.
    pub fn coverage_supports_aggregation(&self) -> bool {
        !self.measured.is_empty() && self.omission_manifest().supports_sufficiency_claim()
    }
}

/// A predeclared weighting, as 33.01 requires: "Predeclare composite weights and run sensitivity
/// analyses." Weights declared after seeing the scores are not a policy, they are a result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeightingPolicy {
    pub intended_use: String,
    weights: BTreeMap<CapabilityId, f64>,
}

#[derive(Debug, Deserialize)]
struct WeightingPolicyFields {
    intended_use: String,
    weights: BTreeMap<CapabilityId, f64>,
}

impl<'de> Deserialize<'de> for WeightingPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = WeightingPolicyFields::deserialize(deserializer)?;
        let policy = WeightingPolicy {
            intended_use: fields.intended_use,
            weights: fields.weights,
        };
        policy.validate().map(|()| policy).map_err(D::Error::custom)
    }
}

impl WeightingPolicy {
    pub fn declare(
        intended_use: impl Into<String>,
        weights: impl IntoIterator<Item = (CapabilityId, f64)>,
    ) -> Result<Self, AtlasError> {
        let weights: BTreeMap<CapabilityId, f64> = weights.into_iter().collect();
        if weights.is_empty() {
            return Err(AtlasError::MalformedWeightingPolicy {
                detail: "no capability is weighted".to_string(),
            });
        }
        for (capability, weight) in &weights {
            if !weight.is_finite() || *weight <= 0.0 {
                return Err(AtlasError::MalformedWeightingPolicy {
                    detail: format!("weight {weight} for {capability} is not positive and finite"),
                });
            }
        }
        let policy = WeightingPolicy {
            intended_use: intended_use.into(),
            weights,
        };
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), AtlasError> {
        if self.weights.is_empty() {
            return Err(AtlasError::MalformedWeightingPolicy {
                detail: "no capability is weighted".to_string(),
            });
        }
        for (capability, weight) in &self.weights {
            if !weight.is_finite() || *weight <= 0.0 {
                return Err(AtlasError::MalformedWeightingPolicy {
                    detail: format!("weight {weight} for {capability} is not positive and finite"),
                });
            }
        }
        Ok(())
    }

    pub fn weights(&self) -> impl Iterator<Item = (&CapabilityId, f64)> {
        self.weights.iter().map(|(c, w)| (c, *w))
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &CapabilityId> {
        self.weights.keys()
    }
}

/// An eligible composite. Carries the tier it may be stated at, because a composite is a claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Composite {
    pub intended_use: String,
    pub value: f64,
    pub weighted_capabilities: usize,
    /// The weakest permitted claim across the weighted set. A composite cannot outrank its
    /// weakest component.
    pub tier: ClaimTier,
}

/// Computes a composite, or refuses.
///
/// Refusal is the common case and the correct one. The four gates, all from 33.01 and 43.40:
/// every weighted capability must exist; none may be a hole; no two may be confounded, because
/// weighting them counts one signal twice; and each must already license a claim, which
/// transitively requires its own subtree to be measured.
///
/// There is no code path that treats an unmeasured capability as a zero. That is the entire
/// reason this function returns `Result` rather than an `f64`.
pub fn composite(atlas: &Atlas, policy: &WeightingPolicy) -> Result<Composite, AtlasError> {
    policy.validate()?;
    let ontology = atlas.ontology();
    let weighted: Vec<&CapabilityId> = policy.capabilities().collect();

    for capability in &weighted {
        ontology.node(capability)?;
    }
    for (position, left) in weighted.iter().enumerate() {
        let confounded = ontology.confounded_with(left)?;
        for right in weighted.iter().skip(position + 1) {
            if confounded.contains(*right) {
                return Err(AtlasError::ConfoundedAggregation {
                    left: left.to_string(),
                    right: right.to_string(),
                });
            }
        }
    }

    let mut total_weight = 0.0f64;
    let mut accumulated = 0.0f64;
    let mut tier = ClaimTier::ProspectiveWorkflow;

    for (capability, weight) in policy.weights() {
        let cell = atlas
            .cell(capability)
            .ok_or_else(|| AtlasError::UnknownCapability {
                capability: capability.to_string(),
                ontology_version: ontology.version().to_string(),
            })?;
        let Some(score) = cell.score() else {
            let Some(reason) = cell.unmeasured_reason() else {
                return Err(AtlasError::CompositeIneligible {
                    reason: format!("{capability} is missing its unmeasured reason"),
                });
            };
            return Err(AtlasError::CompositeIneligible {
                reason: format!("{capability} is unmeasured ({reason})"),
            });
        };
        let permitted = permitted_claim(atlas, capability);
        if permitted == ClaimTier::NoClaim {
            return Err(AtlasError::CompositeIneligible {
                reason: format!("{capability} licenses no claim of its own"),
            });
        }
        tier = tier.min(permitted);
        total_weight += weight;
        accumulated += weight * score;
    }

    Ok(Composite {
        intended_use: policy.intended_use.clone(),
        value: accumulated / total_weight,
        weighted_capabilities: weighted.len(),
        tier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighting_policy_deserialization_rejects_non_positive_weights() {
        let capability = CapabilityId::parse("capability").expect("parses");
        let policy = WeightingPolicy::declare("triage", [(capability, 1.0)]).expect("declares");
        let mut value = serde_json::to_value(&policy).expect("serialises");
        value["weights"]["capability"] = serde_json::json!(0.0);
        assert!(serde_json::from_value::<WeightingPolicy>(value).is_err());

        let value = serde_json::json!({"intended_use": "triage", "weights": {}});
        assert!(serde_json::from_value::<WeightingPolicy>(value).is_err());
    }
}
