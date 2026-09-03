//! Federated continual statistical/causal/ML analysis operations control plane.
//!
//! Atlas feature: `AFA-fiber-P13-F32`.
//!
//! The control plane admits typed analysis-result attestations for an institution-local portfolio
//! and produces a digest-only operations receipt. It never executes a model or exports raw data;
//! it makes the distinction between qualified, unresolved, blocked, missing, and negative results
//! explicit before an operator can manage a local capability or exchange a permitted summary.

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

pub const FEATURE_ID: &str = "AFA-fiber-P13-F32";
pub const CONTRACT_VERSION: &str =
    "fiber-federated-continual-statistical-causal-ml-analysis-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.fiber-qualified-analysis-result-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAnalysisCandidate8 {
    pub candidate_id: String,
    pub study_id: String,
    pub site_id: String,
    pub modality: String,
    pub model_id: String,
    pub result_digest: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub uncertainty_basis_points: u32,
    pub scope_compatible: bool,
    pub permitted: bool,
    pub negative_result: bool,
    pub omissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAnalysisControlRequest {
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub required_models: Vec<String>,
    pub candidates: Vec<FederatedAnalysisCandidate8>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAnalysisControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub model_order: Vec<String>,
    pub selected_study_order: Vec<String>,
    pub selected_modality_order: Vec<String>,
    pub selected_model_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub missing_model_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub result_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: String,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedAnalysisControlError {
    #[error("invalid federated analysis control request: {0}")]
    Invalid(String),
    #[error("federated analysis artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl FederatedAnalysisControlReceipt {
    pub fn validate(&self) -> Result<(), FederatedAnalysisControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.model_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.autonomy_tier != "a2"
            || !self.raw_data_local
            || !self.aggregate_only
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(FederatedAnalysisControlError::Invalid(
                "analysis identity, axes, autonomy, locality, aggregate boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_order,
            &self.study_order,
            &self.modality_order,
            &self.model_order,
            &self.selected_study_order,
            &self.selected_modality_order,
            &self.selected_model_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.missing_model_order,
            &self.uncertainty_order,
            &self.omission_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(FederatedAnalysisControlError::Invalid(
                    "analysis order is not canonical".into(),
                ));
            }
        }
        let partition = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.missing_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.candidate_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.candidate_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(FederatedAnalysisControlError::Invalid(
                "analysis states do not partition candidates".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-summaries:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedAnalysisControlError::Invalid(
                "analysis effect is outside the operations gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedAnalysisControlError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))?,
        )
        .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "fiber".into(),
        consumers: BTreeSet::from(["downstream crate maintainer".into(), "analysis operator".into(), "federated research verifier".into()]),
        behavior: "admits typed federated analysis-result attestations into a deterministic operations receipt without executing models or moving raw data".into(),
        value: "keeps statistical, causal, and ML portfolio effects bounded while preserving replay, provenance, negative evidence, and zero-versus-unknown distinctions".into(),
        inputs: vec![TypedPort { name: "analysis_question".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_analysis_result".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["operate:institution-node".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "institution analysis operator".into(), reason: "capability management and permitted-summary exchange require scoped authority".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &FederatedAnalysisControlRequest,
) -> Result<(), FederatedAnalysisControlError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_studies.is_empty()
        || request.required_modalities.is_empty()
        || request.required_models.is_empty()
        || request.candidates.is_empty()
        || request.budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedAnalysisControlError::Invalid("analysis identity, axes, candidates, budget, locality, aggregate boundary, or schema is invalid".into()));
    }
    let ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.trim().is_empty())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(FederatedAnalysisControlError::Invalid(
            "candidate identifiers must be present and unique".into(),
        ));
    }
    Ok(())
}

pub fn admit_federated_analysis(
    request: &FederatedAnalysisControlRequest,
) -> Result<FederatedAnalysisControlReceipt, FederatedAnalysisControlError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        left.study_id
            .cmp(&right.study_id)
            .then(left.modality.cmp(&right.modality))
            .then(left.model_id.cmp(&right.model_id))
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut missing = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &candidates {
        if candidate.artifact_digest.is_none() || candidate.provenance_digest.is_none() {
            missing.push(candidate.candidate_id.clone());
            omission.insert(format!(
                "{}:artifact-or-provenance-missing",
                candidate.candidate_id
            ));
            continue;
        }
        if !candidate.scope_compatible || !candidate.permitted {
            blocked.push(candidate.candidate_id.clone());
            omission.insert(format!(
                "{}:scope-or-permission-denied",
                candidate.candidate_id
            ));
            continue;
        }
        if candidate.evidence_state == EvidenceState::Contradicted {
            blocked.push(candidate.candidate_id.clone());
            uncertainty.insert(format!("{}:contradicted", candidate.candidate_id));
            continue;
        }
        if matches!(
            candidate.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) || candidate.uncertainty_basis_points >= 5000
            || candidate.replay_identity != request.replay_identity
        {
            unresolved.push(candidate.candidate_id.clone());
            uncertainty.insert(format!(
                "{}:uncertain-or-replay-mismatch",
                candidate.candidate_id
            ));
            continue;
        }
        if candidate.negative_result {
            selected.push(candidate.candidate_id.clone());
            negative.insert(format!("{}:negative-result", candidate.candidate_id));
        } else {
            selected.push(candidate.candidate_id.clone());
        }
        omission.extend(
            candidate
                .omissions
                .iter()
                .map(|entry| format!("{}:{entry}", candidate.candidate_id)),
        );
    }
    let study_order = candidates
        .iter()
        .map(|candidate| candidate.study_id.clone())
        .chain(request.required_studies.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = candidates
        .iter()
        .map(|candidate| candidate.modality.clone())
        .chain(request.required_modalities.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let model_order = candidates
        .iter()
        .map(|candidate| candidate.model_id.clone())
        .chain(request.required_models.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_set = selected.iter().collect::<BTreeSet<_>>();
    let selected_study_order = study_order
        .iter()
        .filter(|axis| {
            candidates.iter().any(|candidate| {
                selected_set.contains(&candidate.candidate_id) && &candidate.study_id == *axis
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_modality_order = modality_order
        .iter()
        .filter(|axis| {
            candidates.iter().any(|candidate| {
                selected_set.contains(&candidate.candidate_id) && &candidate.modality == *axis
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_model_order = model_order
        .iter()
        .filter(|axis| {
            candidates.iter().any(|candidate| {
                selected_set.contains(&candidate.candidate_id) && &candidate.model_id == *axis
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_study_order = request
        .required_studies
        .iter()
        .filter(|axis| !study_order.contains(axis))
        .cloned()
        .collect::<Vec<_>>();
    let missing_modality_order = request
        .required_modalities
        .iter()
        .filter(|axis| !modality_order.contains(axis))
        .cloned()
        .collect::<Vec<_>>();
    let missing_model_order = request
        .required_models
        .iter()
        .filter(|axis| !model_order.contains(axis))
        .cloned()
        .collect::<Vec<_>>();
    omission.extend(
        missing_study_order
            .iter()
            .map(|axis| format!("study:{axis}:missing")),
    );
    omission.extend(
        missing_modality_order
            .iter()
            .map(|axis| format!("modality:{axis}:missing")),
    );
    omission.extend(
        missing_model_order
            .iter()
            .map(|axis| format!("model:{axis}:missing")),
    );
    omission.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("request:adversarial:{event}")),
    );
    let global_open = request.policy_allow
        && request.protected_closure
        && request.federation_approved
        && request.raw_data_local
        && request.aggregate_only
        && request.adversarial_events.is_empty();
    let disposition = if !global_open
        || !blocked.is_empty()
        || !missing_study_order.is_empty()
        || !missing_modality_order.is_empty()
        || !missing_model_order.is_empty()
    {
        "blocked"
    } else if !request.signed_approval {
        "approval_required"
    } else if !unresolved.is_empty() || !missing.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let mut effects = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", request.federation_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    effects.sort();
    let payload = json!({"schema_version": OUTPUT_SCHEMA, "request_id": request.request_id, "candidate_order": candidate_order, "selected_order": selected, "unresolved_order": unresolved, "blocked_order": blocked, "missing_order": missing, "study_order": study_order, "modality_order": modality_order, "model_order": model_order, "disposition": disposition, "replay_identity": request.replay_identity});
    let result_digest = ContentHash::of_value(&payload)
        .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(
        &serde_json::to_value(
            candidates
                .iter()
                .filter_map(|candidate| candidate.provenance_digest.clone())
                .collect::<Vec<_>>(),
        )
        .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))?,
    )
    .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))?;
    let semantic_loss = omission
        .iter()
        .map(|entry| SemanticLoss {
            field: entry.clone(),
            reason: "analysis candidate or axis was omitted or gated".into(),
            severity: LossSeverity::DecisionRelevant,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("qualified-analysis:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.request_id.clone(),
            relation: "fiber-federated-analysis-control".into(),
            digest: result_digest.clone(),
        }],
    )
    .map_err(|error| FederatedAnalysisControlError::Artifact(error.to_string()))?;
    let receipt = FederatedAnalysisControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        federation_id: request.federation_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_order: payload["missing_order"]
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
        model_order: payload["model_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_study_order,
        selected_modality_order,
        selected_model_order,
        missing_study_order,
        missing_modality_order,
        missing_model_order,
        uncertainty_order: uncertainty.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest,
        result_digest,
        artifact,
        effect_receipts: effects,
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
    fn candidate(id: &str, model: &str) -> FederatedAnalysisCandidate8 {
        FederatedAnalysisCandidate8 {
            candidate_id: id.into(),
            study_id: "study:one".into(),
            site_id: "site:a".into(),
            modality: "imaging".into(),
            model_id: model.into(),
            result_digest: hash(id),
            artifact_digest: Some(hash(&format!("artifact:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            uncertainty_basis_points: 100,
            scope_compatible: true,
            permitted: true,
            negative_result: false,
            omissions: Vec::new(),
        }
    }
    fn request() -> FederatedAnalysisControlRequest {
        FederatedAnalysisControlRequest {
            request_id: "request:fiber-analysis".into(),
            requester: "downstream-maintainer".into(),
            purpose: "portfolio-analysis".into(),
            federation_id: "federation:alpha".into(),
            semantic_profile: "analysis:v1".into(),
            schema_version: INPUT_SCHEMA.into(),
            required_studies: vec!["study:one".into()],
            required_modalities: vec!["imaging".into()],
            required_models: vec!["model:a".into()],
            candidates: vec![candidate("candidate:a", "model:a")],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            budget_units: 10,
            max_budget_units: 10,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_analysis_emits_bounded_effects() {
        let receipt = admit_federated_analysis(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("manage:local-capability:")));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("exchange:permitted-summaries:")));
    }
    #[test]
    fn missing_artifact_is_unresolved() {
        let mut value = request();
        value.candidates[0].artifact_digest = None;
        let receipt = admit_federated_analysis(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt.missing_order.contains(&"candidate:a".into()));
    }
    #[test]
    fn unknown_and_replay_mismatch_remain_unresolved() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        value.candidates[0].replay_identity = hash("other");
        let receipt = admit_federated_analysis(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(!receipt.uncertainty_order.is_empty());
    }
    #[test]
    fn policy_federation_and_adversarial_gates_block() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            admit_federated_analysis(&value).unwrap().disposition,
            "blocked"
        );
        value.policy_allow = true;
        value.adversarial_events = vec!["poisoned-artifact".into()];
        assert_eq!(
            admit_federated_analysis(&value).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn approval_gate_blocks_effects_without_reclassifying_evidence() {
        let mut value = request();
        value.signed_approval = false;
        let receipt = admit_federated_analysis(&value).unwrap();
        assert_eq!(receipt.disposition, "approval_required");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn manifest_is_a2_byte_stable_and_preclinical() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
        assert_eq!(manifest.boundary, PRECLINICAL_BOUNDARY);
    }
}
