//! Trust tiers, earned by evidence.
//!
//! Blueprint 10.07 (Trust Tiers and Review), 27.18 (Registry Trust Tiers, Review, and Promotion)
//! and the claim ladder of 43.40, whose governing sentence is *"No lower tier justifies a
//! higher-tier claim."*
//!
//! # Tiers are computed, not stored
//!
//! [`evaluate_tier`] walks the ladder from the bottom and stops at the first rung whose
//! requirements are not all met. Every requirement is a predicate over the pack document, so the
//! answer is a function of the artifact alone: two parties evaluating the same bytes get the same
//! tier, and neither of them has to trust the other. There is no field on a pack that says what
//! tier it is, because such a field would be an assertion and 27.18's abuse list opens with
//! "maintainer promotes own pack without review".
//!
//! Demotion falls out of the same function. [`reassess`] compares a previously published tier
//! against the tier the evidence currently supports and names the requirements that broke.
//!
//! # Where the numbers came from
//!
//! The blueprint fixes the *shape* of the ladder and almost none of its thresholds. 27.16 speaks
//! of "thousands of high-quality parent worlds", but that is a programme target, not a per-pack
//! gate. The one numeric threshold with a source is the effective-diversity floor: three
//! equivalence classes, from `bioprism_mutation::Diversity::is_publishable`. The rest live in
//! [`TierPolicy`] with conservative defaults precisely so that a deployment can argue about them
//! in one place rather than having them scattered through the checks.

use crate::pack::BenchmarkPack;
use bioprism_prism::Attestation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The claim ladder.
///
/// Named after 10.07's `experimental / generated-verified / silver / gold`, with `silver` given
/// its functional name (`Reviewed`) because "silver" says nothing about what was checked.
/// [`TrustTier::Unranked`] is not a badge — it is the absence of one, for a pack that does not
/// even meet the artifact-hygiene floor. 43.40's rule holds between every adjacent pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// Supports no claim at all. The pack exists; nothing follows from it.
    Unranked,
    /// Schema-valid, addressable, lineage-closed. Excluded from primary claims by default.
    Exploratory,
    /// Descends from recorded parents with every declared metamorphic relation observed to hold.
    /// Not individually human-reviewed.
    GeneratedVerified,
    /// Independently rebuilt, licensed, free of unresolved oracle disagreement, and diverse
    /// enough to be a benchmark rather than a robustness check.
    Reviewed,
    /// All of the above plus an approved independent review of this exact content, a higher
    /// parent floor and a cap on generated inflation.
    Gold,
}

impl TrustTier {
    /// The earned rungs, lowest first. [`TrustTier::Unranked`] is excluded: nothing earns it.
    pub const LADDER: [TrustTier; 4] = [
        TrustTier::Exploratory,
        TrustTier::GeneratedVerified,
        TrustTier::Reviewed,
        TrustTier::Gold,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TrustTier::Unranked => "unranked",
            TrustTier::Exploratory => "exploratory",
            TrustTier::GeneratedVerified => "generated-verified",
            TrustTier::Reviewed => "reviewed",
            TrustTier::Gold => "gold",
        }
    }

    /// The rung immediately above, or `None` at the top.
    pub fn next(self) -> Option<TrustTier> {
        match self {
            TrustTier::Unranked => Some(TrustTier::Exploratory),
            TrustTier::Exploratory => Some(TrustTier::GeneratedVerified),
            TrustTier::GeneratedVerified => Some(TrustTier::Reviewed),
            TrustTier::Reviewed => Some(TrustTier::Gold),
            TrustTier::Gold => None,
        }
    }

    /// The requirements this rung adds. Requirements are cumulative up the ladder.
    pub fn requirements(self, policy: &TierPolicy) -> Vec<Requirement> {
        match self {
            TrustTier::Unranked => Vec::new(),
            TrustTier::Exploratory => vec![
                Requirement::AttestationVerifies,
                Requirement::SchemaVersionSupported,
                Requirement::IntendedUseDeclared,
                Requirement::AtLeastOneInstance,
                Requirement::NoOrphanInstances,
            ],
            TrustTier::GeneratedVerified => vec![
                Requirement::LimitationsDeclared,
                Requirement::EveryPostconditionHeld,
                Requirement::DiversityAccountingMatchesInstances,
                Requirement::YieldLedgerConsistent,
                Requirement::IndependentParents {
                    floor: policy.generated_verified_parent_floor,
                },
            ],
            TrustTier::Reviewed => vec![
                Requirement::EffectiveDiversityIsPublishable,
                Requirement::IndependentParents {
                    floor: policy.reviewed_parent_floor,
                },
                Requirement::NoUnresolvedOracleDisagreement,
                Requirement::LicenseDeclared,
                Requirement::IndependentRebuildVerified,
            ],
            TrustTier::Gold => vec![
                Requirement::IndependentParents {
                    floor: policy.gold_parent_floor,
                },
                Requirement::EquivalenceClasses {
                    floor: policy.gold_equivalence_class_floor,
                },
                Requirement::InflationRatioAtMost {
                    ceiling: policy.gold_max_inflation_ratio,
                },
                Requirement::IndependentReviewApproved,
            ],
        }
    }

    /// Every requirement at or below this rung.
    pub fn cumulative_requirements(self, policy: &TierPolicy) -> Vec<Requirement> {
        TrustTier::LADDER
            .iter()
            .filter(|rung| **rung <= self)
            .flat_map(|rung| rung.requirements(policy))
            .collect()
    }
}

impl fmt::Display for TrustTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The thresholds the blueprint leaves open. See the module docs for why they live here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierPolicy {
    pub generated_verified_parent_floor: usize,
    pub reviewed_parent_floor: usize,
    pub gold_parent_floor: usize,
    pub gold_equivalence_class_floor: usize,
    pub gold_max_inflation_ratio: f64,
}

impl Default for TierPolicy {
    fn default() -> Self {
        TierPolicy {
            generated_verified_parent_floor: 1,
            reviewed_parent_floor: 3,
            gold_parent_floor: 5,
            gold_equivalence_class_floor: 8,
            gold_max_inflation_ratio: 4.0,
        }
    }
}

/// A checkable property of a pack document.
///
/// Every variant must be decidable by a third party holding only the pack. That constraint is what
/// excludes the requirements a real hub would also want — "the parent worlds are scientifically
/// sound", "the oracle is strong" — which need a domain expert, not a predicate. 40.32 draws the
/// same line: conformance is not accuracy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "requirement", rename_all = "snake_case")]
pub enum Requirement {
    /// The pack can be canonically serialised and hashes to its own attestation.
    AttestationVerifies,
    SchemaVersionSupported,
    IntendedUseDeclared,
    /// A pack that states no limitations has not thought about any (43.43).
    LimitationsDeclared,
    AtLeastOneInstance,
    /// Every instance names a parent the pack carries (27.16: no orphan descendants).
    NoOrphanInstances,
    /// No instance is `Violated` or `NotChecked`.
    EveryPostconditionHeld,
    /// The diversity report describes this instance list and not a larger one.
    DiversityAccountingMatchesInstances,
    YieldLedgerConsistent,
    IndependentParents {
        floor: usize,
    },
    /// At least three equivalence classes, per `Diversity::is_publishable`.
    EffectiveDiversityIsPublishable,
    EquivalenceClasses {
        floor: usize,
    },
    InflationRatioAtMost {
        ceiling: f64,
    },
    NoUnresolvedOracleDisagreement,
    LicenseDeclared,
    /// Somebody other than the publisher rebuilt this exact content and got the same core digest.
    IndependentRebuildVerified,
    /// Somebody other than the publisher approved this exact content.
    IndependentReviewApproved,
}

impl Requirement {
    pub fn label(self) -> &'static str {
        match self {
            Requirement::AttestationVerifies => "attestation verifies",
            Requirement::SchemaVersionSupported => "schema version supported",
            Requirement::IntendedUseDeclared => "intended use declared",
            Requirement::LimitationsDeclared => "limitations declared",
            Requirement::AtLeastOneInstance => "at least one instance",
            Requirement::NoOrphanInstances => "no orphan instances",
            Requirement::EveryPostconditionHeld => "every postcondition held",
            Requirement::DiversityAccountingMatchesInstances => "diversity accounting matches",
            Requirement::YieldLedgerConsistent => "yield ledger consistent",
            Requirement::IndependentParents { .. } => "independent parents",
            Requirement::EffectiveDiversityIsPublishable => "effective diversity publishable",
            Requirement::EquivalenceClasses { .. } => "equivalence classes",
            Requirement::InflationRatioAtMost { .. } => "inflation ratio capped",
            Requirement::NoUnresolvedOracleDisagreement => "no unresolved oracle disagreement",
            Requirement::LicenseDeclared => "license declared",
            Requirement::IndependentRebuildVerified => "independent rebuild verified",
            Requirement::IndependentReviewApproved => "independent review approved",
        }
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Requirement::IndependentParents { floor } => {
                write!(f, "independent parents >= {floor}")
            }
            Requirement::EquivalenceClasses { floor } => {
                write!(f, "equivalence classes >= {floor}")
            }
            Requirement::InflationRatioAtMost { ceiling } => {
                write!(f, "inflation ratio <= {ceiling:.2}")
            }
            other => f.write_str(other.label()),
        }
    }
}

/// A requirement that was checked and did not hold, with what was actually observed.
///
/// `detail` exists so that a demotion is actionable. "Not gold" is a verdict; "gold needs 5
/// independent parents, this pack has 2" is a work item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnmetRequirement {
    pub tier: TrustTier,
    pub requirement: Requirement,
    pub detail: String,
}

impl fmt::Display for UnmetRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} requires {}: {}",
            self.tier, self.requirement, self.detail
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RungAssessment {
    pub tier: TrustTier,
    pub met: bool,
    pub unmet: Vec<UnmetRequirement>,
}

/// The full ladder, every rung evaluated.
///
/// Every rung is evaluated even after one fails, because "gold needs a review and you also have no
/// license" is more useful than being told about the license, fixing it, and being told about the
/// review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierAssessment {
    pub earned: TrustTier,
    pub rungs: Vec<RungAssessment>,
}

impl TierAssessment {
    /// Everything blocking `target`, across every rung up to and including it.
    pub fn unmet_for(&self, target: TrustTier) -> Vec<UnmetRequirement> {
        self.rungs
            .iter()
            .filter(|rung| rung.tier <= target)
            .flat_map(|rung| rung.unmet.iter().cloned())
            .collect()
    }

    /// Everything blocking the rung immediately above the one that was earned.
    pub fn blockers_for_next(&self) -> Vec<UnmetRequirement> {
        match self.earned.next() {
            Some(next) => self
                .rungs
                .iter()
                .find(|rung| rung.tier == next)
                .map(|rung| rung.unmet.clone())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

/// The tier a pack's evidence supports, and what blocks the next rung up.
///
/// The returned vector is deliberately the blocker list for the *next* tier, not for the earned
/// one — by construction the earned tier has no unmet requirements. Use [`assess`] for the whole
/// ladder.
pub fn evaluate_tier(pack: &BenchmarkPack) -> (TrustTier, Vec<UnmetRequirement>) {
    evaluate_tier_with(pack, &TierPolicy::default())
}

pub fn evaluate_tier_with(
    pack: &BenchmarkPack,
    policy: &TierPolicy,
) -> (TrustTier, Vec<UnmetRequirement>) {
    let assessment = assess(pack, policy);
    let blockers = assessment.blockers_for_next();
    (assessment.earned, blockers)
}

/// Evaluates every rung of the ladder.
pub fn assess(pack: &BenchmarkPack, policy: &TierPolicy) -> TierAssessment {
    let mut rungs = Vec::with_capacity(TrustTier::LADDER.len());
    let mut earned = TrustTier::Unranked;
    let mut still_climbing = true;

    for tier in TrustTier::LADDER {
        let unmet: Vec<UnmetRequirement> = tier
            .requirements(policy)
            .into_iter()
            .filter_map(|requirement| {
                check(pack, requirement, policy)
                    .err()
                    .map(|detail| UnmetRequirement {
                        tier,
                        requirement,
                        detail,
                    })
            })
            .collect();
        let met = unmet.is_empty();
        if met && still_climbing {
            earned = tier;
        } else {
            still_climbing = false;
        }
        rungs.push(RungAssessment { tier, met, unmet });
    }

    TierAssessment { earned, rungs }
}

/// Whether a previously published tier still holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum TierVerdict {
    Upheld {
        tier: TrustTier,
    },
    Demoted {
        claimed: TrustTier,
        earned: TrustTier,
        reasons: Vec<UnmetRequirement>,
    },
}

impl TierVerdict {
    pub fn is_upheld(&self) -> bool {
        matches!(self, TierVerdict::Upheld { .. })
    }

    pub fn tier(&self) -> TrustTier {
        match self {
            TierVerdict::Upheld { tier } => *tier,
            TierVerdict::Demoted { earned, .. } => *earned,
        }
    }
}

/// Re-evaluates a pack against a tier it was previously published at.
///
/// A pack is immutable, so this only changes verdict when the *policy* changes or when the pack
/// was published at a tier its evidence never supported. Both are worth catching: 27.18 lists
/// "maintainer promotes own pack without review" as an abuse, and this is the check that finds it
/// after the fact.
pub fn reassess(pack: &BenchmarkPack, claimed: TrustTier, policy: &TierPolicy) -> TierVerdict {
    let assessment = assess(pack, policy);
    if assessment.earned >= claimed {
        TierVerdict::Upheld {
            tier: assessment.earned,
        }
    } else {
        TierVerdict::Demoted {
            claimed,
            earned: assessment.earned,
            reasons: assessment.unmet_for(claimed),
        }
    }
}

/// Checks one requirement. `Err` carries the observation that failed it.
fn check(
    pack: &BenchmarkPack,
    requirement: Requirement,
    _policy: &TierPolicy,
) -> Result<(), String> {
    match requirement {
        Requirement::AttestationVerifies => match pack.self_attestation() {
            Attestation::Valid => Ok(()),
            Attestation::Mismatch {
                claimed,
                recomputed,
            } => Err(format!("claims {claimed}, hashes to {recomputed}")),
            Attestation::Malformed(detail) => Err(format!("cannot be attested: {detail}")),
        },

        Requirement::SchemaVersionSupported => {
            if pack.schema_version == crate::pack::PACK_SCHEMA_VERSION {
                Ok(())
            } else {
                Err(format!(
                    "pack declares schema {:?}, this build understands {:?}",
                    pack.schema_version,
                    crate::pack::PACK_SCHEMA_VERSION
                ))
            }
        }

        Requirement::IntendedUseDeclared => {
            if pack.intended_use.trim().is_empty() {
                Err(
                    "intended_use is empty; a pack that does not say what it is for cannot be \
                     misused correctly"
                        .into(),
                )
            } else {
                Ok(())
            }
        }

        Requirement::LimitationsDeclared => {
            if pack.limitations.iter().any(|line| !line.trim().is_empty()) {
                Ok(())
            } else {
                Err("no limitations recorded".into())
            }
        }

        Requirement::AtLeastOneInstance => {
            if pack.instances.is_empty() {
                Err("pack carries no instances".into())
            } else {
                Ok(())
            }
        }

        Requirement::NoOrphanInstances => {
            let orphans = pack.orphans();
            if orphans.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{} instance(s) name an absent parent, first: {} -> {}",
                    orphans.len(),
                    orphans[0].id(),
                    orphans[0].parent_sha256
                ))
            }
        }

        Requirement::EveryPostconditionHeld => {
            let unvalidated = pack.unvalidated();
            if unvalidated.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{} of {} instance(s) are not validated, first: {} ({})",
                    unvalidated.len(),
                    pack.instances.len(),
                    unvalidated[0].id(),
                    unvalidated[0].postcondition.reason()
                ))
            }
        }

        Requirement::DiversityAccountingMatchesInstances => {
            if pack.diversity.instances == pack.instances.len() {
                Ok(())
            } else {
                Err(format!(
                    "diversity reports {} instance(s), pack carries {}",
                    pack.diversity.instances,
                    pack.instances.len()
                ))
            }
        }

        Requirement::YieldLedgerConsistent => {
            if !pack.yield_ledger.is_consistent() {
                Err(format!(
                    "ledger does not add up: {} attempted vs {} accepted + {} rejected + {} \
                     duplicate",
                    pack.yield_ledger.attempted,
                    pack.yield_ledger.accepted,
                    pack.yield_ledger.rejected,
                    pack.yield_ledger.duplicates
                ))
            } else if pack.yield_ledger.accepted != pack.instances.len() {
                Err(format!(
                    "ledger accepts {} but the pack carries {} instance(s)",
                    pack.yield_ledger.accepted,
                    pack.instances.len()
                ))
            } else {
                Ok(())
            }
        }

        Requirement::IndependentParents { floor } => {
            let distinct: BTreeSet<&str> = pack
                .parents
                .iter()
                .map(|parent| parent.sha256.as_str())
                .collect();
            if distinct.len() >= floor {
                Ok(())
            } else {
                Err(format!(
                    "{} distinct parent world(s), floor is {floor}",
                    distinct.len()
                ))
            }
        }

        Requirement::EffectiveDiversityIsPublishable => {
            if pack.diversity.is_publishable() {
                Ok(())
            } else {
                Err(format!(
                    "{} equivalence class(es); below three this is a robustness check, not a \
                     benchmark",
                    pack.diversity.equivalence_classes
                ))
            }
        }

        Requirement::EquivalenceClasses { floor } => {
            if pack.diversity.equivalence_classes >= floor {
                Ok(())
            } else {
                Err(format!(
                    "{} equivalence class(es), floor is {floor}",
                    pack.diversity.equivalence_classes
                ))
            }
        }

        Requirement::InflationRatioAtMost { ceiling } => {
            let ratio = pack.diversity.inflation_ratio;
            if !ratio.is_finite() {
                Err(format!("inflation ratio is not a finite number: {ratio}"))
            } else if ratio <= ceiling {
                Ok(())
            } else {
                Err(format!(
                    "{ratio:.2} instances per equivalence class, ceiling is {ceiling:.2}"
                ))
            }
        }

        Requirement::NoUnresolvedOracleDisagreement => {
            let open: Vec<&str> = pack
                .oracle_disagreements
                .iter()
                .filter(|entry| entry.resolution.is_unresolved())
                .map(|entry| entry.instance_id.as_str())
                .collect();
            if open.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{} unresolved disagreement(s), first on instance {}",
                    open.len(),
                    open[0]
                ))
            }
        }

        Requirement::LicenseDeclared => match pack.provenance.license.as_deref() {
            Some(license) if !license.trim().is_empty() => Ok(()),
            _ => Err("no license declared".into()),
        },

        Requirement::IndependentRebuildVerified => {
            let core = pack
                .core_digest()
                .map_err(|error| format!("core digest unavailable: {error}"))?;
            let publisher = pack.provenance.publisher.trim();
            let matching = pack.provenance.rebuilds.iter().any(|rebuild| {
                rebuild.rebuilt_core_sha256 == core.as_str()
                    && rebuild.rebuilt_by.trim() != publisher
                    && !rebuild.rebuilt_by.trim().is_empty()
            });
            if matching {
                Ok(())
            } else if pack.provenance.rebuilds.is_empty() {
                Err("no rebuild attestation".into())
            } else {
                Err(format!(
                    "{} rebuild attestation(s), none both independent of {publisher:?} and naming \
                     core digest {}",
                    pack.provenance.rebuilds.len(),
                    core.as_str()
                ))
            }
        }

        Requirement::IndependentReviewApproved => {
            let core = pack
                .core_digest()
                .map_err(|error| format!("core digest unavailable: {error}"))?;
            let publisher = pack.provenance.publisher.trim();
            let approved = pack.provenance.reviews.iter().any(|review| {
                matches!(review.finding, crate::pack::ReviewFinding::Approved)
                    && review.reviewed_core_sha256 == core.as_str()
                    && review.reviewer.trim() != publisher
                    && !review.reviewer.trim().is_empty()
            });
            if approved {
                Ok(())
            } else if pack.provenance.reviews.is_empty() {
                Err("no review record".into())
            } else {
                Err(format!(
                    "{} review(s), none both independent of {publisher:?} and approving core \
                     digest {}; a review of earlier content does not carry forward",
                    pack.provenance.reviews.len(),
                    core.as_str()
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_strictly_ordered_and_cumulative() {
        assert!(TrustTier::Unranked < TrustTier::Exploratory);
        assert!(TrustTier::Exploratory < TrustTier::GeneratedVerified);
        assert!(TrustTier::GeneratedVerified < TrustTier::Reviewed);
        assert!(TrustTier::Reviewed < TrustTier::Gold);

        let policy = TierPolicy::default();
        let lower = TrustTier::Reviewed.cumulative_requirements(&policy).len();
        let upper = TrustTier::Gold.cumulative_requirements(&policy).len();
        assert!(upper > lower);
        assert_eq!(TrustTier::Gold.next(), None);
    }
}
