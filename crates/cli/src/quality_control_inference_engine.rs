//! Federated continual quality-control inference engine for `AFA-cli-P07-F04`.
//!
//! This engine fuses typed quality summaries across preclinical sites without loading raw
//! images or omics matrices. It produces a deterministic, evidence-bearing verdict that a
//! downstream CLI or governed workbench can consume; it never edits data or releases a sample.

use bioprism_foundation::{EvidenceState, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P07-F04";
pub const CONTRACT_VERSION: &str = "cli-federated-continual-quality-control-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "QualityInferenceRequest4@1";
pub const OUTPUT_SCHEMA: &str = "QualityInferenceReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.quality-inference-receipt-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityEvidence4 {
    pub observation_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub metric: String,
    pub observed_milli: i32,
    pub threshold_milli: i32,
    pub baseline_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub local_data: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityPeerSummary3 {
    pub peer_id: String,
    pub origin: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub report_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityInferenceRequest4 {
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_observation_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub observations: Vec<QualityEvidence4>,
    pub peers: Vec<QualityPeerSummary3>,
    pub checkpoint: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityInferenceArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityInferenceReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub passed_observation_order: Vec<String>,
    pub failed_observation_order: Vec<String>,
    pub unknown_observation_order: Vec<String>,
    pub contradicted_observation_order: Vec<String>,
    pub missing_observation_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub inference_digest: ContentHash,
    pub artifact: QualityInferenceArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityInferenceError {
    #[error("invalid quality inference request: {0}")]
    Invalid(String),
    #[error("quality inference artifact failed: {0}")]
    Artifact(String),
}

pub fn capability_manifest() -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"cli","consumers":["quality-control researcher","CLI operator","federation steward"],"behavior":"fuses typed quality summaries into a deterministic cross-site quality verdict","value":"prevents missing, contradictory, or unmeasured quality evidence from becoming a release pass","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["retain:quality-inference","exchange:aggregate-quality-summary"],"permissions":["retain:quality-receipts","exchange:aggregate-quality"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}

impl QualityInferenceReceipt7 {
    pub fn validate(&self) -> Result<(), QualityInferenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.observation_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(QualityInferenceError::Invalid("quality inference identity, locality, observations, peers, disposition, or effects are incomplete".into()));
        }
        for values in [
            &self.observation_order,
            &self.passed_observation_order,
            &self.failed_observation_order,
            &self.unknown_observation_order,
            &self.contradicted_observation_order,
            &self.missing_observation_order,
            &self.missing_modality_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(QualityInferenceError::Invalid(
                    "quality inference ordering is not canonical".into(),
                ));
            }
        }
        let obs = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let obs_parts = self
            .passed_observation_order
            .iter()
            .chain(&self.failed_observation_order)
            .chain(&self.unknown_observation_order)
            .chain(&self.contradicted_observation_order)
            .chain(&self.missing_observation_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if obs != obs_parts || obs.len() != self.observation_order.len() {
            return Err(QualityInferenceError::Invalid(
                "quality observation states do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != peer_parts || peers.len() != self.peer_order.len() {
            return Err(QualityInferenceError::Invalid(
                "quality peer states do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.inference_digest
        {
            return Err(QualityInferenceError::Artifact(
                "quality inference artifact digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("retain:quality-inference:")
                && !e.starts_with("exchange:aggregate-quality-summary:")
                && e != "block:unsafe-release"
        }) {
            return Err(QualityInferenceError::Invalid(
                "quality inference effect is outside governed gate".into(),
            ));
        }
        Ok(())
    }
}

pub fn infer_quality(
    request: &QualityInferenceRequest4,
) -> Result<QualityInferenceReceipt7, QualityInferenceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|a, b| a.observation_id.cmp(&b.observation_id));
    let observation_order = observations
        .iter()
        .map(|o| o.observation_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_observation_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let required_modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let adversarial = request
        .adversarial_events
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for o in &observations {
        seen.insert(o.observation_id.clone());
        modalities.insert(o.modality.clone());
        if o.negative_result {
            negative.insert(format!("observation:{}:negative-result", o.observation_id));
        }
        match o.evidence_state {
            EvidenceState::Contradicted => {
                contradicted.insert(o.observation_id.clone());
                negative.insert(format!("observation:{}:contradicted", o.observation_id));
            }
            EvidenceState::Unknown | EvidenceState::Unmeasured | EvidenceState::Speculative => {
                unknown.insert(o.observation_id.clone());
                uncertainty.insert(format!("observation:{}:evidence-state", o.observation_id));
            }
            EvidenceState::Proven | EvidenceState::Supported
                if o.observed_milli >= o.threshold_milli =>
            {
                passed.insert(o.observation_id.clone());
            }
            EvidenceState::Proven | EvidenceState::Supported => {
                failed.insert(o.observation_id.clone());
                omissions.insert(format!("observation:{}:threshold-failed", o.observation_id));
            }
        }
    }
    for id in required.difference(&seen) {
        missing.insert(id.clone());
        omissions.insert(format!("observation:{}:missing", id));
        uncertainty.insert(format!("observation:{}:missing", id));
    }
    let missing_modality = required_modalities
        .difference(&modalities)
        .cloned()
        .collect::<BTreeSet<_>>();
    for m in &missing_modality {
        omissions.insert(format!("modality:{}:missing", m));
        uncertainty.insert(format!("modality:{}:missing", m));
    }
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let qualified_peers = peers
        .iter()
        .filter(|p| {
            p.semantic_profile == request.semantic_profile
                && p.checkpoint == request.checkpoint
                && p.signed
                && p.aggregate_only
                && p.raw_data_local
                && matches!(
                    p.evidence_state,
                    EvidenceState::Proven | EvidenceState::Supported
                )
        })
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peers = peer_order
        .iter()
        .filter(|p| !qualified_peers.contains(*p))
        .cloned()
        .collect::<BTreeSet<_>>();
    for p in &missing_peers {
        uncertainty.insert(format!("peer:{}:not-qualified", p));
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !adversarial.is_empty();
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    for e in &adversarial {
        omissions.insert(format!("adversarial:{}", e));
    }
    let disposition = if global_block || !contradicted.is_empty() {
        "blocked"
    } else if !missing.is_empty()
        || !missing_modality.is_empty()
        || !unknown.is_empty()
        || !failed.is_empty()
        || qualified_peers.len() < request.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:quality-not-release-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"observation_order":observation_order,"passed_observation_order":passed,"failed_observation_order":failed,"unknown_observation_order":unknown,"contradicted_observation_order":contradicted,"missing_observation_order":missing,"missing_modality_order":missing_modality,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"adversarial_event_order":adversarial,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| QualityInferenceError::Artifact(e.to_string()))?;
    let artifact = QualityInferenceArtifact7 {
        artifact_id: format!("quality-inference-receipt-7:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: observations
            .iter()
            .map(|o| o.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effect_receipts = if disposition == "qualified" {
        vec![
            format!("retain:quality-inference:{}", request.request_id),
            format!("exchange:aggregate-quality-summary:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = QualityInferenceReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        observation_order: observations
            .iter()
            .map(|o| o.observation_id.clone())
            .collect(),
        passed_observation_order: passed.into_iter().collect(),
        failed_observation_order: failed.into_iter().collect(),
        unknown_observation_order: unknown.into_iter().collect(),
        contradicted_observation_order: contradicted.into_iter().collect(),
        missing_observation_order: missing.into_iter().collect(),
        missing_modality_order: missing_modality.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        adversarial_event_order: adversarial.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        inference_digest: digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(r: &QualityInferenceRequest4) -> Result<(), QualityInferenceError> {
    if ![
        &r.request_id,
        &r.federation_id,
        &r.requester,
        &r.purpose,
        &r.semantic_profile,
    ]
    .iter()
    .all(|v| !v.trim().is_empty())
        || r.required_observation_order.is_empty()
        || r.required_modality_order.is_empty()
        || r.observations.is_empty()
        || r.peers.is_empty()
        || r.checkpoint == 0
        || r.minimum_peer_quorum == 0
        || !r.raw_data_local
        || !r.aggregate_only
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.replay_identity.as_str().len() != 64
    {
        return Err(QualityInferenceError::Invalid("quality inference request identity, observations, peers, checkpoint, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for o in &r.observations {
        if o.observation_id.trim().is_empty()
            || !ids.insert(o.observation_id.clone())
            || o.study_id.trim().is_empty()
            || o.modality.trim().is_empty()
            || !r.required_modality_order.contains(&o.modality)
            || o.semantic_profile != r.semantic_profile
            || o.baseline_digest.as_str().len() != 64
            || o.artifact_digest.as_str().len() != 64
            || o.provenance_digest.as_str().len() != 64
            || o.replay_identity != r.replay_identity
            || !o.local_data
        {
            return Err(QualityInferenceError::Invalid("quality evidence identity, modality, profile, digests, replay, or locality is invalid".into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> QualityInferenceRequest4 {
        let r = h("replay");
        QualityInferenceRequest4 {
            request_id: "request:qc".into(),
            federation_id: "fed:qc".into(),
            requester: "researcher".into(),
            purpose: "qc".into(),
            semantic_profile: "qc:v1".into(),
            required_observation_order: vec!["obs:a".into()],
            required_modality_order: vec!["imaging".into()],
            observations: vec![QualityEvidence4 {
                observation_id: "obs:a".into(),
                study_id: "study:a".into(),
                modality: "imaging".into(),
                semantic_profile: "qc:v1".into(),
                metric: "snr".into(),
                observed_milli: 900,
                threshold_milli: 700,
                baseline_digest: h("base"),
                artifact_digest: h("art"),
                provenance_digest: h("prov"),
                replay_identity: r.clone(),
                evidence_state: EvidenceState::Supported,
                local_data: true,
                negative_result: false,
            }],
            peers: vec![QualityPeerSummary3 {
                peer_id: "peer:a".into(),
                origin: "site:a".into(),
                study_id: "study:a".into(),
                modality: "imaging".into(),
                semantic_profile: "qc:v1".into(),
                checkpoint: 2,
                report_digest: h("report"),
                evidence_state: EvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: r,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_is_deterministic() {
        let x = infer_quality(&req()).unwrap();
        assert_eq!(x.disposition, "qualified");
        assert_eq!(x.inference_digest, x.artifact.content_hash);
    }
    #[test]
    fn unknown_and_contradicted_fail_closed() {
        let mut r = req();
        r.observations[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(infer_quality(&r).unwrap().disposition, "unresolved");
        let mut r = req();
        r.observations[0].evidence_state = EvidenceState::Contradicted;
        assert_eq!(infer_quality(&r).unwrap().disposition, "blocked");
    }
}
