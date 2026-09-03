//! Federated continual decision-context compilation assurance.
//!
//! Atlas feature: `AFA-devplat-P03-F28`.
//!
//! The harness verifies a caller-supplied context compilation receipt without executing a
//! compiler or moving institution-local records. Missing context, missing derivation, policy
//! denial, and incomplete protected closure are typed outcomes; none can be upgraded to a
//! certified decision section by an agent or connector.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-devplat-P03-F28";
pub const CONTRACT_VERSION: &str = "federated-context-compilation-assurance/1.0";
const MAX_CONTEXT_IDS: usize = 512;
const MAX_TEXT_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub required_context_ids: Vec<String>,
    pub resolved_context_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub required_context_ids: Vec<String>,
    pub resolved_context_ids: Vec<String>,
    pub disposition: ContextAssuranceDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ContextCompilationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ContextAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || !valid_text(&self.request_id)
            || !valid_text(&self.federation_id)
            || !valid_text(&self.query_id)
            || self.required_context_ids.is_empty()
            || self.required_context_ids.len() > MAX_CONTEXT_IDS
            || self.resolved_context_ids.len() > MAX_CONTEXT_IDS
            || self.checks.is_empty()
            || self.checks.len() > MAX_CONTEXT_IDS
            || self.omissions.len() > MAX_CONTEXT_IDS
        {
            return Err(ContextAssuranceError::InvalidField(
                "context assurance identity, boundary, or checks are incomplete".into(),
            ));
        }
        validate_context_ids(&self.required_context_ids, "required_context_ids")?;
        let mut canonical_required_context_ids = self.required_context_ids.clone();
        canonical_required_context_ids.sort();
        if canonical_required_context_ids != self.required_context_ids {
            return Err(ContextAssuranceError::InvalidField(
                "required_context_ids must be in canonical order".into(),
            ));
        }
        validate_context_ids(&self.resolved_context_ids, "resolved_context_ids")?;
        let mut canonical_context_ids = self.resolved_context_ids.clone();
        canonical_context_ids.sort();
        if canonical_context_ids != self.resolved_context_ids {
            return Err(ContextAssuranceError::InvalidField(
                "resolved_context_ids must be in canonical order".into(),
            ));
        }
        validate_text_list(&self.checks, "checks")?;
        validate_text_list(&self.omissions, "omissions")?;
        match self.disposition {
            ContextAssuranceDisposition::Passed
                if self.evidence_receipt_digest.is_none() || !self.omissions.is_empty() =>
            {
                return Err(ContextAssuranceError::InvalidField(
                    "passed context assurance requires evidence and no omissions".into(),
                ));
            }
            ContextAssuranceDisposition::Unknown
                if self.evidence_receipt_digest.is_some() && self.omissions.is_empty() =>
            {
                return Err(ContextAssuranceError::InvalidField(
                    "unknown context assurance requires an omission or missing evidence".into(),
                ));
            }
            _ => {}
        }
        let expected_artifact_id = format!("context-compilation-assurance:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type
                != "application/vnd.aurora.context-compilation-assurance+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextAssuranceError::InvalidField(
                "context assurance artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ContextAssuranceError {
    #[error("invalid context assurance field: {0}")]
    InvalidField(String),
    #[error("context assurance artifact error: {0}")]
    Artifact(String),
    #[error("context assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_context_compilation(
    request: &ContextCompilationAssuranceRequest,
) -> Result<ContextCompilationAssuranceReceipt, ContextAssuranceError> {
    validate_request(request)?;
    let mut required_context_ids = request.required_context_ids.clone();
    required_context_ids.sort();
    let mut resolved_context_ids = request.resolved_context_ids.clone();
    resolved_context_ids.sort();
    resolved_context_ids.dedup();
    let missing = required_context_ids
        .iter()
        .filter(|context| !resolved_context_ids.contains(context))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "context identities are canonicalized before certification".to_string(),
        "raw institution-local records remain outside the federation envelope".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented context certification".into());
        ContextAssuranceDisposition::Blocked
    } else if request.evidence_receipt_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|context| format!("required context unavailable: {context}")),
        );
        if request.evidence_receipt_digest.is_none() {
            omissions.push("context derivation receipt is absent".into());
        }
        checks.push("incomplete context remains unknown rather than certified".into());
        ContextAssuranceDisposition::Unknown
    } else {
        checks.push("required context and derivation receipt passed".into());
        ContextAssuranceDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "query_id": request.query_id,
        "required_context_ids": required_context_ids,
        "resolved_context_ids": resolved_context_ids,
        "disposition": disposition,
        "evidence_receipt_digest": request.evidence_receipt_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("context-compilation-assurance:{}", request.request_id),
        "application/vnd.aurora.context-compilation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
    let receipt = ContextCompilationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        required_context_ids,
        resolved_context_ids,
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

fn receipt_payload(receipt: &ContextCompilationAssuranceReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "feature_id": receipt.feature_id,
        "contract_version": receipt.contract_version,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "query_id": receipt.query_id,
        "required_context_ids": receipt.required_context_ids,
        "resolved_context_ids": receipt.resolved_context_ids,
        "disposition": receipt.disposition,
        "evidence_receipt_digest": receipt.evidence_receipt_digest,
        "checks": receipt.checks,
        "omissions": receipt.omissions,
        "boundary": receipt.boundary,
    })
}

fn validate_request(
    request: &ContextCompilationAssuranceRequest,
) -> Result<(), ContextAssuranceError> {
    if !valid_text(&request.request_id)
        || !valid_text(&request.federation_id)
        || !valid_text(&request.query_id)
        || request.required_context_ids.is_empty()
        || request.required_context_ids.len() > MAX_CONTEXT_IDS
        || request.resolved_context_ids.len() > MAX_CONTEXT_IDS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextAssuranceError::InvalidField(
            "context assurance identity, required context, and boundary are required".into(),
        ));
    }
    validate_context_ids(&request.required_context_ids, "required_context_ids")?;
    validate_context_ids(&request.resolved_context_ids, "resolved_context_ids")?;
    Ok(())
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn validate_context_ids(values: &[String], field: &str) -> Result<(), ContextAssuranceError> {
    if values.len() > MAX_CONTEXT_IDS {
        return Err(ContextAssuranceError::InvalidField(format!(
            "{field} exceeds the {MAX_CONTEXT_IDS}-item bound"
        )));
    }
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if !valid_text(value) || !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextAssuranceError::InvalidField(format!(
                "{field} identities must be bounded, visible, and unique without case collisions"
            )));
        }
    }
    Ok(())
}

fn validate_text_list(values: &[String], field: &str) -> Result<(), ContextAssuranceError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if !valid_text(value) || !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextAssuranceError::InvalidField(format!(
                "{field} entries must be bounded visible text without case collisions"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_context_is_unknown_not_certified() {
        let receipt = assure_context_compilation(&ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:a".into(), "context:b".into()],
            resolved_context_ids: vec!["context:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ContextAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn required_context_set_is_canonical_and_digest_bound() {
        let mut reordered = ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:b".into(), "context:a".into()],
            resolved_context_ids: vec!["context:a".into(), "context:b".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"receipt")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let first = assure_context_compilation(&reordered).unwrap();
        assert_eq!(first.required_context_ids, vec!["context:a", "context:b"]);

        reordered.required_context_ids.reverse();
        let second = assure_context_compilation(&reordered).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());

        reordered.required_context_ids = vec!["context:a".into()];
        let different_requirement = assure_context_compilation(&reordered).unwrap();
        assert_ne!(
            first.digest().unwrap(),
            different_requirement.digest().unwrap()
        );
    }

    #[test]
    fn context_assurance_rejects_control_and_case_colliding_context_ids() {
        let mut request = ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:a".into(), "context:A".into()],
            resolved_context_ids: vec!["context:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(assure_context_compilation(&request).is_err());

        request.required_context_ids = vec!["context:a".into()];
        request.resolved_context_ids = vec!["context:\u{0000}a".into()];
        assert!(assure_context_compilation(&request).is_err());
    }

    #[test]
    fn receipt_rejects_padded_identity_noncanonical_order_and_artifact_drift() {
        let mut receipt = assure_context_compilation(&ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:a".into(), "context:b".into()],
            resolved_context_ids: vec!["context:b".into(), "context:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"receipt")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();

        receipt.resolved_context_ids.reverse();
        assert!(receipt.validate().is_err());

        let mut receipt = assure_context_compilation(&ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:a".into()],
            resolved_context_ids: vec!["context:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"receipt")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.request_id = " request:context".into();
        assert!(receipt.validate().is_err());

        let mut receipt = assure_context_compilation(&ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:a".into()],
            resolved_context_ids: vec!["context:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"receipt")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
