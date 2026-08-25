//! Federated continual interpretation and visualization assurance.
//!
//! Atlas feature: `AFA-section-P14-F28`.
//!
//! This A1 verifier consumes evidence-backed interpretation candidates and emits a local,
//! content-addressed release receipt. It is intentionally downstream of the section compiler:
//! it does not invent claims, fetch evidence, or export raw source data. Missing/unknown/
//! contradicted/negative evidence is retained and incomplete protected closure cannot pass.

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

pub const FEATURE_ID: &str = "AFA-section-P14-F28";
pub const CONTRACT_VERSION: &str = "section-federated-interpretation-assurance/1.0";
pub const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceBackedState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBackedResult {
    pub interpretation_id: String,
    pub result_id: String,
    pub visualization_id: String,
    pub scope: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub support_milli: u16,
    pub state: EvidenceBackedState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub competing_explanations: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub scope: String,
    pub required_modalities: Vec<String>,
    pub minimum_studies: usize,
    pub minimum_support_milli: u16,
    pub candidates: Vec<EvidenceBackedResult>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub policy_allow: bool,
    pub permitted_artifacts_only: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub budget: u64,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveInterpretation {
    pub interpretation_id: String,
    pub result_id: String,
    pub visualization_id: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub scope: String,
    pub disposition: InterpretationDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub result_order: Vec<String>,
    pub visualization_order: Vec<String>,
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
    pub interpretations: Vec<InteractiveInterpretation>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InterpretationAssuranceError {
    #[error("invalid section interpretation request: {0}")]
    Invalid(String),
    #[error("section interpretation artifact failed: {0}")]
    Artifact(String),
    #[error("section interpretation serialization failed: {0}")]
    Serialization(String),
}

impl InterpretationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), InterpretationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.support_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(InterpretationAssuranceError::Invalid(
                "identity, ranking, support, locality, effects, or boundary is incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .any(|id| !self.candidate_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(InterpretationAssuranceError::Invalid(
                "interpretation state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.result_order,
            &self.visualization_order,
            &self.study_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InterpretationAssuranceError::Invalid(
                    "interpretation ordering is not canonical".into(),
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
                return Err(InterpretationAssuranceError::Invalid(
                    "interpretation digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release"
                && !effect.starts_with("evaluate:interpretation-assurance:")
        }) {
            return Err(InterpretationAssuranceError::Invalid(
                "effect is outside interpretation assurance gate".into(),
            ));
        }
        for interpretation in &self.interpretations {
            if !interpretation.raw_data_local
                || interpretation.boundary != PRECLINICAL_BOUNDARY
                || interpretation.comparability_digest == ContentHash::of_bytes(b"")
            {
                return Err(InterpretationAssuranceError::Invalid(
                    "interactive interpretation is incomplete or non-local".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, InterpretationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))
    }
}

pub fn interpretation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "section".into(), consumers: ["downstream AURORA crate maintainer".into(), "federation interpretation reviewer".into()].into(), behavior: "verifies federated evidence-backed interpretation candidates with modality, study, comparability, provenance, replay, omission, policy, artifact-only federation, and locality gates without manufacturing a scientific conclusion".into(), value: "makes InteractiveInterpretation release decisions independently replayable at the Decision Section boundary".into(), inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: "EvidenceBackedResult4@1".into(), required: true }], outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: "InteractiveInterpretation7@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_interpretations(
    request: &InterpretationAssuranceRequest,
) -> Result<InterpretationAssuranceReceipt, InterpretationAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then(left.interpretation_id.cmp(&right.interpretation_id))
    });
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.interpretation_id.clone())
        .collect::<Vec<_>>();
    let support_order = candidates
        .iter()
        .map(|candidate| candidate.support_milli)
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut results = BTreeSet::new();
    let mut visualizations = BTreeSet::new();
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
    let mut interpretations = Vec::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let cost = (candidate.interpretation_id.len()
            + candidate.result_id.len()
            + candidate.visualization_id.len()
            + candidate.study_ids.len()
            + candidate.modality_ids.len()) as u64
            + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let required_modalities = request
            .required_modalities
            .iter()
            .all(|required| candidate.modality_ids.contains(required));
        let complete = request.policy_allow
            && request.permitted_artifacts_only
            && request.protected_closure
            && request.raw_data_local
            && candidate.raw_data_local
            && candidate.state == EvidenceBackedState::Supported
            && candidate.scope == request.scope
            && candidate.study_ids.len() >= request.minimum_studies
            && required_modalities
            && candidate.support_milli >= request.minimum_support_milli
            && candidate.comparability_digest.is_some()
            && !candidate.competing_explanations.is_empty()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.negative_evidence.is_empty()
            && candidate.replay_identity == request.replay_identity
            && request.benchmark_digest.is_some()
            && budget_ok;
        if complete {
            spent = spent.saturating_add(cost);
            admitted.push(candidate.interpretation_id.clone());
            results.insert(candidate.result_id.clone());
            visualizations.insert(candidate.visualization_id.clone());
            studies.extend(candidate.study_ids.iter().cloned());
            modalities.extend(candidate.modality_ids.iter().cloned());
            semantics.insert(candidate.semantic_digest.clone());
            artifacts.insert(candidate.artifact_digest.clone());
            evidence.insert(candidate.evidence_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
            let cmp = candidate
                .comparability_digest
                .clone()
                .expect("checked above");
            comparability.insert(cmp.clone());
            interpretations.push(InteractiveInterpretation {
                interpretation_id: candidate.interpretation_id.clone(),
                result_id: candidate.result_id.clone(),
                visualization_id: candidate.visualization_id.clone(),
                study_ids: candidate.study_ids.clone(),
                modality_ids: candidate.modality_ids.clone(),
                semantic_digest: candidate.semantic_digest.clone(),
                artifact_digest: candidate.artifact_digest.clone(),
                evidence_digest: candidate.evidence_digest.clone(),
                provenance_digest: candidate.provenance_digest.clone(),
                comparability_digest: cmp,
                replay_identity: candidate.replay_identity.clone(),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            });
        } else {
            blocked.insert(candidate.interpretation_id.clone());
            if matches!(
                candidate.state,
                EvidenceBackedState::Unknown | EvidenceBackedState::Unmeasured
            ) {
                unknown.insert(candidate.interpretation_id.clone());
                uncertainty.insert(
                    format!(
                        "interpretation:{}:state-{:?}-not-admitted",
                        candidate.interpretation_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.state == EvidenceBackedState::Contradicted {
                negative.insert(format!(
                    "interpretation:{}:contradicted-negative-evidence",
                    candidate.interpretation_id
                ));
            }
            if !request.policy_allow {
                negative.insert("request:policy-denied".into());
            }
            if !request.permitted_artifacts_only {
                negative.insert("request:raw-artifact-exchange-denied".into());
            }
            if !request.protected_closure {
                uncertainty.insert("request:protected-closure-incomplete".into());
            }
            if !request.raw_data_local || !candidate.raw_data_local {
                negative.insert(format!(
                    "interpretation:{}:raw-data-locality-failed",
                    candidate.interpretation_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "interpretation:{}:scope-mismatch",
                    candidate.interpretation_id
                ));
            }
            if candidate.study_ids.len() < request.minimum_studies {
                omissions.insert(format!(
                    "interpretation:{}:study-floor-incomplete",
                    candidate.interpretation_id
                ));
            }
            for required in &request.required_modalities {
                if !candidate.modality_ids.contains(required) {
                    omissions.insert(format!(
                        "interpretation:{}:modality-missing:{}",
                        candidate.interpretation_id, required
                    ));
                }
            }
            if candidate.comparability_digest.is_none() {
                omissions.insert(format!(
                    "interpretation:{}:comparability-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.support_milli < request.minimum_support_milli {
                uncertainty.insert(format!(
                    "interpretation:{}:support-below-threshold",
                    candidate.interpretation_id
                ));
            }
            if candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "interpretation:{}:replay-mismatch",
                    candidate.interpretation_id
                ));
            }
            if request.benchmark_digest.is_none() {
                omissions.insert(format!(
                    "interpretation:{}:benchmark-missing",
                    candidate.interpretation_id
                ));
            }
            if !candidate.omissions.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:protected-closure-incomplete",
                    candidate.interpretation_id
                ));
            }
            if !candidate.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:uncertainty-unresolved",
                    candidate.interpretation_id
                ));
            }
            if !candidate.negative_evidence.is_empty() {
                negative.insert(format!(
                    "interpretation:{}:negative-evidence-present",
                    candidate.interpretation_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "interpretation:{}:budget-exhausted",
                    candidate.interpretation_id
                ));
            }
        }
    }
    if request.benchmark_digest.is_none() {
        uncertainty.insert("request:benchmark-missing".into());
    }
    let disposition = if !request.policy_allow
        || !request.permitted_artifacts_only
        || !request.protected_closure
        || !request.raw_data_local
    {
        InterpretationDisposition::Blocked
    } else if admitted.is_empty() {
        InterpretationDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        InterpretationDisposition::Qualified
    } else {
        InterpretationDisposition::Partial
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "federation_id": request.federation_id, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "result_order": results, "visualization_order": visualizations, "study_order": studies, "modality_order": modalities, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "benchmark_digest": request.benchmark_digest, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("section-interpretation-assurance:{}", request.request_id),
        "application/vnd.aurora.section-interpretation+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "evaluate:interpretation-assurance:{}",
            request.request_id
        )]
    };
    let receipt = InterpretationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        result_order: results.into_iter().collect(),
        visualization_order: visualizations.into_iter().collect(),
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
        effect_receipts,
        interpretations,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &InterpretationAssuranceRequest,
) -> Result<(), InterpretationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.minimum_studies == 0
        || request.minimum_support_milli > 1000
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationAssuranceError::Invalid("section interpretation request identity, modality/study floors, candidates, budget, or boundary is incomplete".into()));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.interpretation_id.trim().is_empty()
            || candidate.result_id.trim().is_empty()
            || candidate.visualization_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.study_ids.is_empty()
            || candidate.modality_ids.is_empty()
            || candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.interpretation_id.clone())
        {
            return Err(InterpretationAssuranceError::Invalid(format!(
                "interpretation {} is invalid or duplicated",
                candidate.interpretation_id
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
    fn candidate(id: &str, state: EvidenceBackedState) -> EvidenceBackedResult {
        EvidenceBackedResult {
            interpretation_id: format!("interpretation:{id}"),
            result_id: format!("result:{id}"),
            visualization_id: format!("view:{id}"),
            scope: "organoid:neural".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            modality_ids: vec!["imaging".into(), "omics".into()],
            support_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: Some(hash(&format!("comparability:{id}"))),
            replay_identity: hash("replay"),
            competing_explanations: vec!["alternative:one".into()],
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(candidates: Vec<EvidenceBackedResult>) -> InterpretationAssuranceRequest {
        InterpretationAssuranceRequest {
            request_id: "request:section".into(),
            workflow_id: "workflow:interpretation".into(),
            federation_id: "federation:commons".into(),
            scope: "organoid:neural".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            minimum_studies: 2,
            minimum_support_milli: 700,
            candidates,
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            policy_allow: true,
            permitted_artifacts_only: true,
            protected_closure: true,
            raw_data_local: true,
            budget: 10_000,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a1() {
        let manifest = interpretation_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_interpretations_are_qualified() {
        let receipt = assure_interpretations(&request(vec![
            candidate("b", EvidenceBackedState::Supported),
            candidate("a", EvidenceBackedState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Qualified);
        assert_eq!(
            receipt.candidate_order,
            vec!["interpretation:a", "interpretation:b"]
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_remain_visible() {
        let receipt = assure_interpretations(&request(vec![
            candidate("a", EvidenceBackedState::Supported),
            candidate("b", EvidenceBackedState::Unknown),
            candidate("c", EvidenceBackedState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"interpretation:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("interpretation:c")));
    }
    #[test]
    fn federation_denial_blocks_release() {
        let mut input = request(vec![candidate("a", EvidenceBackedState::Supported)]);
        input.permitted_artifacts_only = false;
        let receipt = assure_interpretations(&input).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn missing_comparability_is_unknown() {
        let mut input = request(vec![candidate("a", EvidenceBackedState::Supported)]);
        input.candidates[0].comparability_digest = None;
        let receipt = assure_interpretations(&input).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("comparability-missing")));
    }
    #[test]
    fn duplicate_interpretation_is_rejected() {
        let mut duplicate = candidate("a", EvidenceBackedState::Supported);
        duplicate.result_id = "result:other".into();
        assert!(assure_interpretations(&request(vec![
            candidate("a", EvidenceBackedState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
