//! Prospective high-throughput evidence-surveillance research copilot.
//!
//! Atlas feature `AFA-adapter-P01-F11`: a bounded A2 agent surface for
//! EvidenceFeed3 batches with checkpointed queue identity and explicit
//! overflow, uncertainty, negative evidence, and authorization receipts.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceAvailability, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F11";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputCopilotEvidenceObservation {
    pub source_id: String,
    pub sequence: u64,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceResearchCopilotRequest {
    pub request_id: String,
    pub agent_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_reference: Option<String>,
    pub approval_granted: bool,
    pub observations: Vec<ThroughputCopilotEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputResearchCopilotDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputCopilotQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub overflow_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub tool_mode: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: ThroughputEvidenceSurveillanceResearchCopilotRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub agent_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_granted: bool,
    pub approval_reference: Option<String>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: ThroughputResearchCopilotDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub capability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub run_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: ThroughputCopilotQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceResearchCopilotError {
    #[error("invalid throughput copilot request: {0}")]
    Invalid(String),
    #[error("throughput copilot artifact failed: {0}")]
    Artifact(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), ThroughputEvidenceSurveillanceResearchCopilotError> {
    if value.is_empty() || value.trim() != value {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            format!("{field} must be non-empty and trimmed"),
        ));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            format!("{field} is outside its bounded text contract"),
        ));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceResearchCopilotError> {
    if values.len() > MAX_ITEMS {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            format!("{field} exceeds its item bound"),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                format!("{field} contains duplicate values"),
            ));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceResearchCopilotError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            format!("{field} ordering is not canonical"),
        ));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), ThroughputEvidenceSurveillanceResearchCopilotError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            format!("{field} must be a 64-character hex digest"),
        ));
    }
    Ok(())
}

pub(crate) fn canonical_throughput_evidence_surveillance_research_copilot_request(
    request: &ThroughputEvidenceSurveillanceResearchCopilotRequest,
) -> ThroughputEvidenceSurveillanceResearchCopilotRequest {
    let mut canonical = request.clone();
    canonical.declared_tools.sort();
    canonical.observations.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    canonical
}

fn copilot_input_digest(
    request: &ThroughputEvidenceSurveillanceResearchCopilotRequest,
) -> Result<ContentHash, ThroughputEvidenceSurveillanceResearchCopilotError> {
    let canonical = canonical_throughput_evidence_surveillance_research_copilot_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value).map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })
}

impl ThroughputEvidenceSurveillanceResearchCopilotReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
            || self.declared_tools.is_empty()
            || self.requested_tool.trim().is_empty()
            || self.max_tool_calls == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.batch_id != self.batch_id
            || self.qualified_set.checkpoint_seq != self.checkpoint_seq
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid("identity, checkpoint, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("agent_id", &self.agent_id)?;
        validate_text("batch_id", &self.batch_id)?;
        validate_text("requested_tool", &self.requested_tool)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("declared_tools", &self.declared_tools)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("overflow_order", &self.overflow_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("tool_receipts", &self.tool_receipts)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        validate_sorted_strings(
            "qualified_set.selected_order",
            &self.qualified_set.selected_order,
        )?;
        validate_sorted_strings(
            "qualified_set.overflow_order",
            &self.qualified_set.overflow_order,
        )?;
        validate_sorted_strings("qualified_set.omissions", &self.qualified_set.omissions)?;
        validate_sorted_strings("qualified_set.uncertainty", &self.qualified_set.uncertainty)?;
        validate_sorted_strings(
            "qualified_set.negative_order",
            &self.qualified_set.negative_order,
        )?;
        if !self.declared_tools.contains(&self.requested_tool) {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "requested tool must be declared exactly once".into(),
            ));
        }
        if let Some(reference) = &self.approval_reference {
            validate_text("approval_reference", reference)?;
        }
        if self.qualified_set.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.qualified_set.set_id
                != format!("qualified-evidence-throughput-copilot:{}", self.request_id)
            || self.qualified_set.selected_order != self.selected_order
            || self.qualified_set.overflow_order != self.overflow_order
            || self.qualified_set.omissions != self.omissions
            || self.qualified_set.uncertainty != self.uncertainty
            || self.qualified_set.negative_order != self.negative_evidence
            || self.qualified_set.selected_digests.len() != self.selected_order.len()
            || self.qualified_set.tool_mode
                != if self.dry_run {
                    "dry_run"
                } else {
                    "bounded_invocation"
                }
            || self.qualified_set.evidence_state
                != if self.disposition == ThroughputResearchCopilotDisposition::Completed {
                    EvidenceState::Supported
                } else {
                    EvidenceState::Unknown
                }
            || self.qualified_set.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput qualified evidence set is not bound to the receipt".into(),
            ));
        }
        for digest in &self.qualified_set.selected_digests {
            validate_digest("qualified_set.selected_digest", digest)?;
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .chain(self.overflow_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.qualified_set.selected_order != self.selected_order
            || self.qualified_set.overflow_order != self.overflow_order
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.capability_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("throughput receipt digest", digest)?;
        }
        let approval_missing = !self.dry_run
            && (!self.approval_granted
                || self.approval_reference.is_none()
                || self
                    .approval_reference
                    .as_deref()
                    .is_some_and(|reference| reference.trim().is_empty()));
        let should_block = !self.policy_allow
            || !self.protected_closure
            || !self.raw_data_local
            || approval_missing;
        if (self.disposition == ThroughputResearchCopilotDisposition::Blocked) != should_block {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput disposition does not match its global release gates".into(),
            ));
        }
        if self.disposition == ThroughputResearchCopilotDisposition::Blocked
            && !self.selected_order.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "blocked throughput copilot cannot retain selected evidence".into(),
            ));
        }
        if self.disposition == ThroughputResearchCopilotDisposition::Completed
            && (!self.unresolved_order.is_empty()
                || !self.denied_order.is_empty()
                || !self.overflow_order.is_empty())
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "completed throughput copilot cannot retain unresolved, denied, or overflow states"
                    .into(),
            ));
        }
        if matches!(
            self.disposition,
            ThroughputResearchCopilotDisposition::Unknown
                | ThroughputResearchCopilotDisposition::Blocked
        ) && !self.selected_order.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "unknown or blocked throughput copilot cannot retain selected evidence".into(),
            ));
        }
        let expected_tool_receipts =
            if self.disposition == ThroughputResearchCopilotDisposition::Blocked {
                vec![format!("tool:{}:denied", self.requested_tool)]
            } else if self.dry_run {
                vec![format!("tool:{}:dry-run", self.requested_tool)]
            } else {
                vec![format!(
                    "tool:{}:bounded-call:1/{}",
                    self.requested_tool, self.max_tool_calls
                )]
            };
        if self.tool_receipts != expected_tool_receipts {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput tool receipt does not match mode or disposition".into(),
            ));
        }
        let expected_effect = if self.disposition == ThroughputResearchCopilotDisposition::Blocked {
            "block:unsafe-release".to_string()
        } else if self.dry_run {
            format!("dry-run:bounded-tool:{}", self.agent_id)
        } else {
            format!("invoke:declared-tool:{}", self.agent_id)
        };
        if self.effect_receipts != vec![expected_effect] {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput effect receipt does not match mode or disposition".into(),
            ));
        }
        let expected_queue = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "capacity": self.capacity,
            "candidate_order": self.candidate_order,
            "checkpoint_seq": self.checkpoint_seq,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.queue_digest != expected_queue {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput queue digest does not match checkpointed candidates".into(),
            ));
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput checkpoint digest does not match replay identity".into(),
            ));
        }
        let expected_capability = ContentHash::of_value(&json!({
            "agent_id": self.agent_id,
            "declared_tools": self.declared_tools,
            "requested_tool": self.requested_tool,
            "max_tool_calls": self.max_tool_calls,
            "dry_run": self.dry_run,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.capability_digest != expected_capability {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput capability digest does not match declared tools".into(),
            ));
        }
        let expected_evidence = ContentHash::of_value(&json!({
            "min_relevance_score": self.min_relevance_score,
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
            "overflow_order": self.overflow_order,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.evidence_digest != expected_evidence {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput evidence digest does not match queue states".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "agent_id": self.agent_id,
            "replay_identity": self.replay_identity,
            "queue_digest": self.queue_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "capability_digest": self.capability_digest,
            "evidence_digest": self.evidence_digest,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.provenance_digest != expected_provenance {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput provenance digest does not match receipt identity".into(),
            ));
        }
        let expected_run = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "dry_run": self.dry_run,
            "approval_reference": self.approval_reference,
            "tool_receipts": self.tool_receipts,
            "provenance_digest": self.provenance_digest,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.run_digest != expected_run {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput run digest does not match approval and tool receipts".into(),
            ));
        }
        if self.artifact.artifact_id != self.qualified_set.set_id
            || self.artifact.content_type != "application/vnd.aurora.qualified-evidence-set3+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(
                ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(
                    "throughput artifact is not bound to the qualified evidence set".into(),
                ),
            );
        }
        self.artifact
            .verify_payload(&serde_json::to_value(&self.qualified_set).map_err(|error| {
                ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
            })?)
            .map_err(|error| {
                ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
            })?;
        self.artifact.validate_metadata().map_err(|error| {
            ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.input_digest != copilot_input_digest(&self.input)? {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput copilot retained input digest mismatch".into(),
            ));
        }
        let expected = build_throughput_evidence_surveillance_research_copilot(&self.input)?;
        if self != &expected {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput copilot receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn throughput_evidence_surveillance_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: [
            "consortium administrator".into(),
            "MCP host".into(),
            "queue schema steward".into(),
        ]
        .into(),
        behavior: "runs bounded EvidenceFeed3 batches with checkpointed queue identity, explicit overflow, omission, negative evidence, and signed tool effects".into(),
        value: "preserves throughput evidence states while keeping declared-tool effects local and replayable".into(),
        inputs: vec![TypedPort {
            name: "evidence_feed".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "qualified_evidence_set".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
        ]
        .into(),
        permissions: [
            "invoke:declared-tools".into(),
            "read:local-evidence".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: Vec::new(),
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn run_throughput_evidence_surveillance_research_copilot(
    request: &ThroughputEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    ThroughputEvidenceSurveillanceResearchCopilotReceipt,
    ThroughputEvidenceSurveillanceResearchCopilotError,
> {
    let receipt = build_throughput_evidence_surveillance_research_copilot(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_throughput_evidence_surveillance_research_copilot(
    request: &ThroughputEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    ThroughputEvidenceSurveillanceResearchCopilotReceipt,
    ThroughputEvidenceSurveillanceResearchCopilotError,
> {
    if request.request_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.capacity == 0
        || request.max_tool_calls == 0
        || request.declared_tools.is_empty()
        || request.requested_tool.trim().is_empty()
        || !request
            .declared_tools
            .iter()
            .any(|tool| tool == &request.requested_tool)
        || request.observations.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            "identity, checkpoint, capacity, tools, observations, locality, or boundary is invalid"
                .into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("agent_id", &request.agent_id)?;
    validate_text("batch_id", &request.batch_id)?;
    validate_text("requested_tool", &request.requested_tool)?;
    validate_text("boundary", &request.boundary)?;
    if request.capacity > MAX_ITEMS
        || request.declared_tools.len() > MAX_ITEMS
        || request.observations.len() > MAX_ITEMS
    {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            "throughput capacity, tool, or observation count exceeds its bound".into(),
        ));
    }
    validate_unique_strings("declared_tools", &request.declared_tools)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    if let Some(reference) = &request.approval_reference {
        validate_text("approval_reference", reference)?;
    }
    let mut declared_tools = request.declared_tools.clone();
    declared_tools.sort();
    if !declared_tools.contains(&request.requested_tool) {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            "requested tool must be declared exactly once".into(),
        ));
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|a, b| {
        a.sequence
            .cmp(&b.sequence)
            .then_with(|| a.source_id.cmp(&b.source_id))
    });
    let mut source_ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    for item in &observations {
        validate_text("observation.source_id", &item.source_id)?;
        if let Some(digest) = &item.digest {
            validate_digest("observation.digest", digest)?;
        }
        if !source_ids.insert(item.source_id.clone()) {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "observation ids must be unique".into(),
            ));
        }
        if !sequences.insert(item.sequence) {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "queue sequence values must be unique".into(),
            ));
        }
    }
    let candidate_order = source_ids.iter().cloned().collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut selected_digests = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut overflow = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let approval_missing = !request.dry_run
        && (!request.approval_granted
            || request.approval_reference.is_none()
            || request
                .approval_reference
                .as_deref()
                .is_some_and(|reference| reference.trim().is_empty()));
    let global_release_blocked = !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || approval_missing;
    for (index, item) in observations.iter().enumerate() {
        if global_release_blocked {
            denied.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:global-release-gate", item.source_id));
        } else if index >= request.capacity {
            overflow.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:capacity-overflow", item.source_id));
            continue;
        } else if !request.policy_allow || !request.protected_closure {
            denied.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:policy-or-closure", item.source_id));
        } else if item.availability != EvidenceAvailability::Available {
            unresolved.insert(item.source_id.clone());
            omissions.insert(format!(
                "source:{}:availability-{:?}",
                item.source_id, item.availability
            ));
        } else if item.relevance_score < request.min_relevance_score {
            unresolved.insert(item.source_id.clone());
            uncertainty.insert(format!(
                "source:{}:relevance-below-threshold",
                item.source_id
            ));
        } else if item.digest.is_none() {
            unresolved.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:content-digest-missing", item.source_id));
        } else if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(item.source_id.clone());
            uncertainty.insert(format!("source:{}:unknown-not-asserted", item.source_id));
        } else if item.evidence_state == EvidenceState::Contradicted {
            denied.insert(item.source_id.clone());
            negative.insert(format!("source:{}:contradicted", item.source_id));
        } else {
            if let Some(digest) = item.digest.clone() {
                selected.insert(item.source_id.clone());
                selected_digests.insert(item.source_id.clone(), digest);
                if item.negative_result {
                    negative.insert(format!("source:{}:negative-result", item.source_id));
                }
            } else {
                unresolved.insert(item.source_id.clone());
                omissions.insert(format!("source:{}:content-digest-missing", item.source_id));
            }
        }
    }
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if approval_missing {
        omissions.insert("control:signed-approval-required".into());
    }
    let disposition = if global_release_blocked {
        ThroughputResearchCopilotDisposition::Blocked
    } else if selected.is_empty() {
        ThroughputResearchCopilotDisposition::Unknown
    } else if !unresolved.is_empty() || !denied.is_empty() || !overflow.is_empty() {
        ThroughputResearchCopilotDisposition::Partial
    } else {
        ThroughputResearchCopilotDisposition::Completed
    };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let overflow_order = overflow.iter().cloned().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let tool_receipts = if disposition == ThroughputResearchCopilotDisposition::Blocked {
        vec![format!("tool:{}:denied", request.requested_tool)]
    } else if request.dry_run {
        vec![format!("tool:{}:dry-run", request.requested_tool)]
    } else {
        vec![format!(
            "tool:{}:bounded-call:1/{}",
            request.requested_tool, request.max_tool_calls
        )]
    };
    let queue_digest = ContentHash::of_value(&json!({
        "batch_id": request.batch_id,
        "capacity": request.capacity,
        "candidate_order": candidate_order,
        "checkpoint_seq": request.checkpoint_seq,
    }))
    .map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let checkpoint_digest = ContentHash::of_value(&json!({
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let capability_digest = ContentHash::of_value(&json!({
        "agent_id": request.agent_id,
        "declared_tools": declared_tools,
        "requested_tool": request.requested_tool,
        "max_tool_calls": request.max_tool_calls,
        "dry_run": request.dry_run,
    }))
    .map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let evidence_digest = ContentHash::of_value(&json!({
        "min_relevance_score": request.min_relevance_score,
        "selected_order": selected_order,
        "unresolved_order": unresolved_order,
        "denied_order": denied_order,
        "overflow_order": overflow_order,
    }))
    .map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let provenance_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "agent_id": request.agent_id,
        "replay_identity": request.replay_identity,
        "queue_digest": queue_digest,
        "checkpoint_digest": checkpoint_digest,
        "capability_digest": capability_digest,
        "evidence_digest": evidence_digest,
    }))
    .map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let run_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "dry_run": request.dry_run,
        "approval_reference": request.approval_reference,
        "tool_receipts": tool_receipts,
        "provenance_digest": provenance_digest,
    }))
    .map_err(|error| {
        ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let qualified_set = ThroughputCopilotQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!(
            "qualified-evidence-throughput-copilot:{}",
            request.request_id
        ),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        selected_order: selected_order.clone(),
        selected_digests: selected_order
            .iter()
            .filter_map(|id| selected_digests.get(id).cloned())
            .collect(),
        overflow_order: overflow_order.clone(),
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_order: negative_evidence.clone(),
        evidence_state: if disposition == ThroughputResearchCopilotDisposition::Completed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        },
        tool_mode: if request.dry_run {
            "dry_run"
        } else {
            "bounded_invocation"
        }
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let artifact_payload = serde_json::to_value(&qualified_set)
        .map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-evidence-set3+json",
        &artifact_payload,
        vec![],
        vec![],
    )
    .map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let canonical_request =
        canonical_throughput_evidence_surveillance_research_copilot_request(request);
    let receipt = ThroughputEvidenceSurveillanceResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: copilot_input_digest(request)?,
        request_id: request.request_id.clone(),
        agent_id: request.agent_id.clone(),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        capacity: request.capacity,
        declared_tools,
        requested_tool: request.requested_tool.clone(),
        max_tool_calls: request.max_tool_calls,
        dry_run: request.dry_run,
        approval_granted: request.approval_granted,
        approval_reference: request.approval_reference.clone(),
        min_relevance_score: request.min_relevance_score,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        candidate_order,
        selected_order,
        unresolved_order,
        denied_order,
        overflow_order,
        replay_identity: request.replay_identity.clone(),
        queue_digest,
        checkpoint_digest,
        capability_digest,
        evidence_digest,
        provenance_digest,
        run_digest,
        omissions,
        uncertainty,
        negative_evidence,
        tool_receipts,
        effect_receipts: if disposition == ThroughputResearchCopilotDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else if request.dry_run {
            vec![format!("dry-run:bounded-tool:{}", request.agent_id)]
        } else {
            vec![format!("invoke:declared-tool:{}", request.agent_id)]
        },
        qualified_set,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(dry_run: bool) -> ThroughputEvidenceSurveillanceResearchCopilotRequest {
        ThroughputEvidenceSurveillanceResearchCopilotRequest {
            request_id: "f11-test".into(),
            agent_id: "agent".into(),
            batch_id: "batch-1".into(),
            checkpoint_seq: 1,
            capacity: 3,
            declared_tools: vec!["evidence.query".into()],
            requested_tool: "evidence.query".into(),
            max_tool_calls: 2,
            dry_run,
            approval_reference: (!dry_run).then(|| "approval-1".into()),
            approval_granted: !dry_run,
            observations: (0..3)
                .map(|sequence| ThroughputCopilotEvidenceObservation {
                    source_id: format!("source-{sequence}"),
                    sequence,
                    digest: Some(ContentHash::of_bytes(&[sequence as u8])),
                    availability: EvidenceAvailability::Available,
                    evidence_state: EvidenceState::Supported,
                    relevance_score: 90,
                    negative_result: false,
                })
                .collect(),
            min_relevance_score: 80,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: ContentHash::of_bytes(&[7]),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            throughput_evidence_surveillance_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn complete_batch_dry_runs() {
        let receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(true)).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputResearchCopilotDisposition::Completed
        );
    }
    #[test]
    fn capacity_overflow_is_explicit() {
        let mut input = request(true);
        input.capacity = 2;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(receipt.overflow_order, vec!["source-2"]);
    }
    #[test]
    fn approval_required_for_invocation() {
        let mut input = request(false);
        input.approval_granted = false;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputResearchCopilotDisposition::Blocked
        );
        assert!(receipt.selected_order.is_empty());
    }
    #[test]
    fn approved_invocation_is_declared() {
        let receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(false)).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tool:"));
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut input = request(true);
        input.observations[1].evidence_state = EvidenceState::Unknown;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn negative_result_is_retained() {
        let mut input = request(true);
        input.observations[0].negative_result = true;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn policy_blocks() {
        let mut input = request(true);
        input.policy_allow = false;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_is_stable() {
        let input = request(true);
        let first = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        let second = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(first.run_digest, second.run_digest);
    }

    #[test]
    fn reordered_inputs_share_the_same_retained_input_identity() {
        let mut reordered = request(true);
        reordered.declared_tools.reverse();
        reordered.observations.reverse();
        let first = run_throughput_evidence_surveillance_research_copilot(&request(true)).unwrap();
        let second = run_throughput_evidence_surveillance_research_copilot(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.run_digest, second.run_digest);
    }

    #[test]
    fn global_policy_block_cannot_retain_selected_evidence() {
        let mut input = request(true);
        input.policy_allow = false;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert!(receipt.selected_order.is_empty());
        assert!(receipt.overflow_order.is_empty());
        assert_eq!(
            receipt.denied_order,
            vec!["source-0", "source-1", "source-2"]
        );
    }

    #[test]
    fn duplicate_queue_sequence_is_rejected() {
        let mut input = request(true);
        input.observations[1].sequence = input.observations[0].sequence;
        assert!(run_throughput_evidence_surveillance_research_copilot(&input).is_err());
    }

    #[test]
    fn tampered_checkpoint_digest_is_rejected() {
        let mut receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(true)).unwrap();
        receipt.checkpoint_digest = ContentHash::of_bytes(b"tampered-checkpoint");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn tampered_capability_digest_is_rejected() {
        let mut receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(true)).unwrap();
        receipt.capability_digest = ContentHash::of_bytes(b"tampered-capability");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn receipt_rejects_tampered_retained_queue_observation() {
        let mut receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(true)).unwrap();
        receipt.input.observations[0].sequence = 99;
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
