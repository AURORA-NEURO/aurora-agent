//! High-throughput retrieval researcher workbench.
//!
//! Atlas feature: `AFA-brain-P02-F19`. Queue, capacity, and checkpoint evidence are rendered
//! directly so operators can inspect overflow without mistaking it for completion.

use crate::retrieval_synthesis::SynthesisDisposition;
use crate::throughput_retrieval_synthesis::{
    synthesize_throughput_retrieval, ThroughputRetrievalQuery,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F19";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-research-workbench/1.0";
pub const VIEW_ORDER: [&str; 3] = [
    "view:throughput-queue",
    "view:capacity-frontier",
    "view:checkpoint-replay",
];
const WORKBENCH_CONTENT_TYPE: &str =
    "application/vnd.aurora.throughput-retrieval-workbench-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalWorkbenchRequest {
    pub request: ThroughputRetrievalQuery,
    pub workspace_id: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: SynthesisDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub workbench_digest: ContentHash,
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
pub enum ThroughputRetrievalWorkbenchError {
    #[error("invalid throughput retrieval workbench request: {0}")]
    Invalid(String),
    #[error("throughput retrieval workbench artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval workbench engine failed: {0}")]
    Engine(String),
}

impl ThroughputRetrievalWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.view_order.is_empty()
            || self.panel_order.is_empty()
            || self.action_receipts.is_empty()
            || self.candidate_order.is_empty()
            || self.checkpoint_seq == 0
            || self.budget_units == 0
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputRetrievalWorkbenchError::Invalid("throughput workbench identity, queue, checkpoint, views, panels, budget, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workspace_id, "workspace_id"),
            (&self.batch_id, "batch_id"),
            (&self.partition, "partition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.view_order != VIEW_ORDER {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "throughput workbench views are not in canonical order".into(),
            ));
        }
        validate_unique(&self.panel_order, "panel_order")?;
        for (values, field) in [
            (&self.action_receipts, "action_receipts"),
            (&self.candidate_order, "candidate_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for (values, field) in [
            (&self.ranked_order, "ranked_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "throughput workbench ranked order must contain every candidate exactly once"
                    .into(),
            ));
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self
                .ranked_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
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
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "throughput workbench candidate states must partition candidates".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.synthesis_digest,
            &self.workbench_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalWorkbenchError::Invalid(
                    "throughput workbench digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition != SynthesisDisposition::Blocked {
            vec![format!(
                "view:local-throughput-retrieval-artifacts:{}",
                self.workspace_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "throughput workbench effects do not match disposition".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "workbench:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "non-local throughput workbenches must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_workbench_digest = ContentHash::of_value(&json!({
            "workspace_id": self.workspace_id,
            "disposition": self.disposition,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "action_receipts": self.action_receipts,
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "queue_digest": self.queue_digest,
            "synthesis_digest": self.synthesis_digest,
            "checkpoint_seq": self.checkpoint_seq,
            "replay_identity": self.replay_identity,
            "budget_units": self.budget_units,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))?;
        if self.workbench_digest != expected_workbench_digest {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "throughput workbench digest is not bound to rendered state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-workbench:{}", self.workspace_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKBENCH_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(
                "throughput workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["throughput retrieval operator".into(), "platform reliability engineer".into()].into(), behavior: "renders local throughput retrieval queue, capacity frontier, and checkpoint replay views with deterministic read-only receipts".into(), value: "makes batch overflow, queue pressure, and checkpoint continuity inspectable without silent truncation".into(), inputs: vec![TypedPort { name: "throughput_retrieval_workbench_request".into(), schema: "ResearchWorkbenchSpec3@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_retrieval_workbench_receipt".into(), schema: "ThroughputRetrievalWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:local-throughput-retrieval-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_throughput_retrieval_workbench(
    request: &ThroughputRetrievalWorkbenchRequest,
) -> Result<ThroughputRetrievalWorkbenchReceipt, ThroughputRetrievalWorkbenchError> {
    validate_request(request)?;
    let synthesis = synthesize_throughput_retrieval(&request.request)
        .map_err(|error| ThroughputRetrievalWorkbenchError::Engine(error.to_string()))?;
    let view_order = request.requested_view_order.clone();
    let panel_order = request.requested_panel_order.clone();
    let action_receipts = [
        "action:render-throughput-queue",
        "action:render-capacity-frontier",
        "action:render-checkpoint-replay",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let mut action_receipts = action_receipts;
    action_receipts.sort();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && u64::from(request.budget_units)
            >= u64::try_from(action_receipts.len()).unwrap_or(u64::MAX)
        && synthesis.disposition != SynthesisDisposition::Blocked;
    if u64::from(request.budget_units) < u64::try_from(action_receipts.len()).unwrap_or(u64::MAX) {
        omissions.insert("workbench:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workbench:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workbench:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("workbench:raw-data-locality-failed".into());
    }
    let disposition = if actionable {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
    };
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| ThroughputRetrievalWorkbenchError::Engine(error.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({"workspace_id": request.workspace_id, "disposition": disposition, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis_digest, "checkpoint_seq": request.checkpoint_seq, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "raw_data_local": true})).map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))?;
    let effect_receipts = if actionable {
        vec![format!(
            "view:local-throughput-retrieval-artifacts:{}",
            request.workspace_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workspace_id": request.workspace_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "disposition": disposition, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "checkpoint_seq": request.checkpoint_seq, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis_digest, "workbench_digest": workbench_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-retrieval-workbench:{}",
            request.workspace_id
        ),
        WORKBENCH_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalWorkbenchError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        disposition,
        view_order,
        panel_order,
        action_receipts,
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        checkpoint_seq: request.checkpoint_seq,
        queue_digest: synthesis.queue_digest,
        synthesis_digest,
        workbench_digest,
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
    request: &ThroughputRetrievalWorkbenchRequest,
) -> Result<(), ThroughputRetrievalWorkbenchError> {
    let expected = VIEW_ORDER
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    for (value, field) in [
        (&request.workspace_id, "workspace_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.requested_view_order != expected
        || request.workspace_id.trim().is_empty()
        || request.requested_panel_order.is_empty()
        || request.checkpoint_seq == 0
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputRetrievalWorkbenchError::Invalid("throughput workbench identity, canonical views, panels, checkpoint, budget, replay, or boundary is incomplete".into()));
    }
    validate_unique(&request.requested_panel_order, "requested_panel_order")?;
    if request.replay_identity.as_str().len() != 64 {
        return Err(ThroughputRetrievalWorkbenchError::Invalid(
            "throughput workbench replay identity digest is invalid".into(),
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

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalWorkbenchError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalWorkbenchError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalWorkbenchError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalWorkbenchError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalWorkbenchError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalWorkbenchError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workspace_id": receipt.workspace_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "disposition": receipt.disposition,
        "view_order": receipt.view_order,
        "panel_order": receipt.panel_order,
        "action_receipts": receipt.action_receipts,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "checkpoint_seq": receipt.checkpoint_seq,
        "queue_digest": receipt.queue_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "workbench_digest": receipt.workbench_digest,
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
    fn request(state: EvidenceState) -> ThroughputRetrievalWorkbenchRequest {
        let candidates = (0..3)
            .map(|index| RetrievalCandidate {
                evidence_id: format!("evidence:{index}"),
                source_id: format!("source:{index}"),
                study_id: "study:throughput".into(),
                scope: "organoid:neural".into(),
                modality: "imaging".into(),
                support_milli: 900,
                state,
                semantic_digest: hash(&format!("semantic:{index}")),
                artifact_digest: hash(&format!("artifact:{index}")),
                provenance_digest: hash(&format!("provenance:{index}")),
                replay_identity: hash("replay"),
                omissions: Vec::new(),
                negative_evidence: Vec::new(),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            })
            .collect();
        ThroughputRetrievalWorkbenchRequest {
            request: ThroughputRetrievalQuery {
                request_id: "request:throughput-workbench".into(),
                batch_id: "batch:1".into(),
                partition: "partition:0".into(),
                max_items: 2,
                minimum_support_milli: 700,
                candidates,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workspace_id: "workspace:throughput-retrieval".into(),
            requested_view_order: VIEW_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            requested_panel_order: vec!["panel:queue".into(), "panel:overflow".into()]
                .into_iter()
                .collect(),
            checkpoint_seq: 1,
            replay_identity: hash("replay"),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        let manifest = throughput_retrieval_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn overflow_view_is_read_only() {
        let receipt =
            compile_throughput_retrieval_workbench(&request(EvidenceState::Supported)).unwrap();
        assert!(
            receipt.effect_receipts[0].starts_with("view:local-throughput-retrieval-artifacts:")
        );
        assert!(!receipt.omissions.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_throughput_retrieval_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        let receipt = compile_throughput_retrieval_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value == "workbench:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn workbench_artifact_payload_is_bound() {
        let mut receipt =
            compile_throughput_retrieval_workbench(&request(EvidenceState::Supported)).unwrap();
        receipt.workspace_id = "workspace:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_ranked_identity_is_rejected() {
        let mut receipt =
            compile_throughput_retrieval_workbench(&request(EvidenceState::Supported)).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn view_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_view_order.reverse();
        assert!(compile_throughput_retrieval_workbench(&input).is_err());
    }
    #[test]
    fn checkpoint_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.checkpoint_seq = 0;
        assert!(compile_throughput_retrieval_workbench(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_throughput_retrieval_workbench(&input).is_err());
    }
}
