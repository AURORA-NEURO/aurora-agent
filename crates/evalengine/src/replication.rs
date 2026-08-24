//! Deterministic replication and negative-result management.
//!
//! Atlas feature: `AFA-evalengine-P15-F01`.
//!
//! This module turns independently produced preclinical observations into an auditable
//! replication disposition. Null results are retained as evidence, while contradictory effects
//! and incomplete independent-site coverage prevent a positive replication claim. Only typed
//! observation metadata and digests are exported; raw measurements remain local.

use bioprism_foundation::{
    CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState, LossSeverity,
    ProvenanceLink, ResearchContractError, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-evalengine-P15-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationOutcome {
    Positive,
    Null,
    Negative,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationObservation {
    pub site: String,
    pub assay: String,
    pub outcome: ReplicationOutcome,
    pub effect: Option<f64>,
    pub uncertainty: Option<f64>,
    pub artifact_digest: ContentHash,
    pub independent: bool,
    pub preregistered: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationPolicy {
    pub minimum_independent_sites: usize,
    pub require_preregistered: bool,
    /// A range larger than this threshold is treated as contradictory rather than averaged away.
    pub max_effect_disagreement: Option<f64>,
}

impl Default for ReplicationPolicy {
    fn default() -> Self {
        Self {
            minimum_independent_sites: 2,
            require_preregistered: true,
            max_effect_disagreement: Some(0.5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationRequest {
    pub capability_id: String,
    pub hypothesis: String,
    pub observations: Vec<ReplicationObservation>,
    pub policy: ReplicationPolicy,
}

impl ReplicationRequest {
    pub fn validate(&self) -> Result<(), ReplicationError> {
        if self.capability_id.trim().is_empty() || self.hypothesis.trim().is_empty() {
            return Err(ReplicationError::InvalidField(
                "capability_id and hypothesis are required".into(),
            ));
        }
        if self.observations.is_empty() {
            return Err(ReplicationError::InvalidField(
                "at least one observation is required".into(),
            ));
        }
        if self.policy.minimum_independent_sites == 0 {
            return Err(ReplicationError::InvalidField(
                "minimum_independent_sites must be positive".into(),
            ));
        }
        if self
            .policy
            .max_effect_disagreement
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(ReplicationError::InvalidField(
                "max_effect_disagreement must be finite and non-negative".into(),
            ));
        }
        let mut keys = BTreeSet::new();
        for observation in &self.observations {
            if observation.site.trim().is_empty() || observation.assay.trim().is_empty() {
                return Err(ReplicationError::InvalidField(
                    "observation site and assay are required".into(),
                ));
            }
            let key = format!("{}-{}", observation.site, observation.assay);
            if !keys.insert(key) {
                return Err(ReplicationError::DuplicateObservation {
                    site: observation.site.clone(),
                    assay: observation.assay.clone(),
                });
            }
            if observation.effect.is_some_and(|value| !value.is_finite())
                || observation
                    .uncertainty
                    .is_some_and(|value| !value.is_finite() || value < 0.0)
            {
                return Err(ReplicationError::InvalidField(
                    "effect must be finite and uncertainty finite/non-negative".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationDisposition {
    Replicated,
    PartiallyReplicated,
    Contradicted,
    NullResult,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationSummary {
    pub disposition: ReplicationDisposition,
    pub total_observations: usize,
    pub independent_sites: usize,
    pub positive_count: usize,
    pub null_count: usize,
    pub negative_count: usize,
    pub inconclusive_count: usize,
    pub effect_min: Option<f64>,
    pub effect_max: Option<f64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationReport {
    pub schema_version: String,
    pub feature_id: String,
    pub capability_id: String,
    pub request_digest: ContentHash,
    pub summary: ReplicationSummary,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ReplicationReport {
    pub fn validate(&self) -> Result<(), ReplicationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ReplicationError::Contract(
                ResearchContractError::SchemaVersion {
                    expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                    found: self.schema_version.clone(),
                },
            ));
        }
        if self.feature_id != FEATURE_ID || self.capability_id.trim().is_empty() {
            return Err(ReplicationError::InvalidField(
                "replication feature or capability is missing".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ReplicationError::Contract(
                ResearchContractError::BoundaryMismatch {
                    capability: self.capability_id.clone(),
                },
            ));
        }
        if self.summary.total_observations == 0 || self.summary.reasons.is_empty() {
            return Err(ReplicationError::InvalidField(
                "replication summary requires observations and reasons".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(ReplicationError::Contract)
    }

    pub fn verify_payload(&self, payload: &Value) -> Result<(), ReplicationError> {
        self.validate()?;
        self.artifact
            .verify_payload(payload)
            .map_err(ReplicationError::Contract)
    }

    pub fn digest(&self) -> Result<ContentHash, ReplicationError> {
        let value = serde_json::to_value(self)
            .map_err(|error| ReplicationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReplicationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReplicationError {
    #[error("invalid replication request: {0}")]
    InvalidField(String),
    #[error("duplicate observation for {site}/{assay}")]
    DuplicateObservation { site: String, assay: String },
    #[error("research contract error: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "evalengine".into(),
        consumers: ["replication scientist".into(), "research program lead".into()].into(),
        behavior: "compares independent preclinical observations, preserves null and contradictory outcomes, and emits a deterministic replication disposition".into(),
        value: "prevents headline claims from erasing negative evidence or passing without independent-site coverage".into(),
        inputs: vec![TypedPort {
            name: "replication_request".into(),
            schema: "ReplicationRequest@1".into(),
            required: true,
        }],
        outputs: vec![
            TypedPort {
                name: "replication_report".into(),
                schema: "ReplicationReport@1".into(),
                required: true,
            },
            TypedPort {
                name: "typed_artifact".into(),
                schema: "TypedResearchArtifact@1".into(),
                required: true,
            },
        ],
        effects: [
            Effect::ReadLocalData,
            Effect::WriteLocalArtifact,
            Effect::ExecuteLocalComputation,
        ]
        .into(),
        permissions: [
            "read:institution-local-replications".into(),
            "write:local-research-artifact".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "fixture:evalengine-replication-ledger".into(),
            state: EvidenceState::Supported,
            locator: Some("fixtures/replication".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: bioprism_foundation::AutonomyTier::A0,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn evaluate_replication(
    request: &ReplicationRequest,
) -> Result<ReplicationReport, ReplicationError> {
    request.validate()?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| ReplicationError::Serialization(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| ReplicationError::Serialization(error.to_string()))?;

    let mut independent_sites = BTreeSet::new();
    let mut positive_count = 0;
    let mut null_count = 0;
    let mut negative_count = 0;
    let mut inconclusive_count = 0;
    let mut effects = Vec::new();
    let mut reasons = Vec::new();
    let preregistration_ok = !request.policy.require_preregistered
        || request
            .observations
            .iter()
            .all(|observation| observation.preregistered);
    for observation in &request.observations {
        if observation.independent {
            independent_sites.insert(observation.site.clone());
        }
        if let Some(effect) = observation.effect {
            effects.push(effect);
        }
        match observation.outcome {
            ReplicationOutcome::Positive => positive_count += 1,
            ReplicationOutcome::Null => null_count += 1,
            ReplicationOutcome::Negative => negative_count += 1,
            ReplicationOutcome::Inconclusive => inconclusive_count += 1,
        }
    }
    effects.sort_by(f64::total_cmp);
    let effect_min = effects.first().copied();
    let effect_max = effects.last().copied();
    let disagreement = effect_min.zip(effect_max).map(|(min, max)| max - min);
    let effect_consistent = request
        .policy
        .max_effect_disagreement
        .map_or(true, |threshold| {
            disagreement.map_or(true, |delta| delta <= threshold)
        });
    if !preregistration_ok {
        reasons.push("at least one observation lacks preregistration evidence".into());
    }
    if independent_sites.len() < request.policy.minimum_independent_sites {
        reasons.push(format!(
            "independent-site floor unmet: {} < {}",
            independent_sites.len(),
            request.policy.minimum_independent_sites
        ));
    }
    if !effect_consistent {
        reasons.push(format!(
            "effect disagreement exceeds threshold: {} > {}",
            disagreement.unwrap_or_default(),
            request.policy.max_effect_disagreement.unwrap_or_default()
        ));
    }

    let disposition = if negative_count > 0 || !effect_consistent {
        reasons.push("negative or materially contradictory replication evidence retained".into());
        ReplicationDisposition::Contradicted
    } else if positive_count == 0 && null_count == request.observations.len() {
        reasons.push(
            "all observations are null; null result is retained as a publishable state".into(),
        );
        ReplicationDisposition::NullResult
    } else if positive_count > 0
        && preregistration_ok
        && effect_consistent
        && independent_sites.len() >= request.policy.minimum_independent_sites
        && inconclusive_count == 0
    {
        reasons.push("positive effects reproduced across the independent-site floor".into());
        ReplicationDisposition::Replicated
    } else if positive_count > 0 {
        reasons.push(
            "positive observations exist but one or more replication gates remain open".into(),
        );
        ReplicationDisposition::PartiallyReplicated
    } else {
        reasons.push("no decisive positive replication claim is supported".into());
        ReplicationDisposition::InsufficientEvidence
    };

    let summary = ReplicationSummary {
        disposition,
        total_observations: request.observations.len(),
        independent_sites: independent_sites.len(),
        positive_count,
        null_count,
        negative_count,
        inconclusive_count,
        effect_min,
        effect_max,
        reasons,
    };
    let observation_digests = request
        .observations
        .iter()
        .map(|observation| {
            json!({
                "site": observation.site,
                "assay": observation.assay,
                "outcome": observation.outcome,
                "artifact_digest": observation.artifact_digest,
                "independent": observation.independent,
                "preregistered": observation.preregistered,
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "capability_id": request.capability_id,
        "hypothesis": request.hypothesis,
        "request_digest": request_digest,
        "summary": summary,
        "observation_digests": observation_digests,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = request
        .observations
        .iter()
        .map(|observation| ProvenanceLink {
            source_id: format!("{}:{}", observation.site, observation.assay),
            relation: "replication-observation-digest".into(),
            digest: observation.artifact_digest.clone(),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("replication-report:{}", request.capability_id),
        "application/vnd.aurora.replication-report+json",
        &payload,
        vec![SemanticLoss {
            field: "raw_measurements".into(),
            reason:
                "raw experimental data remains institution-local; report exports typed digests only"
                    .into(),
            severity: LossSeverity::Bounded,
        }],
        provenance,
    )?;
    let report = ReplicationReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        capability_id: request.capability_id.clone(),
        request_digest,
        summary,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    report.verify_payload(&payload)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn observation(site: &str, outcome: ReplicationOutcome, effect: f64) -> ReplicationObservation {
        ReplicationObservation {
            site: site.into(),
            assay: "organoid-imaging".into(),
            outcome,
            effect: Some(effect),
            uncertainty: Some(0.1),
            artifact_digest: hash(site),
            independent: true,
            preregistered: true,
        }
    }

    fn request(observations: Vec<ReplicationObservation>) -> ReplicationRequest {
        ReplicationRequest {
            capability_id: "capability-1".into(),
            hypothesis: "preclinical mechanism reproduces".into(),
            observations,
            policy: ReplicationPolicy::default(),
        }
    }

    #[test]
    fn positive_replication_requires_independent_sites() {
        let report = evaluate_replication(&request(vec![observation(
            "site-a",
            ReplicationOutcome::Positive,
            0.2,
        )]))
        .unwrap();
        assert_eq!(
            report.summary.disposition,
            ReplicationDisposition::PartiallyReplicated
        );
        assert!(report
            .summary
            .reasons
            .iter()
            .any(|reason| reason.contains("floor")));
    }

    #[test]
    fn all_null_observations_are_first_class_evidence() {
        let report = evaluate_replication(&request(vec![
            observation("site-a", ReplicationOutcome::Null, 0.0),
            observation("site-b", ReplicationOutcome::Null, 0.0),
        ]))
        .unwrap();
        assert_eq!(
            report.summary.disposition,
            ReplicationDisposition::NullResult
        );
        assert_eq!(report.summary.null_count, 2);
    }

    #[test]
    fn negative_or_disagreeing_effects_are_not_averaged_away() {
        let report = evaluate_replication(&request(vec![
            observation("site-a", ReplicationOutcome::Positive, 0.1),
            observation("site-b", ReplicationOutcome::Negative, 0.2),
        ]))
        .unwrap();
        assert_eq!(
            report.summary.disposition,
            ReplicationDisposition::Contradicted
        );
    }

    #[test]
    fn identical_requests_have_identical_report_digests() {
        let observations = vec![
            observation("site-a", ReplicationOutcome::Positive, 0.2),
            observation("site-b", ReplicationOutcome::Positive, 0.25),
        ];
        let left = evaluate_replication(&request(observations.clone())).unwrap();
        let right = evaluate_replication(&request(observations)).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        manifest().validate().unwrap();
    }
}
