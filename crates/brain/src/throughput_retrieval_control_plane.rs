//! Institution-local high-throughput retrieval control plane.
//!
//! Atlas feature: `AFA-brain-P02-F31`. This product reconciles queue, checkpoint, and budget
//! state before permitting a bounded local retrieval batch to publish.

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

pub const FEATURE_ID: &str = "AFA-brain-P02-F31";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-control-plane/1.0";
pub const ACTION_ORDER: [&str; 4] = [
    "control:observe",
    "control:reconcile",
    "control:authorize",
    "control:publish",
];
const CONTROL_CONTENT_TYPE: &str = "application/vnd.aurora.throughput-retrieval-control-plane+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalControlPlaneRequest {
    pub request: ThroughputRetrievalQuery,
    pub plane_id: String,
    pub session_id: String,
    pub requested_action_order: Vec<String>,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalControlPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub plane_id: String,
    pub session_id: String,
    pub batch_id: String,
    pub partition: String,
    pub checkpoint_seq: u64,
    pub action_order: Vec<String>,
    pub completed_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub disposition: SynthesisDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub queue_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub control_digest: ContentHash,
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
pub enum ThroughputRetrievalControlPlaneError {
    #[error("invalid throughput retrieval control-plane request: {0}")]
    Invalid(String),
    #[error("throughput retrieval control-plane artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval control-plane synthesis failed: {0}")]
    Engine(String),
}

impl ThroughputRetrievalControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.plane_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.action_order
                != ACTION_ORDER
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect::<Vec<_>>()
            || self.completed_action_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(ThroughputRetrievalControlPlaneError::Invalid("throughput control-plane identity, queue, checkpoint, actions, retrieval, locality, budget, or effects are incomplete".into()));
        }
        let action_position =
            |action: &String| ACTION_ORDER.iter().position(|expected| expected == action);
        for values in [&self.completed_action_order, &self.blocked_action_order] {
            let positions = values
                .iter()
                .map(action_position)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    ThroughputRetrievalControlPlaneError::Invalid(
                        "throughput control-plane action is unknown".into(),
                    )
                })?;
            if positions.windows(2).any(|pair| pair[0] >= pair[1])
                || values.iter().any(|action| {
                    self.completed_action_order
                        .iter()
                        .filter(|candidate| *candidate == action)
                        .count()
                        + self
                            .blocked_action_order
                            .iter()
                            .filter(|candidate| *candidate == action)
                            .count()
                        > 1
                })
            {
                return Err(ThroughputRetrievalControlPlaneError::Invalid(
                    "throughput control-plane action transcript is not canonical".into(),
                ));
            }
        }
        if self.completed_action_order.len() > self.action_order.len()
            || self.completed_action_order != self.action_order[..self.completed_action_order.len()]
            || self.blocked_action_order != self.action_order[self.completed_action_order.len()..]
        {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "throughput control-plane actions are not a canonical prefix and suffix".into(),
            ));
        }
        validate_sorted_unique(&self.compensation_order, "compensation_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        for (values, field) in [
            (&self.ranked_order, "ranked_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        for (values, field) in [
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "throughput control-plane ranked order must contain every candidate exactly once"
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
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "throughput control-plane candidate states must partition candidates".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.synthesis_digest,
            &self.control_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalControlPlaneError::Invalid(
                    "throughput control-plane digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!(
                "manage:local-throughput-retrieval-control:{}",
                self.plane_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "throughput control-plane effects do not match disposition".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "control:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "non-local control planes must be blocked and retain locality evidence".into(),
            ));
        }
        let expected_control_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "plane_id": self.plane_id,
            "session_id": self.session_id,
            "batch_id": self.batch_id,
            "partition": self.partition,
            "checkpoint_seq": self.checkpoint_seq,
            "action_order": self.action_order,
            "completed": self.completed_action_order,
            "blocked": self.blocked_action_order,
            "compensation": self.compensation_order,
            "queue_digest": self.queue_digest,
            "synthesis_digest": self.synthesis_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))?;
        if self.control_digest != expected_control_digest {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "throughput control-plane digest is not bound to control state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-control-plane:{}", self.plane_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTROL_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(
                "throughput control-plane artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalControlPlaneError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["throughput retrieval operator".into(), "research reliability engineer".into()].into(), behavior: "manages bounded local throughput retrieval queue and checkpoint control actions with reconciliation, compensation, replay, and permitted summary release receipts".into(), value: "prevents queue overflow, checkpoint gaps, and budget exhaustion from being hidden as successful high-throughput retrieval".into(), inputs: vec![TypedPort { name: "throughput_retrieval_control_plane_request".into(), schema: "ThroughputRetrievalControlPlaneRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_retrieval_control_plane_receipt".into(), schema: "ThroughputRetrievalControlPlaneReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["manage:local-throughput-retrieval-control".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_throughput_retrieval_control_plane(
    request: &ThroughputRetrievalControlPlaneRequest,
) -> Result<ThroughputRetrievalControlPlaneReceipt, ThroughputRetrievalControlPlaneError> {
    for (value, field) in [
        (&request.plane_id, "plane_id"),
        (&request.session_id, "session_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.plane_id.trim().is_empty()
        || request.session_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.requested_action_order
            != ACTION_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || request.budget_units == 0
        || request.request.replay_identity.as_str().len() != 64
    {
        return Err(ThroughputRetrievalControlPlaneError::Invalid(
            "throughput control-plane identity, action order, budget, or boundary is invalid"
                .into(),
        ));
    }
    let synthesis = synthesize_throughput_retrieval(&request.request)
        .map_err(|error| ThroughputRetrievalControlPlaneError::Engine(error.to_string()))?;
    let gate = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && u64::from(request.budget_units) >= u64::try_from(ACTION_ORDER.len()).unwrap_or(u64::MAX);
    let disposition = if gate {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
    };
    let completed_action_order: Vec<String> = if gate {
        ACTION_ORDER
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        ACTION_ORDER[..2]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    };
    let blocked_action_order: Vec<String> = if gate {
        Vec::new()
    } else {
        ACTION_ORDER[2..]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    };
    let mut compensation = BTreeSet::new();
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
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
        compensation.insert("compensate:retain-policy-denial".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
        compensation.insert("compensate:retain-closure-gap".into());
    }
    if !request.raw_data_local {
        omissions.insert("control:raw-data-locality-failed".into());
        compensation.insert("compensate:retain-locality-failure".into());
    }
    if disposition != SynthesisDisposition::Qualified {
        compensation.insert("compensate:retain-unresolved-throughput-retrieval".into());
    }
    let control_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "plane_id": request.plane_id, "session_id": request.session_id, "batch_id": synthesis.batch_id, "partition": synthesis.partition, "checkpoint_seq": synthesis.checkpoint_seq, "action_order": ACTION_ORDER, "completed": completed_action_order, "blocked": blocked_action_order, "compensation": compensation, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis.synthesis_digest, "replay_identity": request.request.replay_identity, "raw_data_local": true})).map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        SynthesisDisposition::Qualified | SynthesisDisposition::Partial
    ) {
        vec![format!(
            "manage:local-throughput-retrieval-control:{}",
            request.plane_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "plane_id": request.plane_id, "session_id": request.session_id, "batch_id": synthesis.batch_id, "partition": synthesis.partition, "checkpoint_seq": synthesis.checkpoint_seq, "disposition": disposition, "action_order": ACTION_ORDER, "completed_action_order": completed_action_order, "blocked_action_order": blocked_action_order, "compensation_order": compensation, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis.synthesis_digest, "control_digest": control_digest, "replay_identity": request.request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-retrieval-control-plane:{}",
            request.plane_id
        ),
        CONTROL_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalControlPlaneError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        plane_id: request.plane_id.clone(),
        session_id: request.session_id.clone(),
        batch_id: synthesis.batch_id,
        partition: synthesis.partition,
        checkpoint_seq: synthesis.checkpoint_seq,
        action_order: ACTION_ORDER.iter().map(|value| (*value).into()).collect(),
        completed_action_order,
        blocked_action_order,
        compensation_order: compensation.into_iter().collect(),
        disposition,
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        queue_digest: synthesis.queue_digest,
        synthesis_digest: synthesis.synthesis_digest,
        control_digest,
        replay_identity: request.request.replay_identity.clone(),
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

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalControlPlaneError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalControlPlaneError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalControlPlaneError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalControlPlaneError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalControlPlaneError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalControlPlaneError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalControlPlaneReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "plane_id": receipt.plane_id,
        "session_id": receipt.session_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "checkpoint_seq": receipt.checkpoint_seq,
        "action_order": receipt.action_order,
        "completed_action_order": receipt.completed_action_order,
        "blocked_action_order": receipt.blocked_action_order,
        "compensation_order": receipt.compensation_order,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "queue_digest": receipt.queue_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "control_digest": receipt.control_digest,
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
    fn request() -> ThroughputRetrievalControlPlaneRequest {
        ThroughputRetrievalControlPlaneRequest {
            request: ThroughputRetrievalQuery {
                request_id: "request:tp-control".into(),
                batch_id: "batch:tp".into(),
                partition: "partition:one".into(),
                max_items: 16,
                minimum_support_milli: 700,
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:tp-control".into(),
                    source_id: "source:tp-control".into(),
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
            plane_id: "plane:tp-local".into(),
            session_id: "session:tp-control".into(),
            requested_action_order: ACTION_ORDER.iter().map(|value| (*value).into()).collect(),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_retrieval_control_plane_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn control_plane_completes() {
        let receipt = operate_throughput_retrieval_control_plane(&request()).unwrap();
        assert_eq!(receipt.completed_action_order.len(), 4);
        assert!(receipt.checkpoint_seq > 0);
    }
    #[test]
    fn denial_compensates() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = operate_throughput_retrieval_control_plane(&value).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(!receipt.compensation_order.is_empty());
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut value = request();
        value.raw_data_local = false;
        let receipt = operate_throughput_retrieval_control_plane(&value).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "control:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn control_artifact_payload_is_bound() {
        let mut receipt = operate_throughput_retrieval_control_plane(&request()).unwrap();
        receipt.plane_id = "plane:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = operate_throughput_retrieval_control_plane(&request()).unwrap();
        receipt.qualified_order[0] = receipt.qualified_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = operate_throughput_retrieval_control_plane(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
