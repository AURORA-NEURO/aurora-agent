//! Federated continual knowledge-representation assurance.
//!
//! Atlas feature: `AFA-ops-P04-F28`.
//!
//! This product gate verifies that a continual knowledge projection is complete enough to expose
//! to a researcher. It exchanges typed fact identities and evidence digests only; absent facts,
//! missing derivation, policy denial, and incomplete protected closure remain explicit outcomes.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ops-P04-F28";
pub const CONTRACT_VERSION: &str = "federated-knowledge-representation-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRepresentationAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub required_fact_ids: Vec<String>,
    pub resolved_fact_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRepresentationAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub resolved_fact_ids: Vec<String>,
    pub disposition: KnowledgeAssuranceDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl KnowledgeRepresentationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(KnowledgeAssuranceError::InvalidField(
                "knowledge assurance identity, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, KnowledgeAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum KnowledgeAssuranceError {
    #[error("invalid knowledge assurance field: {0}")]
    InvalidField(String),
    #[error("knowledge assurance artifact error: {0}")]
    Artifact(String),
    #[error("knowledge assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_knowledge_representation(
    request: &KnowledgeRepresentationAssuranceRequest,
) -> Result<KnowledgeRepresentationAssuranceReceipt, KnowledgeAssuranceError> {
    validate_request(request)?;
    let mut resolved_fact_ids = request.resolved_fact_ids.clone();
    resolved_fact_ids.sort();
    resolved_fact_ids.dedup();
    let missing = request
        .required_fact_ids
        .iter()
        .filter(|fact| !resolved_fact_ids.contains(fact))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "fact identities are canonicalized before projection admission".to_string(),
        "raw institution-local knowledge remains outside the federation envelope".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented knowledge projection".into());
        KnowledgeAssuranceDisposition::Blocked
    } else if request.evidence_receipt_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|fact| format!("required fact unavailable: {fact}")),
        );
        if request.evidence_receipt_digest.is_none() {
            omissions.push("knowledge derivation receipt is absent".into());
        }
        checks.push("incomplete representation remains unknown rather than asserted".into());
        KnowledgeAssuranceDisposition::Unknown
    } else {
        checks.push("required facts and derivation receipt passed".into());
        KnowledgeAssuranceDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "query_id": request.query_id,
        "resolved_fact_ids": resolved_fact_ids,
        "disposition": disposition,
        "evidence_receipt_digest": request.evidence_receipt_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("knowledge-representation-assurance:{}", request.request_id),
        "application/vnd.aurora.knowledge-representation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| KnowledgeAssuranceError::Artifact(error.to_string()))?;
    let receipt = KnowledgeRepresentationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        resolved_fact_ids,
        disposition,
        evidence_receipt_digest: request.evidence_receipt_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &KnowledgeRepresentationAssuranceRequest,
) -> Result<(), KnowledgeAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.required_fact_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(KnowledgeAssuranceError::InvalidField(
            "knowledge assurance identity, required facts, and boundary are required".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for fact in &request.required_fact_ids {
        if fact.trim().is_empty() || !seen.insert(fact.clone()) {
            return Err(KnowledgeAssuranceError::InvalidField(
                "required fact identities must be non-empty and unique".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fact_is_unknown_not_asserted() {
        let receipt = assure_knowledge_representation(&KnowledgeRepresentationAssuranceRequest {
            request_id: "request:knowledge".into(),
            federation_id: "federation:knowledge".into(),
            query_id: "query:mechanism".into(),
            required_fact_ids: vec!["fact:a".into(), "fact:b".into()],
            resolved_fact_ids: vec!["fact:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, KnowledgeAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
