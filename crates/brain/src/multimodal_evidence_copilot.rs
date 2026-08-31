//! Multimodal multi-study evidence research copilot.
//!
//! Atlas feature: `AFA-brain-P01-F10`. The copilot compiles caller-supplied multimodal
//! evidence into a bounded declared-tool plan. It never fetches sources, moves raw data,
//! or makes a clinical decision.

use crate::multimodal_evidence_surveillance::{
    surveil_multimodal_evidence, MultimodalEvidenceDisposition, MultimodalEvidenceFeedRequest,
};
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F10";
pub const CONTRACT_VERSION: &str = "brain-multimodal-evidence-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
pub const MAX_ACTIONS: usize = 96;
const COPILOT_CONTENT_TYPE: &str = "application/vnd.aurora.qualified-evidence-set-3+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalCopilotRequest {
    pub request: MultimodalEvidenceFeedRequest,
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
pub struct MultimodalCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub study_order: Vec<String>,
    pub scope: String,
    pub disposition: MultimodalEvidenceDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub evidence_receipt_digest: ContentHash,
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
pub enum MultimodalCopilotError {
    #[error("invalid multimodal copilot request: {0}")]
    Invalid(String),
    #[error("multimodal copilot artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal copilot engine failed: {0}")]
    Engine(String),
}

impl MultimodalCopilotReceipt {
    pub fn validate(&self) -> Result<(), MultimodalCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.study_order.len() < 2
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.len() != 1
            || self.effect_receipts.len() != 1
            || self.budget_units == 0
        {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot identity, study floor, bounded plan, tool, locality, budget, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.operator_id, "operator_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self
            .plan_order
            .iter()
            .zip(&self.action_order)
            .any(|(plan, action)| plan.strip_prefix("plan:") != action.strip_prefix("action:"))
        {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot plan and action orders are not paired".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.plan_order,
            &self.action_order,
            &self.tool_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            validate_sorted_unique(values, "multimodal copilot collection")?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if qualified_keys
            .union(&blocked_keys)
            .cloned()
            .collect::<BTreeSet<_>>()
            != candidate_keys
            || !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
        {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot states are not a disjoint candidate partition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect = if self.disposition != MultimodalEvidenceDisposition::Blocked
            && !self.qualified_order.is_empty()
        {
            format!("invoke:declared-tool:{}", self.tool_order[0])
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot effect does not match disposition and qualification".into(),
            ));
        }
        for digest in [
            &self.evidence_receipt_digest,
            &self.plan_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalCopilotError::Invalid(
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
        }))
        .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_plan_digest {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot plan digest is not bound to plan".into(),
            ));
        }
        let expected_artifact_id = format!("brain-multimodal-evidence-copilot:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != COPILOT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalCopilotError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalCopilotError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), MultimodalCopilotError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalCopilotError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), MultimodalCopilotError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalCopilotError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &MultimodalCopilotReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_order": receipt.study_order,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "plan_order": receipt.plan_order,
        "action_order": receipt.action_order,
        "tool_order": receipt.tool_order,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "modality_order": receipt.modality_order,
        "evidence_receipt_digest": receipt.evidence_receipt_digest,
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

pub fn multimodal_evidence_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["research workflow operator".into(), "agent developer".into()].into(),
        behavior: "compiles local multimodal EvidenceFeed receipts into a bounded declared-tool research plan without raw-data or clinical effects".into(),
        value: "turns comparable evidence across studies and modalities into replayable researcher actions while preserving uncertainty, omissions, and negative results".into(),
        inputs: vec![TypedPort { name: "multimodal_evidence_feed".into(), schema: "EvidenceFeed2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::WriteLocalArtifact].into(),
        permissions: ["invoke:declared-tools".into(), "read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "signed-tool approver".into(), reason: "authorize the bounded declared tool before any multimodal research effect".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_multimodal_evidence_copilot(
    request: &MultimodalCopilotRequest,
) -> Result<MultimodalCopilotReceipt, MultimodalCopilotError> {
    validate_request(request)?;
    let evidence = surveil_multimodal_evidence(&request.request)
        .map_err(|error| MultimodalCopilotError::Engine(error.to_string()))?;
    let mut action_order = BTreeSet::new();
    let mut plan_order = BTreeSet::new();
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for evidence_id in &evidence.qualified_order {
        action_order.insert(format!("action:inspect-multimodal:{evidence_id}"));
        plan_order.insert(format!("plan:inspect-multimodal:{evidence_id}"));
    }
    if !evidence.qualified_order.is_empty() {
        action_order.insert("action:compare-required-modalities".into());
        plan_order.insert("plan:compare-required-modalities".into());
    } else {
        action_order.insert("action:retain-multimodal-unknown".into());
        plan_order.insert("plan:retain-multimodal-unknown".into());
    }
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-local-multimodal-evidence")
    {
        negative.insert("copilot:inspect-local-multimodal-evidence-not-allowed".into());
    }
    if !evidence.qualified_order.is_empty()
        && !request
            .action_allow_list
            .iter()
            .any(|item| item == "compare-required-modalities")
    {
        negative.insert("copilot:compare-required-modalities-not-allowed".into());
    }
    if u64::from(request.budget_units) < u64::try_from(action_order.len()).unwrap_or(u64::MAX) {
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
        .any(|item| item == "inspect-local-multimodal-evidence")
        && (evidence.qualified_order.is_empty()
            || request
                .action_allow_list
                .iter()
                .any(|item| item == "compare-required-modalities"))
        && u64::from(request.budget_units) >= u64::try_from(action_order.len()).unwrap_or(u64::MAX)
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && !request.declared_tool_id.trim().is_empty();
    let disposition = if !actionable {
        MultimodalEvidenceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let action_vec = action_order.into_iter().collect::<Vec<_>>();
    let tool_vec = vec![request.declared_tool_id.clone()];
    let effect_receipts = if disposition != MultimodalEvidenceDisposition::Blocked
        && !evidence.qualified_order.is_empty()
    {
        vec![format!("invoke:declared-tool:{}", request.declared_tool_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| MultimodalCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({
        "request_id": request.request.request_id,
        "plan_order": plan_vec,
        "action_order": action_vec,
        "tool_order": tool_vec,
        "budget_units": request.budget_units,
        "approval_reference": request.approval_reference,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request.request_id,
        "study_order": request.request.study_ids,
        "scope": request.request.scope,
        "disposition": disposition,
        "plan_order": plan_vec,
        "action_order": action_vec,
        "tool_order": tool_vec,
        "candidate_order": evidence.candidate_order,
        "qualified_order": evidence.qualified_order,
        "blocked_order": evidence.blocked_order,
        "unknown_order": evidence.unknown_order,
        "modality_order": evidence.modality_order,
        "evidence_receipt_digest": evidence_digest,
        "plan_digest": plan_digest,
        "approval_reference": request.approval_reference,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-multimodal-evidence-copilot:{}",
            request.request.request_id
        ),
        COPILOT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalCopilotError::Artifact(error.to_string()))?;
    let receipt = MultimodalCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        study_order: request.request.study_ids.clone(),
        scope: request.request.scope.clone(),
        disposition,
        plan_order: payload
            .get("plan_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        action_order: payload
            .get("action_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        tool_order: tool_vec,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        modality_order: evidence.modality_order.clone(),
        evidence_receipt_digest: evidence_digest,
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

fn validate_request(request: &MultimodalCopilotRequest) -> Result<(), MultimodalCopilotError> {
    if request.operator_id.trim().is_empty()
        || request.action_allow_list.is_empty()
        || request.declared_tool_id.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.replay_identity != request.replay_identity
        || request.request.policy_allow != request.policy_allow
        || request.request.protected_closure != request.protected_closure
        || request.request.raw_data_local != request.raw_data_local
        || request.approval_reference == ContentHash::of_bytes(&[])
    {
        return Err(MultimodalCopilotError::Invalid("multimodal copilot operator, declared tool, signed approval, bounded action allow-list, budget, replay, or boundary is incomplete".into()));
    }
    if request.request.observations.len() > request.max_actions.saturating_mul(64) {
        return Err(MultimodalCopilotError::Invalid(
            "multimodal evidence feed exceeds bounded plan capacity".into(),
        ));
    }
    for (value, field) in [
        (&request.operator_id, "operator_id"),
        (&request.declared_tool_id, "declared_tool_id"),
        (&request.request.request_id, "request_id"),
        (&request.request.scope, "scope"),
        (&request.request.query, "query"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.action_allow_list, "action_allow_list")?;
    validate_unique(&request.request.study_ids, "study_ids")?;
    validate_unique(&request.request.required_modalities, "required_modalities")?;
    for digest in [&request.approval_reference, &request.replay_identity] {
        if digest.as_str().len() != 64 {
            return Err(MultimodalCopilotError::Invalid(
                "multimodal copilot approval or replay digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(
        id: &str,
        study: &str,
        modality: &str,
        state: EvidenceState,
    ) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: study.into(),
            modality: modality.into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(observations: Vec<EvidenceObservation>) -> MultimodalCopilotRequest {
        MultimodalCopilotRequest {
            request: MultimodalEvidenceFeedRequest {
                request_id: "request:multimodal-copilot".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_relevance_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                observations,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operator_id: "operator:researcher".into(),
            action_allow_list: vec![
                "inspect-local-multimodal-evidence".into(),
                "compare-required-modalities".into(),
            ],
            declared_tool_id: "tool:multimodal-evidence".into(),
            approval_reference: hash("signed-approval"),
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
    fn manifest_is_a2_and_declared_tool_scoped() {
        let manifest = multimodal_evidence_research_copilot_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn comparable_evidence_invokes_declared_tool() {
        let receipt = compile_multimodal_evidence_copilot(&request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Supported),
            observation("b", "study:a", "transcriptomics", EvidenceState::Supported),
            observation("c", "study:b", "imaging", EvidenceState::Supported),
            observation("d", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tool:"));
    }
    #[test]
    fn unknown_evidence_is_retained() {
        let receipt = compile_multimodal_evidence_copilot(&request(vec![observation(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert!(!receipt.unknown_order.is_empty());
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Unknown);
    }
    #[test]
    fn tool_allowance_denial_blocks() {
        let mut input = request(vec![observation(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]);
        input.action_allow_list = vec!["write-external".into()];
        let receipt = compile_multimodal_evidence_copilot(&input).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Blocked);
    }
    #[test]
    fn approval_identity_is_required() {
        let mut input = request(vec![observation(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_multimodal_evidence_copilot(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(vec![observation(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]);
        input.replay_identity = hash("different");
        assert!(compile_multimodal_evidence_copilot(&input).is_err());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(vec![observation(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]);
        input.raw_data_local = false;
        input.request.raw_data_local = false;
        let receipt = compile_multimodal_evidence_copilot(&input).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn plan_and_payload_drift_are_rejected() {
        let receipt = compile_multimodal_evidence_copilot(&request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Supported),
            observation("b", "study:a", "transcriptomics", EvidenceState::Supported),
            observation("c", "study:b", "imaging", EvidenceState::Supported),
            observation("d", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        let mut plan_drift = receipt.clone();
        plan_drift.action_order.pop();
        assert!(plan_drift.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.scope = "organoid:other".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_operator_identity_is_rejected() {
        let mut input = request(vec![observation(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]);
        input.operator_id = " operator:researcher".into();
        assert!(compile_multimodal_evidence_copilot(&input).is_err());
    }
}
