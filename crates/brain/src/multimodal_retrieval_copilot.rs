//! Multimodal retrieval-and-synthesis research copilot.
//!
//! Atlas feature: `AFA-brain-P02-F10`. It compiles comparable multi-study retrieval into a
//! bounded declared-tool plan with signed approval and explicit modality closure.

use crate::multimodal_retrieval_synthesis::{
    synthesize_multimodal_retrieval, MultimodalRetrievalQuery,
};
use crate::retrieval_synthesis::SynthesisDisposition;
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F10";
pub const CONTRACT_VERSION: &str = "brain-multimodal-retrieval-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "MultimodalEvidenceSynthesisCopilot1@1";
const COPILOT_CONTENT_TYPE: &str =
    "application/vnd.aurora.multimodal-evidence-synthesis-copilot+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalCopilotRequest {
    pub request: MultimodalRetrievalQuery,
    pub operator_id: String,
    pub action_allow_list: Vec<String>,
    pub declared_tool_id: String,
    pub approval_reference: ContentHash,
    pub max_actions: usize,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub scope: String,
    pub disposition: SynthesisDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub plan_digest: ContentHash,
    pub approval_reference: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalRetrievalCopilotError {
    #[error("invalid multimodal retrieval copilot request: {0}")]
    Invalid(String),
    #[error("multimodal retrieval copilot artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal retrieval copilot engine failed: {0}")]
    Engine(String),
}

impl MultimodalRetrievalCopilotReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.scope.trim().is_empty()
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(MultimodalRetrievalCopilotError::Invalid("multimodal copilot identity, coverage, bounded plan, tool, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.operator_id, "operator_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.study_order, "study_order"),
            (&self.modality_order, "modality_order"),
            (&self.plan_order, "plan_order"),
            (&self.action_order, "action_order"),
            (&self.tool_order, "tool_order"),
            (&self.candidate_order, "candidate_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for (values, field) in [
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        if self.plan_order.len() != self.action_order.len()
            || self.tool_order.len() != 1
            || self
                .plan_order
                .iter()
                .zip(self.action_order.iter())
                .any(|(plan, action)| {
                    !plan.starts_with("plan:")
                        || !action.starts_with("action:")
                        || action.strip_prefix("action:") != plan.strip_prefix("plan:")
                })
            || self
                .tool_order
                .iter()
                .any(|tool| !tool.starts_with("tool:"))
        {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "multimodal copilot plans, actions, and declared tool are not paired".into(),
            ));
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        if !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
        {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "multimodal copilot candidate states must partition candidates".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition != SynthesisDisposition::Blocked {
            vec![format!("invoke:declared-tool:{}", self.tool_order[0])]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "multimodal copilot effects do not match disposition and declared tool".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .negative_evidence
                    .iter()
                    .any(|item| item == "request:raw-data-locality-failed"))
        {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "non-local multimodal copilots must be blocked and retain locality evidence".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.synthesis_digest,
            &self.plan_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalRetrievalCopilotError::Invalid(
                    "multimodal copilot digest is invalid".into(),
                ));
            }
        }
        let expected_plan_digest = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "plan_order": self.plan_order,
            "action_order": self.action_order,
            "tool_order": self.tool_order,
            "budget_units": self.budget_units,
            "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_plan_digest {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "multimodal copilot plan digest is not bound to the declared plan".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-multimodal-retrieval-copilot:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != COPILOT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "multimodal copilot artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalRetrievalCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))
    }
}

pub fn multimodal_retrieval_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "agent developer".into()].into(), behavior: "compiles comparable multimodal retrieval into a bounded declared-tool plan with signed approval, modality closure, replay, and local-data gates".into(), value: "turns multisite imaging and omics retrieval into replayable researcher actions without raw-data movement or silent modality completion".into(), inputs: vec![TypedPort { name: "multimodal_retrieval_copilot_request".into(), schema: "ScopedRetrievalQuery2@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_synthesis_copilot_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into(), "read:local-multimodal-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: vec![AuthorityRequirement { role: "signed multimodal-tool approver".into(), reason: "authorize the bounded declared tool before multimodal research effects".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_multimodal_retrieval_copilot(
    request: &MultimodalRetrievalCopilotRequest,
) -> Result<MultimodalRetrievalCopilotReceipt, MultimodalRetrievalCopilotError> {
    validate_request(request)?;
    let mut normalized_request = request.request.clone();
    normalized_request.study_ids.sort();
    normalized_request.required_modalities.sort();
    let synthesis = synthesize_multimodal_retrieval(&normalized_request)
        .map_err(|error| MultimodalRetrievalCopilotError::Engine(error.to_string()))?;
    let mut actions = BTreeSet::new();
    let mut plans = BTreeSet::new();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for evidence_id in &synthesis.qualified_order {
        actions.insert(format!("action:inspect-multimodal:{evidence_id}"));
        plans.insert(format!("plan:inspect-multimodal:{evidence_id}"));
    }
    if !synthesis.qualified_order.is_empty() {
        actions.insert("action:compare-required-modalities".into());
        plans.insert("plan:compare-required-modalities".into());
    } else {
        actions.insert("action:retain-multimodal-unknown".into());
        plans.insert("plan:retain-multimodal-unknown".into());
    }
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-multimodal-evidence")
    {
        negative.insert("copilot:inspect-multimodal-evidence-not-allowed".into());
    }
    if u64::from(request.budget_units) < u64::try_from(actions.len()).unwrap_or(u64::MAX)
        || actions.len() > request.max_actions
    {
        omissions.insert("copilot:action-budget-exhausted".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let actionable = request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-multimodal-evidence")
        && !request.declared_tool_id.trim().is_empty()
        && request.approval_reference.as_str().len() == 64
        && request.approval_reference != ContentHash::of_bytes(b"")
        && u64::from(request.budget_units) >= u64::try_from(actions.len()).unwrap_or(u64::MAX)
        && actions.len() <= request.max_actions
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local;
    let disposition = if !actionable {
        SynthesisDisposition::Blocked
    } else {
        synthesis.disposition
    };
    let plan_order = plans.into_iter().collect::<Vec<_>>();
    let action_order = actions.into_iter().collect::<Vec<_>>();
    let tool_order = vec![request.declared_tool_id.clone()];
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| MultimodalRetrievalCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "budget_units": request.budget_units, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "raw_data_local": true})).map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))?;
    let effect_receipts = if actionable {
        vec![format!("invoke:declared-tool:{}", request.declared_tool_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "study_order": normalized_request.study_ids, "modality_order": normalized_request.required_modalities, "scope": request.request.scope, "disposition": disposition, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "comparability_digest": synthesis.comparability_digest, "synthesis_digest": synthesis_digest, "plan_digest": plan_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-multimodal-retrieval-copilot:{}",
            request.request.request_id
        ),
        COPILOT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalCopilotError::Artifact(error.to_string()))?;
    let receipt = MultimodalRetrievalCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        study_order: normalized_request.study_ids,
        modality_order: normalized_request.required_modalities,
        scope: request.request.scope.clone(),
        disposition,
        plan_order,
        action_order,
        tool_order,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        comparability_digest: synthesis.comparability_digest,
        synthesis_digest,
        plan_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &MultimodalRetrievalCopilotRequest,
) -> Result<(), MultimodalRetrievalCopilotError> {
    for (value, field) in [
        (&request.operator_id, "operator_id"),
        (&request.declared_tool_id, "declared_tool_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.operator_id.trim().is_empty()
        || request.declared_tool_id.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > 96
        || request.budget_units == 0
        || request.request.study_ids.len() < 2
        || request.request.required_modalities.len() < 2
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.candidates.is_empty()
        || request.request.replay_identity != request.replay_identity
    {
        return Err(MultimodalRetrievalCopilotError::Invalid("multimodal copilot operator, tool, coverage, capacity, budget, candidates, or boundary is incomplete".into()));
    }
    validate_unique(&request.request.study_ids, "study_ids")?;
    validate_unique(&request.request.required_modalities, "required_modalities")?;
    validate_unique(&request.action_allow_list, "action_allow_list")?;
    for digest in [&request.approval_reference, &request.replay_identity] {
        if digest.as_str().len() != 64 {
            return Err(MultimodalRetrievalCopilotError::Invalid(
                "multimodal copilot request digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalRetrievalCopilotError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalRetrievalCopilotError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), MultimodalRetrievalCopilotError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalRetrievalCopilotError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), MultimodalRetrievalCopilotError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalRetrievalCopilotError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &MultimodalRetrievalCopilotReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "operator_id": receipt.operator_id,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "plan_order": receipt.plan_order,
        "action_order": receipt.action_order,
        "tool_order": receipt.tool_order,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "comparability_digest": receipt.comparability_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "plan_digest": receipt.plan_digest,
        "approval_reference": receipt.approval_reference,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> MultimodalRetrievalCopilotRequest {
        MultimodalRetrievalCopilotRequest {
            request: MultimodalRetrievalQuery {
                request_id: "request:mm-copilot".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "synaptic phenotype".into(),
                minimum_support_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:a".into(),
                    scope: "organoid:neural".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operator_id: "operator:research".into(),
            action_allow_list: vec!["inspect-multimodal-evidence".into()],
            declared_tool_id: "tool:multimodal-review".into(),
            approval_reference: hash("approval"),
            max_actions: 8,
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let m = multimodal_retrieval_copilot_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn approved_plan_is_partial_or_qualified() {
        let r = compile_multimodal_retrieval_copilot(&request()).unwrap();
        assert!(matches!(
            r.disposition,
            SynthesisDisposition::Partial | SynthesisDisposition::Qualified
        ));
    }
    #[test]
    fn missing_approval_blocks() {
        let mut q = request();
        q.approval_reference = ContentHash::of_bytes(b"");
        let r = compile_multimodal_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn missing_tool_permission_blocks() {
        let mut q = request();
        q.action_allow_list.clear();
        let r = compile_multimodal_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn missing_modality_is_retained() {
        let mut q = request();
        q.request
            .required_modalities
            .push("electrophysiology".into());
        let r = compile_multimodal_retrieval_copilot(&q).unwrap();
        assert!(!r.omissions.is_empty());
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request();
        q.raw_data_local = false;
        let r = compile_multimodal_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
        assert!(r.raw_data_local);
        assert!(r
            .negative_evidence
            .iter()
            .any(|value| value == "request:raw-data-locality-failed"));
        assert!(r.validate().is_ok());
    }

    #[test]
    fn copilot_artifact_payload_is_bound() {
        let mut r = compile_multimodal_retrieval_copilot(&request()).unwrap();
        r.operator_id = "operator:tampered".into();
        assert!(r.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = compile_multimodal_retrieval_copilot(&request()).unwrap();
        receipt.qualified_order[0] = receipt.qualified_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let r = compile_multimodal_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
