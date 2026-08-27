//! Multimodal ingestion research copilot (`AFA-ids-P06-F09`).
//!
//! Validates caller-supplied modality manifests and QC summaries before a study enters a local
//! research workflow. It is a copilot receipt, not a file reader or instrument controller.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P06-F09";
pub const CONTRACT_VERSION: &str =
    "ids-local-single-study-multimodal-ingestion-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "MultimodalIngestionRequest4@1";
pub const OUTPUT_SCHEMA: &str = "HarmonizedResearchObject8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.harmonized-research-object-8+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionRequest4 {
    pub request_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_modalities: Vec<String>,
    pub minimum_quality_milli: i64,
    pub checkpoint: u64,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityObservation4 {
    pub observation_id: String,
    pub modality: String,
    pub study_id: String,
    pub origin: String,
    pub schema_version: String,
    pub semantic_profile: String,
    pub unit_profile: String,
    pub coordinate_profile: String,
    pub quality_milli: i64,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: IngestionEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedResearchObject8Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedResearchObject8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub quality_scores_milli: Vec<i64>,
    pub replay_identity: ContentHash,
    pub object_digest: ContentHash,
    pub artifact: HarmonizedResearchObject8Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalIngestionError {
    #[error("invalid multimodal ingestion request: {0}")]
    Invalid(String),
    #[error("multimodal ingestion artifact failed: {0}")]
    Artifact(String),
}

pub fn multimodal_ingestion_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["computational biologist","imaging scientist","omics scientist"],"behavior":"validates bounded local modality manifests and quality summaries into a harmonized research-object receipt","value":"prevents incompatible, incomplete, or unauthorized modalities from silently entering a preclinical study workflow","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability"],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl HarmonizedResearchObject8 {
    pub fn validate(&self) -> Result<(), MultimodalIngestionError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.observation_order.is_empty()
            || self.modality_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(MultimodalIngestionError::Invalid("ingestion identity, checkpoint, locality, observations, modalities, or effects are incomplete".into()));
        }
        for values in [
            &self.observation_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.modality_order,
            &self.selected_modality_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(MultimodalIngestionError::Invalid(
                    "ingestion ordering is not canonical".into(),
                ));
            }
        }
        let obs = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if obs != parts || obs.len() != self.observation_order.len() {
            return Err(MultimodalIngestionError::Invalid(
                "observation dispositions do not partition".into(),
            ));
        }
        let modalities = BTreeSet::from_iter(self.modality_order.iter().cloned());
        let modality_parts = self
            .selected_modality_order
            .iter()
            .chain(&self.missing_modality_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if modalities != modality_parts || modalities.len() != self.modality_order.len() {
            return Err(MultimodalIngestionError::Invalid(
                "modality dispositions do not partition".into(),
            ));
        }
        if self.selected_order.len() != self.quality_scores_milli.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.object_digest
        {
            return Err(MultimodalIngestionError::Artifact(
                "artifact metadata, quality cardinality, or digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| !e.starts_with("manage:local-capability:") && e != "block:unsafe-release")
        {
            return Err(MultimodalIngestionError::Invalid(
                "effect is outside ingestion gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalIngestionError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| MultimodalIngestionError::Artifact(e.to_string()))?,
        )
        .map_err(|e| MultimodalIngestionError::Artifact(e.to_string()))
    }
}

pub fn operate_multimodal_ingestion(
    request: &MultimodalIngestionRequest4,
    observations: &[ModalityObservation4],
) -> Result<HarmonizedResearchObject8, MultimodalIngestionError> {
    validate_request(request, observations)?;
    let mut rows = observations.to_vec();
    rows.sort_by(|a, b| {
        b.quality_milli
            .cmp(&a.quality_milli)
            .then(a.observation_id.cmp(&b.observation_id))
    });
    let observation_order = rows
        .iter()
        .map(|x| x.observation_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut selected_modalities = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut scores = Vec::new();
    for o in &rows {
        modalities.insert(o.modality.clone());
        if o.negative_result {
            negative.insert(format!("{}:negative-result", o.observation_id));
        }
        for reason in &o.omission_reasons {
            omission.insert(format!("{}:{}", o.observation_id, reason));
        }
        let mut reasons = Vec::new();
        if o.study_id != request.study_id {
            reasons.push("study-mismatch");
        }
        if o.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        if o.quality_milli < request.minimum_quality_milli {
            reasons.push("quality-threshold-failed");
            omission.insert(format!("{}:quality-threshold", o.observation_id));
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
        if o.evidence_state == IngestionEvidenceState::Contradicted {
            blocked.insert(o.observation_id.clone());
            negative.insert(format!("{}:contradicted", o.observation_id));
        } else if !matches!(
            o.evidence_state,
            IngestionEvidenceState::Proven | IngestionEvidenceState::Supported
        ) || !reasons.is_empty()
        {
            unresolved.insert(o.observation_id.clone());
            uncertainty.insert(format!("{}:unresolved", o.observation_id));
        } else {
            selected.insert(o.observation_id.clone());
            selected_modalities.insert(o.modality.clone());
            scores.push(o.quality_milli);
        }
    }
    let required = BTreeSet::from_iter(request.required_modalities.iter().cloned());
    modalities.extend(required.iter().cloned());
    let missing_modalities = required
        .difference(&selected_modalities)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing_modalities.is_empty() {
        uncertainty.insert("modality:required-closure-incomplete".into());
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty() || !missing_modalities.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked.extend(observation_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        scores.clear();
    }
    if disposition != "qualified" {
        omission.insert("request:ingestion-gates-incomplete".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let selected_modality_order = selected_modalities.into_iter().collect::<Vec<_>>();
    let missing_modality_order = missing_modalities.into_iter().collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"study_id":request.study_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"observation_order":observation_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"modality_order":modality_order,"selected_modality_order":selected_modality_order,"missing_modality_order":missing_modality_order,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"quality_scores_milli":scores,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let object_digest = ContentHash::of_value(&payload)
        .map_err(|e| MultimodalIngestionError::Artifact(e.to_string()))?;
    let artifact = HarmonizedResearchObject8Artifact {
        artifact_id: format!("harmonized-research-object-8:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: object_digest.clone(),
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
        vec![format!("manage:local-capability:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = HarmonizedResearchObject8 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        observation_order: payload["observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
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
        selected_modality_order: payload["selected_modality_order"]
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
        quality_scores_milli: scores,
        replay_identity: request.replay_identity.clone(),
        object_digest,
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
    request: &MultimodalIngestionRequest4,
    observations: &[ModalityObservation4],
) -> Result<(), MultimodalIngestionError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.checkpoint == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || observations.is_empty()
    {
        return Err(MultimodalIngestionError::Invalid("ingestion identity, modalities, checkpoint, budget, replay, locality, observations, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for o in observations {
        if o.observation_id.trim().is_empty()
            || o.modality.trim().is_empty()
            || o.study_id.trim().is_empty()
            || o.origin.trim().is_empty()
            || o.schema_version.trim().is_empty()
            || o.semantic_profile.trim().is_empty()
            || o.unit_profile.trim().is_empty()
            || o.coordinate_profile.trim().is_empty()
            || o.artifact_digest.as_str().len() != 64
            || o.provenance_digest.as_str().len() != 64
            || o.replay_identity.as_str().len() != 64
            || !ids.insert(o.observation_id.clone())
        {
            return Err(MultimodalIngestionError::Invalid(
                "observation identity, uniqueness, profiles, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(s: &str) -> ContentHash {
        ContentHash::of_bytes(s.as_bytes())
    }
    fn req() -> MultimodalIngestionRequest4 {
        MultimodalIngestionRequest4 {
            request_id: "request:ingest".into(),
            study_id: "study:1".into(),
            requester: "imaging-scientist".into(),
            purpose: "harmonize".into(),
            semantic_profile: "multi:v1".into(),
            required_modalities: vec!["imaging".into()],
            minimum_quality_milli: 70,
            checkpoint: 1,
            budget_units: 10,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn obs(id: &str, state: IngestionEvidenceState) -> ModalityObservation4 {
        ModalityObservation4 {
            observation_id: id.into(),
            modality: "imaging".into(),
            study_id: "study:1".into(),
            origin: "site-a".into(),
            schema_version: "ome-ngff:0.5".into(),
            semantic_profile: "multi:v1".into(),
            unit_profile: "um".into(),
            coordinate_profile: "xyz".into(),
            quality_milli: 90,
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
        assert_eq!(multimodal_ingestion_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn qualified_is_replayable() {
        let r = operate_multimodal_ingestion(
            &req(),
            &[
                obs("b", IngestionEvidenceState::Supported),
                obs("a", IngestionEvidenceState::Proven),
            ],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_multimodal_ingestion(&req(), &[obs("a", IngestionEvidenceState::Unknown)])
            .unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn contradiction_blocks() {
        let r =
            operate_multimodal_ingestion(&req(), &[obs("a", IngestionEvidenceState::Contradicted)])
                .unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn missing_modality_is_unresolved() {
        let mut q = req();
        q.required_modalities = vec!["omics".into()];
        let r = operate_multimodal_ingestion(&q, &[obs("a", IngestionEvidenceState::Supported)])
            .unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn duplicate_is_rejected() {
        assert!(operate_multimodal_ingestion(
            &req(),
            &[
                obs("a", IngestionEvidenceState::Supported),
                obs("a", IngestionEvidenceState::Supported)
            ]
        )
        .is_err());
    }
}
