//! The claim ladder.
//!
//! Implements blueprint 43.40. The normative sentence is one line long — "No lower tier justifies
//! a higher-tier claim" — and everything here exists to make it mechanical rather than editorial.
//!
//! [`permitted_claim`] returns the highest tier the atlas actually supports for a capability, and
//! [`license_claim`] refuses anything above it. Four rules bind the result downward:
//!
//! 1. **A hole is not a low score, and a hole licenses nothing.** If any *leaf* capability in the
//!    subtree is unmeasured for a reason that does not close it, the answer is
//!    [`ClaimTier::NoClaim`]. This is 33.01's worked interpretation: "A system with excellent
//!    literature synthesis but no executable-analysis coverage cannot receive an overall
//!    research-agent rank." Interior capabilities are aggregates and are exempt from the check —
//!    their tier comes from what sits beneath them, so having no direct evidence of their own is
//!    not a gap. The leaves are where measurement is possible and therefore where a hole means
//!    something.
//! 2. **A claim about a parent is the weakest claim about its children.** The tier is the minimum
//!    over the subtree, because a parent capability is a claim about everything underneath it.
//! 3. **Structure caps the tier regardless of the tier the evidence was filed under.** One parent
//!    world cannot license a cross-world claim; one site cannot license a multi-site claim; a
//!    model judge alone cannot license a public-world claim. 43.40: "Insufficient sample size
//!    yields uncertainty, not a favorable point estimate."
//! 4. **Safety gates are noncompensatory.** A failing — or unmeasured — capability that holds a
//!    `safety_constraint_on` edge into the target zeroes the claim outright. No amount of
//!    competence buys it back.
//!
//! NOT implemented: claim cards. 43.40 requires every public statement to carry exact wording,
//! workload domain, comparator, tier and evidence bundle. [`ClaimLicence`] carries the tier and
//! the binding constraint; the wording and the signed bundle belong to the release process.

use crate::atlas::Atlas;
use crate::error::AtlasError;
use crate::evidence::{EvidenceTier, Measurement, OracleTier};
use crate::ontology::CapabilityId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The release ladder of 43.40, in ascending order, with an explicit bottom rung.
///
/// [`ClaimTier::NoClaim`] is not "tier zero evidence". It is the refusal: nothing about this
/// capability may be asserted publicly, and that is a different statement from asserting that it
/// performs badly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimTier {
    NoClaim,
    UnitConformance,
    SyntheticStructural,
    PublicObservedWorlds,
    CrossDomainPublic,
    ControlledHiddenMultiSite,
    ProspectiveWorkflow,
}

impl ClaimTier {
    pub const LADDER: [ClaimTier; 7] = [
        ClaimTier::NoClaim,
        ClaimTier::UnitConformance,
        ClaimTier::SyntheticStructural,
        ClaimTier::PublicObservedWorlds,
        ClaimTier::CrossDomainPublic,
        ClaimTier::ControlledHiddenMultiSite,
        ClaimTier::ProspectiveWorkflow,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ClaimTier::NoClaim => "no_claim",
            ClaimTier::UnitConformance => "unit_conformance",
            ClaimTier::SyntheticStructural => "synthetic_structural",
            ClaimTier::PublicObservedWorlds => "public_observed_worlds",
            ClaimTier::CrossDomainPublic => "cross_domain_public",
            ClaimTier::ControlledHiddenMultiSite => "controlled_hidden_multi_site",
            ClaimTier::ProspectiveWorkflow => "prospective_workflow",
        }
    }

    pub fn from_evidence_tier(tier: EvidenceTier) -> Self {
        match tier {
            EvidenceTier::UnitConformance => ClaimTier::UnitConformance,
            EvidenceTier::SyntheticStructural => ClaimTier::SyntheticStructural,
            EvidenceTier::PublicObservedWorld => ClaimTier::PublicObservedWorlds,
            EvidenceTier::CrossDomainPublic => ClaimTier::CrossDomainPublic,
            EvidenceTier::ControlledHiddenMultiSite => ClaimTier::ControlledHiddenMultiSite,
            EvidenceTier::ProspectiveWorkflow => ClaimTier::ProspectiveWorkflow,
        }
    }

    pub fn licenses_anything(self) -> bool {
        self != ClaimTier::NoClaim
    }
}

impl fmt::Display for ClaimTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why the permitted tier is where it is.
///
/// A tier without a reason is unactionable: the point of returning
/// [`ClaimTier::SyntheticStructural`] when the team expected `CrossDomainPublic` is to say which
/// capability, and which missing structure, is holding the ladder down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaimConstraint {
    /// Nothing in the subtree was measured at all.
    NoEvidence { capability: String },
    /// A capability in the subtree is a hole whose reason does not close it.
    UnmeasuredInSubtree { capability: String, reason: String },
    /// A safety constraint on the target, or on an ancestor of it, is failing or unmeasured.
    SafetyGate { guard: String, detail: String },
    /// A measured capability in the subtree limits the tier, for the stated structural reason.
    LimitedByMeasurement { capability: String, detail: String },
}

impl ClaimConstraint {
    pub fn capability(&self) -> &str {
        match self {
            ClaimConstraint::NoEvidence { capability }
            | ClaimConstraint::UnmeasuredInSubtree { capability, .. }
            | ClaimConstraint::LimitedByMeasurement { capability, .. } => capability,
            ClaimConstraint::SafetyGate { guard, .. } => guard,
        }
    }
}

/// A permitted tier together with the constraint that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAssessment {
    pub capability: CapabilityId,
    pub ontology_version: String,
    pub permitted: ClaimTier,
    /// Empty only when the tier is the highest the ladder offers.
    #[serde(default)]
    pub constraints: Vec<ClaimConstraint>,
}

/// A licensed claim. Only [`license_claim`] issues one, and only at or below the permitted tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimLicence {
    pub capability: CapabilityId,
    pub ontology_version: String,
    pub tier: ClaimTier,
    pub permitted: ClaimTier,
    #[serde(default)]
    pub constraints: Vec<ClaimConstraint>,
}

/// The highest tier at which a claim about `capability` may be made.
///
/// An unknown capability yields [`ClaimTier::NoClaim`] rather than an error: asking about
/// something the ontology does not contain is exactly the case where nothing may be claimed, and
/// making the caller unwrap a `Result` to learn that invites `unwrap_or(default)`.
pub fn permitted_claim(atlas: &Atlas, capability: &CapabilityId) -> ClaimTier {
    assess_claim(atlas, capability).permitted
}

/// [`permitted_claim`] with the reasoning attached.
pub fn assess_claim(atlas: &Atlas, capability: &CapabilityId) -> ClaimAssessment {
    let version = atlas.ontology().version().to_string();
    let refuse = |constraints: Vec<ClaimConstraint>| ClaimAssessment {
        capability: capability.clone(),
        ontology_version: version.clone(),
        permitted: ClaimTier::NoClaim,
        constraints,
    };

    let Ok(subtree) = atlas.ontology().subtree(capability) else {
        return refuse(vec![ClaimConstraint::NoEvidence {
            capability: capability.to_string(),
        }]);
    };

    if let Some(constraint) = failing_safety_gate(atlas, capability) {
        return refuse(vec![constraint]);
    }

    let mut constraints = Vec::new();
    let mut tier: Option<ClaimTier> = None;

    for member in &subtree {
        let Some(cell) = atlas.cell(member) else {
            return refuse(vec![ClaimConstraint::UnmeasuredInSubtree {
                capability: member.to_string(),
                reason: "absent from atlas".to_string(),
            }]);
        };
        match cell.measurement() {
            None => {
                let Some(reason) = cell.unmeasured_reason() else {
                    return refuse(vec![ClaimConstraint::UnmeasuredInSubtree {
                        capability: member.to_string(),
                        reason: "cell is missing its unmeasured reason".into(),
                    }]);
                };
                if reason.supports_claim() {
                    continue;
                }
                let is_aggregate = atlas
                    .ontology()
                    .children(member)
                    .is_ok_and(|children| !children.is_empty());
                if is_aggregate {
                    continue;
                }
                return refuse(vec![ClaimConstraint::UnmeasuredInSubtree {
                    capability: member.to_string(),
                    reason: reason.as_str().to_string(),
                }]);
            }
            Some(measurement) => {
                let (member_tier, detail) = measured_tier(measurement);
                if let Some(detail) = detail {
                    constraints.push(ClaimConstraint::LimitedByMeasurement {
                        capability: member.to_string(),
                        detail: detail.to_string(),
                    });
                }
                tier = Some(match tier {
                    Some(current) => current.min(member_tier),
                    None => member_tier,
                });
            }
        }
    }

    match tier {
        None => refuse(vec![ClaimConstraint::NoEvidence {
            capability: capability.to_string(),
        }]),
        Some(permitted) => ClaimAssessment {
            capability: capability.clone(),
            ontology_version: version,
            permitted,
            constraints,
        },
    }
}

/// Issues a licence, or refuses with [`AtlasError::ClaimAboveEvidence`].
///
/// Requesting [`ClaimTier::NoClaim`] is an error rather than a trivial success: a licence to
/// assert nothing is not a thing the release process should be able to hold up.
pub fn license_claim(
    atlas: &Atlas,
    capability: &CapabilityId,
    requested: ClaimTier,
) -> Result<ClaimLicence, AtlasError> {
    if requested == ClaimTier::NoClaim {
        return Err(AtlasError::VacuousClaim {
            capability: capability.to_string(),
        });
    }
    let assessment = assess_claim(atlas, capability);
    if requested > assessment.permitted {
        return Err(AtlasError::ClaimAboveEvidence {
            capability: capability.to_string(),
            requested,
            permitted: assessment.permitted,
        });
    }
    Ok(ClaimLicence {
        capability: assessment.capability,
        ontology_version: assessment.ontology_version,
        tier: requested,
        permitted: assessment.permitted,
        constraints: assessment.constraints,
    })
}

/// The tier one measurement supports, after the structural caps of rule 3.
///
/// Returns the binding detail when a cap actually bit, so the caller can report *why* the tier is
/// lower than the tier the evidence was filed under.
fn measured_tier(measurement: &Measurement) -> (ClaimTier, Option<&'static str>) {
    let filed = ClaimTier::from_evidence_tier(measurement.highest_tier());
    let mut tier = filed;
    let mut detail: Option<&'static str> = None;

    let mut cap = |limit: ClaimTier, reason: &'static str, tier: &mut ClaimTier| {
        if *tier > limit {
            *tier = limit;
            detail = Some(reason);
        }
    };

    if measurement.independent_sites() < 2 {
        cap(
            ClaimTier::CrossDomainPublic,
            "fewer than two independent sites",
            &mut tier,
        );
    }
    if measurement.domains() < 2 {
        cap(
            ClaimTier::PublicObservedWorlds,
            "fewer than two domains",
            &mut tier,
        );
    }
    if measurement.independent_parents() < 2 {
        cap(
            ClaimTier::SyntheticStructural,
            "fewer than two independent parent worlds",
            &mut tier,
        );
    }
    if measurement.strongest_oracle() == OracleTier::ModelJudge {
        cap(
            ClaimTier::SyntheticStructural,
            "judged only by a model judge",
            &mut tier,
        );
    }
    if measurement.evaluable() < 2 {
        cap(
            ClaimTier::UnitConformance,
            "a single evaluable trial",
            &mut tier,
        );
    }
    (tier, detail)
}

/// A safety constraint that is failing or unmeasured, if any binds this capability.
///
/// 43.40: "Safety and protected-completeness gates are noncompensatory." An unmeasured guard
/// blocks just as hard as a failing one — an unrun safety check is not a passed safety check.
fn failing_safety_gate(atlas: &Atlas, capability: &CapabilityId) -> Option<ClaimConstraint> {
    let guards = atlas.ontology().safety_constraints_on(capability).ok()?;
    for guard in guards {
        match atlas.cell(&guard) {
            None => {
                return Some(ClaimConstraint::SafetyGate {
                    guard: guard.to_string(),
                    detail: "guard is absent from the atlas".to_string(),
                })
            }
            Some(cell) => match cell.measurement() {
                None => {
                    let Some(reason) = cell.unmeasured_reason() else {
                        return Some(ClaimConstraint::SafetyGate {
                            guard: guard.to_string(),
                            detail: "cell is missing its unmeasured reason".into(),
                        });
                    };
                    if reason.supports_claim() {
                        continue;
                    }
                    return Some(ClaimConstraint::SafetyGate {
                        guard: guard.to_string(),
                        detail: format!("guard is unmeasured ({reason})"),
                    });
                }
                Some(measurement) => {
                    if measurement.failures() > 0 {
                        return Some(ClaimConstraint::SafetyGate {
                            guard: guard.to_string(),
                            detail: format!(
                                "guard failed {} of {} evaluable trials",
                                measurement.failures(),
                                measurement.evaluable()
                            ),
                        });
                    }
                }
            },
        }
    }
    None
}
