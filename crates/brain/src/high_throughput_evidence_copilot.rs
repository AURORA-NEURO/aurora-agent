//! Prospective high-throughput evidence research copilot.
//!
//! Atlas feature: `AFA-brain-P01-F11`. This A2 capability turns a bounded local batch
//! admission receipt into a declared-tool plan with explicit capacity and checkpoint
//! evidence. It never silently drops observations or creates clinical conclusions.

use crate::high_throughput_evidence_surveillance::{
    admit_high_throughput_evidence, HighThroughputDisposition, HighThroughputEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F11";
pub const CONTRACT_VERSION: &str = "brain-high-throughput-evidence-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
pub const MAX_ACTIONS: usize = 128;
const COPILOT_CONTENT_TYPE: &str = "application/vnd.aurora.qualified-evidence-set-3+json";
const MAX_ITEMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighThroughputCopilotRequest {
    pub request: HighThroughputEvidenceFeedRequest,
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
pub struct HighThroughputCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub batch_id: String,
    pub partition: String,
    pub checkpoint_seq: u64,
    pub disposition: HighThroughputDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub queue_digest: ContentHash,
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
pub enum HighThroughputCopilotError {
    #[error("invalid high-throughput copilot request: {0}")]
    Invalid(String),
    #[error("high-throughput copilot artifact failed: {0}")]
    Artifact(String),
    #[error("high-throughput copilot engine failed: {0}")]
    Engine(String),
}

impl HighThroughputCopilotReceipt {
    pub fn validate(&self) -> Result<(), HighThroughputCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot identity, batch, bounded plan, tool, locality, budget, or effects are incomplete".into(),
            ));
        }
        let collections = [
            &self.plan_order,
            &self.action_order,
            &self.tool_order,
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ];
        if collections.iter().any(|values| values.len() > MAX_ITEMS) {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot collection exceeds the bounded contract limit".into(),
            ));
        }
        if self.plan_order.len() > MAX_ACTIONS
            || self.action_order.len() > MAX_ACTIONS
            || self.tool_order.len() != 1
        {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot plan or tool cardinality exceeds the contract".into(),
            ));
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().collect::<BTreeSet<_>>();
        let unknown = self.unknown_order.iter().collect::<BTreeSet<_>>();
        let mut covered = admitted.clone();
        covered.extend(blocked.iter());
        if covered != candidates || !admitted.is_disjoint(&blocked) || !unknown.is_subset(&blocked)
        {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot states must partition candidate order without overlap".into(),
            ));
        }
        for values in collections {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(HighThroughputCopilotError::Invalid(
                    "throughput copilot ordering is not canonical".into(),
                ));
            }
        }
        if self
            .plan_order
            .iter()
            .zip(self.action_order.iter())
            .any(|(plan, action)| plan.strip_prefix("plan:") != action.strip_prefix("action:"))
            || self
                .plan_order
                .iter()
                .any(|item| !item.starts_with("plan:"))
            || self
                .action_order
                .iter()
                .any(|item| !item.starts_with("action:"))
        {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot plan and action order are not bound".into(),
            ));
        }
        let gate_blocked = self
            .omissions
            .iter()
            .any(|item| item == "copilot:action-budget-exhausted")
            || self.negative_evidence.iter().any(|item| {
                item == "copilot:admit-throughput-batch-not-allowed"
                    || item == "copilot:checkpoint-throughput-batch-not-allowed"
                    || item == "request:policy-denied"
                    || item == "request:raw-data-locality-failed"
            })
            || self
                .uncertainty
                .iter()
                .any(|item| item == "request:protected-closure-incomplete");
        let expected_disposition = if gate_blocked {
            HighThroughputDisposition::Blocked
        } else if self.admitted_order.is_empty() {
            HighThroughputDisposition::Unknown
        } else if self.blocked_order.is_empty()
            && self.omissions.is_empty()
            && self.uncertainty.is_empty()
            && self.negative_evidence.is_empty()
        {
            HighThroughputDisposition::Qualified
        } else {
            HighThroughputDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot disposition does not match state or gates".into(),
            ));
        }
        let expected_queue_digest = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "partition": self.partition,
            "candidate_order": self.candidate_order,
            "checkpoint_seq": self.checkpoint_seq,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))?;
        if self.queue_digest != expected_queue_digest {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot queue digest is not bound to batch state".into(),
            ));
        }
        let expected_plan_digest = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "batch_id": self.batch_id,
            "plan_order": self.plan_order,
            "action_order": self.action_order,
            "tool_order": self.tool_order,
            "checkpoint_seq": self.checkpoint_seq,
            "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_plan_digest {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot plan digest is not bound to plan state".into(),
            ));
        }
        let expected_effects = if self.disposition != HighThroughputDisposition::Blocked
            && !self.admitted_order.is_empty()
        {
            vec![format!("invoke:declared-tool:{}", self.tool_order[0])]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effects {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot effect does not match disposition".into(),
            ));
        }
        if self.approval_reference.as_str().len() != 64
            || self.evidence_receipt_digest.as_str().len() != 64
            || self.queue_digest.as_str().len() != 64
            || self.plan_digest.as_str().len() != 64
            || self.replay_identity.as_str().len() != 64
        {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot digest length is invalid".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-high-throughput-evidence-copilot:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != COPILOT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(HighThroughputCopilotError::Invalid(
                "throughput copilot artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, HighThroughputCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))
    }
}

pub fn high_throughput_evidence_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["agent developer".into(), "research operations steward".into()].into(),
        behavior: "compiles bounded local EvidenceFeed3 batch admission into a checkpointed declared-tool plan without silent capacity loss".into(),
        value: "automates prospective evidence streams while preserving queue identity, capacity omissions, negative results, and authority receipts".into(),
        inputs: vec![TypedPort { name: "throughput_evidence_feed".into(), schema: "EvidenceFeed3@1".into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::WriteLocalArtifact].into(),
        permissions: ["invoke:declared-tools".into(), "read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "signed-tool approver".into(), reason: "authorize the bounded tool before prospective batch effects".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_high_throughput_evidence_copilot(
    request: &HighThroughputCopilotRequest,
) -> Result<HighThroughputCopilotReceipt, HighThroughputCopilotError> {
    validate_request(request)?;
    let evidence = admit_high_throughput_evidence(&request.request)
        .map_err(|error| HighThroughputCopilotError::Engine(error.to_string()))?;
    let mut plan_order = BTreeSet::new();
    let mut action_order = BTreeSet::new();
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
    for id in &evidence.admitted_order {
        plan_order.insert(format!("plan:admit-throughput:{id}"));
        action_order.insert(format!("action:admit-throughput:{id}"));
    }
    if evidence.admitted_order.is_empty() {
        plan_order.insert("plan:retain-throughput-unknown".into());
        action_order.insert("action:retain-throughput-unknown".into());
    }
    plan_order.insert(format!("plan:checkpoint:{}", evidence.checkpoint_seq));
    action_order.insert(format!("action:checkpoint:{}", evidence.checkpoint_seq));
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "admit-throughput-batch")
    {
        negative.insert("copilot:admit-throughput-batch-not-allowed".into());
    }
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "checkpoint-throughput-batch")
    {
        negative.insert("copilot:checkpoint-throughput-batch-not-allowed".into());
    }
    let action_count = u64::try_from(action_order.len()).unwrap_or(u64::MAX);
    if u64::from(request.budget_units) < action_count {
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
        .any(|item| item == "admit-throughput-batch")
        && request
            .action_allow_list
            .iter()
            .any(|item| item == "checkpoint-throughput-batch")
        && u64::from(request.budget_units) >= action_count
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && !request.declared_tool_id.trim().is_empty();
    let disposition = if !actionable {
        HighThroughputDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let action_vec = action_order.into_iter().collect::<Vec<_>>();
    let tool_vec = vec![request.declared_tool_id.clone()];
    let evidence_digest = evidence
        .digest()
        .map_err(|error| HighThroughputCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "batch_id": request.request.batch_id, "plan_order": plan_vec, "action_order": action_vec, "tool_order": tool_vec, "checkpoint_seq": evidence.checkpoint_seq, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity})).map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "checkpoint_seq": evidence.checkpoint_seq, "disposition": disposition, "plan_order": plan_vec, "action_order": action_vec, "tool_order": tool_vec, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "queue_digest": evidence.queue_digest, "evidence_receipt_digest": evidence_digest, "plan_digest": plan_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-high-throughput-evidence-copilot:{}",
            request.request.request_id
        ),
        COPILOT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| HighThroughputCopilotError::Artifact(error.to_string()))?;
    let has_effect = actionable && !evidence.admitted_order.is_empty();
    let receipt = HighThroughputCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        checkpoint_seq: evidence.checkpoint_seq,
        disposition,
        plan_order: plan_vec.clone(),
        action_order: action_vec.clone(),
        tool_order: tool_vec,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        queue_digest: evidence.queue_digest.clone(),
        evidence_receipt_digest: evidence_digest,
        plan_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if has_effect {
            vec![format!("invoke:declared-tool:{}", request.declared_tool_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn receipt_payload(receipt: &HighThroughputCopilotReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "operator_id": receipt.operator_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "checkpoint_seq": receipt.checkpoint_seq,
        "disposition": receipt.disposition,
        "plan_order": receipt.plan_order,
        "action_order": receipt.action_order,
        "tool_order": receipt.tool_order,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "queue_digest": receipt.queue_digest,
        "evidence_receipt_digest": receipt.evidence_receipt_digest,
        "plan_digest": receipt.plan_digest,
        "approval_reference": receipt.approval_reference,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_request(
    request: &HighThroughputCopilotRequest,
) -> Result<(), HighThroughputCopilotError> {
    if request.operator_id.trim().is_empty()
        || request.action_allow_list.is_empty()
        || request.declared_tool_id.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.replay_identity != request.replay_identity
        || request.approval_reference == ContentHash::of_bytes(&[])
    {
        return Err(HighThroughputCopilotError::Invalid("throughput copilot operator, declared tool, signed approval, bounded actions, budget, replay, or boundary is incomplete".into()));
    }
    if request.request.observations.len() > request.max_actions.saturating_mul(64) {
        return Err(HighThroughputCopilotError::Invalid(
            "throughput evidence feed exceeds bounded plan capacity".into(),
        ));
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
    fn observation(id: &str, state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
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
    fn request(observations: Vec<EvidenceObservation>) -> HighThroughputCopilotRequest {
        HighThroughputCopilotRequest {
            request: HighThroughputEvidenceFeedRequest {
                request_id: "request:throughput-copilot".into(),
                batch_id: "batch:001".into(),
                partition: "partition:imaging".into(),
                max_items: 2,
                observations,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operator_id: "operator:researcher".into(),
            action_allow_list: vec![
                "admit-throughput-batch".into(),
                "checkpoint-throughput-batch".into(),
            ],
            declared_tool_id: "tool:throughput-evidence".into(),
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
    fn manifest_is_a2_and_checkpointed() {
        let manifest = high_throughput_evidence_research_copilot_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn admitted_batch_invokes_declared_tool() {
        let receipt = compile_high_throughput_evidence_copilot(&request(vec![observation(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tool:"));
        assert!(receipt
            .action_order
            .iter()
            .any(|item| item.contains("checkpoint")));
    }
    #[test]
    fn capacity_omission_is_retained() {
        let receipt = compile_high_throughput_evidence_copilot(&request(vec![
            observation("a", EvidenceState::Supported),
            observation("b", EvidenceState::Supported),
            observation("c", EvidenceState::Supported),
        ]))
        .unwrap();
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("capacity")));
    }
    #[test]
    fn unknown_batch_is_blocked_without_admission() {
        let receipt = compile_high_throughput_evidence_copilot(&request(vec![observation(
            "a",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Unknown);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn tool_allowance_denial_blocks() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.action_allow_list = vec!["write-external".into()];
        let receipt = compile_high_throughput_evidence_copilot(&input).unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Blocked);
    }
    #[test]
    fn approval_identity_is_required() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_high_throughput_evidence_copilot(&input).is_err());
    }
}
