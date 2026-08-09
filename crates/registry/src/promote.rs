//! Promotion: asking for a tier, and being told what is missing.
//!
//! Blueprint 27.18 ("publish at initial tier; promote only with additional evidence") and 10.07
//! ("support tier promotion and demotion"). The workflow in 27.18 runs submit → conformance →
//! author card → security scan → domain review → publish → promote; this crate implements the
//! *last two steps as predicates* and none of the preceding process, because process is people.
//!
//! Promotion here is not a state transition on the pack. Nothing is written back, because writing
//! to a pack changes its digest and therefore its identity — the immutability rule of 10.02.
//! Promotion is a **decision about evidence**: [`Promotion`] is a receipt naming the requirements
//! satisfied and the exact content they were satisfied against, and the registry records it in its
//! own append-only log ([`crate::index`]) rather than in the artifact.
//!
//! The asymmetry is intentional. Promoting is a request that can be refused; demoting is an
//! observation that cannot ([`crate::tier::reassess`]). Evidence can rot; permission cannot heal.

use crate::pack::BenchmarkPack;
use crate::tier::{assess, Requirement, TierPolicy, TrustTier, UnmetRequirement};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A receipt for a tier that was granted.
///
/// Carries both digests: `pack_sha256` identifies the artifact, `core_sha256` identifies the
/// benchmark content, so a later revision that merely adds provenance can be recognised as the
/// same content rather than reviewed from scratch.
///
/// `earned` may exceed `granted`. A publisher is free to ask for less than the evidence supports,
/// and recording the gap keeps that choice visible instead of silently upgrading them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Promotion {
    pub pack_sha256: String,
    pub core_sha256: String,
    pub granted: TrustTier,
    pub earned: TrustTier,
    /// Every requirement checked to grant this tier, in ladder order.
    pub satisfied: Vec<Requirement>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PromotionError {
    #[error(
        "{target} is not supported by this pack's evidence, which earns {earned}; {} requirement(s) unmet",
        unmet.len()
    )]
    EvidenceInsufficient {
        target: TrustTier,
        earned: TrustTier,
        unmet: Vec<UnmetRequirement>,
    },

    #[error("{target} is the absence of a tier and cannot be granted")]
    NotATier { target: TrustTier },

    #[error("pack cannot be addressed: {0}")]
    Undigestible(String),
}

impl PromotionError {
    /// The requirements that blocked the request; empty for the non-evidential errors.
    pub fn unmet(&self) -> &[UnmetRequirement] {
        match self {
            PromotionError::EvidenceInsufficient { unmet, .. } => unmet,
            _ => &[],
        }
    }
}

/// Grants `target` if and only if every requirement at or below `target` is met.
///
/// There is no force, override or waiver parameter. 43.40 does permit a "time-limited signed
/// waiver with remediation" — that needs signing keys and a clock, neither of which exists here,
/// and a waiver that is merely a boolean argument is not a waiver, it is a bypass.
pub fn promote(pack: &BenchmarkPack, target: TrustTier) -> Result<Promotion, PromotionError> {
    promote_with(pack, target, &TierPolicy::default())
}

pub fn promote_with(
    pack: &BenchmarkPack,
    target: TrustTier,
    policy: &TierPolicy,
) -> Result<Promotion, PromotionError> {
    if target == TrustTier::Unranked {
        return Err(PromotionError::NotATier { target });
    }

    let assessment = assess(pack, policy);
    let unmet = assessment.unmet_for(target);
    if !unmet.is_empty() {
        return Err(PromotionError::EvidenceInsufficient {
            target,
            earned: assessment.earned,
            unmet,
        });
    }

    let pack_sha256 = pack
        .digest()
        .map_err(|error| PromotionError::Undigestible(error.to_string()))?
        .as_str()
        .to_string();
    let core_sha256 = pack
        .core_digest()
        .map_err(|error| PromotionError::Undigestible(error.to_string()))?
        .as_str()
        .to_string();

    Ok(Promotion {
        pack_sha256,
        core_sha256,
        granted: target,
        earned: assessment.earned,
        satisfied: target.cumulative_requirements(policy),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::BenchmarkPack;

    #[test]
    fn the_absence_of_a_tier_cannot_be_granted() {
        let pack = BenchmarkPack::builder("demo", "0.1.0")
            .build()
            .expect("empty pack builds");
        let error = promote(&pack, TrustTier::Unranked).expect_err("unranked is not a tier");
        assert!(matches!(error, PromotionError::NotATier { .. }));
        assert!(error.unmet().is_empty());
    }
}
