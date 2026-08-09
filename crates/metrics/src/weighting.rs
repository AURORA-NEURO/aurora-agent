//! Predeclared weights, and the receipt that makes them citable.
//!
//! Blueprint 33.01: "Predeclare composite weights and run sensitivity analyses." A weighting
//! chosen after the scores are known is not a policy, it is a result — and the only difference
//! visible from outside is whether the weighting can be cited as a fixed object. So
//! [`DeclaredWeighting`] carries a `bioprism_ids::ContentHash` of its own policy, recomputed on
//! deserialization, and every ranking or aggregate built from it records that hash.
//!
//! The weighting itself is `bioprism_atlas::WeightingPolicy`, reused rather than reimplemented.
//! That crate already refuses an empty policy and a non-positive weight, and having two definitions
//! of "a declared weighting" in one workspace is how they drift.
//!
//! # Why the digest matters more than it looks
//!
//! ADR-006 permits "pack-specific aggregates ... when their construction is transparent". Transparent
//! means a reader can tell whether the weighting that produced today's number is the one that
//! produced last quarter's. A hash answers that in constant time; a prose methods section does not.
//!
//! # Not implemented
//!
//! No weight elicitation, no optimisation of weights against an outcome, and no default weighting.
//! A default would be the universal scalar score ADR-006 rejects, wearing a different name: every
//! weighting here has a declared intended use, and there is no constructor that omits one.

use crate::error::MetricsError;
use bioprism_atlas::{CapabilityId, WeightingPolicy};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// The wire form of a [`DeclaredWeighting`], and the only route in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclaredWeightingFields {
    pub policy: WeightingPolicy,
    pub digest: ContentHash,
}

/// A weighting policy plus the digest that identifies it.
///
/// Deserialization recomputes the digest and refuses a document whose stored hash disagrees, so a
/// weighting cannot claim the identity of a different one. Fields are private and construction
/// goes through [`DeclaredWeighting::declare`] or [`DeclaredWeighting::adopt`], which compute the
/// digest rather than accept it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DeclaredWeightingFields", into = "DeclaredWeightingFields")]
pub struct DeclaredWeighting {
    policy: WeightingPolicy,
    digest: ContentHash,
}

impl TryFrom<DeclaredWeightingFields> for DeclaredWeighting {
    type Error = MetricsError;

    fn try_from(fields: DeclaredWeightingFields) -> Result<Self, Self::Error> {
        let declared = DeclaredWeighting::adopt(fields.policy)?;
        if declared.digest != fields.digest {
            return Err(MetricsError::MalformedWeighting {
                detail: format!(
                    "stored digest {} does not match the policy, which digests to {}",
                    fields.digest.as_str(),
                    declared.digest.as_str()
                ),
            });
        }
        Ok(declared)
    }
}

impl From<DeclaredWeighting> for DeclaredWeightingFields {
    fn from(value: DeclaredWeighting) -> Self {
        DeclaredWeightingFields {
            policy: value.policy,
            digest: value.digest,
        }
    }
}

impl DeclaredWeighting {
    /// Declares a weighting for a stated intended use.
    pub fn declare(
        intended_use: impl Into<String>,
        weights: impl IntoIterator<Item = (CapabilityId, f64)>,
    ) -> Result<Self, MetricsError> {
        let policy = WeightingPolicy::declare(intended_use, weights).map_err(|error| {
            MetricsError::MalformedWeighting {
                detail: error.to_string(),
            }
        })?;
        DeclaredWeighting::adopt(policy)
    }

    /// Takes over a `bioprism-atlas` policy, revalidating it and computing its digest.
    ///
    /// Revalidation is not paranoia: `WeightingPolicy` derives `Deserialize`, so a policy that
    /// arrived as JSON has not passed through `WeightingPolicy::declare` and may carry an empty or
    /// non-positive weight set. Rebuilding through the validating constructor closes that door
    /// without reaching into another crate's internals.
    pub fn adopt(policy: WeightingPolicy) -> Result<Self, MetricsError> {
        let weights: Vec<(CapabilityId, f64)> = policy
            .weights()
            .map(|(capability, weight)| (capability.clone(), weight))
            .collect();
        let policy =
            WeightingPolicy::declare(policy.intended_use.clone(), weights).map_err(|error| {
                MetricsError::MalformedWeighting {
                    detail: error.to_string(),
                }
            })?;
        let value = serde_json::to_value(&policy).map_err(|error| MetricsError::Encoding {
            subject: "weighting policy".to_string(),
            detail: error.to_string(),
        })?;
        let digest = ContentHash::of_value(&value).map_err(|error| MetricsError::Encoding {
            subject: "weighting policy".to_string(),
            detail: error.to_string(),
        })?;
        Ok(DeclaredWeighting { policy, digest })
    }

    pub fn policy(&self) -> &WeightingPolicy {
        &self.policy
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    pub fn intended_use(&self) -> &str {
        &self.policy.intended_use
    }

    pub fn weights(&self) -> impl Iterator<Item = (&CapabilityId, f64)> {
        self.policy.weights()
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &CapabilityId> {
        self.policy.capabilities()
    }

    pub fn len(&self) -> usize {
        self.policy.capabilities().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The same weighting with one capability removed, or `None` when removing it would leave
    /// nothing to weight.
    ///
    /// This is the perturbation [`crate::ranking::RankInstability`] uses. It is leave-one-out
    /// rather than a random reweighting because a sensitivity analysis has to be reproducible from
    /// the declaration alone: there is no RNG anywhere in this crate, and a caller who cannot
    /// regenerate the perturbation set cannot check the number.
    pub fn without(&self, capability: &CapabilityId) -> Result<Option<Self>, MetricsError> {
        let remaining: Vec<(CapabilityId, f64)> = self
            .weights()
            .filter(|(id, _)| *id != capability)
            .map(|(id, weight)| (id.clone(), weight))
            .collect();
        if remaining.is_empty() {
            return Ok(None);
        }
        DeclaredWeighting::declare(
            format!("{} without {capability}", self.intended_use()),
            remaining,
        )
        .map(Some)
    }

    /// Every leave-one-out perturbation of this weighting, paired with the capability dropped.
    ///
    /// Deterministic and complete: the set is exactly as large as the weighting, and a reader with
    /// the declaration can rebuild it.
    pub fn leave_one_out(&self) -> Result<Vec<(CapabilityId, Self)>, MetricsError> {
        let mut out = Vec::new();
        for capability in self.capabilities().cloned().collect::<Vec<_>>() {
            if let Some(perturbed) = self.without(&capability)? {
                out.push((capability, perturbed));
            }
        }
        Ok(out)
    }
}
