//! Local single-study retrieval and synthesis assurance.
//!
//! Atlas feature: `AFA-ops-P02-F25`.
//!
//! This A0 verifier evaluates caller-supplied evidence candidates. It never fetches sources or
//! upgrades an unknown claim: every omission, contradiction, negative result, and policy failure
//! stays in the content-addressed receipt.

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

pub const FEATURE_ID: &str = "AFA-ops-P02-F25";
pub const CONTRACT_VERSION: &str = "ops-local-retrieval-assurance/1.0";
pub const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub query_text: String,
    pub required_modalities: Vec<String>,
    pub minimum_support_milli: u16,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
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
    pub modality: String,
    pub scope: String,
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
pub enum RetrievalDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: RetrievalDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub source_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalAssuranceError {
    #[error("invalid retrieval assurance request: {0}")]
    Invalid(String),
    #[error("retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval assurance serialization failed: {0}")]
    Serialization(String),
}

impl EvidenceSynthesisReceipt {
    pub fn validate(&self) -> Result<(), RetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.support_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval identity, ranking, support, locality, boundary, or effects are incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(RetrievalAssuranceError::Invalid(
                "candidate state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.source_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RetrievalAssuranceError::Invalid(
                    "retrieval ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RetrievalAssuranceError::Invalid(
                    "retrieval digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("evaluate:retrieval-assurance:")
        }) {
            return Err(RetrievalAssuranceError::Invalid(
                "effect is outside the retrieval assurance gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))
    }
}

pub fn retrieval_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "ops".into(),
        consumers: [
            "research data steward".into(),
            "downstream evidence compiler".into(),
        ]
        .into(),
        behavior: "verifies a scoped local retrieval corpus into an omission-aware evidence synthesis without fetching data or asserting unsupported conclusions".into(),
        value: "makes retrieval and synthesis releases replayable, provenance-bearing, and fail-closed for a single preclinical study".into(),
        inputs: vec![TypedPort {
            name: "scoped_retrieval_query".into(),
            schema: "ScopedRetrievalQuery1@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "evidence_synthesis".into(),
            schema: "EvidenceSynthesis7@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
            },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_retrieval(
    request: &ScopedRetrievalQuery,
) -> Result<EvidenceSynthesisReceipt, RetrievalAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then(left.evidence_id.cmp(&right.evidence_id))
    });
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.evidence_id.clone())
        .collect::<Vec<_>>();
    let support_order = candidates
        .iter()
        .map(|candidate| candidate.support_milli)
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
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
    for candidate in &candidates {
        let required_modality = request
            .required_modalities
            .iter()
            .all(|modality| modality == &candidate.modality);
        let complete = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && candidate.raw_data_local
            && candidate.state == EvidenceState::Supported
            && candidate.study_id == request.study_id
            && candidate.scope == request.scope
            && candidate.support_milli >= request.minimum_support_milli
            && required_modality
            && candidate.omissions.is_empty()
            && candidate.negative_evidence.is_empty()
            && candidate.replay_identity == request.replay_identity
            && request.benchmark_digest.is_some();
        if complete {
            admitted.push(candidate.evidence_id.clone());
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
                uncertainty.insert(
                    format!(
                        "evidence:{}:state-{:?}-not-admitted",
                        candidate.evidence_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.state == EvidenceState::Contradicted {
                negative.insert(format!(
                    "evidence:{}:contradicted-negative-evidence",
                    candidate.evidence_id
                ));
            }
            if !request.policy_allow {
                negative.insert("request:policy-denied".into());
            }
            if !request.protected_closure {
                uncertainty.insert("request:protected-closure-incomplete".into());
            }
            if !request.raw_data_local || !candidate.raw_data_local {
                negative.insert(format!(
                    "evidence:{}:raw-data-locality-failed",
                    candidate.evidence_id
                ));
            }
            if candidate.study_id != request.study_id {
                omissions.insert(format!("evidence:{}:study-mismatch", candidate.evidence_id));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!("evidence:{}:scope-mismatch", candidate.evidence_id));
            }
            if !required_modality {
                omissions.insert(format!(
                    "evidence:{}:required-modality-missing",
                    candidate.evidence_id
                ));
            }
            if candidate.support_milli < request.minimum_support_milli {
                uncertainty.insert(format!(
                    "evidence:{}:support-below-threshold",
                    candidate.evidence_id
                ));
            }
            if candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "evidence:{}:replay-mismatch",
                    candidate.evidence_id
                ));
            }
            if request.benchmark_digest.is_none() {
                omissions.insert(format!(
                    "evidence:{}:benchmark-missing",
                    candidate.evidence_id
                ));
            }
            if !candidate.omissions.is_empty() {
                uncertainty.insert(format!(
                    "evidence:{}:protected-closure-incomplete",
                    candidate.evidence_id
                ));
            }
            if !candidate.negative_evidence.is_empty() {
                negative.insert(format!(
                    "evidence:{}:negative-result-retained",
                    candidate.evidence_id
                ));
            }
        }
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            RetrievalDisposition::Blocked
        } else if admitted.is_empty() {
            RetrievalDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            RetrievalDisposition::Qualified
        } else {
            RetrievalDisposition::Partial
        };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "study_id": request.study_id,
        "scope": request.scope,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "admitted_order": admitted,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "source_order": sources,
        "modality_order": modalities,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "replay_identity": request.replay_identity,
        "benchmark_digest": request.benchmark_digest,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("ops-retrieval-assurance:{}", request.request_id),
        "application/vnd.aurora.evidence-synthesis+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))?;
    let has_admitted = !admitted.is_empty();
    let receipt = EvidenceSynthesisReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        source_order: sources.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        support_order,
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        effect_receipts: if has_admitted {
            vec![format!(
                "evaluate:retrieval-assurance:{}",
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

fn validate_request(request: &ScopedRetrievalQuery) -> Result<(), RetrievalAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query_text.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.minimum_support_milli > 1000
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalAssuranceError::Invalid(
            "retrieval identity, query, modality, candidates, support, or boundary is incomplete"
                .into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.evidence_id.trim().is_empty()
            || candidate.source_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.modality.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !seen.insert(candidate.evidence_id.clone())
        {
            return Err(RetrievalAssuranceError::Invalid(format!(
                "candidate {} is invalid or duplicated",
                candidate.evidence_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn candidate(id: &str, state: EvidenceState) -> RetrievalCandidate {
        RetrievalCandidate {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            support_milli: 900,
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
            query_text: "synaptic density".into(),
            required_modalities: vec!["imaging".into()],
            minimum_support_milli: 700,
            candidates,
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a0_and_typed() {
        let manifest = retrieval_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }

    #[test]
    fn supported_candidates_are_qualified_and_ranked() {
        let receipt = assure_retrieval(&request(vec![
            candidate("b", EvidenceState::Supported),
            candidate("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Qualified);
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn unknown_and_contradicted_evidence_remain_visible() {
        let receipt = assure_retrieval(&request(vec![
            candidate("a", EvidenceState::Supported),
            candidate("b", EvidenceState::Unknown),
            candidate("c", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"evidence:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("evidence:c")));
    }

    #[test]
    fn policy_denial_is_blocked() {
        let mut input = request(vec![candidate("a", EvidenceState::Supported)]);
        input.policy_allow = false;
        let receipt = assure_retrieval(&input).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn missing_benchmark_is_unknown() {
        let mut input = request(vec![candidate("a", EvidenceState::Supported)]);
        input.benchmark_digest = None;
        let receipt = assure_retrieval(&input).unwrap();
        assert_eq!(receipt.disposition, RetrievalDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value.contains("benchmark-missing")));
    }

    #[test]
    fn duplicate_evidence_is_rejected() {
        let mut duplicate = candidate("a", EvidenceState::Supported);
        duplicate.source_id = "source:other".into();
        assert!(assure_retrieval(&request(vec![
            candidate("a", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
