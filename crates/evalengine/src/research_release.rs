//! Evidence-backed production release review for research capabilities.
//!
//! Atlas feature: `AFA-evalengine-P13-F01`.
//!
//! A benchmark score is not a release decision. This adapter joins the typed
//! [`EvaluationCard`] with independent replication, adversarial checks, and
//! provenance completeness, then emits a deterministic review artifact. It is
//! deliberately fail-closed: a card cannot be promoted merely because its
//! headline verdict says `Pass`.

use bioprism_foundation::{
    EvaluationCard, ReleaseVerdict, ResearchContractError, RESEARCH_CONTRACT_SCHEMA_VERSION,
    PRECLINICAL_BOUNDARY,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// Stable atlas identity for this implementation slice.
pub const FEATURE_ID: &str = "AFA-evalengine-P13-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

/// One independently replayed result at a named preclinical research site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationEvidence {
    pub site: String,
    pub artifact_digest: ContentHash,
    pub verdict: ReleaseVerdict,
}

/// A named adversarial or safety check with an immutable evidence reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdversarialCheck {
    pub check_id: String,
    pub evidence_digest: ContentHash,
    pub passed: bool,
}

/// The explicit contract for a production release review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReviewPolicy {
    pub minimum_baselines: usize,
    pub minimum_metrics: usize,
    pub minimum_replication_sites: usize,
    pub require_adversarial_checks: bool,
    pub require_complete_provenance: bool,
}

impl Default for ReleaseReviewPolicy {
    fn default() -> Self {
        Self {
            minimum_baselines: 2,
            minimum_metrics: 1,
            minimum_replication_sites: 2,
            require_adversarial_checks: true,
            require_complete_provenance: true,
        }
    }
}

/// The immutable output of a review. `Blocked` is a positive result: it says
/// the evidence was inspected and the production claim was not admitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReview {
    pub schema_version: String,
    pub feature_id: String,
    pub capability_id: String,
    pub card_digest: ContentHash,
    pub verdict: ReleaseVerdict,
    pub reasons: Vec<String>,
    pub replications: Vec<ReplicationEvidence>,
    pub checks: Vec<AdversarialCheck>,
    pub provenance_complete: bool,
    pub boundary: String,
}

impl ReleaseReview {
    pub fn digest(&self) -> Result<ContentHash, serde_json::Error> {
        serde_json::to_value(self)
            .map_err(|error| serde_json::Error::io(std::io::Error::other(error)))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|error| serde_json::Error::io(std::io::Error::other(error)))
            })
    }

    pub fn validate(&self) -> Result<(), ResearchContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.feature_id != FEATURE_ID {
            return Err(ResearchContractError::MissingField { field: "feature_id" });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.capability_id.clone(),
            });
        }
        if self.reasons.is_empty() {
            return Err(ResearchContractError::MissingReason {
                item: self.capability_id.clone(),
            });
        }
        Ok(())
    }
}

/// Validate a typed card and issue a deterministic, fail-closed release review.
pub fn review_release(
    card: &EvaluationCard,
    replications: Vec<ReplicationEvidence>,
    checks: Vec<AdversarialCheck>,
    provenance_complete: bool,
    policy: &ReleaseReviewPolicy,
) -> Result<ReleaseReview, ResearchContractError> {
    card.validate()?;
    let card_value = serde_json::to_value(card).map_err(|error| {
        ResearchContractError::Serialization {
            item: "evaluation_card".into(),
            message: error.to_string(),
        }
    })?;
    let card_digest = ContentHash::of_value(&card_value).map_err(|error| {
        ResearchContractError::Serialization {
            item: "evaluation_card".into(),
            message: error.to_string(),
        }
    })?;

    let mut reasons = Vec::new();
    let mut blocked = false;
    if card.baselines.len() < policy.minimum_baselines {
        blocked = true;
        reasons.push(format!(
            "baseline floor unmet: {} < {}",
            card.baselines.len(), policy.minimum_baselines
        ));
    }
    if card.metrics.len() < policy.minimum_metrics {
        blocked = true;
        reasons.push(format!(
            "metric floor unmet: {} < {}",
            card.metrics.len(), policy.minimum_metrics
        ));
    }
    let distinct_sites = replications
        .iter()
        .map(|replication| replication.site.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if distinct_sites.len() < policy.minimum_replication_sites {
        blocked = true;
        reasons.push(format!(
            "independent replication floor unmet: {} < {}",
            distinct_sites.len(), policy.minimum_replication_sites
        ));
    }
    if replications
        .iter()
        .any(|replication| replication.verdict != ReleaseVerdict::Pass)
    {
        blocked = true;
        reasons.push("at least one replication did not pass".into());
    }
    if policy.require_adversarial_checks
        && (checks.is_empty() || checks.iter().any(|check| !check.passed))
    {
        blocked = true;
        reasons.push("adversarial/safety checks are missing or failed".into());
    }
    if policy.require_complete_provenance && !provenance_complete {
        blocked = true;
        reasons.push("provenance completeness is not established".into());
    }
    if !blocked {
        reasons.push("all declared release gates passed".into());
    }
    let verdict = match card.release_verdict {
        ReleaseVerdict::NotEvaluated => ReleaseVerdict::NotEvaluated,
        ReleaseVerdict::Blocked => ReleaseVerdict::Blocked,
        ReleaseVerdict::Conditional => ReleaseVerdict::Conditional,
        ReleaseVerdict::Pass if blocked => ReleaseVerdict::Blocked,
        ReleaseVerdict::Pass => ReleaseVerdict::Pass,
    };
    let review = ReleaseReview {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        capability_id: card.capability_id.clone(),
        card_digest,
        verdict,
        reasons,
        replications,
        checks,
        provenance_complete,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    review.validate()?;
    Ok(review)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{EvaluationMetric, UncertaintyStatement};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn card(verdict: ReleaseVerdict) -> EvaluationCard {
        EvaluationCard {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            capability_id: "AFA-evalengine-P13-F01-demo".into(),
            benchmark_world: "preclinical-synthetic-world-v1".into(),
            baselines: vec!["fixed-search-v1".into(), "human-retrieval-v1".into()],
            metrics: vec![EvaluationMetric {
                name: "auditable_discovery_rate".into(),
                value: "0.42 conclusions/hour".into(),
                uncertainty: "95% bootstrap interval [0.31,0.52]".into(),
            }],
            uncertainty: vec![UncertaintyStatement {
                kind: "sampling".into(),
                statement: "synthetic benchmark only".into(),
            }],
            limitations: vec!["no human-subject or clinical-source data".into()],
            release_verdict: verdict,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn replications() -> Vec<ReplicationEvidence> {
        vec![
            ReplicationEvidence {
                site: "site-a".into(),
                artifact_digest: hash("a"),
                verdict: ReleaseVerdict::Pass,
            },
            ReplicationEvidence {
                site: "site-b".into(),
                artifact_digest: hash("b"),
                verdict: ReleaseVerdict::Pass,
            },
        ]
    }

    fn checks() -> Vec<AdversarialCheck> {
        vec![AdversarialCheck {
            check_id: "prompt-injection".into(),
            evidence_digest: hash("check"),
            passed: true,
        }]
    }

    #[test]
    fn pass_requires_replication_checks_and_provenance() {
        let review = review_release(
            &card(ReleaseVerdict::Pass),
            replications(),
            checks(),
            true,
            &ReleaseReviewPolicy::default(),
        )
        .unwrap();
        assert_eq!(review.verdict, ReleaseVerdict::Pass);
        review.validate().unwrap();
    }

    #[test]
    fn a_headline_pass_is_blocked_when_replication_is_missing() {
        let review = review_release(
            &card(ReleaseVerdict::Pass),
            vec![replications().remove(0)],
            checks(),
            true,
            &ReleaseReviewPolicy::default(),
        )
        .unwrap();
        assert_eq!(review.verdict, ReleaseVerdict::Blocked);
        assert!(review.reasons.iter().any(|reason| reason.contains("replication")));
    }

    #[test]
    fn identical_inputs_have_identical_review_digests() {
        let left = review_release(
            &card(ReleaseVerdict::Pass),
            replications(),
            checks(),
            true,
            &ReleaseReviewPolicy::default(),
        )
        .unwrap();
        let right = review_release(
            &card(ReleaseVerdict::Pass),
            replications(),
            checks(),
            true,
            &ReleaseReviewPolicy::default(),
        )
        .unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }
}
