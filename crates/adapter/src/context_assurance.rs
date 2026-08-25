//! Prospective high-throughput decision-context compilation assurance.
//!
//! Atlas feature: `AFA-adapter-P03-F27`.
//!
//! This adapter-side harness certifies the closure of a typed decision query before a
//! high-throughput research workflow can consume it.  It is a verification product, not a
//! compiler: missing facts, absent derivation, policy denial, or incomplete protected closure
//! remain explicit and cannot be promoted by an agent.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P03-F27";
pub const CONTRACT_VERSION: &str = "prospective-context-compilation-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub query_id: String,
    pub requester: String,
    pub intent: String,
    pub required_fact_ids: Vec<String>,
    pub resolved_fact_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationRequest {
    pub request_id: String,
    pub query: DecisionQuery,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub admission_reference: Option<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompilationDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub query_id: String,
    pub resolved_fact_ids: Vec<String>,
    pub disposition: ContextCompilationDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ContextCompilationReceipt {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(ContextCompilationError::InvalidField(
                "context compilation identity, boundary, or checks are incomplete".into(),
            ));
        }
        if self
            .resolved_fact_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != self.resolved_fact_ids.len()
        {
            return Err(ContextCompilationError::InvalidField(
                "resolved fact identities are not unique".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextCompilationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextCompilationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ContextCompilationError {
    #[error("invalid context compilation field: {0}")]
    InvalidField(String),
    #[error("context compilation artifact error: {0}")]
    Artifact(String),
    #[error("context compilation serialization error: {0}")]
    Serialization(String),
}

pub fn assure_context_compilation(
    request: &ContextCompilationRequest,
) -> Result<ContextCompilationReceipt, ContextCompilationError> {
    validate_request(request)?;
    let mut resolved = request.query.resolved_fact_ids.clone();
    resolved.sort();
    resolved.dedup();
    let missing = request
        .query
        .required_fact_ids
        .iter()
        .filter(|fact| !resolved.contains(fact))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = missing
        .iter()
        .map(|fact| format!("required decision fact unavailable: {fact}"))
        .collect::<Vec<_>>();
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || request.admission_reference.is_none();
    if blocked {
        omissions.push(
            "policy, admission, or protected-closure gate blocked context certification".into(),
        );
    }
    if request.query.evidence_receipt_digest.is_none() {
        omissions.push("decision-context derivation receipt is absent".into());
    }
    let disposition = if blocked {
        ContextCompilationDisposition::Blocked
    } else if missing.len() > 0 || request.query.evidence_receipt_digest.is_none() {
        ContextCompilationDisposition::Unknown
    } else {
        ContextCompilationDisposition::Passed
    };
    let mut checks = vec![
        "decision fact identities are canonicalized".into(),
        "protected closure and derivation are checked before admission".into(),
        "local research context remains outside any federation envelope".into(),
    ];
    checks.push(match disposition {
        ContextCompilationDisposition::Passed => {
            "required facts, derivation, and admission reference passed".into()
        }
        ContextCompilationDisposition::Blocked => {
            "policy, admission, or protected closure blocked certification".into()
        }
        ContextCompilationDisposition::Unknown => {
            "incomplete decision context remains unknown rather than certified".into()
        }
    });
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "query_id": request.query.query_id, "resolved_fact_ids": resolved, "disposition": disposition, "evidence_receipt_digest": request.query.evidence_receipt_digest, "checks": checks, "omissions": omissions, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("certified-decision-section:{}", request.query.query_id),
        "application/vnd.aurora.certified-decision-section+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let receipt = ContextCompilationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        query_id: request.query.query_id.clone(),
        resolved_fact_ids: resolved,
        disposition,
        evidence_receipt_digest: request.query.evidence_receipt_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ContextCompilationRequest) -> Result<(), ContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.query.query_id.trim().is_empty()
        || request.query.requester.trim().is_empty()
        || request.query.intent.trim().is_empty()
        || request.query.required_fact_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextCompilationError::InvalidField(
            "decision query identity, required facts, requester, and boundary are required".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for fact in &request.query.required_fact_ids {
        if fact.trim().is_empty() || !seen.insert(fact.clone()) {
            return Err(ContextCompilationError::InvalidField(
                "required fact identities must be non-empty and unique".into(),
            ));
        }
    }
    if request.policy_decision == PolicyDecision::Allow && request.admission_reference.is_none() {
        return Err(ContextCompilationError::InvalidField(
            "prospective context admission requires a reference".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fact_stays_unknown() {
        let receipt = assure_context_compilation(&ContextCompilationRequest {
            request_id: "request:context".into(),
            query: DecisionQuery {
                query_id: "query:mechanism".into(),
                requester: "compiler".into(),
                intent: "compile bounded context".into(),
                required_fact_ids: vec!["fact:a".into(), "fact:b".into()],
                resolved_fact_ids: vec!["fact:a".into()],
                evidence_receipt_digest: None,
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            admission_reference: Some("admission:context".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
