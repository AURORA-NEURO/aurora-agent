//! Prospective high-throughput retrieval research copilot.
//!
//! Atlas feature: `AFA-brain-P02-F11`. It compiles bounded batch retrieval into a declared-tool
//! plan while retaining queue, capacity, checkpoint, and recovery evidence.

use crate::retrieval_synthesis::SynthesisDisposition;
use crate::throughput_retrieval_synthesis::{
    synthesize_throughput_retrieval, ThroughputRetrievalQuery,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F11";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "ThroughputEvidenceSynthesisCopilot1@1";
const COPILOT_CONTENT_TYPE: &str =
    "application/vnd.aurora.throughput-evidence-synthesis-copilot+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalCopilotRequest {
    pub request: ThroughputRetrievalQuery,
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
pub struct ThroughputRetrievalCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: SynthesisDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
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
pub enum ThroughputRetrievalCopilotError {
    #[error("invalid throughput retrieval copilot request: {0}")]
    Invalid(String),
    #[error("throughput retrieval copilot artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval copilot engine failed: {0}")]
    Engine(String),
}

impl ThroughputRetrievalCopilotReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.is_empty()
            || self.checkpoint_seq == 0
            || self.budget_units == 0
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputRetrievalCopilotError::Invalid("throughput copilot identity, queue, checkpoint, bounded plan, tool, budget, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.operator_id, "operator_id"),
            (&self.batch_id, "batch_id"),
            (&self.partition, "partition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
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
            return Err(ThroughputRetrievalCopilotError::Invalid(
                "throughput copilot plans, actions, and declared tool are not paired".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self
                .qualified_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .blocked_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .unknown_order
                .iter()
                .any(|candidate| !self.blocked_order.contains(candidate))
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
        {
            return Err(ThroughputRetrievalCopilotError::Invalid(
                "throughput copilot candidate states must partition candidates".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition != SynthesisDisposition::Blocked {
            vec![format!("invoke:declared-tool:{}", self.tool_order[0])]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalCopilotError::Invalid(
                "throughput copilot effects do not match disposition and declared tool".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .negative_evidence
                    .iter()
                    .any(|item| item == "request:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalCopilotError::Invalid(
                "non-local throughput copilots must be blocked and retain locality evidence".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.synthesis_digest,
            &self.plan_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalCopilotError::Invalid(
                    "throughput copilot digest is invalid".into(),
                ));
            }
        }
        let expected_plan_digest = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "plan_order": self.plan_order,
            "action_order": self.action_order,
            "tool_order": self.tool_order,
            "checkpoint_seq": self.checkpoint_seq,
            "queue_digest": self.queue_digest,
            "budget_units": self.budget_units,
            "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_plan_digest {
            return Err(ThroughputRetrievalCopilotError::Invalid(
                "throughput copilot plan digest is not bound to the declared plan".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-copilot:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != COPILOT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalCopilotError::Invalid(
                "throughput copilot artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "batch retrieval operator".into()].into(), behavior: "compiles bounded throughput retrieval into a declared-tool plan with capacity, queue, checkpoint, approval, replay, and local-data gates".into(), value: "automates prospective evidence batches without silent overflow, dropped checkpoints, or unreviewed tool effects".into(), inputs: vec![TypedPort { name: "throughput_retrieval_copilot_request".into(), schema: "ScopedRetrievalQuery3@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_synthesis_copilot_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into(), "read:local-throughput-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "signed throughput-tool approver".into(), reason: "authorize bounded queue actions and checkpoint recovery before prospective effects".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_throughput_retrieval_copilot(
    request: &ThroughputRetrievalCopilotRequest,
) -> Result<ThroughputRetrievalCopilotReceipt, ThroughputRetrievalCopilotError> {
    validate_request(request)?;
    let synthesis = synthesize_throughput_retrieval(&request.request)
        .map_err(|error| ThroughputRetrievalCopilotError::Engine(error.to_string()))?;
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
        actions.insert(format!("action:inspect-throughput:{evidence_id}"));
        plans.insert(format!("plan:inspect-throughput:{evidence_id}"));
    }
    actions.insert("action:checkpoint-throughput-batch".into());
    plans.insert("plan:checkpoint-throughput-batch".into());
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "inspect-throughput-evidence")
    {
        negative.insert("copilot:inspect-throughput-evidence-not-allowed".into());
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
        .any(|item| item == "inspect-throughput-evidence")
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
        .map_err(|error| ThroughputRetrievalCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "checkpoint_seq": synthesis.checkpoint_seq, "queue_digest": synthesis.queue_digest, "budget_units": request.budget_units, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "raw_data_local": true})).map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))?;
    let effect_receipts = if actionable {
        vec![format!("invoke:declared-tool:{}", request.declared_tool_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "disposition": disposition, "plan_order": plan_order, "action_order": action_order, "tool_order": tool_order, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "checkpoint_seq": synthesis.checkpoint_seq, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis_digest, "plan_digest": plan_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-retrieval-copilot:{}",
            request.request.request_id
        ),
        COPILOT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalCopilotError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        disposition,
        plan_order,
        action_order,
        tool_order,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        checkpoint_seq: synthesis.checkpoint_seq,
        queue_digest: synthesis.queue_digest,
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
    request: &ThroughputRetrievalCopilotRequest,
) -> Result<(), ThroughputRetrievalCopilotError> {
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
        || request.max_actions > 128
        || request.budget_units == 0
        || request.request.batch_id.trim().is_empty()
        || request.request.partition.trim().is_empty()
        || request.request.max_items == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.candidates.is_empty()
        || request.request.replay_identity != request.replay_identity
    {
        return Err(ThroughputRetrievalCopilotError::Invalid("throughput copilot operator, tool, batch, capacity, budget, candidates, or boundary is incomplete".into()));
    }
    validate_unique(&request.action_allow_list, "action_allow_list")?;
    if request.replay_identity.as_str().len() != 64 {
        return Err(ThroughputRetrievalCopilotError::Invalid(
            "throughput copilot replay identity digest is invalid".into(),
        ));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalCopilotError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalCopilotError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputRetrievalCopilotError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalCopilotError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalCopilotError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalCopilotError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalCopilotReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "operator_id": receipt.operator_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "disposition": receipt.disposition,
        "plan_order": receipt.plan_order,
        "action_order": receipt.action_order,
        "tool_order": receipt.tool_order,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "checkpoint_seq": receipt.checkpoint_seq,
        "queue_digest": receipt.queue_digest,
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
    fn request() -> ThroughputRetrievalCopilotRequest {
        ThroughputRetrievalCopilotRequest {
            request: ThroughputRetrievalQuery {
                request_id: "request:tp-copilot".into(),
                batch_id: "batch:001".into(),
                partition: "partition:imaging".into(),
                max_items: 2,
                minimum_support_milli: 700,
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:organoid".into(),
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
            operator_id: "operator:batch".into(),
            action_allow_list: vec!["inspect-throughput-evidence".into()],
            declared_tool_id: "tool:batch-review".into(),
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
        let m = throughput_retrieval_copilot_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn approved_plan_is_qualified() {
        let r = compile_throughput_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Qualified);
    }
    #[test]
    fn overflow_remains_partial() {
        let mut q = request();
        q.request.candidates.push(q.request.candidates[0].clone());
        q.request.candidates.push(q.request.candidates[0].clone());
        let r = compile_throughput_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Partial);
    }
    #[test]
    fn missing_approval_blocks() {
        let mut q = request();
        q.approval_reference = ContentHash::of_bytes(b"");
        let r = compile_throughput_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn missing_permission_blocks() {
        let mut q = request();
        q.action_allow_list.clear();
        let r = compile_throughput_retrieval_copilot(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request();
        q.raw_data_local = false;
        let r = compile_throughput_retrieval_copilot(&q).unwrap();
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
        let mut r = compile_throughput_retrieval_copilot(&request()).unwrap();
        r.operator_id = "operator:tampered".into();
        assert!(r.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut r = compile_throughput_retrieval_copilot(&request()).unwrap();
        r.qualified_order[0] = r.qualified_order[0].to_ascii_uppercase();
        assert!(r.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let r = compile_throughput_retrieval_copilot(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
