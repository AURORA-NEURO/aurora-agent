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
const SYNTHESIS_CONTENT_TYPE: &str = "application/vnd.aurora.multimodal-evidence-synthesis+json";
const MAX_CANDIDATES: usize = 4096;
const MAX_TEXT_BYTES: usize = 512;

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
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.ranked_order.len() != self.support_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalRetrievalError::Invalid("multimodal synthesis identity, coverage, ranking, support, or effects are incomplete".into()));
        }
        validate_text(&self.request_id, "request_id")?;
        validate_text(&self.scope, "scope")?;
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        validate_unique(&self.ranked_order, "ranked_order")?;
        validate_unique(&self.qualified_order, "qualified_order")?;
        for (values, field) in [
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        if self.support_order.iter().any(|support| *support > 1000) {
            return Err(MultimodalRetrievalError::Invalid(
                "support_order contains a value above the 1000 milli-point bound".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "request:raw-data-locality-failed"))
        {
            return Err(MultimodalRetrievalError::Invalid(
                "non-local multimodal synthesis must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranked_values = self.ranked_order.iter().cloned().collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        if ranked_values != candidate_values {
            return Err(MultimodalRetrievalError::Invalid(
                "ranked_order must contain every candidate exactly once".into(),
            ));
        }
        if !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
        {
            return Err(MultimodalRetrievalError::Invalid(
                "qualified and blocked states must partition candidates; unknown must remain blocked".into(),
            ));
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
        let expected_comparability_digest = ContentHash::of_value(&json!({
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "support_order": self.support_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
        if self.comparability_digest != expected_comparability_digest {
            return Err(MultimodalRetrievalError::Invalid(
                "multimodal comparability digest is not bound to ranking".into(),
            ));
        }
        let expected_synthesis_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "qualified_order": self.qualified_order,
            "support_order": self.support_order,
            "comparability_digest": self.comparability_digest,
            "disposition": self.disposition,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
        if self.synthesis_digest != expected_synthesis_digest {
            return Err(MultimodalRetrievalError::Invalid(
                "multimodal synthesis digest is not bound to ranked evidence".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!(
                "read:local-multimodal-artifacts:{}",
                self.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(MultimodalRetrievalError::Invalid(
                "multimodal synthesis effect receipts do not match disposition".into(),
            ));
        }
        let expected_artifact_id = format!("brain-multimodal-retrieval:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != SYNTHESIS_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalRetrievalError::Invalid(
                "multimodal synthesis artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
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
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "candidate_order": candidate_order, "ranked_order": ranked_order, "support_order": support_order, "replay_identity": request.replay_identity})).map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        SynthesisDisposition::Qualified | SynthesisDisposition::Partial
    ) {
        vec![format!(
            "read:local-multimodal-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let synthesis_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "support_order": support_order, "comparability_digest": comparability_digest, "disposition": disposition, "raw_data_local": true})).map_err(|error| MultimodalRetrievalError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": study_order, "modality_order": modality_order, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "support_order": support_order, "comparability_digest": comparability_digest, "synthesis_digest": synthesis_digest, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-retrieval:{}", request.request_id),
        SYNTHESIS_CONTENT_TYPE,
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
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &MultimodalRetrievalQuery) -> Result<(), MultimodalRetrievalError> {
    if request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.minimum_support_milli > 1000
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalRetrievalError::Invalid("multimodal retrieval identity, floors, threshold, candidates, or boundary is incomplete".into()));
    }
    validate_sorted_unique(&request.study_ids, "study_ids")?;
    validate_sorted_unique(&request.required_modalities, "required_modalities")?;
    validate_text(&request.request_id, "request_id")?;
    validate_text(&request.scope, "scope")?;
    validate_text(&request.query, "query")?;
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        for (value, field) in [
            (&candidate.evidence_id, "candidate.evidence_id"),
            (&candidate.study_id, "candidate.study_id"),
            (&candidate.scope, "candidate.scope"),
            (&candidate.modality, "candidate.modality"),
        ] {
            validate_text(value, field)?;
        }
        if candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !candidate_ids.insert(candidate.evidence_id.to_ascii_lowercase())
        {
            return Err(MultimodalRetrievalError::Invalid(
                "candidate identity, support, boundary, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn receipt_payload(receipt: &MultimodalEvidenceSynthesis) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "support_order": receipt.support_order,
        "comparability_digest": receipt.comparability_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "replay_identity": receipt.replay_identity,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalRetrievalError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalRetrievalError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), MultimodalRetrievalError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalRetrievalError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), MultimodalRetrievalError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalRetrievalError::Invalid(format!(
            "{field} must use canonical sorted order"
        )));
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
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]);
        q.raw_data_local = false;
        let r = synthesize_multimodal_retrieval(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
        assert!(r.raw_data_local);
        assert!(r
            .omissions
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        r.validate().unwrap();
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

    #[test]
    fn multimodal_ranking_preserves_semantic_order_and_state_partition() {
        let mut first = candidate("b", "study:a", "imaging", EvidenceState::Supported);
        first.support_milli = 950;
        let second = candidate("a", "study:b", "transcriptomics", EvidenceState::Supported);
        let receipt = synthesize_multimodal_retrieval(&request(vec![first, second])).unwrap();
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
        assert_eq!(receipt.ranked_order, vec!["evidence:b", "evidence:a"]);
        receipt.validate().unwrap();
    }

    #[test]
    fn multimodal_artifact_identity_and_payload_are_bound() {
        let mut drifted = synthesize_multimodal_retrieval(&request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        drifted.synthesis_digest = hash("drift");
        assert!(drifted.validate().is_err());

        let mut identity_drift = synthesize_multimodal_retrieval(&request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        identity_drift.artifact.artifact_id = "artifact:other".into();
        assert!(identity_drift.validate().is_err());

        let mut padded = request(vec![candidate(
            "a",
            "study:a",
            "imaging",
            EvidenceState::Supported,
        )]);
        padded.request_id = " request:mm-retrieval".into();
        assert!(synthesize_multimodal_retrieval(&padded).is_err());
    }

    #[test]
    fn support_order_is_bound_to_comparability_and_synthesis() {
        let mut receipt = synthesize_multimodal_retrieval(&request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        receipt.support_order[0] = 1;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = synthesize_multimodal_retrieval(&request(vec![
            candidate("a", "study:a", "imaging", EvidenceState::Supported),
            candidate("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }
}
