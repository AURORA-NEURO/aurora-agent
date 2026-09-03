//! Federated continual quality-control research workbench (`AFA-factory-P07-F20`).
//!
//! The harness evaluates typed metric summaries, preserving failed and unmeasured evidence. It
//! does not inspect raw arrays, images, sequencing reads, or instrument state.

use bioprism_ids::ContentHash;
use bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-factory-P07-F20";
pub const CONTRACT_VERSION: &str = "factory-federated-continual-quality-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ResearchObject4@1";
pub const OUTPUT_SCHEMA: &str = "FactoryQualityVerdict5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.factory-quality-verdict-5+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryQualityWorkbenchRequest {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_modalities: Vec<String>,
    pub minimum_pass_fraction_milli: i64,
    pub checkpoint: u64,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub continual_epoch: u64,
    pub required_peer_order: Vec<String>,
    pub peers: Vec<FactoryQualityPeer4>,
    pub minimum_peer_quorum: usize,
    pub adversarial_event_order: Vec<String>,
    pub observations: Vec<QualityObservation4>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryQualityPeer4 {
    pub peer_id: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub evidence_state: QualityEvidenceState,
}

impl FactoryQualityPeer4 {
    fn required_peer_is_qualified(&self, request: &FactoryQualityWorkbenchRequest) -> bool {
        self.peer_id.trim() != ""
            && self.semantic_profile == request.semantic_profile
            && self.replay_identity == request.replay_identity
            && self.signed
            && self.permitted
            && self.raw_data_local
            && self.aggregate_only
            && matches!(self.evidence_state, QualityEvidenceState::Proven | QualityEvidenceState::Supported)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityObservation4 {
    pub observation_id: String,
    pub modality: String,
    pub metric_id: String,
    pub study_id: String,
    pub origin: String,
    pub semantic_profile: String,
    pub value_milli: i64,
    pub threshold_milli: i64,
    pub baseline_milli: i64,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: QualityEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryQualityVerdict5Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryQualityVerdict5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub continual_epoch: u64,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub passed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub unmeasured_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub passed_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub quorum: usize,
    pub minimum_peer_quorum: usize,
    pub pass_fraction_milli: i64,
    pub replay_identity: ContentHash,
    pub report_digest: ContentHash,
    pub artifact: FactoryQualityVerdict5Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FactoryQualityWorkbenchError {
    #[error("invalid quality-control request: {0}")]
    Invalid(String),
    #[error("quality-control artifact failed: {0}")]
    Artifact(String),
}

pub fn factory_federated_quality_workbench_manifest() -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"factory","consumers":["platform reliability engineer","federation steward","research administrator"],"behavior":"evaluates a federated continual multimodal quality envelope against typed thresholds, peer quorum, and locality closure","value":"prevents failed, missing, contradictory, or unmeasured QC evidence from silently entering a research workflow while preserving institution-local data","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute_local_computation","write_local_artifact"],"permissions":["read:local-research-artifacts","view:quality-workbench"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl FactoryQualityVerdict5 {
    pub fn validate(&self) -> Result<(), FactoryQualityWorkbenchError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.observation_order.is_empty()
            || self.modality_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.continual_epoch == 0
            || self.peer_order.is_empty()
            || self.minimum_peer_quorum == 0
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(FactoryQualityWorkbenchError::Invalid("quality identity, checkpoint, locality, observations, modalities, or effects are incomplete".into()));
        }
        for values in [
            &self.observation_order,
            &self.passed_order,
            &self.failed_order,
            &self.unknown_order,
            &self.unmeasured_order,
            &self.blocked_order,
            &self.modality_order,
            &self.passed_modality_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.adversarial_event_order,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(FactoryQualityWorkbenchError::Invalid(
                    "quality ordering is not canonical".into(),
                ));
            }
        }
        let obs = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let parts = self
            .passed_order
            .iter()
            .chain(&self.failed_order)
            .chain(&self.unknown_order)
            .chain(&self.unmeasured_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if obs != parts || obs.len() != self.observation_order.len() {
            return Err(FactoryQualityWorkbenchError::Invalid(
                "quality observation dispositions do not partition".into(),
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
            return Err(FactoryQualityWorkbenchError::Invalid(
                "quality peer dispositions do not partition".into(),
            ));
        }
        if self.quorum != self.qualified_peer_order.len()
            || self.quorum > self.peer_order.len()
            || self.minimum_peer_quorum > self.peer_order.len()
        {
            return Err(FactoryQualityWorkbenchError::Invalid(
                "quality peer quorum is inconsistent".into(),
            ));
        }
        let modalities = BTreeSet::from_iter(self.modality_order.iter().cloned());
        let modality_parts = self
            .passed_modality_order
            .iter()
            .chain(&self.missing_modality_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if modalities != modality_parts || modalities.len() != self.modality_order.len() {
            return Err(FactoryQualityWorkbenchError::Invalid(
                "quality modality dispositions do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.report_digest
        {
            return Err(FactoryQualityWorkbenchError::Artifact(
                "quality artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
        .iter()
            .any(|e| !e.starts_with("view:quality-workbench:") && e != "block:unsafe-release")
        {
            return Err(FactoryQualityWorkbenchError::Invalid(
                "effect is outside quality gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, FactoryQualityWorkbenchError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| FactoryQualityWorkbenchError::Artifact(e.to_string()))?,
        )
        .map_err(|e| FactoryQualityWorkbenchError::Artifact(e.to_string()))
    }
}

pub fn assure_factory_federated_quality_workbench(
    request: &FactoryQualityWorkbenchRequest,
) -> Result<FactoryQualityVerdict5, FactoryQualityWorkbenchError> {
    validate_request(request)?;
    let mut rows = request.observations.clone();
    rows.sort_by(|a, b| a.observation_id.cmp(&b.observation_id));
    let observation_order = rows
        .iter()
        .map(|x| x.observation_id.clone())
        .collect::<Vec<_>>();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut unmeasured = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut passed_modalities = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for o in &rows {
        modalities.insert(o.modality.clone());
        if o.negative_result {
            negative.insert(format!("{}:negative-result", o.observation_id));
        }
        for r in &o.omission_reasons {
            omission.insert(format!("{}:{}", o.observation_id, r));
        }
        let mut reasons = Vec::new();
        if o.study_id != request.study_id {
            reasons.push("study-mismatch");
        }
        if o.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        if o.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch");
        }
        if !o.signed || !o.permitted {
            reasons.push("authorization-missing");
        }
        if !o.raw_data_local || !o.aggregate_only {
            reasons.push("locality-or-aggregate-only-failed");
        }
        if o.evidence_state == QualityEvidenceState::Contradicted {
            blocked.insert(o.observation_id.clone());
            negative.insert(format!("{}:contradicted", o.observation_id));
        } else if o.evidence_state == QualityEvidenceState::Unknown {
            unknown.insert(o.observation_id.clone());
            uncertainty.insert(format!("{}:unknown", o.observation_id));
        } else if o.evidence_state == QualityEvidenceState::Unmeasured {
            unmeasured.insert(o.observation_id.clone());
            uncertainty.insert(format!("{}:unmeasured", o.observation_id));
        } else if !reasons.is_empty() {
            unknown.insert(o.observation_id.clone());
            uncertainty.insert(format!("{}:unresolved", o.observation_id));
        } else if o.value_milli < o.threshold_milli {
            failed.insert(o.observation_id.clone());
            omission.insert(format!("{}:threshold-failed", o.observation_id));
        } else {
            passed.insert(o.observation_id.clone());
            passed_modalities.insert(o.modality.clone());
        }
    }
    let required = BTreeSet::from_iter(request.required_modalities.iter().cloned());
    modalities.extend(required.iter().cloned());
    let missing = required
        .difference(&passed_modalities)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        uncertainty.insert("modality:required-closure-incomplete".into());
    }
    let denom = rows.len() as i64;
    let pass_fraction = if denom == 0 {
        0
    } else {
        (passed.len() as i64 * 1000) / denom
    };
    let peer_order = request
        .required_peer_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let qualified_peer_order = request
        .peers
        .iter()
        .filter(|peer| peer_order.contains(&peer.peer_id) && peer.required_peer_is_qualified(request))
        .map(|peer| peer.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peer_order = peer_order
        .difference(&qualified_peer_order)
        .cloned()
        .collect::<BTreeSet<_>>();
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty()
        || qualified_peer_order.len() < request.minimum_peer_quorum;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !missing_peer_order.is_empty() {
        omission.insert("federation:peer-quorum-incomplete".into());
        uncertainty.extend(
            missing_peer_order
                .iter()
                .map(|peer| format!("peer:{peer}:unresolved")),
        );
    }
    if !request.adversarial_event_order.is_empty() {
        negative.insert("federation:adversarial-event-retained".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if pass_fraction < request.minimum_pass_fraction_milli
        || !missing.is_empty()
        || !failed.is_empty()
        || !unknown.is_empty()
        || !unmeasured.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked.extend(observation_order.iter().cloned());
        passed.clear();
        failed.clear();
        unknown.clear();
        unmeasured.clear();
    }
    if disposition != "qualified" {
        omission.insert("request:quality-gates-incomplete".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"study_id":request.study_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"continual_epoch":request.continual_epoch,"disposition":disposition,"observation_order":observation_order,"passed_order":passed,"failed_order":failed,"unknown_order":unknown,"unmeasured_order":unmeasured,"blocked_order":blocked,"modality_order":modalities,"passed_modality_order":passed_modalities,"missing_modality_order":missing,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"adversarial_event_order":request.adversarial_event_order,"quorum":qualified_peer_order.len(),"minimum_peer_quorum":request.minimum_peer_quorum,"pass_fraction_milli":pass_fraction,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let report_digest = ContentHash::of_value(&payload)
        .map_err(|e| FactoryQualityWorkbenchError::Artifact(e.to_string()))?;
    let artifact = FactoryQualityVerdict5Artifact {
        artifact_id: format!("factory-quality-verdict-5:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: report_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: rows
            .iter()
            .map(|x| x.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![format!("view:quality-workbench:{}", request.federation_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = FactoryQualityVerdict5 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        study_id: request.study_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        continual_epoch: request.continual_epoch,
        disposition: disposition.into(),
        observation_order: payload["observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        passed_order: payload["passed_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        failed_order: payload["failed_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unknown_order: payload["unknown_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unmeasured_order: payload["unmeasured_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        modality_order: payload["modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        passed_modality_order: payload["passed_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_modality_order: payload["missing_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        quorum: qualified_peer_order.len(),
        minimum_peer_quorum: request.minimum_peer_quorum,
        pass_fraction_milli: pass_fraction,
        replay_identity: request.replay_identity.clone(),
        report_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FactoryQualityWorkbenchRequest,
) -> Result<(), FactoryQualityWorkbenchError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.minimum_pass_fraction_milli < 0
        || request.minimum_pass_fraction_milli > 1000
        || request.checkpoint == 0
        || request.budget_units == 0
        || request.continual_epoch == 0
        || request.required_peer_order.is_empty()
        || request.minimum_peer_quorum == 0
        || request.minimum_peer_quorum > request.required_peer_order.len()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.observations.is_empty()
        || request.required_peer_order.windows(2).any(|w| w[0] >= w[1])
        || request.adversarial_event_order.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(FactoryQualityWorkbenchError::Invalid("quality identity, modalities, threshold, checkpoint, budget, replay, locality, observations, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for o in &request.observations {
        if o.observation_id.trim().is_empty()
            || o.modality.trim().is_empty()
            || o.metric_id.trim().is_empty()
            || o.study_id.trim().is_empty()
            || o.origin.trim().is_empty()
            || o.semantic_profile.trim().is_empty()
            || o.artifact_digest.as_str().len() != 64
            || o.provenance_digest.as_str().len() != 64
            || o.replay_identity.as_str().len() != 64
            || !ids.insert(o.observation_id.clone())
        {
            return Err(FactoryQualityWorkbenchError::Invalid(
                "observation identity, uniqueness, profiles, or digest is invalid".into(),
            ));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peers.insert(peer.peer_id.clone())
            || !request.required_peer_order.contains(&peer.peer_id)
            || peer.semantic_profile.trim().is_empty()
            || peer.artifact_digest.as_str().len() != 64
            || peer.replay_identity.as_str().len() != 64
        {
            return Err(FactoryQualityWorkbenchError::Invalid(
                "peer identity, uniqueness, profile, required-peer closure, or digest is invalid".into(),
            ));
        }
    }
    if request
        .required_peer_order
        .iter()
        .any(|id| !peers.contains(id))
    {
        return Err(FactoryQualityWorkbenchError::Invalid(
            "required peer closure references an unknown peer".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(s: &str) -> ContentHash {
        ContentHash::of_bytes(s.as_bytes())
    }
    fn req() -> FactoryQualityWorkbenchRequest {
        FactoryQualityWorkbenchRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request:qc".into(),
            federation_id: "federation:qc".into(),
            study_id: "study:1".into(),
            requester: "quality-scientist".into(),
            purpose: "qc".into(),
            semantic_profile: "multi:v1".into(),
            required_modalities: vec!["imaging".into()],
            minimum_pass_fraction_milli: 500,
            checkpoint: 1,
            budget_units: 10,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            continual_epoch: 1,
            required_peer_order: vec!["peer:a".into()],
            peers: vec![FactoryQualityPeer4 {
                peer_id: "peer:a".into(),
                semantic_profile: "multi:v1".into(),
                artifact_digest: h("peer-artifact"),
                replay_identity: h("r"),
                signed: true,
                permitted: true,
                raw_data_local: true,
                aggregate_only: true,
                evidence_state: QualityEvidenceState::Supported,
            }],
            minimum_peer_quorum: 1,
            adversarial_event_order: Vec::new(),
            observations: vec![obs("metric:a", QualityEvidenceState::Supported), obs("metric:b", QualityEvidenceState::Supported)],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn obs(id: &str, state: QualityEvidenceState) -> QualityObservation4 {
        QualityObservation4 {
            observation_id: id.into(),
            modality: "imaging".into(),
            metric_id: "snr".into(),
            study_id: "study:1".into(),
            origin: "site-a".into(),
            semantic_profile: "multi:v1".into(),
            value_milli: 90,
            threshold_milli: 70,
            baseline_milli: 80,
            artifact_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("r"),
            evidence_state: state,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(factory_federated_quality_workbench_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn qualified_is_replayable() {
        let mut request = req();
        request.observations = vec![
            obs("b", QualityEvidenceState::Supported),
            obs("a", QualityEvidenceState::Proven),
        ];
        let r = assure_factory_federated_quality_workbench(&request).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut request = req(); request.observations = vec![obs("a", QualityEvidenceState::Unknown)];
        let r = assure_factory_federated_quality_workbench(&request).unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn contradiction_blocks() {
        let mut request = req(); request.observations = vec![obs("a", QualityEvidenceState::Contradicted)];
        let r = assure_factory_federated_quality_workbench(&request).unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn failed_threshold_is_unresolved() {
        let mut o = obs("a", QualityEvidenceState::Supported);
        o.value_milli = 10;
        let mut request = req(); request.observations = vec![o];
        let r = assure_factory_federated_quality_workbench(&request).unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn duplicate_is_rejected() {
        let mut request = req();
        request.observations = vec![obs("a", QualityEvidenceState::Supported), obs("a", QualityEvidenceState::Supported)];
        assert!(assure_factory_federated_quality_workbench(&request).is_err());
    }
}

