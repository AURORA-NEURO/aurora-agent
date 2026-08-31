//! Local single-study retrieval-and-synthesis inference engine.
//!
//! Atlas feature: `AFA-brain-P02-F01`. The engine ranks caller-supplied, institution-local
//! evidence deterministically; it never fetches sources, treats absence as support, or upgrades
//! unknown and contradicted evidence into a conclusion.

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

pub const FEATURE_ID: &str = "AFA-brain-P02-F01";
pub const CONTRACT_VERSION: &str = "brain-retrieval-synthesis/1.0";
pub const MAX_CANDIDATES: usize = 4096;
const SYNTHESIS_CONTENT_TYPE: &str = "application/vnd.aurora.evidence-synthesis+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub query: String,
    pub minimum_support_milli: u16,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    pub evidence_id: String,
    pub source_id: String,
    pub study_id: String,
    pub scope: String,
    pub modality: String,
    pub support_milli: u16,
    pub state: EvidenceState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: SynthesisDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub source_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalSynthesisError {
    #[error("invalid retrieval query: {0}")]
    Invalid(String),
    #[error("retrieval synthesis artifact failed: {0}")]
    Artifact(String),
}

impl EvidenceSynthesis {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.ranked_order.len() != self.support_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(RetrievalSynthesisError::Invalid(
                "synthesis identity, ranking, support, locality, or effects are incomplete".into(),
            ));
        }
        validate_text(&self.request_id, "request_id")?;
        validate_text(&self.study_id, "study_id")?;
        validate_text(&self.scope, "scope")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        validate_unique(&self.ranked_order, "ranked_order")?;
        validate_unique(&self.qualified_order, "qualified_order")?;
        for (values, field) in [
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.source_order, "source_order"),
            (&self.modality_order, "modality_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        validate_digest_list(&self.semantic_order, "semantic_order")?;
        validate_digest_list(&self.artifact_order, "artifact_order")?;
        validate_digest_list(&self.provenance_order, "provenance_order")?;
        if self.support_order.iter().any(|support| *support > 1000) {
            return Err(RetrievalSynthesisError::Invalid(
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
            return Err(RetrievalSynthesisError::Invalid(
                "non-local retrieval synthesis must be blocked and retain locality evidence".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(RetrievalSynthesisError::Invalid(
                "ranked_order must contain every candidate exactly once".into(),
            ));
        }
        if self
            .ranked_order
            .iter()
            .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .qualified_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .blocked_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .unknown_order
                .iter()
                .any(|candidate| !self.blocked_order.contains(candidate))
        {
            return Err(RetrievalSynthesisError::Invalid(
                "retrieval synthesis state contains an identity outside its candidate set".into(),
            ));
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
        {
            return Err(RetrievalSynthesisError::Invalid(
                "qualified and blocked states must partition candidates; unknown must remain blocked".into(),
            ));
        }
        let qualified_exact = self.qualified_order.iter().collect::<BTreeSet<_>>();
        let expected_qualified = self
            .ranked_order
            .iter()
            .filter(|candidate| qualified_exact.contains(candidate))
            .cloned()
            .collect::<Vec<_>>();
        if self.qualified_order != expected_qualified {
            return Err(RetrievalSynthesisError::Invalid(
                "qualified order must retain ranked candidate order".into(),
            ));
        }
        for digest in [&self.replay_identity, &self.synthesis_digest] {
            if digest.as_str().len() != 64 {
                return Err(RetrievalSynthesisError::Invalid(
                    "synthesis digest is invalid".into(),
                ));
            }
        }
        let expected_synthesis_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "qualified_order": self.qualified_order,
            "replay_identity": self.replay_identity,
            "disposition": self.disposition,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
        if self.synthesis_digest != expected_synthesis_digest {
            return Err(RetrievalSynthesisError::Invalid(
                "synthesis digest is not bound to ranked evidence".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!("read:local-research-artifacts:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(RetrievalSynthesisError::Invalid(
                "synthesis effect receipts do not match disposition".into(),
            ));
        }
        let expected_artifact_id = format!("brain-retrieval-synthesis:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != SYNTHESIS_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(RetrievalSynthesisError::Invalid(
                "synthesis artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalSynthesisError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))
    }
}

pub fn retrieval_synthesis_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "local retrieval operator".into()].into(), behavior: "deterministically ranks scoped caller-supplied retrieval candidates and emits omission-aware EvidenceSynthesis artifacts".into(), value: "improves auditable discovery without external fetching, silent fallback, or unsupported scientific conclusions".into(), inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: "ScopedRetrievalQuery1@1".into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: "EvidenceSynthesis1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn synthesize_retrieval(
    request: &ScopedRetrievalQuery,
) -> Result<EvidenceSynthesis, RetrievalSynthesisError> {
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
    let mut qualified = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &ranked {
        let in_scope = candidate.study_id == request.study_id && candidate.scope == request.scope;
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
            sources.insert(candidate.source_id.clone());
            modalities.insert(candidate.modality.clone());
            semantics.insert(candidate.semantic_digest.clone());
            artifacts.insert(candidate.artifact_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
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
                omissions.insert(format!("evidence:{}:scope-mismatch", candidate.evidence_id));
            }
            if candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "evidence:{}:replay-mismatch",
                    candidate.evidence_id
                ));
            }
            if !candidate.omissions.is_empty() {
                omissions.insert(format!(
                    "evidence:{}:protected-closure-incomplete",
                    candidate.evidence_id
                ));
            }
        }
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
    let effect_receipts = if matches!(
        disposition,
        SynthesisDisposition::Qualified | SynthesisDisposition::Partial
    ) {
        vec![format!(
            "read:local-research-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let synthesis_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "replay_identity": request.replay_identity, "disposition": disposition, "raw_data_local": true})).map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "support_order": support_order, "source_order": sources, "modality_order": modalities, "semantic_order": semantics, "artifact_order": artifacts, "provenance_order": provenance, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "synthesis_digest": synthesis_digest, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-synthesis:{}", request.request_id),
        SYNTHESIS_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
    let receipt = EvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        ranked_order,
        qualified_order: qualified,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        support_order,
        source_order: sources.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        synthesis_digest,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ScopedRetrievalQuery) -> Result<(), RetrievalSynthesisError> {
    if request.minimum_support_milli > 1000
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalSynthesisError::Invalid(
            "retrieval request identity, scope, threshold, candidates, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.evidence_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.modality.trim().is_empty()
            || candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.evidence_id.clone())
        {
            return Err(RetrievalSynthesisError::Invalid(format!(
                "candidate {} is invalid or duplicated",
                candidate.evidence_id
            )));
        }
        for (value, field) in [
            (&candidate.evidence_id, "candidate.evidence_id"),
            (&candidate.source_id, "candidate.source_id"),
            (&candidate.study_id, "candidate.study_id"),
            (&candidate.scope, "candidate.scope"),
            (&candidate.modality, "candidate.modality"),
        ] {
            validate_text(value, field)?;
        }
    }
    validate_text(&request.request_id, "request_id")?;
    validate_text(&request.study_id, "study_id")?;
    validate_text(&request.scope, "scope")?;
    validate_text(&request.query, "query")?;
    Ok(())
}

fn receipt_payload(receipt: &EvidenceSynthesis) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_id": receipt.study_id,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "support_order": receipt.support_order,
        "source_order": receipt.source_order,
        "modality_order": receipt.modality_order,
        "semantic_order": receipt.semantic_order,
        "artifact_order": receipt.artifact_order,
        "provenance_order": receipt.provenance_order,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "replay_identity": receipt.replay_identity,
        "synthesis_digest": receipt.synthesis_digest,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_text(value: &str, field: &str) -> Result<(), RetrievalSynthesisError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(RetrievalSynthesisError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), RetrievalSynthesisError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(RetrievalSynthesisError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), RetrievalSynthesisError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RetrievalSynthesisError::Invalid(format!(
            "{field} must use canonical sorted order"
        )));
    }
    Ok(())
}

fn validate_digest_list(
    values: &[ContentHash],
    field: &str,
) -> Result<(), RetrievalSynthesisError> {
    let mut previous = None;
    for value in values {
        if value.as_str().len() != 64 || previous.is_some_and(|prior: &str| prior >= value.as_str())
        {
            return Err(RetrievalSynthesisError::Invalid(format!(
                "{field} must contain canonical 64-character digests"
            )));
        }
        previous = Some(value.as_str());
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, support: u16, state: EvidenceState) -> RetrievalCandidate {
        RetrievalCandidate {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            modality: "imaging".into(),
            support_milli: support,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(candidates: Vec<RetrievalCandidate>) -> ScopedRetrievalQuery {
        ScopedRetrievalQuery {
            request_id: "request:retrieval".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            query: "synaptic morphology".into(),
            minimum_support_milli: 700,
            candidates,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        let m = retrieval_synthesis_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn ranks_support_deterministically() {
        let r = synthesize_retrieval(&request(vec![
            candidate("b", 800, EvidenceState::Supported),
            candidate("a", 900, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Qualified);
        assert_eq!(r.ranked_order, vec!["evidence:a", "evidence:b"]);
    }
    #[test]
    fn unknown_is_retained() {
        let r = synthesize_retrieval(&request(vec![candidate("a", 900, EvidenceState::Unknown)]))
            .unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Unknown);
        assert_eq!(r.unknown_order, vec!["evidence:a"]);
    }
    #[test]
    fn contradiction_is_negative_evidence() {
        let r = synthesize_retrieval(&request(vec![candidate(
            "a",
            900,
            EvidenceState::Contradicted,
        )]))
        .unwrap();
        assert!(!r.negative_evidence.is_empty());
    }
    #[test]
    fn policy_blocks() {
        let mut q = request(vec![candidate("a", 900, EvidenceState::Supported)]);
        q.policy_allow = false;
        let r = synthesize_retrieval(&q).unwrap();
        assert_eq!(r.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request(vec![candidate("a", 900, EvidenceState::Supported)]);
        q.raw_data_local = false;
        let r = synthesize_retrieval(&q).unwrap();
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
        let r = synthesize_retrieval(&request(vec![candidate(
            "a",
            900,
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }

    #[test]
    fn ranking_order_can_differ_from_canonical_candidate_order() {
        let r = synthesize_retrieval(&request(vec![
            candidate("b", 900, EvidenceState::Supported),
            candidate("a", 800, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(r.candidate_order, vec!["evidence:a", "evidence:b"]);
        assert_eq!(r.ranked_order, vec!["evidence:b", "evidence:a"]);
        r.validate().unwrap();
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut r = synthesize_retrieval(&request(vec![candidate(
            "a",
            900,
            EvidenceState::Supported,
        )]))
        .unwrap();
        r.qualified_order[0] = r.qualified_order[0].to_ascii_uppercase();
        assert!(r.validate().is_err());
    }

    #[test]
    fn synthesis_artifact_identity_and_payload_are_bound() {
        let mut drifted = synthesize_retrieval(&request(vec![candidate(
            "a",
            900,
            EvidenceState::Supported,
        )]))
        .unwrap();
        drifted.synthesis_digest = hash("drift");
        assert!(drifted.validate().is_err());

        let mut identity_drift = synthesize_retrieval(&request(vec![candidate(
            "a",
            900,
            EvidenceState::Supported,
        )]))
        .unwrap();
        identity_drift.artifact.artifact_id = "artifact:other".into();
        assert!(identity_drift.validate().is_err());

        let mut padded = request(vec![candidate("a", 900, EvidenceState::Supported)]);
        padded.request_id = " request:retrieval".into();
        assert!(synthesize_retrieval(&padded).is_err());
    }
}
