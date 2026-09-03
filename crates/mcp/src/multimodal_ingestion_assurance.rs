//! Federated continual multimodal-ingestion assurance for MCP.
//!
//! Atlas feature: `AFA-mcp-P06-F28`.  The harness verifies modality manifests
//! and aggregate-only peer attestations before a harmonized research object
//! can be exposed through an MCP surface. It never reads raw imaging/omics
//! bytes, performs harmonization, or exports institution-local data.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mcp-P06-F28";
pub const CONTRACT_VERSION: &str =
    "mcp-federated-continual-multimodal-ingestion-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "RawModalityBundle4@1";
pub const OUTPUT_SCHEMA: &str = "HarmonizedResearchObject7@1";
pub const TOOL_NAME: &str = "mcp_multimodal_ingestion_assurance";
const CONTENT_TYPE: &str = "application/vnd.aurora.mcp-harmonized-research-object-7+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityState {
    Complete,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawModalityAttestation {
    pub modality_id: String,
    pub study_id: String,
    pub modality_kind: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub state: ModalityState,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub qc_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub raw_bytes_carried: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerModalitySummary {
    pub institution_id: String,
    pub harmonization_digest: ContentHash,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionRequest {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_modality_order: Vec<String>,
    pub modalities: Vec<RawModalityAttestation>,
    pub peers: Vec<PeerModalitySummary>,
    pub minimum_peer_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedResearchObjectReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: IngestionDisposition,
    pub modality_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub harmonization_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MultimodalIngestionError {
    #[error("invalid multimodal ingestion request: {0}")]
    Invalid(String),
    #[error("multimodal ingestion artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal ingestion JSON failed: {0}")]
    Json(String),
}

fn invalid(value: impl Into<String>) -> MultimodalIngestionError {
    MultimodalIngestionError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl HarmonizedResearchObjectReceipt {
    pub fn validate(&self) -> Result<(), MultimodalIngestionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.modality_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "ingestion identity, locality, modalities, peers, or effects are incomplete",
            ));
        }
        for values in [
            &self.modality_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_modality_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.semantic_loss_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("ingestion ordering is not canonical"));
            }
        }
        let ids = self.modality_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("modality states do not partition modalities"));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_parts.len() != peers.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(invalid("peer states do not partition peers"));
        }
        for value in [
            &self.replay_identity,
            &self.harmonization_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("ingestion digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalIngestionError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("ingestion artifact type is invalid"));
        }
        if self.disposition == IngestionDisposition::Qualified
            && self.effect_receipts
                != [format!(
                    "verify:mcp-multimodal-ingestion:{}",
                    self.request_id
                )]
        {
            return Err(invalid("qualified ingestion effect is invalid"));
        }
        if self.disposition != IngestionDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified ingestion must block release"));
        }
        Ok(())
    }
}

pub fn multimodal_ingestion_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "mcp".into(),
        consumers: BTreeSet::from([String::from("AURORA extension developer"), String::from("multimodal data steward"), String::from("federation operator")]),
        behavior: "verifies typed local imaging and omics modality attestations and aggregate-only peer summaries under schema, QC, provenance, replay, semantic-loss, policy, locality, and federation gates without reading raw bytes".into(),
        value: "prevents incomplete, contradictory, unmeasured, semantically drifting, or unauthorized modalities from silently becoming a harmonized research object".into(),
        inputs: vec![TypedPort { name: "raw_modality_bundle".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "harmonized_research_object".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport]), permissions: BTreeSet::from([String::from("evaluate:capability-runs")]), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }, EvidenceReference { source_id: "anndata-format".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) }, EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_multimodal_ingestion(
    request: &MultimodalIngestionRequest,
) -> Result<HarmonizedResearchObjectReceipt, MultimodalIngestionError> {
    validate_request(request)?;
    let mut modalities = request.modalities.clone();
    modalities.sort_by(|a, b| a.modality_id.cmp(&b.modality_id));
    let modality_order = modalities
        .iter()
        .map(|m| m.modality_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    for m in &modalities {
        if m.negative_result {
            negative.insert(format!("{}:negative-result", m.modality_id));
        }
        omissions.extend(m.omissions.iter().map(|v| format!("{}:{v}", m.modality_id)));
        uncertainty.extend(
            m.uncertainty
                .iter()
                .map(|v| format!("{}:{v}", m.modality_id)),
        );
        if m.state == ModalityState::Contradicted {
            blocked.insert(m.modality_id.clone());
        } else if !m.local_only || !m.permitted || m.raw_bytes_carried {
            blocked.insert(m.modality_id.clone());
        } else if m.semantic_profile != request.semantic_profile
            || m.replay_identity != request.replay_identity
            || m.schema_version.trim().is_empty()
            || !digest(&m.content_digest)
            || !digest(&m.provenance_digest)
            || !digest(&m.qc_digest)
            || m.state == ModalityState::Unmeasured
        {
            unresolved.insert(m.modality_id.clone());
            semantic_loss.insert(format!("{}:unmeasured-or-unverified", m.modality_id));
        } else if m.state == ModalityState::Unknown
            || !m.omissions.is_empty()
            || !m.uncertainty.is_empty()
        {
            unresolved.insert(m.modality_id.clone());
        } else {
            selected.insert(m.modality_id.clone());
        }
    }
    let missing = required
        .difference(&modality_order.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omissions.insert(format!("{id}:required-modality-missing"));
    }
    let mut peer_order = request
        .peers
        .iter()
        .map(|p| p.institution_id.clone())
        .collect::<Vec<_>>();
    peer_order.sort();
    let mut qualified_peer = BTreeSet::new();
    let mut missing_peer = BTreeSet::new();
    for p in &request.peers {
        if p.signed
            && p.permitted
            && p.aggregate_only
            && p.semantic_profile == request.semantic_profile
            && p.replay_identity == request.replay_identity
            && digest(&p.harmonization_digest)
        {
            qualified_peer.insert(p.institution_id.clone());
        } else {
            missing_peer.insert(p.institution_id.clone());
        }
    }
    if qualified_peer.len() < request.minimum_peer_quorum as usize {
        uncertainty.insert("request:peer-quorum-incomplete".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(modality_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:mcp-multimodal-release-gate-blocked".into());
    }
    let required_block = required.iter().any(|id| blocked.contains(id));
    let disposition = if global_block || required_block {
        IngestionDisposition::Blocked
    } else if required.is_subset(&selected)
        && missing.is_empty()
        && qualified_peer.len() >= request.minimum_peer_quorum as usize
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        IngestionDisposition::Qualified
    } else {
        IngestionDisposition::Unresolved
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_modality_order = missing.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peer.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peer.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let semantic_loss_order = semantic_loss.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == IngestionDisposition::Qualified {
        vec![format!(
            "verify:mcp-multimodal-ingestion:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"semantic_profile":request.semantic_profile,"disposition":disposition,"modality_order":modality_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_modality_order":missing_modality_order,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order,"semantic_loss_order":semantic_loss_order,"replay_identity":request.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":request.raw_data_local,"aggregate_only":request.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let harmonization_digest = ContentHash::of_value(&payload)
        .map_err(|e| MultimodalIngestionError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("mcp-harmonized-research-object:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| MultimodalIngestionError::Artifact(e.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect::<Vec<String>>()
    };
    let result = HarmonizedResearchObjectReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        modality_order: strings("modality_order"),
        selected_order: strings("selected_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        missing_modality_order: strings("missing_modality_order"),
        peer_order: strings("peer_order"),
        qualified_peer_order: strings("qualified_peer_order"),
        missing_peer_order: strings("missing_peer_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        semantic_loss_order: strings("semantic_loss_order"),
        replay_identity: request.replay_identity.clone(),
        harmonization_digest,
        artifact,
        effect_receipts: strings("effect_receipts"),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

pub fn assure_multimodal_ingestion_json(value: &Value) -> Result<Value, String> {
    let request: MultimodalIngestionRequest = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid MCP multimodal ingestion request: {e}"))?;
    let receipt = assure_multimodal_ingestion(&request).map_err(|e| e.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|e| format!("cannot serialize MCP multimodal ingestion receipt: {e}"))
}
pub fn validate_multimodal_ingestion_json(
    value: &Value,
) -> Result<HarmonizedResearchObjectReceipt, String> {
    let receipt: HarmonizedResearchObjectReceipt = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid MCP multimodal ingestion receipt: {e}"))?;
    receipt.validate().map_err(|e| e.to_string())?;
    if receipt.feature_id != FEATURE_ID {
        return Err("MCP multimodal ingestion feature id mismatch".into());
    }
    Ok(receipt)
}

fn validate_request(request: &MultimodalIngestionRequest) -> Result<(), MultimodalIngestionError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_modality_order.is_empty()
        || !canonical(&request.required_modality_order)
        || request.modalities.is_empty()
        || request.peers.is_empty()
        || request.minimum_peer_quorum == 0
        || request.minimum_peer_quorum as usize > request.peers.len()
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid("MCP ingestion identity, modality closure, quorum, replay, locality, or boundary is invalid"));
    }
    let mut ids = BTreeSet::new();
    for m in &request.modalities {
        if m.modality_id.trim().is_empty()
            || !ids.insert(m.modality_id.clone())
            || m.study_id.trim().is_empty()
            || m.modality_kind.trim().is_empty()
            || m.semantic_profile.trim().is_empty()
            || m.schema_version.trim().is_empty()
            || !digest(&m.content_digest)
            || !digest(&m.provenance_digest)
            || !digest(&m.qc_digest)
            || !digest(&m.replay_identity)
            || !canonical(&m.omissions)
            || !canonical(&m.uncertainty)
        {
            return Err(invalid(format!(
                "modality {} is malformed or duplicated",
                m.modality_id
            )));
        }
    }
    let mut peers = BTreeSet::new();
    for p in &request.peers {
        if p.institution_id.trim().is_empty()
            || !peers.insert(p.institution_id.clone())
            || !digest(&p.harmonization_digest)
            || !digest(&p.replay_identity)
            || p.semantic_profile.trim().is_empty()
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                p.institution_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> MultimodalIngestionRequest {
        let d = hash("mcp");
        let m = |id: &str| RawModalityAttestation {
            modality_id: id.into(),
            study_id: format!("study:{id}"),
            modality_kind: id.into(),
            semantic_profile: "ome-v1".into(),
            schema_version: "RawModalityBundle4@1".into(),
            state: ModalityState::Complete,
            content_digest: d.clone(),
            provenance_digest: d.clone(),
            qc_digest: d.clone(),
            replay_identity: d.clone(),
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_only: true,
            permitted: true,
            raw_bytes_carried: false,
        };
        let p = |id: &str| PeerModalitySummary {
            institution_id: id.into(),
            harmonization_digest: d.clone(),
            semantic_profile: "ome-v1".into(),
            replay_identity: d.clone(),
            signed: true,
            permitted: true,
            aggregate_only: true,
        };
        MultimodalIngestionRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "mcp:one".into(),
            federation_id: "fed:mcp".into(),
            semantic_profile: "ome-v1".into(),
            required_modality_order: vec!["imaging".into()],
            modalities: vec![m("imaging")],
            peers: vec![p("inst:a"), p("inst:b")],
            minimum_peer_quorum: 2,
            replay_identity: d,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_ingestion_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_multimodal_ingestion(&request()).unwrap().disposition,
            IngestionDisposition::Qualified
        )
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            assure_multimodal_ingestion(&request())
                .unwrap()
                .harmonization_digest,
            assure_multimodal_ingestion(&request())
                .unwrap()
                .harmonization_digest
        )
    }
    #[test]
    fn unmeasured_unresolved() {
        let mut q = request();
        q.modalities[0].state = ModalityState::Unmeasured;
        assert_eq!(
            assure_multimodal_ingestion(&q).unwrap().disposition,
            IngestionDisposition::Unresolved
        )
    }
    #[test]
    fn carried_raw_blocks() {
        let mut q = request();
        q.modalities[0].raw_bytes_carried = true;
        assert_eq!(
            assure_multimodal_ingestion(&q).unwrap().disposition,
            IngestionDisposition::Blocked
        )
    }
    #[test]
    fn quorum_unresolved() {
        let mut q = request();
        q.peers[1].signed = false;
        assert_eq!(
            assure_multimodal_ingestion(&q).unwrap().disposition,
            IngestionDisposition::Unresolved
        )
    }
    #[test]
    fn adversarial_blocks() {
        let mut q = request();
        q.adversarial_events.push("poisoned-modality".into());
        assert_eq!(
            assure_multimodal_ingestion(&q).unwrap().disposition,
            IngestionDisposition::Blocked
        )
    }
}
