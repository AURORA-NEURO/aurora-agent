//! Multimodal provenance/signing research copilot.
//!
//! Atlas feature: `AFA-lens-P18-F10`.
//!
//! The copilot compiles caller-supplied artifact/derivation attestations into a deterministic
//! signed-provenance envelope. It never authenticates a key or moves an artifact; it only allows a
//! bounded declared-tool invocation when every lineage, evidence, replay, scope, policy, and
//! locality gate is closed.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lens-P18-F10";
pub const CONTRACT_VERSION: &str = "lens-multimodal-provenance-signing-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ArtifactAndDerivation2@1";
pub const OUTPUT_SCHEMA: &str = "SignedProvenanceEnvelope3@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.lens-signed-provenance-envelope-3+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAndDerivation2 {
    pub artifact_id: String,
    pub study_id: String,
    pub site_id: String,
    pub modality: String,
    pub derivation_id: String,
    pub artifact_digest: ContentHash,
    pub source_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub signer_id: String,
    pub signer_key_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub scope_compatible: bool,
    pub policy_allowed: bool,
    pub negative_result: bool,
    pub omissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSigningRequest {
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub artifacts: Vec<ArtifactAndDerivation2>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceEnvelope3 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub artifact_order: Vec<String>,
    pub selected_artifact_order: Vec<String>,
    pub unresolved_artifact_order: Vec<String>,
    pub blocked_artifact_order: Vec<String>,
    pub missing_artifact_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_study_order: Vec<String>,
    pub selected_modality_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub signer_order: Vec<String>,
    pub selected_signer_order: Vec<String>,
    pub missing_signer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub envelope_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: String,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProvenanceSigningError {
    #[error("invalid provenance-signing request: {0}")]
    Invalid(String),
    #[error("provenance-signing artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl SignedProvenanceEnvelope3 {
    pub fn validate(&self) -> Result<(), ProvenanceSigningError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.artifact_order.is_empty()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.signer_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.autonomy_tier != "a2"
            || !self.raw_data_local
            || !self.aggregate_only
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ProvenanceSigningError::Invalid(
                "envelope identity, axes, locality, autonomy, boundary, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.artifact_order,
            &self.selected_artifact_order,
            &self.unresolved_artifact_order,
            &self.blocked_artifact_order,
            &self.missing_artifact_order,
            &self.study_order,
            &self.modality_order,
            &self.selected_study_order,
            &self.selected_modality_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.signer_order,
            &self.selected_signer_order,
            &self.missing_signer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(ProvenanceSigningError::Invalid(
                    "provenance order is not canonical".into(),
                ));
            }
        }
        let partition = self
            .selected_artifact_order
            .iter()
            .chain(self.unresolved_artifact_order.iter())
            .chain(self.blocked_artifact_order.iter())
            .chain(self.missing_artifact_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.artifact_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.artifact_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(ProvenanceSigningError::Invalid(
                "artifact states do not partition envelope".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:declared-tools:") && effect != "block:unsafe-release"
        }) {
            return Err(ProvenanceSigningError::Invalid(
                "effect is outside declared-tool gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ProvenanceSigningError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "lens".into(), consumers: BTreeSet::from(["bioinformatician".into(),"provenance reviewer".into(),"research object publisher".into()]), behavior: "compiles multimodal artifact and derivation attestations into a deterministic signed-provenance envelope without authenticating keys or moving raw data".into(), value: "makes cross-study lineage, signing coverage, replay identity, omissions, and negative results auditable before bounded tool invocation".into(), inputs: vec![TypedPort{name:"artifact_and_derivation".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs: vec![TypedPort{name:"signed_provenance_envelope".into(),schema:OUTPUT_SCHEMA.into(),required:true}], effects: BTreeSet::from([Effect::ReadLocalData,Effect::WriteLocalArtifact]), permissions: BTreeSet::from(["invoke:declared-tools".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())},EvidenceReference{source_id:"ro-crate-1.3".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())}], authority_requirements: vec![AuthorityRequirement{role:"provenance reviewer".into(),reason:"declared-tool invocation requires explicit lineage review".into()}], autonomy_tier: AutonomyTier::A2, surfaces: BTreeSet::from([ResearchSurface::McpTool,ResearchSurface::Sdk,ResearchSurface::Protocol,ResearchSurface::Api,ResearchSurface::Policy,ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(request: &ProvenanceSigningRequest) -> Result<(), ProvenanceSigningError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_studies.is_empty()
        || request.required_modalities.is_empty()
        || request.artifacts.is_empty()
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ProvenanceSigningError::Invalid(
            "request identity, closure, locality, boundary, or schema is invalid".into(),
        ));
    }
    let ids = request
        .artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.trim().is_empty())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(ProvenanceSigningError::Invalid(
            "artifact identifiers must be present and unique".into(),
        ));
    }
    Ok(())
}

pub fn compile_provenance_envelope(
    request: &ProvenanceSigningRequest,
) -> Result<SignedProvenanceEnvelope3, ProvenanceSigningError> {
    validate_request(request)?;
    let mut artifacts = request.artifacts.clone();
    artifacts.sort_by(|left, right| {
        left.study_id
            .cmp(&right.study_id)
            .then(left.modality.cmp(&right.modality))
            .then(left.artifact_id.cmp(&right.artifact_id))
    });
    let artifact_order = artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut missing = Vec::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for artifact in &artifacts {
        if artifact.source_digest.is_none()
            || artifact.provenance_digest.is_none()
            || artifact.signer_key_digest.is_none()
        {
            missing.push(artifact.artifact_id.clone());
            omission.insert(format!(
                "{}:lineage-or-signer-missing",
                artifact.artifact_id
            ));
        } else if !artifact.scope_compatible || !artifact.policy_allowed {
            blocked.push(artifact.artifact_id.clone());
            omission.insert(format!("{}:scope-or-policy-denied", artifact.artifact_id));
        } else if artifact.evidence_state == EvidenceState::Contradicted {
            blocked.push(artifact.artifact_id.clone());
            uncertainty.insert(format!("{}:contradicted", artifact.artifact_id));
        } else if matches!(
            artifact.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) || artifact.replay_identity != request.replay_identity
        {
            unresolved.push(artifact.artifact_id.clone());
            uncertainty.insert(format!(
                "{}:unknown-or-replay-mismatch",
                artifact.artifact_id
            ));
        } else {
            selected.push(artifact.artifact_id.clone());
            if artifact.negative_result {
                negative.insert(format!("{}:negative-result", artifact.artifact_id));
            }
            omission.extend(
                artifact
                    .omissions
                    .iter()
                    .map(|entry| format!("{}:{entry}", artifact.artifact_id)),
            );
        }
    }
    let study_order = artifacts
        .iter()
        .map(|artifact| artifact.study_id.clone())
        .chain(request.required_studies.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = artifacts
        .iter()
        .map(|artifact| artifact.modality.clone())
        .chain(request.required_modalities.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let present_studies = artifacts
        .iter()
        .map(|artifact| artifact.study_id.clone())
        .collect::<BTreeSet<_>>();
    let present_modalities = artifacts
        .iter()
        .map(|artifact| artifact.modality.clone())
        .collect::<BTreeSet<_>>();
    let missing_study_order = request
        .required_studies
        .iter()
        .filter(|id| !present_studies.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let missing_modality_order = request
        .required_modalities
        .iter()
        .filter(|id| !present_modalities.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    omission.extend(
        missing_study_order
            .iter()
            .map(|id| format!("study:{id}:missing")),
    );
    omission.extend(
        missing_modality_order
            .iter()
            .map(|id| format!("modality:{id}:missing")),
    );
    omission.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("request:adversarial:{event}")),
    );
    let signer_order = artifacts
        .iter()
        .map(|artifact| artifact.signer_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_set = selected.iter().collect::<BTreeSet<_>>();
    let selected_study_order = study_order
        .iter()
        .filter(|id| {
            artifacts.iter().any(|artifact| {
                selected_set.contains(&artifact.artifact_id) && &artifact.study_id == *id
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_modality_order = modality_order
        .iter()
        .filter(|id| {
            artifacts.iter().any(|artifact| {
                selected_set.contains(&artifact.artifact_id) && &artifact.modality == *id
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_signer_order = signer_order
        .iter()
        .filter(|id| {
            artifacts.iter().any(|artifact| {
                selected_set.contains(&artifact.artifact_id) && &artifact.signer_id == *id
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_signer_order = signer_order
        .iter()
        .filter(|id| !selected_signer_order.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.federation_approved
        && request.raw_data_local
        && request.aggregate_only
        && request.signed_approval
        && request.adversarial_events.is_empty();
    let disposition = if !global_open
        || !blocked.is_empty()
        || !missing_study_order.is_empty()
        || !missing_modality_order.is_empty()
    {
        "blocked"
    } else if !missing.is_empty() || !unresolved.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let effect = if disposition == "qualified" {
        vec![format!("invoke:declared-tools:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":OUTPUT_SCHEMA,"request_id":request.request_id,"artifact_order":artifact_order,"selected_artifact_order":selected,"unresolved_artifact_order":unresolved,"blocked_artifact_order":blocked,"missing_artifact_order":missing,"study_order":study_order,"modality_order":modality_order,"disposition":disposition,"replay_identity":request.replay_identity});
    let envelope_digest = ContentHash::of_value(&payload)
        .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))?;
    let semantic_loss = omission
        .iter()
        .map(|entry| SemanticLoss {
            field: entry.clone(),
            reason: "lineage or evidence was omitted or gated".into(),
            severity: LossSeverity::DecisionRelevant,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("signed-provenance:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.request_id.clone(),
            relation: "lens-provenance-signing".into(),
            digest: envelope_digest.clone(),
        }],
    )
    .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))?;
    let receipt = SignedProvenanceEnvelope3 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        artifact_order: payload["artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_artifact_order: payload["selected_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_artifact_order: payload["unresolved_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_artifact_order: payload["blocked_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_artifact_order: payload["missing_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        study_order: payload["study_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        modality_order: payload["modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_study_order,
        selected_modality_order,
        missing_study_order,
        missing_modality_order,
        signer_order,
        selected_signer_order,
        missing_signer_order,
        omission_order: omission.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        envelope_digest,
        artifact,
        effect_receipts: effect,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        autonomy_tier: "a2".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }
    fn artifact(id: &str) -> ArtifactAndDerivation2 {
        ArtifactAndDerivation2 {
            artifact_id: id.into(),
            study_id: "study:one".into(),
            site_id: "site:a".into(),
            modality: "imaging".into(),
            derivation_id: "derive:1".into(),
            artifact_digest: hash(id),
            source_digest: Some(hash(&format!("source:{id}"))),
            provenance_digest: Some(hash(&format!("prov:{id}"))),
            signer_id: "signer:a".into(),
            signer_key_digest: Some(hash("key")),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            scope_compatible: true,
            policy_allowed: true,
            negative_result: false,
            omissions: Vec::new(),
        }
    }
    fn request() -> ProvenanceSigningRequest {
        ProvenanceSigningRequest {
            request_id: "request:lens".into(),
            requester: "bioinformatician".into(),
            purpose: "publish".into(),
            scope: "organoid".into(),
            semantic_profile: "provenance:v1".into(),
            schema_version: INPUT_SCHEMA.into(),
            required_studies: vec!["study:one".into()],
            required_modalities: vec!["imaging".into()],
            artifacts: vec![artifact("artifact:a")],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn complete_envelope_qualifies() {
        let receipt = compile_provenance_envelope(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tools:"));
    }
    #[test]
    fn missing_lineage_is_unresolved() {
        let mut value = request();
        value.artifacts[0].source_digest = None;
        assert_eq!(
            compile_provenance_envelope(&value).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn contradiction_and_negative_evidence_remain_visible() {
        let mut value = request();
        value.artifacts[0].evidence_state = EvidenceState::Contradicted;
        let receipt = compile_provenance_envelope(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(!receipt.uncertainty_order.is_empty());
    }
    #[test]
    fn policy_and_adversarial_gates_block() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            compile_provenance_envelope(&value).unwrap().disposition,
            "blocked"
        );
        value.policy_allow = true;
        value.adversarial_events = vec!["prompt-injection".into()];
        assert_eq!(
            compile_provenance_envelope(&value).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn approval_gate_blocks_without_invocation() {
        let mut value = request();
        value.signed_approval = false;
        assert_eq!(
            compile_provenance_envelope(&value).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn manifest_is_a2_and_byte_stable() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
    }
}
