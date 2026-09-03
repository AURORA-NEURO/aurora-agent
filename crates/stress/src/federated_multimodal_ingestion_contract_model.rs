//! Federated continual multimodal-ingestion contract model (`AFA-stress-P06-F08`).
//!
//! The model operates on modality manifests and digests rather than raw imaging or omics bytes.
//! It gives a preclinical neuroscientist a deterministic harmonization contract while keeping
//! locality, omissions, semantic loss, and federation failures explicit.
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
pub const FEATURE_ID: &str = "AFA-stress-P06-F08";
pub const CONTRACT_VERSION: &str =
    "stress-federated-continual-multimodal-ingestion-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "RawModalityBundle4@1";
pub const OUTPUT_SCHEMA: &str = "HarmonizedResearchObject2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.harmonized-research-object-2+json";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityManifest4 {
    pub modality_id: String,
    pub modality_type: String,
    pub schema_version: String,
    pub source_digest: ContentHash,
    pub qc_digest: Option<ContentHash>,
    pub units: BTreeMap<String, String>,
    pub coordinate_system: Option<String>,
    pub feature_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawModalityBundle4 {
    pub schema_version: String,
    pub request_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub required_modalities: Vec<String>,
    pub modalities: Vec<ModalityManifest4>,
    pub peer_order: Vec<String>,
    pub peer_digests: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedResearchObject2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub modality_order: Vec<String>,
    pub qualified_modality_order: Vec<String>,
    pub unresolved_modality_order: Vec<String>,
    pub blocked_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub object_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MultimodalIngestionContractError {
    #[error("invalid multimodal ingestion request: {0}")]
    Invalid(String),
    #[error("invalid harmonized object: {0}")]
    Output(String),
    #[error("harmonized object artifact failed: {0}")]
    Artifact(String),
}
fn text(v: &str) -> bool {
    !v.trim().is_empty()
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
pub fn federated_multimodal_ingestion_contract_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"stress".into(),consumers:["preclinical neuroscientist".into(),"multimodal ingestion operator".into(),"federation steward".into()].into(),behavior:"validate multimodal modality manifests into a deterministic harmonized research-object contract without reading raw bytes".into(),value:"lets imaging and multi-omics studies exchange comparable metadata while preserving semantic loss, omissions, and local-data boundaries".into(),inputs:vec![TypedPort{name:"raw_modality_bundle".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"harmonized_research_object".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::WriteLocalArtifact].into(),permissions:["read:local-research-artifacts".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"ro-crate-1.3".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())},EvidenceReference{source_id:"ome-ngff-rfc5".into(),state:EvidenceState::Supported,locator:Some("https://ngff.openmicroscopy.org/rfc/5/".into())},EvidenceReference{source_id:"anndata-format".into(),state:EvidenceState::Supported,locator:Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
fn validate_request(r: &RawModalityBundle4) -> Result<(), MultimodalIngestionContractError> {
    if r.schema_version != INPUT_SCHEMA
        || !text(&r.request_id)
        || !text(&r.study_id)
        || !text(&r.semantic_profile)
        || r.required_modalities.is_empty()
        || !ordered(&r.required_modalities)
        || r.modalities.is_empty()
        || !ordered(&r.peer_order)
        || r.peer_order.len() != r.peer_digests.len()
        || !digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
    {
        return Err(MultimodalIngestionContractError::Invalid(
            "identity, required modalities, peers, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for m in &r.modalities {
        if !text(&m.modality_id)
            || !ids.insert(m.modality_id.clone())
            || !text(&m.modality_type)
            || !text(&m.schema_version)
            || !digest(&m.source_digest)
            || !ordered(&m.feature_order)
            || m.qc_digest.as_ref().is_some_and(|d| !digest(d))
        {
            return Err(MultimodalIngestionContractError::Invalid(
                "modality identity, ordering, schema, or digest is invalid".into(),
            ));
        }
    }
    for d in &r.peer_digests {
        if !digest(d) {
            return Err(MultimodalIngestionContractError::Invalid(
                "peer digest is invalid".into(),
            ));
        }
    }
    Ok(())
}
impl HarmonizedResearchObject2 {
    pub fn validate(&self) -> Result<(), MultimodalIngestionContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.modality_order.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
        {
            return Err(MultimodalIngestionContractError::Output(
                "harmonized identity, locality, disposition, or modalities are incomplete".into(),
            ));
        }
        for v in [
            &self.modality_order,
            &self.qualified_modality_order,
            &self.unresolved_modality_order,
            &self.blocked_modality_order,
            &self.missing_modality_order,
            &self.semantic_loss_order,
            &self.peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
        ] {
            if !ordered(v) {
                return Err(MultimodalIngestionContractError::Output(
                    "harmonized ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.modality_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .qualified_modality_order
            .iter()
            .chain(&self.unresolved_modality_order)
            .chain(&self.blocked_modality_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.modality_order.len() || parts != ids {
            return Err(MultimodalIngestionContractError::Output(
                "modality states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.object_digest)
            || self.artifact.content_hash != self.object_digest
        {
            return Err(MultimodalIngestionContractError::Output(
                "harmonized digest is invalid".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| MultimodalIngestionContractError::Output(e.to_string()))
    }
}
pub fn harmonize_federated_multimodal(
    r: &RawModalityBundle4,
) -> Result<HarmonizedResearchObject2, MultimodalIngestionContractError> {
    validate_request(r)?;
    let mut mods = r.modalities.clone();
    mods.sort_by(|a, b| a.modality_id.cmp(&b.modality_id));
    let modality_order = mods
        .iter()
        .map(|m| m.modality_id.clone())
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut loss = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for m in &mods {
        if m.negative_result {
            negative.insert(format!("{}:negative-result", m.modality_id));
        }
        if m.qc_digest.is_none() {
            loss.insert(format!("{}:qc-digest-missing", m.modality_id));
        }
        if m.evidence_state == EvidenceState::Contradicted {
            blocked.insert(m.modality_id.clone());
            loss.insert(format!("{}:contradicted", m.modality_id));
        } else if matches!(
            m.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) || m.qc_digest.is_none()
        {
            unresolved.insert(m.modality_id.clone());
            uncertainty.insert(format!("{}:quality-or-evidence-uncertain", m.modality_id));
        } else {
            qualified.insert(m.modality_id.clone());
        }
    }
    for req in &r.required_modalities {
        if !mods
            .iter()
            .any(|m| &m.modality_id == req || &m.modality_type == req)
        {
            missing.insert(req.clone());
            omissions.insert(format!("request:missing-modality:{req}"));
        }
    }
    let mut missing_peer: BTreeSet<String> = BTreeSet::new();
    if r.peer_order.is_empty() {
        missing_peer.insert("request:peer-quorum-missing".into());
    }
    let global = !r.policy_allow || !r.protected_closure || !r.raw_data_local || !r.aggregate_only;
    if global {
        blocked.extend(modality_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        omissions.insert("request:policy-protected-closure-or-locality-blocked".into());
    }
    let disposition = if global || (!blocked.is_empty() && qualified.is_empty()) {
        "blocked"
    } else if !unresolved.is_empty()
        || !missing.is_empty()
        || !missing_peer.is_empty()
        || !blocked.is_empty()
    {
        "partial"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:harmonization-closure-not-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r.request_id,"study_id":r.study_id,"semantic_profile":r.semantic_profile,"disposition":disposition,"modality_order":modality_order,"qualified_modality_order":qualified,"unresolved_modality_order":unresolved,"blocked_modality_order":blocked,"missing_modality_order":missing,"semantic_loss_order":loss,"peer_order":r.peer_order,"missing_peer_order":missing_peer,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":r.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("stress-harmonized:{}", r.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| MultimodalIngestionContractError::Artifact(e.to_string()))?;
    let object_digest = artifact.content_hash.clone();
    let out = HarmonizedResearchObject2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        study_id: r.study_id.clone(),
        semantic_profile: r.semantic_profile.clone(),
        disposition: disposition.into(),
        modality_order: serde_json::from_value(payload["modality_order"].clone()).unwrap(),
        qualified_modality_order: serde_json::from_value(
            payload["qualified_modality_order"].clone(),
        )
        .unwrap(),
        unresolved_modality_order: serde_json::from_value(
            payload["unresolved_modality_order"].clone(),
        )
        .unwrap(),
        blocked_modality_order: serde_json::from_value(payload["blocked_modality_order"].clone())
            .unwrap(),
        missing_modality_order: serde_json::from_value(payload["missing_modality_order"].clone())
            .unwrap(),
        semantic_loss_order: serde_json::from_value(payload["semantic_loss_order"].clone())
            .unwrap(),
        peer_order: r.peer_order.clone(),
        missing_peer_order: serde_json::from_value(payload["missing_peer_order"].clone()).unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        replay_identity: r.replay_identity.clone(),
        object_digest,
        artifact,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
pub fn harmonize_federated_multimodal_json(
    v: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let r: RawModalityBundle4 = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(harmonize_federated_multimodal(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> RawModalityBundle4 {
        RawModalityBundle4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "ingest-1".into(),
            study_id: "study-a".into(),
            semantic_profile: "profile:v1".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            modalities: vec![
                ModalityManifest4 {
                    modality_id: "imaging".into(),
                    modality_type: "imaging".into(),
                    schema_version: "ome-ngff-0.5".into(),
                    source_digest: h("source"),
                    qc_digest: Some(h("qc")),
                    units: BTreeMap::new(),
                    coordinate_system: Some("lab".into()),
                    feature_order: vec!["f1".into()],
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                },
                ModalityManifest4 {
                    modality_id: "omics".into(),
                    modality_type: "omics".into(),
                    schema_version: "anndata-0.9".into(),
                    source_digest: h("source2"),
                    qc_digest: Some(h("qc2")),
                    units: BTreeMap::new(),
                    coordinate_system: None,
                    feature_order: vec!["g1".into()],
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                },
            ],
            peer_order: vec!["site-a".into()],
            peer_digests: vec![h("peer")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_multimodal_ingestion_contract_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified_object() {
        assert_eq!(
            harmonize_federated_multimodal(&req()).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn missing_modality_partial() {
        let mut r = req();
        r.modalities.pop();
        assert_eq!(
            harmonize_federated_multimodal(&r).unwrap().disposition,
            "partial"
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allow = false;
        assert_eq!(
            harmonize_federated_multimodal(&r).unwrap().disposition,
            "blocked"
        )
    }
    #[test]
    fn unknown_quality_retained() {
        let mut r = req();
        r.modalities[0].evidence_state = EvidenceState::Unknown;
        assert!(!harmonize_federated_multimodal(&r)
            .unwrap()
            .uncertainty_order
            .is_empty())
    }
}
