//! Federated continual multimodal-ingestion assurance (`AFA-worldgen-P06-F28`).
//!
//! Qualifies synthetic-world modality summaries before downstream research workflows consume
//! them. The contract is digest-only: it never imports raw images/omics, executes a pipeline, or
//! turns incomplete modality closure into a pass.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P06-F28";
pub const CONTRACT_VERSION: &str =
    "worldgen-federated-continual-multimodal-ingestion-assurance/1.0";
pub const INPUT_SCHEMA: &str = "WorldgenMultimodalIngestionRequest8@1";
pub const OUTPUT_SCHEMA: &str = "WorldgenHarmonizedIngestionReceipt10@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen-harmonized-ingestion-receipt-10+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenModalityObservation6 {
    pub observation_id: String,
    pub world_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub quality_milli: u32,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: IngestionEvidenceState,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenMultimodalIngestionRequest8 {
    pub request_id: String,
    pub world_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_modality_order: Vec<String>,
    pub observations: Vec<WorldgenModalityObservation6>,
    pub replay_identity: ContentHash,
    pub quality_floor_milli: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenHarmonizedIngestionReceipt10Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenHarmonizedIngestionReceipt10 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub world_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub required_modality_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub selected_observation_order: Vec<String>,
    pub unresolved_observation_order: Vec<String>,
    pub blocked_observation_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub quality_failed_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub harmonization_digest: ContentHash,
    pub artifact: WorldgenHarmonizedIngestionReceipt10Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalIngestionAssuranceError {
    #[error("invalid worldgen multimodal-ingestion request: {0}")]
    Invalid(String),
    #[error("worldgen multimodal-ingestion report failed validation: {0}")]
    Report(String),
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

pub fn multimodal_ingestion_assurance_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","multimodal ingestion operator","benchmark curator"],"behavior":"qualify synthetic-world multimodal observation summaries with deterministic quality, modality closure, semantic-profile, evidence, replay, provenance, policy, and locality gates","value":"prevents incomplete or unmeasured worldgen modality bundles from entering downstream research workflows","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:worldgen-harmonized-digests","manage:local-capability"],"permissions":["read:local-modality-summaries","request:worldgen-multimodal-ingestion"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl WorldgenHarmonizedIngestionReceipt10 {
    pub fn validate(&self) -> Result<(), MultimodalIngestionAssuranceError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.world_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_modality_order.is_empty()
            || self.observation_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(MultimodalIngestionAssuranceError::Report("ingestion identity, modalities, observations, effects, locality, or disposition is incomplete".into()));
        }
        for values in [
            &self.required_modality_order,
            &self.observation_order,
            &self.selected_observation_order,
            &self.unresolved_observation_order,
            &self.blocked_observation_order,
            &self.modality_order,
            &self.missing_modality_order,
            &self.quality_failed_order,
            &self.contradiction_order,
            &self.stale_order,
            &self.evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(MultimodalIngestionAssuranceError::Report(
                    "ingestion ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let parts = self
            .selected_observation_order
            .iter()
            .chain(&self.unresolved_observation_order)
            .chain(&self.blocked_observation_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.observation_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.harmonization_digest)
            || self.artifact.content_hash != self.harmonization_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(MultimodalIngestionAssuranceError::Report(
                "ingestion states, digests, or artifact metadata do not partition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:worldgen-harmonized-digests:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalIngestionAssuranceError::Report(
                "effect is outside governed ingestion gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalIngestionAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalIngestionAssuranceError::Report(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalIngestionAssuranceError::Report(error.to_string()))
    }
}

fn validate_request(
    request: &WorldgenMultimodalIngestionRequest8,
) -> Result<(), MultimodalIngestionAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.world_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_modality_order.is_empty()
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(MultimodalIngestionAssuranceError::Invalid("ingestion identity, modality requirements, observation bound, replay, locality, or boundary is invalid".into()));
    }
    let required = BTreeSet::from_iter(request.required_modality_order.iter().cloned());
    if required.len() != request.required_modality_order.len()
        || request
            .required_modality_order
            .iter()
            .any(|modality| modality.trim().is_empty())
    {
        return Err(MultimodalIngestionAssuranceError::Invalid(
            "required modalities must be unique and non-empty".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || !ids.insert(observation.observation_id.clone())
            || observation.world_id.trim().is_empty()
            || observation.modality.trim().is_empty()
            || observation.semantic_profile.trim().is_empty()
            || !valid_digest(&observation.artifact_digest)
            || !valid_digest(&observation.provenance_digest)
            || !valid_digest(&observation.replay_identity)
            || !observation.local
            || !observation.aggregate_only
        {
            return Err(MultimodalIngestionAssuranceError::Invalid(format!(
                "observation {} is invalid, duplicated, non-local, or not digest-bound",
                observation.observation_id
            )));
        }
    }
    Ok(())
}

pub fn assure_worldgen_multimodal_ingestion(
    request: &WorldgenMultimodalIngestionRequest8,
) -> Result<WorldgenHarmonizedIngestionReceipt10, MultimodalIngestionAssuranceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let observation_order = observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut quality_failed = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let stale: BTreeSet<String> = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for observation in &observations {
        modalities.insert(observation.modality.clone());
        provenance.insert(observation.provenance_digest.clone());
        if observation.world_id != request.world_id {
            unresolved.insert(observation.observation_id.clone());
            uncertainty.insert(format!("{}:world-id", observation.observation_id));
        } else if observation.semantic_profile != request.semantic_profile {
            unresolved.insert(observation.observation_id.clone());
            uncertainty.insert(format!("{}:semantic-profile", observation.observation_id));
        } else if observation.replay_identity != request.replay_identity {
            unresolved.insert(observation.observation_id.clone());
            uncertainty.insert(format!("{}:replay-identity", observation.observation_id));
        } else if observation.quality_milli < request.quality_floor_milli {
            unresolved.insert(observation.observation_id.clone());
            quality_failed.insert(observation.observation_id.clone());
            negative.insert(format!(
                "{}:quality-below-floor",
                observation.observation_id
            ));
        } else if observation.evidence_state == IngestionEvidenceState::Contradicted {
            blocked.insert(observation.observation_id.clone());
            contradiction.insert(observation.observation_id.clone());
            negative.insert(format!("{}:contradicted", observation.observation_id));
        } else if !matches!(
            observation.evidence_state,
            IngestionEvidenceState::Proven | IngestionEvidenceState::Supported
        ) {
            unresolved.insert(observation.observation_id.clone());
            evidence.insert(observation.observation_id.clone());
            uncertainty.insert(format!("{}:evidence-state", observation.observation_id));
        } else {
            selected.insert(observation.observation_id.clone());
        }
    }
    for modality in &request.required_modality_order {
        if !modalities.contains(modality) {
            missing.insert(modality.clone());
            omissions.insert(format!("modality:{}:missing", modality));
            negative.insert(format!("modality:{}:no-observation", modality));
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(observation_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global || selected_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !blocked_order.is_empty() || !unresolved_order.is_empty() || !missing.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:multimodal-ingestion-not-closed".into());
    }
    let mut effect_order = if disposition == "qualified" {
        vec![
            "exchange:worldgen-harmonized-digests".to_string(),
            "manage:local-capability".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    effect_order.sort();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"world_id":request.world_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"required_modality_order":request.required_modality_order,"observation_order":observation_order,"selected_observation_order":selected_order,"unresolved_observation_order":unresolved_order,"blocked_observation_order":blocked_order,"modality_order":modalities.into_iter().collect::<Vec<_>>(),"missing_modality_order":missing.into_iter().collect::<Vec<_>>(),"quality_failed_order":quality_failed.into_iter().collect::<Vec<_>>(),"contradiction_order":contradiction.into_iter().collect::<Vec<_>>(),"stale_order":stale.into_iter().collect::<Vec<_>>(),"evidence_order":evidence.into_iter().collect::<Vec<_>>(),"omission_order":omissions.into_iter().collect::<Vec<_>>(),"uncertainty_order":uncertainty.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| MultimodalIngestionAssuranceError::Report(error.to_string()))?;
    let report = WorldgenHarmonizedIngestionReceipt10 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        world_id: request.world_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        required_modality_order: serde_json::from_value(payload["required_modality_order"].clone())
            .unwrap(),
        observation_order: serde_json::from_value(payload["observation_order"].clone()).unwrap(),
        selected_observation_order: serde_json::from_value(
            payload["selected_observation_order"].clone(),
        )
        .unwrap(),
        unresolved_observation_order: serde_json::from_value(
            payload["unresolved_observation_order"].clone(),
        )
        .unwrap(),
        blocked_observation_order: serde_json::from_value(
            payload["blocked_observation_order"].clone(),
        )
        .unwrap(),
        modality_order: serde_json::from_value(payload["modality_order"].clone()).unwrap(),
        missing_modality_order: serde_json::from_value(payload["missing_modality_order"].clone())
            .unwrap(),
        quality_failed_order: serde_json::from_value(payload["quality_failed_order"].clone())
            .unwrap(),
        contradiction_order: serde_json::from_value(payload["contradiction_order"].clone())
            .unwrap(),
        stale_order: serde_json::from_value(payload["stale_order"].clone()).unwrap(),
        evidence_order: serde_json::from_value(payload["evidence_order"].clone()).unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        effect_order: serde_json::from_value(payload["effect_order"].clone()).unwrap(),
        replay_identity: request.replay_identity.clone(),
        harmonization_digest: digest.clone(),
        artifact: WorldgenHarmonizedIngestionReceipt10Artifact {
            artifact_id: format!(
                "worldgen-harmonized-ingestion-receipt-10:{}",
                request.request_id
            ),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: effect_order
            .iter()
            .map(|effect| {
                if effect == "block:unsafe-release" {
                    effect.clone()
                } else {
                    format!("{effect}:{}", request.request_id)
                }
            })
            .collect(),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(id: &str, modality: &str) -> WorldgenModalityObservation6 {
        WorldgenModalityObservation6 {
            observation_id: id.into(),
            world_id: "world:synthetic".into(),
            modality: modality.into(),
            semantic_profile: "ome-v1".into(),
            quality_milli: 950,
            artifact_digest: h(id),
            provenance_digest: h(&format!("prov:{id}")),
            evidence_state: IngestionEvidenceState::Supported,
            replay_identity: h("replay"),
            local: true,
            aggregate_only: true,
        }
    }
    fn request() -> WorldgenMultimodalIngestionRequest8 {
        WorldgenMultimodalIngestionRequest8 {
            request_id: "request:worldgen".into(),
            world_id: "world:synthetic".into(),
            purpose: "harmonize".into(),
            semantic_profile: "ome-v1".into(),
            required_modality_order: vec!["imaging".into(), "transcriptomics".into()],
            observations: vec![
                observation("obs:b", "transcriptomics"),
                observation("obs:a", "imaging"),
            ],
            replay_identity: h("replay"),
            quality_floor_milli: 900,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_ingestion_assurance_manifest()["autonomy_tier"],
            "A1"
        );
    }
    #[test]
    fn complete_bundle_is_qualified() {
        assert_eq!(
            assure_worldgen_multimodal_ingestion(&request())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn missing_modality_is_unresolved() {
        let mut q = request();
        q.observations.pop();
        assert_eq!(
            assure_worldgen_multimodal_ingestion(&q)
                .unwrap()
                .disposition,
            "unresolved"
        );
    }
    #[test]
    fn quality_failure_is_unresolved() {
        let mut q = request();
        q.observations[0].quality_milli = 1;
        assert_eq!(
            assure_worldgen_multimodal_ingestion(&q)
                .unwrap()
                .disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            assure_worldgen_multimodal_ingestion(&q)
                .unwrap()
                .disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = assure_worldgen_multimodal_ingestion(&request()).unwrap();
        let b = assure_worldgen_multimodal_ingestion(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
