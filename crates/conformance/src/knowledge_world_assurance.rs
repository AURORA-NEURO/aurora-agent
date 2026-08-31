//! Multimodal knowledge-world conformance assurance.
//!
//! Atlas feature: `AFA-conformance-P04-F26`.
//!
//! The verifier admits only comparable, evidence-backed claims from institution-local studies.
//! It does not infer facts, export raw data, or treat an incomplete protected closure as a pass.

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

pub const FEATURE_ID: &str = "AFA-conformance-P04-F26";
pub const CONTRACT_VERSION: &str = "conformance-knowledge-world-assurance/1.0";
pub const MAX_CLAIMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaimsRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub required_modalities: Vec<String>,
    pub minimum_studies: usize,
    pub minimum_support_milli: u16,
    pub claims: Vec<ScopedResearchClaim>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaim {
    pub claim_id: String,
    pub predicate: String,
    pub scope: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub support_milli: u16,
    pub state: EvidenceState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeWorldDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorldReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub disposition: KnowledgeWorldDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub predicate_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub comparability_order: Vec<ContentHash>,
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
pub enum KnowledgeWorldAssuranceError {
    #[error("invalid knowledge-world assurance request: {0}")]
    Invalid(String),
    #[error("knowledge-world artifact failed: {0}")]
    Artifact(String),
    #[error("knowledge-world serialization failed: {0}")]
    Serialization(String),
}

impl TypedKnowledgeWorldReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeWorldAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.support_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(KnowledgeWorldAssuranceError::Invalid(
                "knowledge-world identity, ranking, support, locality, or effects are incomplete"
                    .into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(KnowledgeWorldAssuranceError::Invalid(
                "claim state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.predicate_order,
            &self.study_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeWorldAssuranceError::Invalid(
                    "knowledge-world ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.evidence_order,
            &self.provenance_order,
            &self.comparability_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeWorldAssuranceError::Invalid(
                    "knowledge-world digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release"
                && !effect.starts_with("evaluate:knowledge-world-assurance:")
        }) {
            return Err(KnowledgeWorldAssuranceError::Invalid(
                "effect is outside the knowledge-world gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeWorldAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, KnowledgeWorldAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeWorldAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeWorldAssuranceError::Serialization(error.to_string()))
    }
}

pub fn knowledge_world_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "conformance".into(),
        consumers: ["release governance board".into(), "typed knowledge compiler".into()].into(),
        behavior: "verifies scoped multimodal research claims into a comparable typed knowledge world while preserving omission, uncertainty, contradiction, and negative evidence".into(),
        value: "makes cross-study knowledge representation releases deterministic, replayable, and fail-closed".into(),
        inputs: vec![TypedPort { name: "scoped_research_claims".into(), schema: "ScopedResearchClaims2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "typed_knowledge_world".into(), schema: "TypedKnowledgeWorld7@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_knowledge_world(
    request: &ScopedResearchClaimsRequest,
) -> Result<TypedKnowledgeWorldReceipt, KnowledgeWorldAssuranceError> {
    validate_request(request)?;
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then(left.claim_id.cmp(&right.claim_id))
    });
    let candidate_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let support_order = claims
        .iter()
        .map(|claim| claim.support_milli)
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut predicates = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut comparability = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for claim in &claims {
        let modalities_ok = request
            .required_modalities
            .iter()
            .all(|required| claim.modality_ids.contains(required));
        let complete = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && claim.raw_data_local
            && claim.state == EvidenceState::Supported
            && claim.scope == request.scope
            && claim.study_ids.len() >= request.minimum_studies
            && modalities_ok
            && claim.support_milli >= request.minimum_support_milli
            && claim.comparability_digest.is_some()
            && claim.omissions.is_empty()
            && claim.negative_evidence.is_empty()
            && claim.replay_identity == request.replay_identity
            && request.benchmark_digest.is_some();
        if complete {
            let Some(comparability_digest) = claim.comparability_digest.clone() else {
                continue;
            };
            admitted.push(claim.claim_id.clone());
            predicates.insert(claim.predicate.clone());
            studies.extend(claim.study_ids.iter().cloned());
            modalities.extend(claim.modality_ids.iter().cloned());
            semantics.insert(claim.semantic_digest.clone());
            artifacts.insert(claim.artifact_digest.clone());
            evidence.insert(claim.evidence_digest.clone());
            provenance.insert(claim.provenance_digest.clone());
            comparability.insert(comparability_digest);
        } else {
            blocked.insert(claim.claim_id.clone());
            if matches!(
                claim.state,
                EvidenceState::Unknown | EvidenceState::Speculative
            ) {
                unknown.insert(claim.claim_id.clone());
                uncertainty.insert(
                    format!(
                        "claim:{}:state-{:?}-not-admitted",
                        claim.claim_id, claim.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if claim.state == EvidenceState::Contradicted {
                negative.insert(format!(
                    "claim:{}:contradicted-negative-evidence",
                    claim.claim_id
                ));
            }
            if !request.policy_allow {
                negative.insert("request:policy-denied".into());
            }
            if !request.protected_closure {
                uncertainty.insert("request:protected-closure-incomplete".into());
            }
            if !request.raw_data_local || !claim.raw_data_local {
                negative.insert(format!("claim:{}:raw-data-locality-failed", claim.claim_id));
            }
            if claim.scope != request.scope {
                omissions.insert(format!("claim:{}:scope-mismatch", claim.claim_id));
            }
            if claim.study_ids.len() < request.minimum_studies {
                omissions.insert(format!("claim:{}:study-floor-incomplete", claim.claim_id));
            }
            for required in &request.required_modalities {
                if !claim.modality_ids.contains(required) {
                    omissions.insert(format!(
                        "claim:{}:modality-missing:{}",
                        claim.claim_id, required
                    ));
                }
            }
            if claim.support_milli < request.minimum_support_milli {
                uncertainty.insert(format!("claim:{}:support-below-threshold", claim.claim_id));
            }
            if claim.comparability_digest.is_none() {
                omissions.insert(format!("claim:{}:comparability-missing", claim.claim_id));
            }
            if claim.replay_identity != request.replay_identity {
                uncertainty.insert(format!("claim:{}:replay-mismatch", claim.claim_id));
            }
            if request.benchmark_digest.is_none() {
                omissions.insert(format!("claim:{}:benchmark-missing", claim.claim_id));
            }
            if !claim.omissions.is_empty() {
                uncertainty.insert(format!(
                    "claim:{}:protected-closure-incomplete",
                    claim.claim_id
                ));
            }
            if !claim.negative_evidence.is_empty() {
                negative.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            KnowledgeWorldDisposition::Blocked
        } else if admitted.is_empty() {
            KnowledgeWorldDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            KnowledgeWorldDisposition::Qualified
        } else {
            KnowledgeWorldDisposition::Partial
        };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "predicate_order": predicates, "study_order": studies, "modality_order": modalities, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "benchmark_digest": request.benchmark_digest, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("conformance-knowledge-world:{}", request.request_id),
        "application/vnd.aurora.typed-knowledge-world+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| KnowledgeWorldAssuranceError::Artifact(error.to_string()))?;
    let has_admitted = !admitted.is_empty();
    let receipt = TypedKnowledgeWorldReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        predicate_order: predicates.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        support_order,
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        comparability_order: comparability.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        effect_receipts: if has_admitted {
            vec![format!(
                "evaluate:knowledge-world-assurance:{}",
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

fn validate_request(
    request: &ScopedResearchClaimsRequest,
) -> Result<(), KnowledgeWorldAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.minimum_studies == 0
        || request.minimum_support_milli > 1000
        || request.claims.is_empty()
        || request.claims.len() > MAX_CLAIMS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(KnowledgeWorldAssuranceError::Invalid("knowledge-world identity, modality/study floors, claims, support, or boundary is incomplete".into()));
    }
    let mut ids = BTreeSet::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty()
            || claim.predicate.trim().is_empty()
            || claim.scope.trim().is_empty()
            || claim.study_ids.is_empty()
            || claim.modality_ids.is_empty()
            || claim.support_milli > 1000
            || claim.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(claim.claim_id.clone())
        {
            return Err(KnowledgeWorldAssuranceError::Invalid(format!(
                "claim {} is invalid or duplicated",
                claim.claim_id
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
    fn claim(id: &str, state: EvidenceState) -> ScopedResearchClaim {
        ScopedResearchClaim {
            claim_id: format!("claim:{id}"),
            predicate: format!("expresses:{id}"),
            scope: "organoid:neural".into(),
            study_ids: vec!["study:imaging".into(), "study:omics".into()],
            modality_ids: vec!["imaging".into(), "omics".into()],
            support_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: Some(hash(&format!("comparability:{id}"))),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(claims: Vec<ScopedResearchClaim>) -> ScopedResearchClaimsRequest {
        ScopedResearchClaimsRequest {
            request_id: "request:world".into(),
            workflow_id: "workflow:knowledge".into(),
            scope: "organoid:neural".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            minimum_studies: 2,
            minimum_support_milli: 700,
            claims,
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let manifest = knowledge_world_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_claims_are_qualified_and_ranked() {
        let receipt = assure_knowledge_world(&request(vec![
            claim("b", EvidenceState::Supported),
            claim("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorldDisposition::Qualified);
        assert_eq!(receipt.candidate_order, vec!["claim:a", "claim:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_claims_remain_visible() {
        let receipt = assure_knowledge_world(&request(vec![
            claim("a", EvidenceState::Supported),
            claim("b", EvidenceState::Unknown),
            claim("c", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorldDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"claim:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("claim:c")));
    }
    #[test]
    fn policy_denial_blocks_world_release() {
        let mut input = request(vec![claim("a", EvidenceState::Supported)]);
        input.policy_allow = false;
        let receipt = assure_knowledge_world(&input).unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorldDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn missing_comparability_is_unknown() {
        let mut input = request(vec![claim("a", EvidenceState::Supported)]);
        input.claims[0].comparability_digest = None;
        let receipt = assure_knowledge_world(&input).unwrap();
        assert_eq!(receipt.disposition, KnowledgeWorldDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value.contains("comparability-missing")));
    }
    #[test]
    fn duplicate_claim_is_rejected() {
        let mut duplicate = claim("a", EvidenceState::Supported);
        duplicate.predicate = "expresses:other".into();
        assert!(assure_knowledge_world(&request(vec![
            claim("a", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
