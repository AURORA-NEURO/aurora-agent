//! Federated continual retrieval-and-synthesis release gate.
//!
//! Atlas feature: `AFA-brain-P02-F04`. Only permitted aggregate digests cross an institution
//! boundary; raw observations, incomplete closure, and unapproved evidence remain local.

use crate::retrieval_synthesis::RetrievalCandidate;
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F04";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-synthesis/1.0";
pub const PERMITTED_ARTIFACT: &str = "qualified-evidence-summary";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalQuery {
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub allowed_artifacts: Vec<String>,
    pub study_ids: Vec<String>,
    pub scope: String,
    pub minimum_support_milli: u16,
    pub required_modalities: Vec<String>,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub approval_valid: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedRetrievalDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceSynthesis {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub scope: String,
    pub disposition: FederatedRetrievalDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub support_order: Vec<u16>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
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
pub enum FederatedRetrievalError {
    #[error("invalid federated retrieval query: {0}")]
    Invalid(String),
    #[error("federated retrieval artifact failed: {0}")]
    Artifact(String),
}

impl FederatedEvidenceSynthesis {
    pub fn validate(&self) -> Result<(), FederatedRetrievalError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.ranked_order.len() != self.support_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedRetrievalError::Invalid(
                "federation identity, coverage, ranking, support, or effects are incomplete".into(),
            ));
        }
        if self
            .ranked_order
            .iter()
            .chain(self.qualified_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedRetrievalError::Invalid(
                "federated retrieval state is not covered".into(),
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
                return Err(FederatedRetrievalError::Invalid(
                    "federated retrieval ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedRetrievalError::Invalid(
                "aggregate ordering is not canonical".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.synthesis_digest,
            &self.replay_identity,
            &self
                .aggregate_order
                .first()
                .cloned()
                .unwrap_or_else(|| ContentHash::of_bytes(&[])),
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalError::Invalid(
                    "federated retrieval digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedRetrievalError::Invalid(
                "effect is outside federation exchange gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))
    }
}

pub fn federated_retrieval_synthesis_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["federation steward".into(), "multisite retrieval operator".into()].into(), behavior: "deterministically synthesizes comparable retrieval evidence into a purpose-bound aggregate-only federation envelope".into(), value: "enables consortium evidence synthesis without raw-data movement, unsupported admission, or hidden federation denial".into(), inputs: vec![TypedPort { name: "federated_retrieval_query".into(), schema: "FederatedRetrievalQuery1@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_evidence_synthesis".into(), schema: "FederatedEvidenceSynthesis1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federated retrieval approver".into(), reason: "approve purpose-bound aggregate-only evidence exchange after signer, comparability, and locality gates close".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn synthesize_federated_retrieval(
    request: &FederatedRetrievalQuery,
) -> Result<FederatedEvidenceSynthesis, FederatedRetrievalError> {
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
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let observed_studies = candidates
        .iter()
        .map(|item| item.study_id.clone())
        .collect::<BTreeSet<_>>();
    let observed_modalities = candidates
        .iter()
        .map(|item| item.modality.clone())
        .collect::<BTreeSet<_>>();
    for study in &request.study_ids {
        if !observed_studies.contains(study) {
            omissions.insert(format!("study:{}:missing", study));
        }
    }
    for modality in &request.required_modalities {
        if !observed_modalities.contains(modality) {
            omissions.insert(format!("modality:{}:missing", modality));
        }
    }
    for candidate in &ranked {
        let admissible = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && request.signer_valid
            && request.approval_valid
            && candidate.raw_data_local
            && candidate.support_milli >= request.minimum_support_milli
            && candidate.state == EvidenceState::Supported
            && candidate.omissions.is_empty()
            && candidate.replay_identity == request.replay_identity
            && request.study_ids.contains(&candidate.study_id)
            && request.required_modalities.contains(&candidate.modality)
            && request
                .allowed_artifacts
                .iter()
                .any(|artifact| artifact == PERMITTED_ARTIFACT);
        if admissible {
            qualified.push(candidate.evidence_id.clone());
            aggregate.insert(ContentHash::of_value(&json!({"evidence_id": candidate.evidence_id, "study_id": candidate.study_id, "modality": candidate.modality, "semantic_digest": candidate.semantic_digest, "artifact_digest": candidate.artifact_digest, "provenance_digest": candidate.provenance_digest})).map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))?);
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
    if !request.signer_valid {
        omissions.insert("request:signer-invalid".into());
    }
    if !request.approval_valid {
        omissions.insert("request:approval-required".into());
    }
    if !request
        .allowed_artifacts
        .iter()
        .any(|artifact| artifact == PERMITTED_ARTIFACT)
    {
        omissions.insert("request:permitted-artifact-missing".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !request.signer_valid
        || !request.approval_valid
    {
        FederatedRetrievalDisposition::Blocked
    } else if qualified.is_empty() {
        FederatedRetrievalDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        FederatedRetrievalDisposition::Qualified
    } else {
        FederatedRetrievalDisposition::Partial
    };
    let study_order = request
        .study_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let aggregate_order = aggregate.into_iter().collect::<Vec<_>>();
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "scope": request.scope, "semantic_profile": request.semantic_profile})).map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))?;
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "endpoint": request.endpoint, "allowed_artifacts": request.allowed_artifacts, "aggregate_order": aggregate_order})).map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))?;
    let synthesis_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "ranked_order": ranked_order, "qualified_order": qualified, "comparability_digest": comparability_digest, "envelope_digest": envelope_digest, "disposition": disposition})).map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "study_order": study_order, "modality_order": modality_order, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "aggregate_order": aggregate_order, "comparability_digest": comparability_digest, "envelope_digest": envelope_digest, "synthesis_digest": synthesis_digest, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-retrieval:{}", request.request_id),
        "application/vnd.aurora.federated-evidence-synthesis+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalError::Artifact(error.to_string()))?;
    let exchange_allowed = matches!(
        disposition,
        FederatedRetrievalDisposition::Qualified | FederatedRetrievalDisposition::Partial
    ) && !aggregate_order.is_empty();
    let receipt = FederatedEvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        endpoint: request.endpoint.clone(),
        study_order,
        modality_order,
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        ranked_order,
        qualified_order: qualified,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        aggregate_order,
        support_order,
        comparability_digest,
        envelope_digest,
        synthesis_digest,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if exchange_allowed {
            vec![format!(
                "exchange:permitted-artifacts:{}",
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

fn validate_request(request: &FederatedRetrievalQuery) -> Result<(), FederatedRetrievalError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.scope.trim().is_empty()
        || request.minimum_support_milli > 1000
        || request.candidates.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedRetrievalError::Invalid("federated retrieval identity, coverage, threshold, candidates, or boundary is incomplete".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, state: EvidenceState, modality: &str) -> RetrievalCandidate {
        RetrievalCandidate {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: if id == "a" {
                "study:organoid-a".into()
            } else {
                "study:organoid-b".into()
            },
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
    fn request(candidates: Vec<RetrievalCandidate>) -> FederatedRetrievalQuery {
        FederatedRetrievalQuery {
            request_id: "request:federated-retrieval".into(),
            federation_id: "federation:consortium".into(),
            institution_id: "institution:local".into(),
            purpose: "preclinical replication benchmark".into(),
            semantic_profile: "ome-ngff:5".into(),
            endpoint: "https://federation.invalid/admit".into(),
            allowed_artifacts: vec![PERMITTED_ARTIFACT.into()],
            study_ids: vec!["study:organoid-a".into(), "study:organoid-b".into()],
            scope: "organoid:neural".into(),
            minimum_support_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            candidates,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            approval_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let m = federated_retrieval_synthesis_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn aggregate_exchange_is_qualified() {
        let r = synthesize_federated_retrieval(&request(vec![
            candidate("a", EvidenceState::Supported, "imaging"),
            candidate("b", EvidenceState::Supported, "transcriptomics"),
        ]))
        .unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Qualified);
        assert_eq!(r.aggregate_order.len(), 2);
        assert!(r.effect_receipts[0].starts_with("exchange:permitted-artifacts:"));
    }
    #[test]
    fn missing_modality_is_partial() {
        let r = synthesize_federated_retrieval(&request(vec![
            candidate("a", EvidenceState::Supported, "imaging"),
            candidate("b", EvidenceState::Supported, "imaging"),
        ]))
        .unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Partial);
        assert!(r.omissions.iter().any(|v| v.contains("modality")));
    }
    #[test]
    fn unknown_is_retained() {
        let r = synthesize_federated_retrieval(&request(vec![
            candidate("a", EvidenceState::Unknown, "imaging"),
            candidate("b", EvidenceState::Supported, "transcriptomics"),
        ]))
        .unwrap();
        assert!(r.unknown_order.contains(&"evidence:a".into()));
    }
    #[test]
    fn signer_blocks_exchange() {
        let mut q = request(vec![
            candidate("a", EvidenceState::Supported, "imaging"),
            candidate("b", EvidenceState::Supported, "transcriptomics"),
        ]);
        q.signer_valid = false;
        let r = synthesize_federated_retrieval(&q).unwrap();
        assert_eq!(r.disposition, FederatedRetrievalDisposition::Blocked);
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn digest_is_stable() {
        let r = synthesize_federated_retrieval(&request(vec![
            candidate("a", EvidenceState::Supported, "imaging"),
            candidate("b", EvidenceState::Supported, "transcriptomics"),
        ]))
        .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
