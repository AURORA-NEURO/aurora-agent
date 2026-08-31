//! Federated continual evidence-surveillance research copilot.
//!
//! Atlas feature `AFA-adapter-P01-F12`.  Only signed, permitted aggregate
//! contributions cross an institution boundary; raw observations remain local.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceState, ResearchSurface,
    TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F12";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCopilotEvidenceContribution {
    pub peer_id: String,
    pub institution_id: String,
    pub source_id: String,
    pub semantic_profile: String,
    pub artifact_kind: String,
    pub digest: Option<ContentHash>,
    pub signed: bool,
    pub permitted_artifact: bool,
    pub aggregate_only: bool,
    pub evidence_state: EvidenceState,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
    pub request_id: String,
    pub agent_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_reference: Option<String>,
    pub approval_granted: bool,
    pub contributions: Vec<FederatedCopilotEvidenceContribution>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualResearchCopilotDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCopilotQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub peer_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub aggregate_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub tool_mode: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub agent_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_granted: bool,
    pub approval_reference: Option<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: FederatedContinualResearchCopilotDisposition,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub capability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub run_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: FederatedCopilotQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedContinualEvidenceSurveillanceResearchCopilotError {
    #[error("invalid federated continual copilot request: {0}")]
    Invalid(String),
    #[error("federated continual copilot artifact failed: {0}")]
    Artifact(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchCopilotError> {
    if value.is_empty() || value.trim() != value {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(format!(
                "{field} must be non-empty and trimmed"
            )),
        );
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(format!(
                "{field} is outside its bounded text contract"
            )),
        );
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchCopilotError> {
    if values.len() > MAX_ITEMS {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(format!(
                "{field} exceeds its item bound"
            )),
        );
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(format!(
                    "{field} contains duplicate values"
                )),
            );
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchCopilotError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(format!(
                "{field} ordering is not canonical"
            )),
        );
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchCopilotError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(format!(
                "{field} must be a 64-character hex digest"
            )),
        );
    }
    Ok(())
}

pub(crate) fn canonical_federated_continual_evidence_surveillance_research_copilot_request(
    request: &FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
) -> FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
    let mut canonical = request.clone();
    canonical.allowed_artifacts.sort();
    canonical.declared_tools.sort();
    canonical.contributions.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then_with(|| left.peer_id.cmp(&right.peer_id))
    });
    canonical
}

fn copilot_input_digest(
    request: &FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
) -> Result<ContentHash, FederatedContinualEvidenceSurveillanceResearchCopilotError> {
    let canonical =
        canonical_federated_continual_evidence_surveillance_research_copilot_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value).map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })
}

impl FederatedContinualEvidenceSurveillanceResearchCopilotReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualEvidenceSurveillanceResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.allowed_artifacts.is_empty()
            || self.min_peer_quorum == 0
            || self.declared_tools.is_empty()
            || self.requested_tool.trim().is_empty()
            || self.max_tool_calls == 0
            || self.qualified_set.federation_id != self.federation_id
            || self.qualified_set.purpose != self.purpose
            || self.qualified_set.semantic_profile != self.semantic_profile
        {
            return Err(FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid("federation identity, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("agent_id", &self.agent_id)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("purpose", &self.purpose)?;
        validate_text("endpoint", &self.endpoint)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_text("requested_tool", &self.requested_tool)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("allowed_artifacts", &self.allowed_artifacts)?;
        validate_sorted_strings("declared_tools", &self.declared_tools)?;
        validate_sorted_strings("peer_order", &self.peer_order)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("aggregate_order", &self.aggregate_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("tool_receipts", &self.tool_receipts)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        validate_sorted_strings("qualified_set.peer_order", &self.qualified_set.peer_order)?;
        validate_sorted_strings(
            "qualified_set.selected_order",
            &self.qualified_set.selected_order,
        )?;
        validate_sorted_strings(
            "qualified_set.aggregate_order",
            &self.qualified_set.aggregate_order,
        )?;
        validate_sorted_strings("qualified_set.omissions", &self.qualified_set.omissions)?;
        validate_sorted_strings("qualified_set.uncertainty", &self.qualified_set.uncertainty)?;
        validate_sorted_strings(
            "qualified_set.negative_order",
            &self.qualified_set.negative_order,
        )?;
        if !self.declared_tools.contains(&self.requested_tool) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "requested tool must be declared exactly once".into(),
                ),
            );
        }
        if let Some(reference) = &self.approval_reference {
            validate_text("approval_reference", reference)?;
        }
        if self.qualified_set.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.qualified_set.set_id
                != format!(
                    "qualified-evidence-federated-continual-copilot:{}",
                    self.request_id
                )
            || self.qualified_set.peer_order != self.peer_order
            || self.qualified_set.selected_order != self.selected_order
            || self.qualified_set.aggregate_order != self.aggregate_order
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
                != if self.disposition == FederatedContinualResearchCopilotDisposition::Completed {
                    EvidenceState::Supported
                } else {
                    EvidenceState::Unknown
                }
            || self.qualified_set.boundary != PRECLINICAL_BOUNDARY
            || self.aggregate_order != self.selected_order
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated qualified evidence set is not bound to the receipt".into(),
                ),
            );
        }
        for digest in &self.qualified_set.selected_digests {
            validate_digest("qualified_set.selected_digest", digest)?;
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.qualified_set.selected_order != self.selected_order
            || self.qualified_set.aggregate_order != self.aggregate_order
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated states do not partition candidates".into(),
                ),
            );
        }
        for digest in [
            &self.replay_identity,
            &self.federation_digest,
            &self.envelope_digest,
            &self.capability_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("federated receipt digest", digest)?;
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
            || self.peer_order.len() < self.min_peer_quorum
            || approval_missing;
        if (self.disposition == FederatedContinualResearchCopilotDisposition::Blocked)
            != should_block
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated disposition does not match its global release gates".into(),
                ),
            );
        }
        if self.disposition == FederatedContinualResearchCopilotDisposition::Blocked
            && !self.selected_order.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "blocked federation cannot retain selected evidence".into(),
                ),
            );
        }
        if self.disposition == FederatedContinualResearchCopilotDisposition::Completed
            && (!self.unresolved_order.is_empty() || !self.denied_order.is_empty())
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "completed federation cannot retain unresolved or denied evidence".into(),
                ),
            );
        }
        if self.disposition == FederatedContinualResearchCopilotDisposition::Unknown
            && !self.selected_order.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "unknown federation cannot retain selected evidence".into(),
                ),
            );
        }
        let expected_tool_receipts =
            if self.disposition == FederatedContinualResearchCopilotDisposition::Blocked {
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
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated tool receipt does not match mode or disposition".into(),
                ),
            );
        }
        let expected_effect =
            if self.disposition == FederatedContinualResearchCopilotDisposition::Blocked {
                "block:unsafe-release".to_string()
            } else if self.dry_run {
                format!("dry-run:bounded-tool:{}", self.agent_id)
            } else {
                format!("invoke:declared-tool:{}", self.agent_id)
            };
        if self.effect_receipts != vec![expected_effect] {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated effect receipt does not match mode or disposition".into(),
                ),
            );
        }
        let expected_federation = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "peer_order": self.peer_order,
            "min_peer_quorum": self.min_peer_quorum,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.federation_digest != expected_federation {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated identity digest does not match federation scope".into(),
                ),
            );
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "allowed_artifacts": self.allowed_artifacts,
            "semantic_profile": self.semantic_profile,
            "aggregate_order": self.aggregate_order,
            "raw_data_local": self.raw_data_local,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.envelope_digest != expected_envelope {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated envelope digest does not match artifact and policy scope".into(),
                ),
            );
        }
        let expected_capability = ContentHash::of_value(&json!({
            "agent_id": self.agent_id,
            "declared_tools": self.declared_tools,
            "requested_tool": self.requested_tool,
            "max_tool_calls": self.max_tool_calls,
            "dry_run": self.dry_run,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.capability_digest != expected_capability {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated capability digest does not match declared tools".into(),
                ),
            );
        }
        let expected_evidence = ContentHash::of_value(&json!({
            "candidate_order": self.candidate_order,
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.evidence_digest != expected_evidence {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated evidence digest does not match state partitions".into(),
                ),
            );
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "agent_id": self.agent_id,
            "replay_identity": self.replay_identity,
            "federation_digest": self.federation_digest,
            "envelope_digest": self.envelope_digest,
            "capability_digest": self.capability_digest,
            "evidence_digest": self.evidence_digest,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.provenance_digest != expected_provenance {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated provenance digest does not match receipt identity".into(),
                ),
            );
        }
        let expected_run = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "dry_run": self.dry_run,
            "approval_reference": self.approval_reference,
            "tool_receipts": self.tool_receipts,
            "provenance_digest": self.provenance_digest,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.run_digest != expected_run {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated run digest does not match approval and tool receipts".into(),
                ),
            );
        }
        if self.artifact.artifact_id != self.qualified_set.set_id
            || self.artifact.content_type != "application/vnd.aurora.qualified-evidence-set3+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(
                    "federated artifact is not bound to the qualified evidence set".into(),
                ),
            );
        }
        self.artifact
            .verify_payload(&serde_json::to_value(&self.qualified_set).map_err(|error| {
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(
                    error.to_string(),
                )
            })?)
            .map_err(|error| {
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(
                    error.to_string(),
                )
            })?;
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })?;
        if self.input_digest != copilot_input_digest(&self.input)? {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated copilot retained input digest mismatch".into(),
                ),
            );
        }
        let expected =
            build_federated_continual_evidence_surveillance_research_copilot(&self.input)?;
        if self != &expected {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated copilot receipt does not match its retained input".into(),
                ),
            );
        }
        Ok(())
    }
}

pub fn federated_continual_evidence_surveillance_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: [
            "integration engineer".into(),
            "MCP host".into(),
            "consortium steward".into(),
        ]
        .into(),
        behavior: "qualifies signed aggregate-only evidence contributions under purpose, signer, quorum, locality, and policy gates".into(),
        value: "preserves federated evidence provenance while preventing raw-data exchange and unsafe release".into(),
        inputs: vec![TypedPort {
            name: "federated_evidence_envelope".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "qualified_aggregate_evidence_set".into(),
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
            "exchange:aggregate-evidence".into(),
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

pub fn run_federated_continual_evidence_surveillance_research_copilot(
    request: &FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceResearchCopilotReceipt,
    FederatedContinualEvidenceSurveillanceResearchCopilotError,
> {
    let receipt = build_federated_continual_evidence_surveillance_research_copilot(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_federated_continual_evidence_surveillance_research_copilot(
    request: &FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceResearchCopilotReceipt,
    FederatedContinualEvidenceSurveillanceResearchCopilotError,
> {
    let canonical_request =
        canonical_federated_continual_evidence_surveillance_research_copilot_request(request);
    let request = &canonical_request;
    if request.request_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.min_peer_quorum == 0
        || request.max_tool_calls == 0
        || request.declared_tools.is_empty()
        || !request
            .declared_tools
            .iter()
            .any(|tool| tool == &request.requested_tool)
        || request.contributions.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid("federation identity, quorum, tools, contributions, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("agent_id", &request.agent_id)?;
    validate_text("federation_id", &request.federation_id)?;
    validate_text("purpose", &request.purpose)?;
    validate_text("endpoint", &request.endpoint)?;
    validate_text("semantic_profile", &request.semantic_profile)?;
    validate_text("requested_tool", &request.requested_tool)?;
    validate_text("boundary", &request.boundary)?;
    if request.allowed_artifacts.len() > MAX_ITEMS
        || request.declared_tools.len() > MAX_ITEMS
        || request.contributions.len() > MAX_ITEMS
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                "federated artifact, tool, or contribution count exceeds its bound".into(),
            ),
        );
    }
    validate_unique_strings("allowed_artifacts", &request.allowed_artifacts)?;
    validate_unique_strings("declared_tools", &request.declared_tools)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    if let Some(reference) = &request.approval_reference {
        validate_text("approval_reference", reference)?;
    }
    let mut allowed_artifacts = request.allowed_artifacts.clone();
    allowed_artifacts.sort();
    let mut declared_tools = request.declared_tools.clone();
    declared_tools.sort();
    if !declared_tools.contains(&request.requested_tool) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                "requested tool must be declared exactly once".into(),
            ),
        );
    }
    let mut contributions = request.contributions.clone();
    contributions.sort_by(|a, b| {
        a.source_id
            .cmp(&b.source_id)
            .then_with(|| a.peer_id.cmp(&b.peer_id))
    });
    let mut peers = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for item in &contributions {
        validate_text("contribution.peer_id", &item.peer_id)?;
        validate_text("contribution.institution_id", &item.institution_id)?;
        validate_text("contribution.source_id", &item.source_id)?;
        validate_text("contribution.semantic_profile", &item.semantic_profile)?;
        validate_text("contribution.artifact_kind", &item.artifact_kind)?;
        if let Some(digest) = &item.digest {
            validate_digest("contribution.digest", digest)?;
        }
        if !peers.insert(item.peer_id.clone()) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "each federated peer may contribute only once per receipt".into(),
                ),
            );
        }
        if !sources.insert(item.source_id.clone()) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated source identities must be unique".into(),
                ),
            );
        }
    }
    let peer_order = contributions
        .iter()
        .map(|item| item.peer_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let candidate_order = contributions
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut digests = std::collections::BTreeMap::new();
    let approval_missing = !request.dry_run
        && (!request.approval_granted
            || request
                .approval_reference
                .as_deref()
                .is_some_and(|reference| reference.trim().is_empty())
            || request.approval_reference.is_none());
    let quorum_incomplete = peer_order.len() < request.min_peer_quorum;
    let global_release_blocked = !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || quorum_incomplete
        || approval_missing;
    for item in &contributions {
        if global_release_blocked {
            denied.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:global-release-gate", item.source_id));
        } else if item.semantic_profile != request.semantic_profile {
            denied.insert(item.source_id.clone());
            omissions.insert(format!(
                "source:{}:semantic-profile-mismatch",
                item.source_id
            ));
        } else if !item.signed
            || !item.permitted_artifact
            || !item.aggregate_only
            || !allowed_artifacts
                .iter()
                .any(|kind| kind == &item.artifact_kind)
        {
            denied.insert(item.source_id.clone());
            omissions.insert(format!(
                "source:{}:signer-permission-or-artifact-gate",
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
                aggregate.insert(item.source_id.clone());
                digests.insert(item.source_id.clone(), digest);
                if item.negative_result {
                    negative.insert(format!("source:{}:negative-result", item.source_id));
                }
            } else {
                unresolved.insert(item.source_id.clone());
                omissions.insert(format!("source:{}:content-digest-missing", item.source_id));
            }
        }
    }
    if quorum_incomplete {
        omissions.insert("control:peer-quorum-incomplete".into());
    }
    if approval_missing {
        omissions.insert("control:signed-approval-required".into());
    }
    let disposition = if global_release_blocked {
        FederatedContinualResearchCopilotDisposition::Blocked
    } else if selected.is_empty() {
        FederatedContinualResearchCopilotDisposition::Unknown
    } else if !unresolved.is_empty() || !denied.is_empty() {
        FederatedContinualResearchCopilotDisposition::Partial
    } else {
        FederatedContinualResearchCopilotDisposition::Completed
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let denied_order = denied.into_iter().collect::<Vec<_>>();
    let aggregate_order = aggregate.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let tool_receipts = if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
        vec![format!("tool:{}:denied", request.requested_tool)]
    } else if request.dry_run {
        vec![format!("tool:{}:dry-run", request.requested_tool)]
    } else {
        vec![format!(
            "tool:{}:bounded-call:1/{}",
            request.requested_tool, request.max_tool_calls
        )]
    };
    let federation_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "endpoint": request.endpoint,
        "peer_order": peer_order,
        "min_peer_quorum": request.min_peer_quorum,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let envelope_digest = ContentHash::of_value(&json!({
        "allowed_artifacts": allowed_artifacts,
        "semantic_profile": request.semantic_profile,
        "aggregate_order": aggregate_order,
        "raw_data_local": request.raw_data_local,
        "policy_allow": request.policy_allow,
        "protected_closure": request.protected_closure,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let capability_digest = ContentHash::of_value(&json!({
        "agent_id": request.agent_id,
        "declared_tools": declared_tools,
        "requested_tool": request.requested_tool,
        "max_tool_calls": request.max_tool_calls,
        "dry_run": request.dry_run,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let evidence_digest = ContentHash::of_value(&json!({
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "unresolved_order": unresolved_order,
        "denied_order": denied_order,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let provenance_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "agent_id": request.agent_id,
        "replay_identity": request.replay_identity,
        "federation_digest": federation_digest,
        "envelope_digest": envelope_digest,
        "capability_digest": capability_digest,
        "evidence_digest": evidence_digest,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let run_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "dry_run": request.dry_run,
        "approval_reference": request.approval_reference,
        "tool_receipts": tool_receipts,
        "provenance_digest": provenance_digest,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let qualified_set = FederatedCopilotQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!(
            "qualified-evidence-federated-continual-copilot:{}",
            request.request_id
        ),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        peer_order: peer_order.clone(),
        selected_order: selected_order.clone(),
        selected_digests: selected_order
            .iter()
            .filter_map(|id| digests.get(id).cloned())
            .collect(),
        aggregate_order: aggregate_order.clone(),
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_order: negative_evidence.clone(),
        evidence_state: if disposition == FederatedContinualResearchCopilotDisposition::Completed {
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
    let payload = serde_json::to_value(&qualified_set).map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-evidence-set3+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string())
    })?;
    let input_digest = copilot_input_digest(request)?;
    let receipt = FederatedContinualEvidenceSurveillanceResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.request_id.clone(),
        agent_id: request.agent_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        semantic_profile: request.semantic_profile.clone(),
        allowed_artifacts,
        min_peer_quorum: request.min_peer_quorum,
        declared_tools,
        requested_tool: request.requested_tool.clone(),
        max_tool_calls: request.max_tool_calls,
        dry_run: request.dry_run,
        approval_granted: request.approval_granted,
        approval_reference: request.approval_reference.clone(),
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        peer_order,
        candidate_order,
        selected_order,
        unresolved_order,
        denied_order,
        aggregate_order,
        replay_identity: request.replay_identity.clone(),
        federation_digest,
        envelope_digest,
        capability_digest,
        evidence_digest,
        provenance_digest,
        run_digest,
        omissions,
        uncertainty,
        negative_evidence,
        tool_receipts,
        effect_receipts: if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
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
    fn request(dry_run: bool) -> FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
        FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
            request_id: "f12-test".into(),
            agent_id: "agent".into(),
            federation_id: "fed-1".into(),
            purpose: "preclinical-evidence".into(),
            endpoint: "local://aggregate".into(),
            semantic_profile: "profile-v1".into(),
            allowed_artifacts: vec!["qualified-evidence".into()],
            min_peer_quorum: 2,
            declared_tools: vec!["evidence.aggregate".into()],
            requested_tool: "evidence.aggregate".into(),
            max_tool_calls: 2,
            dry_run,
            approval_reference: (!dry_run).then(|| "approval-1".into()),
            approval_granted: !dry_run,
            contributions: (0..2)
                .map(|i| FederatedCopilotEvidenceContribution {
                    peer_id: format!("peer-{i}"),
                    institution_id: format!("inst-{i}"),
                    source_id: format!("source-{i}"),
                    semantic_profile: "profile-v1".into(),
                    artifact_kind: "qualified-evidence".into(),
                    digest: Some(ContentHash::of_bytes(&[i as u8])),
                    signed: true,
                    permitted_artifact: true,
                    aggregate_only: true,
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                })
                .collect(),
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
            federated_continual_evidence_surveillance_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A2
        )
    }
    #[test]
    fn quorum_complete_dry_runs() {
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&request(true))
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Completed
        )
    }
    #[test]
    fn raw_export_gate_denies() {
        let mut i = request(true);
        i.contributions[0].aggregate_only = false;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Partial
        )
    }
    #[test]
    fn quorum_blocks() {
        let mut i = request(true);
        i.min_peer_quorum = 3;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Blocked
        )
    }
    #[test]
    fn approval_required() {
        let mut i = request(false);
        i.approval_granted = false;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Blocked
        )
    }
    #[test]
    fn unknown_not_asserted() {
        let mut i = request(true);
        i.contributions[0].evidence_state = EvidenceState::Unknown;
        let r = run_federated_continual_evidence_surveillance_research_copilot(&i).unwrap();
        assert!(!r.uncertainty.is_empty())
    }
    #[test]
    fn contradiction_is_negative() {
        let mut i = request(true);
        i.contributions[0].evidence_state = EvidenceState::Contradicted;
        let r = run_federated_continual_evidence_surveillance_research_copilot(&i).unwrap();
        assert!(!r.negative_evidence.is_empty())
    }
    #[test]
    fn policy_blocks() {
        let mut i = request(true);
        i.policy_allow = false;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn replay_stable() {
        let i = request(true);
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .run_digest,
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .run_digest
        )
    }

    #[test]
    fn reordered_contributions_and_declarations_have_stable_identity() {
        let mut reordered = request(true);
        reordered.allowed_artifacts.reverse();
        reordered.declared_tools.reverse();
        reordered.contributions.reverse();
        let first =
            run_federated_continual_evidence_surveillance_research_copilot(&request(true)).unwrap();
        let second =
            run_federated_continual_evidence_surveillance_research_copilot(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.run_digest, second.run_digest);
        assert_eq!(first, second);
    }

    #[test]
    fn global_quorum_block_cannot_retain_selected_evidence() {
        let mut i = request(true);
        i.min_peer_quorum = 3;
        let receipt = run_federated_continual_evidence_surveillance_research_copilot(&i).unwrap();
        assert!(receipt.selected_order.is_empty());
        assert!(receipt.aggregate_order.is_empty());
        assert_eq!(receipt.denied_order, vec!["source-0", "source-1"]);
    }

    #[test]
    fn duplicate_peer_contribution_is_rejected() {
        let mut i = request(true);
        i.contributions[1].peer_id = i.contributions[0].peer_id.clone();
        assert!(run_federated_continual_evidence_surveillance_research_copilot(&i).is_err());
    }

    #[test]
    fn missing_contribution_digest_is_unresolved() {
        let mut i = request(true);
        i.contributions[0].digest = None;
        let receipt = run_federated_continual_evidence_surveillance_research_copilot(&i).unwrap();
        assert!(receipt.unresolved_order.contains(&"source-0".to_string()));
    }

    #[test]
    fn tampered_envelope_digest_is_rejected() {
        let mut receipt =
            run_federated_continual_evidence_surveillance_research_copilot(&request(true)).unwrap();
        receipt.envelope_digest = ContentHash::of_bytes(b"tampered-envelope");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn tampered_capability_digest_is_rejected() {
        let mut receipt =
            run_federated_continual_evidence_surveillance_research_copilot(&request(true)).unwrap();
        receipt.capability_digest = ContentHash::of_bytes(b"tampered-capability");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn receipt_rejects_tampered_retained_contribution() {
        let mut receipt =
            run_federated_continual_evidence_surveillance_research_copilot(&request(true)).unwrap();
        receipt.input.contributions[0].artifact_kind = "tampered-artifact".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
