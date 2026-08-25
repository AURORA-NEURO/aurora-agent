//! Multimodal multi-study retrieval-and-synthesis inference engine.
//!
//! Atlas feature: `AFA-brain-P02-F02`. This engine extends local retrieval with explicit study,
//! modality, and comparability closure; incomplete cross-study coverage remains partial or
//! unknown rather than being silently completed.

use crate::retrieval_synthesis::{RetrievalCandidate, SynthesisDisposition};
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F02";
pub const CONTRACT_VERSION: &str = "brain-multimodal-retrieval-synthesis/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalQuery {
    pub request_id: String,
    pub study_ids: Vec<String>,
    pub scope: String,
    pub query: String,
    pub minimum_support_milli: u16,
    pub required_modalities: Vec<String>,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub scope: String,
    pub disposition: SynthesisDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub comparability_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalRetrievalError {
    #[error("invalid multimodal retrieval query: {0}")]
    Invalid(String),
    #[error("multimodal retrieval artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalEvidenceSynthesis {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.ranked_order.len() != self.support_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalRetrievalError::Invalid("multimodal synthesis identity, coverage, ranking, support, or effects are incomplete".into()));
        }
        if self
            .ranked_order
            .iter()
            .chain(self.qualified_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(MultimodalRetrievalError::Invalid(
                "multimodal synthesis state is not covered".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalRetrievalError::Invalid(
                    "multimodal synthesis ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.comparability_digest,
            &self.synthesis_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalRetrievalError::Invalid(
                    "multimodal synthesis digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-multimodal-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalRetrievalError::Invalid(
                "multimodal synthesis effect is outside the local gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalRetrievalError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))
    }
}

pub fn multimodal_retrieval_synthesis_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "multimodal retrieval operator".into()].into(), behavior: "deterministically ranks scoped retrieval candidates across multiple studies and modalities with comparability and closure receipts".into(), value: "provides auditable multimodal evidence synthesis without treating missing modality or semantic disagreement as support".into(), inputs: vec![TypedPort { name: "multimodal_retrieval_query".into(), schema: "ScopedRetrievalQuery2@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_evidence_synthesis".into(), schema: "EvidenceSynthesis1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn synthesize_multimodal_retrieval(
    request: &MultimodalRetrievalQuery,
) -> Result<MultimodalEvidenceSynthesis, MultimodalRetrievalError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let candidate_order = candidates
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut ranked = candidates.clone();
    ranked.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let ranked_order = ranked
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let support_order = ranked
        .iter()
        .map(|item| item.support_milli)
        .collect::<Vec<_>>();
    let study_order = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modality_order = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observed_studies = BTreeSet::new();
    let mut observed_modalities = BTreeSet::new();
    let mut qualified = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &ranked {
        let in_scope = study_order.contains(&candidate.study_id)
            && candidate.scope == request.scope
            && modality_order.contains(&candidate.modality);
        let admissible = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && candidate.raw_data_local
            && in_scope
            && candidate.support_milli >= request.minimum_support_milli
            && candidate.state == EvidenceState::Supported
            && candidate.omissions.is_empty()
            && candidate.replay_identity == request.replay_identity;
        if admissible {
            qualified.push(candidate.evidence_id.clone());
            observed_studies.insert(candidate.study_id.clone());
            observed_modalities.insert(candidate.modality.clone());
        } else {
            blocked.insert(candidate.evidence_id.clone());
            if matches!(
                candidate.state,
                EvidenceState::Unknown | EvidenceState::Speculative
            ) {
                unknown.insert(candidate.evidence_id.clone());
                uncertainty.insert(format!(
                    "evidence:{}:state-not-qualified",
                    candidate.evidence_id
                ));
            }
            if candidate.state == EvidenceState::Contradicted
                || !candidate.negative_evidence.is_empty()
            {
                negative.insert(format!(
                    "evidence:{}:negative-result-retained",
                    candidate.evidence_id
                ));
            }
            if !in_scope {
                omissions.insert(format!(
                    "evidence:{}:scope-or-modality-mismatch",
                    candidate.evidence_id
                ));
            }
            if candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "evidence:{}:replay-mismatch",
                    candidate.evidence_id
                ));
            }
        }
    }
    for study in study_order.difference(&observed_studies) {
        omissions.insert(format!("study:{}:required-coverage-missing", study));
    }
    for modality in modality_order.difference(&observed_modalities) {
        omissions.insert(format!("modality:{}:required-coverage-missing", modality));
    }
    if !request.policy_allow {
        omissions.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("request:raw-data-locality-failed".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            SynthesisDisposition::Blocked
        } else if qualified.is_empty() {
            SynthesisDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            SynthesisDisposition::Qualified
        } else {
            SynthesisDisposition::Partial
        };
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "candidate_order": candidate_order, "ranked_order": ranked_order, "replay_identity": request.replay_identity})).map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
    let synthesis_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "comparability_digest": comparability_digest, "disposition": disposition})).map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": study_order, "modality_order": modality_order, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "support_order": support_order, "comparability_digest": comparability_digest, "synthesis_digest": synthesis_digest, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-retrieval:{}", request.request_id),
        "application/vnd.aurora.multimodal-evidence-synthesis+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
    let receipt = MultimodalEvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_order: study_order.into_iter().collect(),
        modality_order: modality_order.into_iter().collect(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        ranked_order,
        qualified_order: qualified,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        support_order,
        comparability_digest,
        synthesis_digest,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if matches!(
            disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!(
                "read:local-multimodal-artifacts:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &MultimodalRetrievalQuery) -> Result<(), MultimodalRetrievalError> {
    if request.request_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.minimum_support_milli > 1000
        || request.candidates.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalRetrievalError::Invalid("multimodal retrieval identity, floors, threshold, candidates, or boundary is incomplete".into()));
    }
    if request.study_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(MultimodalRetrievalError::Invalid(
            "study and modality requirements must be unique and canonical".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(
        id: &str,
        study: &str,
        modality: &str,
        state: EvidenceState,
    ) -> RetrievalCandidate {
        RetrievalCandidate {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: study.into(),
            scope: "organoid:neural".into(),
            modality: modality.into(),
            support_milli: 900,
            state,
            semantic_digest: hash(id),
            artifact_digest: hash(&format!("a:{id}")),
            provenance_digest: hash(&format!("p:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(candidates: Vec<RetrievalCandidate>) -> MultimodalRetrievalQuery {
        MultimodalRetrievalQuery {
            request_id: "request:mm-retrieval".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            query: "synaptic morphology".into(),
            minimum_support_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            candidates,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = multimodal_retrieval_synthesis_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_is_qualified() {
        let r = synthesize_multimodal_retrieval(&request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Qualified);
    }
    #[test]
    fn missing_modality_is_partial() {
        let r = synthesize_multimodal_retrieval(&request(vec![candidate(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Partial);
        assert!(!r.omissions.is_empty());
    }
    #[test]
    fn unknown_is_retained() {
        let r = synthesize_multimodal_retrieval(&request(vec![candidate(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Unknown);
    }
    #[test]
    fn policy_blocks() {
        let mut q = request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]);
        q.policy_allow = false;
        let r = synthesize_multimodal_retrieval(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = synthesize_multimodal_retrieval(&request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
