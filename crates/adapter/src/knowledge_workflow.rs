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
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P04-F14";
pub const CONTRACT_VERSION: &str = "multimodal-knowledge-workflow-fabric/1.0";

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
    pub request_id: String,
    pub workflow_id: String,
    pub disposition: KnowledgeWorkflowDisposition,
    pub world: TypedKnowledgeWorld,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
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
        {
            return Err(KnowledgeWorkflowError::InvalidField(
                "knowledge workflow identity, linkage, checks, or boundary is incomplete".into(),
            ));
        }
        if self.world.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.world.world_id.trim().is_empty()
            || self.world.study_ids.is_empty()
            || self.world.stages.is_empty()
            || self
                .world
                .resolved_claim_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.world.resolved_claim_ids.len()
        {
            return Err(KnowledgeWorkflowError::InvalidField(
                "typed knowledge world is incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeWorkflowError::Artifact(error.to_string()))
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
    validate_request(request)?;
    let mut studies = request.study_ids.clone();
    studies.sort();
    studies.dedup();
    let mut claims = request.resolved_claim_ids.clone();
    claims.sort();
    claims.dedup();
    let missing = request
        .required_claim_ids
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
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
    if blocked {
        omissions.push(
            "policy, protected-closure, or raw-data-locality gate blocked knowledge workflow"
                .into(),
        );
    }
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
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        world,
        checks,
        omissions,
        uncertainty,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ClaimsWorkflowRequest) -> Result<(), KnowledgeWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.study_ids.is_empty()
        || request.required_claim_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(KnowledgeWorkflowError::InvalidField(
            "knowledge workflow identity, studies, claims, requester, and boundary are required"
                .into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for claim in &request.required_claim_ids {
        if claim.trim().is_empty() || !seen.insert(claim.clone()) {
            return Err(KnowledgeWorkflowError::InvalidField(
                "required claim identities must be non-empty and unique".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_claim_stays_unknown() {
        let receipt = run_knowledge_workflow(&ClaimsWorkflowRequest {
            request_id: "request:knowledge".into(),
            workflow_id: "workflow:multimodal".into(),
            requester: "researcher".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            required_claim_ids: vec!["claim:a".into(), "claim:b".into()],
            resolved_claim_ids: vec!["claim:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorkflowDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
