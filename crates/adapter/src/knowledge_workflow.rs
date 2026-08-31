//! Multimodal knowledge-representation workflow fabric.
//!
//! Atlas feature: `AFA-adapter-P04-F14`.
//!
//! This fabric orchestrates a bounded claim-to-knowledge workflow across imaging and omics
//! studies.  It only admits typed, scoped claims with an evidence derivation receipt; missing,
//! contradictory, or out-of-scope claims remain explicit omissions and cannot be asserted by a
//! downstream agent.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P04-F14";
pub const CONTRACT_VERSION: &str = "multimodal-knowledge-workflow-fabric/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimsWorkflowRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub study_ids: Vec<String>,
    pub required_claim_ids: Vec<String>,
    pub resolved_claim_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeWorkflowDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorld {
    pub schema_version: String,
    pub world_id: String,
    pub workflow_id: String,
    pub study_ids: Vec<String>,
    pub resolved_claim_ids: Vec<String>,
    pub disposition: KnowledgeWorkflowDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub stages: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeWorkflowReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub input: ClaimsWorkflowRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub required_claim_ids: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub disposition: KnowledgeWorkflowDisposition,
    pub world: TypedKnowledgeWorld,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

fn validate_text(field: &str, value: &str) -> Result<(), KnowledgeWorkflowError> {
    if value.is_empty() || value.trim() != value {
        return Err(KnowledgeWorkflowError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(KnowledgeWorkflowError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn knowledge_workflow_input_digest(
    request: &ClaimsWorkflowRequest,
) -> Result<ContentHash, KnowledgeWorkflowError> {
    let value = serde_json::to_value(&canonical_knowledge_workflow_request(request))
        .map_err(|error| KnowledgeWorkflowError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| KnowledgeWorkflowError::Serialization(error.to_string()))
}

fn canonical_knowledge_workflow_request(request: &ClaimsWorkflowRequest) -> ClaimsWorkflowRequest {
    let mut canonical = request.clone();
    canonical.study_ids.sort();
    canonical.required_claim_ids.sort();
    canonical.resolved_claim_ids.sort();
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), KnowledgeWorkflowError> {
    if values.len() > MAX_ITEMS {
        return Err(KnowledgeWorkflowError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(KnowledgeWorkflowError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), KnowledgeWorkflowError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(KnowledgeWorkflowError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &ContentHash) -> Result<(), KnowledgeWorkflowError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(KnowledgeWorkflowError::InvalidField(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

impl KnowledgeWorkflowReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.workflow_id != self.world.workflow_id
            || self.checks.is_empty()
            || self.omissions != self.world.omissions
            || self.uncertainty != self.world.uncertainty
            || self.effect_receipts.is_empty()
        {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow identity, linkage, checks, or boundary is incomplete".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("requester", &self.requester)?;
        validate_text("boundary", &self.boundary)?;
        validate_unique_strings("required_claim_ids", &self.required_claim_ids)?;
        validate_sorted_strings("required_claim_ids", &self.required_claim_ids)?;
        validate_sorted_strings("world.study_ids", &self.world.study_ids)?;
        validate_sorted_strings("world.resolved_claim_ids", &self.world.resolved_claim_ids)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_unique_strings("stages", &self.world.stages)?;
        validate_unique_strings("checks", &self.checks)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        if let Some(digest) = &self.world.evidence_receipt_digest {
            validate_digest("evidence_receipt_digest", digest)?;
        }
        if self.world.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.world.world_id != format!("typed-knowledge-world:{}", self.workflow_id)
            || self.world.boundary != PRECLINICAL_BOUNDARY
            || self.world.study_ids.len() < 2
            || !self
                .world
                .study_ids
                .iter()
                .all(|study| study.starts_with("study:"))
        {
            return Err(KnowledgeWorkflowError::InvalidField(
                "typed knowledge world is incomplete".into(),
            ));
        }
        let required = self
            .required_claim_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let resolved = self
            .world
            .resolved_claim_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !resolved.is_subset(&required) {
            return Err(KnowledgeWorkflowError::InvalidField(
                "resolved claims must remain within the requested claim closure".into(),
            ));
        }
        let blocked = self.policy_decision != PolicyDecision::Allow
            || !self.protected_closure_satisfied
            || !self.raw_data_local;
        if blocked && !resolved.is_empty() {
            return Err(KnowledgeWorkflowError::InvalidField(
                "blocked knowledge workflow cannot retain resolved claims".into(),
            ));
        }
        let missing = required
            .difference(&resolved)
            .map(|claim| format!("required claim unavailable: {claim}"))
            .collect::<Vec<_>>();
        let mut expected_omissions = missing;
        if blocked {
            expected_omissions.push(
                "policy, protected-closure, or raw-data-locality gate blocked knowledge workflow"
                    .into(),
            );
        }
        expected_omissions.sort();
        if self.omissions != expected_omissions {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow omissions do not match claim and policy closure".into(),
            ));
        }
        let mut expected_uncertainty: Vec<String> = Vec::new();
        if self.world.evidence_receipt_digest.is_none() {
            expected_uncertainty.push("claim derivation receipt is absent".into());
        }
        if self.uncertainty != expected_uncertainty {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow uncertainty does not match derivation evidence".into(),
            ));
        }
        let expected_disposition = if blocked {
            KnowledgeWorkflowDisposition::Blocked
        } else if !expected_omissions.is_empty() || !self.uncertainty.is_empty() {
            KnowledgeWorkflowDisposition::Unknown
        } else {
            KnowledgeWorkflowDisposition::Passed
        };
        if self.disposition != expected_disposition || self.world.disposition != self.disposition {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow disposition is inconsistent with closure".into(),
            ));
        }
        let expected_stages = vec![
            "scope_studies".to_string(),
            "resolve_claim_identities".to_string(),
            "attach_evidence_derivation".to_string(),
            "emit_typed_knowledge_world".to_string(),
        ];
        if self.world.stages != expected_stages {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow stages are not canonical".into(),
            ));
        }
        let expected_checks = vec![
            "study scope and claim identities are canonicalized".to_string(),
            "knowledge stages are deterministic and replayable".to_string(),
            "raw multimodal records remain institution-local".to_string(),
            match self.disposition {
                KnowledgeWorkflowDisposition::Passed => {
                    "claims, derivation, and policy gates passed".to_string()
                }
                KnowledgeWorkflowDisposition::Blocked => {
                    "policy, protected closure, or locality blocked workflow".to_string()
                }
                KnowledgeWorkflowDisposition::Unknown => {
                    "incomplete claim closure remains unknown rather than asserted".to_string()
                }
            },
        ];
        if self.checks != expected_checks {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow checks are not bound to its disposition".into(),
            ));
        }
        let expected_effect = if blocked {
            vec!["block:knowledge-workflow".to_string()]
        } else {
            vec![format!("read:local-knowledge-workflow:{}", self.request_id)]
        };
        if self.effect_receipts != expected_effect {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow effect does not match its release state".into(),
            ));
        }
        let expected_world = TypedKnowledgeWorld {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            world_id: format!("typed-knowledge-world:{}", self.workflow_id),
            workflow_id: self.workflow_id.clone(),
            study_ids: self.world.study_ids.clone(),
            resolved_claim_ids: self.world.resolved_claim_ids.clone(),
            disposition: self.disposition,
            evidence_receipt_digest: self.world.evidence_receipt_digest.clone(),
            omissions: self.omissions.clone(),
            uncertainty: self.uncertainty.clone(),
            stages: self.world.stages.clone(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        if self.world != expected_world {
            return Err(KnowledgeWorkflowError::InvalidField(
                "typed knowledge world is not bound to the receipt".into(),
            ));
        }
        let payload = serde_json::to_value(&self.world)
            .map_err(|error| KnowledgeWorkflowError::Serialization(error.to_string()))?;
        if self.artifact.artifact_id != self.world.world_id
            || self.artifact.content_type != "application/vnd.aurora.typed-knowledge-world+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(KnowledgeWorkflowError::Artifact(
                "knowledge world artifact is not bound to the typed world".into(),
            ));
        }
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| KnowledgeWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeWorkflowError::Artifact(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != knowledge_workflow_input_digest(&self.input)? {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow retained input digest does not match the request".into(),
            ));
        }
        let expected = build_knowledge_workflow(&self.input)?;
        if self != &expected {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, KnowledgeWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeWorkflowError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeWorkflowError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum KnowledgeWorkflowError {
    #[error("invalid knowledge workflow field: {0}")]
    InvalidField(String),
    #[error("knowledge workflow artifact error: {0}")]
    Artifact(String),
    #[error("knowledge workflow serialization error: {0}")]
    Serialization(String),
}

pub fn run_knowledge_workflow(
    request: &ClaimsWorkflowRequest,
) -> Result<KnowledgeWorkflowReceipt, KnowledgeWorkflowError> {
    let receipt = build_knowledge_workflow(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_knowledge_workflow(
    request: &ClaimsWorkflowRequest,
) -> Result<KnowledgeWorkflowReceipt, KnowledgeWorkflowError> {
    validate_request(request)?;
    let mut studies = request.study_ids.clone();
    studies.sort();
    let mut required_claims = request.required_claim_ids.clone();
    required_claims.sort();
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
    let mut claims = if blocked {
        Vec::new()
    } else {
        request.resolved_claim_ids.clone()
    };
    claims.sort();
    let missing = required_claims
        .iter()
        .filter(|claim| !claims.contains(claim))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = missing
        .iter()
        .map(|claim| format!("required claim unavailable: {claim}"))
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    if request.evidence_receipt_digest.is_none() {
        uncertainty.push("claim derivation receipt is absent".into());
    }
    if blocked {
        omissions.push(
            "policy, protected-closure, or raw-data-locality gate blocked knowledge workflow"
                .into(),
        );
    }
    omissions.sort();
    let disposition = if blocked {
        KnowledgeWorkflowDisposition::Blocked
    } else if !missing.is_empty() || request.evidence_receipt_digest.is_none() {
        KnowledgeWorkflowDisposition::Unknown
    } else {
        KnowledgeWorkflowDisposition::Passed
    };
    let stages = vec![
        "scope_studies".into(),
        "resolve_claim_identities".into(),
        "attach_evidence_derivation".into(),
        "emit_typed_knowledge_world".into(),
    ];
    let mut checks = vec![
        "study scope and claim identities are canonicalized".into(),
        "knowledge stages are deterministic and replayable".into(),
        "raw multimodal records remain institution-local".into(),
    ];
    checks.push(match disposition {
        KnowledgeWorkflowDisposition::Passed => {
            "claims, derivation, and policy gates passed".into()
        }
        KnowledgeWorkflowDisposition::Blocked => {
            "policy, protected closure, or locality blocked workflow".into()
        }
        KnowledgeWorkflowDisposition::Unknown => {
            "incomplete claim closure remains unknown rather than asserted".into()
        }
    });
    let world = TypedKnowledgeWorld {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        world_id: format!("typed-knowledge-world:{}", request.workflow_id),
        workflow_id: request.workflow_id.clone(),
        study_ids: studies,
        resolved_claim_ids: claims,
        disposition,
        evidence_receipt_digest: request.evidence_receipt_digest.clone(),
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        stages,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&world)
        .map_err(|error| KnowledgeWorkflowError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        world.world_id.clone(),
        "application/vnd.aurora.typed-knowledge-world+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| KnowledgeWorkflowError::Artifact(error.to_string()))?;
    let receipt = KnowledgeWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        input: canonical_knowledge_workflow_request(request),
        input_digest: knowledge_workflow_input_digest(request)?,
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        requester: request.requester.clone(),
        required_claim_ids: required_claims,
        policy_decision: request.policy_decision,
        protected_closure_satisfied: request.protected_closure_satisfied,
        raw_data_local: request.raw_data_local,
        disposition,
        world,
        checks,
        omissions,
        uncertainty,
        effect_receipts: if blocked {
            vec!["block:knowledge-workflow".into()]
        } else {
            vec![format!(
                "read:local-knowledge-workflow:{}",
                request.request_id
            )]
        },
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &ClaimsWorkflowRequest) -> Result<(), KnowledgeWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_claim_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(KnowledgeWorkflowError::InvalidField(
            "knowledge workflow identity, studies, claims, requester, and boundary are required"
                .into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("requester", &request.requester)?;
    validate_text("boundary", &request.boundary)?;
    validate_unique_strings("study_ids", &request.study_ids)?;
    validate_unique_strings("required_claim_ids", &request.required_claim_ids)?;
    if !request
        .study_ids
        .iter()
        .all(|study_id| study_id.starts_with("study:"))
    {
        return Err(KnowledgeWorkflowError::InvalidField(
            "study identities must use the study namespace".into(),
        ));
    }
    validate_unique_strings("resolved_claim_ids", &request.resolved_claim_ids)?;
    let required = request
        .required_claim_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !request
        .resolved_claim_ids
        .iter()
        .all(|claim| required.contains(claim))
    {
        return Err(KnowledgeWorkflowError::InvalidField(
            "resolved claims must be requested claims".into(),
        ));
    }
    if let Some(digest) = &request.evidence_receipt_digest {
        validate_digest("evidence_receipt_digest", digest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ClaimsWorkflowRequest {
        ClaimsWorkflowRequest {
            request_id: "request:knowledge".into(),
            workflow_id: "workflow:multimodal".into(),
            requester: "researcher".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            required_claim_ids: vec!["claim:a".into(), "claim:b".into()],
            resolved_claim_ids: vec!["claim:a".into(), "claim:b".into()],
            evidence_receipt_digest: Some(ContentHash::of_bytes(b"derivation")),
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn missing_claim_stays_unknown() {
        let mut value = request();
        value.resolved_claim_ids.pop();
        let receipt = run_knowledge_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorkflowDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn complete_workflow_is_deterministic_under_input_order() {
        let mut reversed = request();
        reversed.study_ids.reverse();
        reversed.required_claim_ids.reverse();
        reversed.resolved_claim_ids.reverse();
        let first = run_knowledge_workflow(&request()).unwrap();
        let second = run_knowledge_workflow(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.disposition, KnowledgeWorkflowDisposition::Passed);
    }

    #[test]
    fn policy_block_clears_resolved_claims() {
        let mut value = request();
        value.policy_decision = PolicyDecision::Deny;
        let receipt = run_knowledge_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorkflowDisposition::Blocked);
        assert!(receipt.world.resolved_claim_ids.is_empty());
        assert_eq!(receipt.effect_receipts, vec!["block:knowledge-workflow"]);
    }

    #[test]
    fn out_of_scope_resolved_claim_is_rejected() {
        let mut value = request();
        value.resolved_claim_ids.push("claim:unrequested".into());
        assert!(run_knowledge_workflow(&value).is_err());
    }

    #[test]
    fn duplicate_study_is_rejected() {
        let mut value = request();
        value.study_ids.push("study:a".into());
        assert!(run_knowledge_workflow(&value).is_err());
    }

    #[test]
    fn tampered_world_artifact_is_rejected() {
        let mut receipt = run_knowledge_workflow(&request()).unwrap();
        receipt.world.workflow_id = "workflow:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = run_knowledge_workflow(&request()).unwrap();
        receipt.input.requester = "tampered requester".into();
        assert!(receipt.validate().is_err());
    }
}
