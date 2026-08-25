//! Federated replication and negative-result assurance.
//!
//! Atlas feature: `AFA-adapter-P15-F28`.
//!
//! This product evaluates typed, policy-separated replication observations without moving raw
//! data. It preserves null, negative, contradictory, incomplete, and partitioned evidence as
//! explicit outcomes; no missing site or protocol is silently promoted to replication success.

use bioprism_foundation::{
    LossSeverity, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P15-F28";
pub const CONTRACT_VERSION: &str = "federated-replication-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol {
    pub claim_id: String,
    pub claim: String,
    pub protocol_digest: ContentHash,
    pub minimum_independent_sites: u32,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationOutcome {
    Positive,
    Null,
    Negative,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationObservation {
    pub observation_id: String,
    pub site_id: String,
    pub protocol_digest: ContentHash,
    pub outcome: ReplicationOutcome,
    pub result_digest: Option<ContentHash>,
    pub uncertainty: String,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationAssuranceRequest {
    pub claim_and_protocol: ClaimAndProtocol,
    pub observations: Vec<ReplicationObservation>,
    pub protected_omissions: Vec<String>,
    pub network_partition: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationVerdict {
    Replicated,
    PartiallyReplicated,
    Contradicted,
    NullResult,
    InsufficientEvidence,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplicationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub claim_id: String,
    pub protocol_digest: ContentHash,
    pub verdict: ReplicationVerdict,
    pub observation_order: Vec<String>,
    pub independent_site_order: Vec<String>,
    pub positive_count: u32,
    pub null_count: u32,
    pub negative_count: u32,
    pub inconclusive_count: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ReplicationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ReplicationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ReplicationAssuranceError::Contract(
                "replication assurance identity mismatch".into(),
            ));
        }
        if self.claim_id.trim().is_empty()
            || self.observation_order.is_empty()
            || self.reasons.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ReplicationAssuranceError::InvalidRequest(
                "replication identity, observations, reasons, locality, and boundary are required"
                    .into(),
            ));
        }
        if self
            .observation_order
            .windows(2)
            .any(|pair| pair[0] > pair[1])
            || self.observation_order.iter().collect::<BTreeSet<_>>().len()
                != self.observation_order.len()
            || self
                .independent_site_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
        {
            return Err(ReplicationAssuranceError::InvalidRequest(
                "replication ordering must be canonical and unique".into(),
            ));
        }
        if self.positive_count + self.null_count + self.negative_count + self.inconclusive_count
            != self.observation_order.len() as u32
        {
            return Err(ReplicationAssuranceError::InvalidRequest(
                "replication outcome counts do not match observations".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReplicationAssuranceError::Contract(error.to_string()))?;
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ReplicationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReplicationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReplicationAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReplicationAssuranceError {
    #[error("invalid replication assurance request: {0}")]
    InvalidRequest(String),
    #[error("replication assurance contract rejected: {0}")]
    Contract(String),
    #[error("duplicate replication observation {0}")]
    DuplicateObservation(String),
    #[error("replication observation protocol mismatch {0}")]
    ProtocolMismatch(String),
    #[error("replication assurance serialization failed: {0}")]
    Serialization(String),
}

pub fn assure_replication(
    request: &ReplicationAssuranceRequest,
) -> Result<ReplicationAssuranceReceipt, ReplicationAssuranceError> {
    validate_request(request)?;
    let protocol = &request.claim_and_protocol;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let observation_order = observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    let independent_site_order = observations
        .iter()
        .map(|observation| observation.site_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let positive_count = observations
        .iter()
        .filter(|observation| observation.outcome == ReplicationOutcome::Positive)
        .count() as u32;
    let null_count = observations
        .iter()
        .filter(|observation| observation.outcome == ReplicationOutcome::Null)
        .count() as u32;
    let negative_count = observations
        .iter()
        .filter(|observation| observation.outcome == ReplicationOutcome::Negative)
        .count() as u32;
    let inconclusive_count = observations
        .iter()
        .filter(|observation| observation.outcome == ReplicationOutcome::Inconclusive)
        .count() as u32;
    let mut omissions = request.protected_omissions.clone();
    let mut uncertainty = observations
        .iter()
        .map(|observation| {
            format!(
                "{}: {}",
                observation.observation_id, observation.uncertainty
            )
        })
        .collect::<Vec<_>>();
    let negative_evidence = observations
        .iter()
        .flat_map(|observation| {
            observation
                .negative_evidence
                .iter()
                .map(move |evidence| format!("{}: {}", observation.observation_id, evidence))
        })
        .collect::<Vec<_>>();
    if request.network_partition {
        omissions.push(
            "federation partition prevents confirmation of all permitted observations".into(),
        );
    }
    let verdict = if !protocol.policy_allow {
        ReplicationVerdict::Blocked
    } else if negative_count > 0 {
        ReplicationVerdict::Contradicted
    } else if positive_count == 0 && null_count == observations.len() as u32 {
        ReplicationVerdict::NullResult
    } else if positive_count >= protocol.minimum_independent_sites
        && independent_site_order.len() >= protocol.minimum_independent_sites as usize
        && omissions.is_empty()
    {
        ReplicationVerdict::Replicated
    } else if positive_count > 0 {
        ReplicationVerdict::PartiallyReplicated
    } else {
        ReplicationVerdict::InsufficientEvidence
    };
    let mut reasons = vec![format!(
        "{} observations evaluated with deterministic order and independent-site accounting",
        observations.len()
    )];
    let mut semantic_loss = Vec::new();
    if independent_site_order.len() < protocol.minimum_independent_sites as usize {
        reasons.push(format!(
            "independent-site floor unmet: {} < {}",
            independent_site_order.len(),
            protocol.minimum_independent_sites
        ));
    }
    if !omissions.is_empty() {
        reasons.push(
            "protected or partition omissions prevent an unconditional replication claim".into(),
        );
        semantic_loss.push(SemanticLoss { field: "omissions".into(), reason: "unobserved sites and partitioned institutions cannot be treated as negative or positive evidence".into(), severity: LossSeverity::DecisionRelevant });
    }
    if negative_count > 0 || !negative_evidence.is_empty() {
        reasons.push("negative and contradictory results remain first-class evidence".into());
    }
    if verdict == ReplicationVerdict::Blocked {
        reasons.push("policy denied replication assurance".into());
        uncertainty.push("policy denial is not evidence about the scientific claim".into());
    }
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "claim_id": protocol.claim_id, "protocol_digest": protocol.protocol_digest, "verdict": verdict, "observation_order": observation_order, "independent_site_order": independent_site_order, "positive_count": positive_count, "null_count": null_count, "negative_count": negative_count, "inconclusive_count": inconclusive_count, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "semantic_loss": semantic_loss, "reasons": reasons, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("replication-assurance:{}", protocol.claim_id),
        "application/vnd.aurora.federated-replication-assurance+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| ReplicationAssuranceError::Contract(error.to_string()))?;
    let receipt = ReplicationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        claim_id: protocol.claim_id.clone(),
        protocol_digest: protocol.protocol_digest.clone(),
        verdict,
        observation_order,
        independent_site_order,
        positive_count,
        null_count,
        negative_count,
        inconclusive_count,
        omissions,
        uncertainty,
        negative_evidence,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ReplicationAssuranceRequest,
) -> Result<(), ReplicationAssuranceError> {
    let protocol = &request.claim_and_protocol;
    if protocol.claim_id.trim().is_empty()
        || protocol.claim.trim().is_empty()
        || protocol.minimum_independent_sites == 0
        || !protocol.raw_data_local
        || protocol.boundary != PRECLINICAL_BOUNDARY
        || request.observations.is_empty()
    {
        return Err(ReplicationAssuranceError::InvalidRequest("claim, protocol, positive site floor, observations, locality, and boundary are required".into()));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.site_id.trim().is_empty()
            || observation.uncertainty.trim().is_empty()
        {
            return Err(ReplicationAssuranceError::InvalidRequest(
                "observation identity and uncertainty are required".into(),
            ));
        }
        if !ids.insert(observation.observation_id.clone()) {
            return Err(ReplicationAssuranceError::DuplicateObservation(
                observation.observation_id.clone(),
            ));
        }
        if observation.protocol_digest != protocol.protocol_digest {
            return Err(ReplicationAssuranceError::ProtocolMismatch(
                observation.observation_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ReplicationAssuranceRequest {
        let digest = ContentHash::of_bytes(b"protocol");
        ReplicationAssuranceRequest {
            claim_and_protocol: ClaimAndProtocol {
                claim_id: "claim:mechanism".into(),
                claim: "perturbation changes organoid morphology".into(),
                protocol_digest: digest.clone(),
                minimum_independent_sites: 2,
                policy_allow: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            observations: vec![
                ReplicationObservation {
                    observation_id: "obs:b".into(),
                    site_id: "site:b".into(),
                    protocol_digest: digest.clone(),
                    outcome: ReplicationOutcome::Positive,
                    result_digest: Some(ContentHash::of_bytes(b"result-b")),
                    uncertainty: "interval remains wide".into(),
                    negative_evidence: Vec::new(),
                },
                ReplicationObservation {
                    observation_id: "obs:a".into(),
                    site_id: "site:a".into(),
                    protocol_digest: digest,
                    outcome: ReplicationOutcome::Positive,
                    result_digest: Some(ContentHash::of_bytes(b"result-a")),
                    uncertainty: "measurement uncertainty".into(),
                    negative_evidence: vec!["null secondary endpoint".into()],
                },
            ],
            protected_omissions: Vec::new(),
            network_partition: false,
        }
    }
    #[test]
    fn independent_positive_replication_is_deterministic() {
        let mut reversed = request();
        reversed.observations.reverse();
        let first = assure_replication(&request()).unwrap();
        let second = assure_replication(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.verdict, ReplicationVerdict::Replicated);
    }
    #[test]
    fn null_results_are_publishable_not_silently_dropped() {
        let mut request = request();
        request
            .observations
            .iter_mut()
            .for_each(|observation| observation.outcome = ReplicationOutcome::Null);
        let receipt = assure_replication(&request).unwrap();
        assert_eq!(receipt.verdict, ReplicationVerdict::NullResult);
    }
    #[test]
    fn negative_result_contradicts_positive_claim() {
        let mut request = request();
        request.observations[0].outcome = ReplicationOutcome::Negative;
        let receipt = assure_replication(&request).unwrap();
        assert_eq!(receipt.verdict, ReplicationVerdict::Contradicted);
    }
    #[test]
    fn partition_lowers_replication_verdict() {
        let mut request = request();
        request.network_partition = true;
        let receipt = assure_replication(&request).unwrap();
        assert_eq!(receipt.verdict, ReplicationVerdict::PartiallyReplicated);
        assert!(!receipt.omissions.is_empty());
    }
}
