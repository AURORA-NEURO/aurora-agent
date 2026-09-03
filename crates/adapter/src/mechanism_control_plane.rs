//! Prospective high-throughput mechanism-exploration federation control plane.
//!
//! Atlas feature: `AFA-adapter-P08-F31`.

use bioprism_foundation::{
    PolicyDecision, ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P08-F31";
pub const CONTRACT_VERSION: &str = "federated-mechanism-control-plane/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismControlPlaneRequest {
    pub request_id: String,
    pub federation_id: String,
    pub question_id: String,
    pub required_candidate_ids: Vec<String>,
    pub admitted_candidate_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub approval_reference: Option<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismControlDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismControlPlaneReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub input: MechanismControlPlaneRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub federation_id: String,
    pub question_id: String,
    pub required_candidate_ids: Vec<String>,
    pub admitted_candidate_ids: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub approval_reference: Option<String>,
    pub disposition: MechanismControlDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl MechanismControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), MechanismControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.question_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(MechanismControlError::InvalidField(
                "mechanism control identity, boundary, or checks are incomplete".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("question_id", &self.question_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("admitted_candidate_ids", &self.admitted_candidate_ids)?;
        validate_sorted_strings("checks", &self.checks)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        if self.evidence_receipt_digest == Some(ContentHash::of_bytes(b"")) {
            return Err(MechanismControlError::InvalidField(
                "evidence receipt digest must be non-empty".into(),
            ));
        }
        if self.disposition == MechanismControlDisposition::Passed
            && (!self.omissions.is_empty() || self.evidence_receipt_digest.is_none())
        {
            return Err(MechanismControlError::InvalidField(
                "passed mechanism control cannot omit evidence or candidate closure".into(),
            ));
        }
        if self.disposition != MechanismControlDisposition::Passed && self.omissions.is_empty() {
            return Err(MechanismControlError::InvalidField(
                "non-passed mechanism control must explain its omissions".into(),
            ));
        }
        let expected_provenance = mechanism_provenance(&self.evidence_receipt_digest);
        if self.artifact.artifact_id != format!("mechanism-control-plane:{}", self.question_id)
            || self.artifact.content_type != "application/vnd.aurora.mechanism-control-plane+json"
            || !self.artifact.semantic_loss.is_empty()
            || self.artifact.provenance != expected_provenance
        {
            return Err(MechanismControlError::Artifact(
                "mechanism artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismControlError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&mechanism_payload(self))
            .map_err(|error| MechanismControlError::Artifact(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != mechanism_input_digest(&self.input)? {
            return Err(MechanismControlError::Artifact(
                "mechanism control retained input digest does not match the request".into(),
            ));
        }
        let expected = operate_mechanism_control_plane_internal(&self.input, false)?;
        if self != &expected {
            return Err(MechanismControlError::Artifact(
                "mechanism receipt is not derived from its retained candidate and policy inputs"
                    .into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, MechanismControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MechanismControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MechanismControlError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), MechanismControlError> {
    if value.is_empty() || value.trim() != value {
        return Err(MechanismControlError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(MechanismControlError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn mechanism_input_digest(
    request: &MechanismControlPlaneRequest,
) -> Result<ContentHash, MechanismControlError> {
    let canonical = canonical_mechanism_control_plane_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| MechanismControlError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| MechanismControlError::Serialization(error.to_string()))
}

fn canonical_mechanism_control_plane_request(
    request: &MechanismControlPlaneRequest,
) -> MechanismControlPlaneRequest {
    let mut canonical = request.clone();
    canonical.required_candidate_ids.sort();
    canonical.admitted_candidate_ids.sort();
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), MechanismControlError> {
    if values.len() > MAX_ITEMS {
        return Err(MechanismControlError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = std::collections::BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(MechanismControlError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), MechanismControlError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MechanismControlError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn mechanism_provenance(evidence_receipt_digest: &Option<ContentHash>) -> Vec<ProvenanceLink> {
    evidence_receipt_digest
        .as_ref()
        .map(|digest| {
            vec![ProvenanceLink {
                source_id: "evidence-receipt".into(),
                relation: "mechanism-control-evidence-receipt".into(),
                digest: digest.clone(),
            }]
        })
        .unwrap_or_default()
}

fn mechanism_payload(receipt: &MechanismControlPlaneReceipt) -> serde_json::Value {
    mechanism_payload_from_parts(
        &receipt.schema_version,
        &receipt.feature_id,
        &receipt.contract_version,
        &receipt.request_id,
        &receipt.federation_id,
        &receipt.question_id,
        &receipt.required_candidate_ids,
        &receipt.admitted_candidate_ids,
        &receipt.evidence_receipt_digest,
        receipt.policy_decision,
        receipt.protected_closure_satisfied,
        &receipt.approval_reference,
        receipt.disposition,
        &receipt.checks,
        &receipt.omissions,
        &receipt.artifact.provenance,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn mechanism_payload_from_parts(
    schema_version: &str,
    feature_id: &str,
    contract_version: &str,
    request_id: &str,
    federation_id: &str,
    question_id: &str,
    required_candidate_ids: &[String],
    admitted_candidate_ids: &[String],
    evidence_receipt_digest: &Option<ContentHash>,
    policy_decision: PolicyDecision,
    protected_closure_satisfied: bool,
    approval_reference: &Option<String>,
    disposition: MechanismControlDisposition,
    checks: &[String],
    omissions: &[String],
    provenance: &[ProvenanceLink],
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "feature_id": feature_id,
        "contract_version": contract_version,
        "request_id": request_id,
        "federation_id": federation_id,
        "question_id": question_id,
        "required_candidate_ids": required_candidate_ids,
        "admitted_candidate_ids": admitted_candidate_ids,
        "evidence_receipt_digest": evidence_receipt_digest,
        "policy_decision": policy_decision,
        "protected_closure_satisfied": protected_closure_satisfied,
        "approval_reference": approval_reference,
        "disposition": disposition,
        "checks": checks,
        "omissions": omissions,
        "provenance": provenance,
        "boundary": boundary,
    })
}

#[derive(Debug, Error)]
pub enum MechanismControlError {
    #[error("invalid mechanism control field: {0}")]
    InvalidField(String),
    #[error("mechanism control artifact error: {0}")]
    Artifact(String),
    #[error("mechanism control serialization error: {0}")]
    Serialization(String),
}

pub fn operate_mechanism_control_plane(
    request: &MechanismControlPlaneRequest,
) -> Result<MechanismControlPlaneReceipt, MechanismControlError> {
    operate_mechanism_control_plane_internal(request, true)
}

fn operate_mechanism_control_plane_internal(
    request: &MechanismControlPlaneRequest,
    validate_output: bool,
) -> Result<MechanismControlPlaneReceipt, MechanismControlError> {
    let input = canonical_mechanism_control_plane_request(request);
    let request = &input;
    validate_request(request)?;
    let request = request.clone();
    let mut admitted = request.admitted_candidate_ids.clone();
    admitted.sort();
    let mut required = request.required_candidate_ids.clone();
    required.sort();
    let missing = required
        .iter()
        .filter(|candidate| !admitted.contains(candidate))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "mechanism candidate identities are canonicalized".to_string(),
        "raw institution-local evidence remains outside the federation envelope".to_string(),
    ];
    let mut omissions = Vec::new();
    let admission_ready = request
        .approval_reference
        .as_ref()
        .is_some_and(|reference| !reference.trim().is_empty());
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !admission_ready
    {
        omissions.push("policy, approval, or protected-closure gate blocked admission".into());
        checks.push("policy, approval, or protected closure blocked mechanism admission".into());
        MechanismControlDisposition::Blocked
    } else if request.evidence_receipt_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|candidate| format!("required mechanism candidate unavailable: {candidate}")),
        );
        if request.evidence_receipt_digest.is_none() {
            omissions.push("mechanism evidence receipt is absent".into());
        }
        checks.push("incomplete mechanism evidence remains unknown rather than admitted".into());
        MechanismControlDisposition::Unknown
    } else {
        checks.push("candidate set, evidence receipt, and approval passed".into());
        MechanismControlDisposition::Passed
    };
    checks.sort();
    checks.dedup();
    omissions.sort();
    omissions.dedup();
    let provenance = mechanism_provenance(&request.evidence_receipt_digest);
    let payload = mechanism_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        FEATURE_ID,
        CONTRACT_VERSION,
        &request.request_id,
        &request.federation_id,
        &request.question_id,
        &required,
        &admitted,
        &request.evidence_receipt_digest,
        request.policy_decision,
        request.protected_closure_satisfied,
        &request.approval_reference,
        disposition,
        &checks,
        &omissions,
        &provenance,
        &request.boundary,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-control-plane:{}", request.question_id),
        "application/vnd.aurora.mechanism-control-plane+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| MechanismControlError::Artifact(error.to_string()))?;
    let receipt = MechanismControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        input_digest: mechanism_input_digest(&input)?,
        input,
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        question_id: request.question_id.clone(),
        required_candidate_ids: required,
        admitted_candidate_ids: admitted,
        policy_decision: request.policy_decision,
        protected_closure_satisfied: request.protected_closure_satisfied,
        approval_reference: request.approval_reference.clone(),
        disposition,
        evidence_receipt_digest: request.evidence_receipt_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        receipt.validate()?;
    }
    Ok(receipt)
}

fn validate_request(request: &MechanismControlPlaneRequest) -> Result<(), MechanismControlError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.question_id.trim().is_empty()
        || request.required_candidate_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MechanismControlError::InvalidField(
            "mechanism identity, candidates, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("federation_id", &request.federation_id)?;
    validate_text("question_id", &request.question_id)?;
    validate_text("boundary", &request.boundary)?;
    if request.required_candidate_ids.len() > MAX_ITEMS
        || request.admitted_candidate_ids.len() > MAX_ITEMS
    {
        return Err(MechanismControlError::InvalidField(
            "mechanism candidate count exceeds its bound".into(),
        ));
    }
    validate_unique_strings("required_candidate_ids", &request.required_candidate_ids)?;
    validate_unique_strings("admitted_candidate_ids", &request.admitted_candidate_ids)?;
    let required = request
        .required_candidate_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    if request
        .admitted_candidate_ids
        .iter()
        .any(|candidate| !required.contains(candidate))
    {
        return Err(MechanismControlError::InvalidField(
            "admitted candidates must be required candidates".into(),
        ));
    }
    if let Some(reference) = &request.approval_reference {
        validate_text("approval_reference", reference)?;
    }
    if request
        .evidence_receipt_digest
        .as_ref()
        .is_some_and(|digest| *digest == ContentHash::of_bytes(b""))
    {
        return Err(MechanismControlError::InvalidField(
            "evidence receipt digest must be non-empty".into(),
        ));
    }
    if request.policy_decision == PolicyDecision::Allow
        && !request
            .approval_reference
            .as_ref()
            .is_some_and(|reference| !reference.trim().is_empty())
    {
        return Err(MechanismControlError::InvalidField(
            "A2 mechanism control requires an approval reference".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_candidate_is_unknown() {
        let receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, MechanismControlDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn complete_mechanism_admission_passes() {
        let request = MechanismControlPlaneRequest {
            request_id: "request:mechanism-pass".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:b".into(), "candidate:a".into()],
            admitted_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"mechanism-evidence")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let receipt = operate_mechanism_control_plane(&request).unwrap();
        let mut reordered = request;
        reordered.required_candidate_ids.reverse();
        reordered.admitted_candidate_ids.reverse();
        let reordered_receipt = operate_mechanism_control_plane(&reordered).unwrap();

        assert_eq!(receipt.disposition, MechanismControlDisposition::Passed);
        assert!(receipt.omissions.is_empty());
        assert_eq!(receipt, reordered_receipt);
        assert_eq!(receipt.input_digest, reordered_receipt.input_digest);
    }

    #[test]
    fn whitespace_approval_reference_is_rejected() {
        let request = MechanismControlPlaneRequest {
            request_id: "request:mechanism".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"mechanism-evidence")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("   ".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(operate_mechanism_control_plane(&request).is_err());
    }

    #[test]
    fn out_of_scope_admitted_candidate_is_rejected() {
        let request = MechanismControlPlaneRequest {
            request_id: "request:mechanism".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into()],
            admitted_candidate_ids: vec!["candidate:z".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Deny,
            protected_closure_satisfied: false,
            approval_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(operate_mechanism_control_plane(&request).is_err());
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.admitted_candidate_ids[0] = "candidate:z".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn retained_candidate_gate_tampering_is_rejected() {
        let mut receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism-pass".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            admitted_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"mechanism-evidence")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.required_candidate_ids.pop();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_policy_gate_tampering_is_rejected() {
        let mut receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism-pass".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"mechanism-evidence")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.policy_decision = PolicyDecision::Deny;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn mechanism_evidence_provenance_tampering_is_rejected() {
        let mut receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism-pass".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"mechanism-evidence")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = operate_mechanism_control_plane(&MechanismControlPlaneRequest {
            request_id: "request:mechanism-pass".into(),
            federation_id: "federation:mechanism".into(),
            question_id: "question:organoid".into(),
            required_candidate_ids: vec!["candidate:a".into()],
            admitted_candidate_ids: vec!["candidate:a".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"mechanism-evidence")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            approval_reference: Some("approval:mechanism".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        receipt.input.question_id = "question:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
