//! Multimodal typed research-context compilation with closure certificates.
//!
//! Atlas feature: `AFA-brain-P03-F02`. Study and modality coverage are release predicates;
//! incomplete matrices remain partial or blocked rather than being silently completed.

use crate::context_compilation::ContextCompilationDisposition;
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

pub const FEATURE_ID: &str = "AFA-brain-P03-F02";
pub const CONTRACT_VERSION: &str = "brain-multimodal-context-compilation/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextFact {
    pub fact_id: String,
    pub study_id: String,
    pub modality: String,
    pub support_milli: u16,
    pub state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextCompilationRequest {
    pub request_id: String,
    pub objective: String,
    pub scope: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub required_fact_ids: Vec<String>,
    pub minimum_support_milli: u16,
    pub facts: Vec<MultimodalContextFact>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextCompilationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub scope: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: ContextCompilationDisposition,
    pub required_fact_order: Vec<String>,
    pub resolved_fact_order: Vec<String>,
    pub missing_fact_order: Vec<String>,
    pub blocked_fact_order: Vec<String>,
    pub unknown_fact_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalContextCompilationError {
    #[error("invalid multimodal context compilation request: {0}")]
    Invalid(String),
    #[error("multimodal context compilation artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalContextCompilationReceipt {
    pub fn validate(&self) -> Result<(), MultimodalContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.required_fact_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalContextCompilationError::Invalid("multimodal context identity, study/modality closure, required facts, locality, or effects are incomplete".into()));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.required_fact_order,
            &self.resolved_fact_order,
            &self.missing_fact_order,
            &self.blocked_fact_order,
            &self.unknown_fact_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalContextCompilationError::Invalid(
                    "multimodal context vectors are not canonical".into(),
                ));
            }
        }
        let required = self
            .required_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = self
            .resolved_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.missing_fact_order.iter().cloned());
        classified.extend(self.blocked_fact_order.iter().cloned());
        classified.extend(self.unknown_fact_order.iter().cloned());
        if classified != required {
            return Err(MultimodalContextCompilationError::Invalid(
                "multimodal context fact states do not partition required facts".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.context_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalContextCompilationError::Invalid(
                    "multimodal context digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("compile:local-multimodal-research-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalContextCompilationError::Invalid(
                "multimodal context effect is outside local compilation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalContextCompilationError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalContextCompilationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalContextCompilationError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalContextCompilationError::Artifact(error.to_string()))
    }
}

pub fn multimodal_context_compilation_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["multimodal researcher".into(), "context compiler".into(), "imaging/omics workflow".into()].into(), behavior: "compiles typed preclinical context across studies and modalities with deterministic comparability and omission certificates".into(), value: "prevents incomplete imaging/omics context matrices from becoming apparently complete research context".into(), inputs: vec![TypedPort { name: "multimodal_context_compilation_request".into(), schema: "MultimodalContextCompilationRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_context_compilation_receipt".into(), schema: "MultimodalContextCompilationReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-multimodal-research-context".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_multimodal_context(
    request: &MultimodalContextCompilationRequest,
) -> Result<MultimodalContextCompilationReceipt, MultimodalContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.required_fact_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(MultimodalContextCompilationError::Invalid(
            "multimodal context request identity, coverage, replay, or boundary is invalid".into(),
        ));
    }
    let study_order = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modality_order = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required = request
        .required_fact_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if study_order.len() != request.study_ids.len()
        || modality_order.len() != request.required_modalities.len()
        || required.len() != request.required_fact_ids.len()
        || request
            .required_fact_ids
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(MultimodalContextCompilationError::Invalid(
            "multimodal context identities must be non-empty and unique".into(),
        ));
    }
    let mut facts = request.facts.clone();
    facts.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));
    let mut resolved = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let negative = BTreeSet::new();
    let mut observed_studies = BTreeSet::new();
    let mut observed_modalities = BTreeSet::new();
    for id in &required {
        match facts.iter().find(|fact| fact.fact_id == *id) {
            None => {
                missing.insert(id.clone());
                omissions.insert(format!("fact:{}:missing", id));
            }
            Some(fact)
                if !request.policy_allow
                    || !request.protected_closure
                    || !request.raw_data_local
                    || !fact.raw_data_local
                    || fact.boundary != PRECLINICAL_BOUNDARY
                    || !study_order.contains(&fact.study_id)
                    || !modality_order.contains(&fact.modality) =>
            {
                blocked.insert(id.clone());
                omissions.insert(format!("fact:{}:scope-or-policy-blocked", id));
            }
            Some(fact) if fact.replay_identity != request.replay_identity => {
                unknown.insert(id.clone());
                uncertainty.insert(format!("fact:{}:replay-mismatch", id));
            }
            Some(fact)
                if fact.state == EvidenceState::Supported
                    && fact.support_milli >= request.minimum_support_milli =>
            {
                resolved.insert(id.clone());
                observed_studies.insert(fact.study_id.clone());
                observed_modalities.insert(fact.modality.clone());
            }
            Some(fact)
                if matches!(
                    fact.state,
                    EvidenceState::Unknown | EvidenceState::Speculative
                ) =>
            {
                unknown.insert(id.clone());
                uncertainty.insert(format!("fact:{}:state-unknown", id));
            }
            Some(fact) => {
                blocked.insert(id.clone());
                omissions.insert(format!(
                    "fact:{}:unsupported-or-below-threshold",
                    fact.fact_id
                ));
            }
        }
    }
    for study in &study_order {
        if !observed_studies.contains(study) {
            omissions.insert(format!("study:{}:missing-qualified-fact", study));
        }
    }
    for modality in &modality_order {
        if !observed_modalities.contains(modality) {
            omissions.insert(format!("modality:{}:missing-qualified-fact", modality));
        }
    }
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "scope": request.scope})).map_err(|error| MultimodalContextCompilationError::Artifact(error.to_string()))?;
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            ContextCompilationDisposition::Blocked
        } else if resolved.is_empty() {
            ContextCompilationDisposition::Unknown
        } else if resolved.len() == required.len() && omissions.is_empty() && uncertainty.is_empty()
        {
            ContextCompilationDisposition::Qualified
        } else {
            ContextCompilationDisposition::Partial
        };
    let effect_receipts = if matches!(
        disposition,
        ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
    ) {
        vec![format!(
            "compile:local-multimodal-research-context:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "required_fact_order": required, "resolved_fact_order": resolved, "missing_fact_order": missing, "blocked_fact_order": blocked, "unknown_fact_order": unknown, "comparability_digest": comparability_digest, "replay_identity": request.replay_identity})).map_err(|error| MultimodalContextCompilationError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "scope": request.scope, "study_order": study_order, "modality_order": modality_order, "disposition": disposition, "required_fact_order": required, "resolved_fact_order": resolved, "missing_fact_order": missing, "blocked_fact_order": blocked, "unknown_fact_order": unknown, "comparability_digest": comparability_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-research-context:{}", request.request_id),
        "application/vnd.aurora.multimodal-research-context+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalContextCompilationError::Artifact(error.to_string()))?;
    let receipt = MultimodalContextCompilationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective: request.objective.clone(),
        scope: request.scope.clone(),
        study_order: study_order.into_iter().collect(),
        modality_order: modality_order.into_iter().collect(),
        disposition,
        required_fact_order: required.into_iter().collect(),
        resolved_fact_order: resolved.into_iter().collect(),
        missing_fact_order: missing.into_iter().collect(),
        blocked_fact_order: blocked.into_iter().collect(),
        unknown_fact_order: unknown.into_iter().collect(),
        comparability_digest,
        context_digest,
        replay_identity: request.replay_identity.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> MultimodalContextCompilationRequest {
        MultimodalContextCompilationRequest {
            request_id: "request:mm-context".into(),
            objective: "compile cross-modal mechanism context".into(),
            scope: "organoid:neural".into(),
            study_ids: vec!["study:one".into(), "study:two".into()],
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            required_fact_ids: vec!["fact:imaging".into(), "fact:omics".into()],
            minimum_support_milli: 700,
            facts: vec![
                MultimodalContextFact {
                    fact_id: "fact:imaging".into(),
                    study_id: "study:one".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
                    evidence_digest: hash("evidence-i"),
                    provenance_digest: hash("provenance-i"),
                    artifact_digest: hash("artifact-i"),
                    replay_identity: hash("replay"),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                MultimodalContextFact {
                    fact_id: "fact:omics".into(),
                    study_id: "study:two".into(),
                    modality: "transcriptomics".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
                    evidence_digest: hash("evidence-o"),
                    provenance_digest: hash("provenance-o"),
                    artifact_digest: hash("artifact-o"),
                    replay_identity: hash("replay"),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
            ],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_context_compilation_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn complete_context_qualifies() {
        let receipt = compile_multimodal_context(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            ContextCompilationDisposition::Qualified
        );
    }
    #[test]
    fn missing_modality_is_partial() {
        let mut value = request();
        value.facts.pop();
        let receipt = compile_multimodal_context(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial);
        assert!(!receipt.omissions.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_multimodal_context(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_multimodal_context(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
