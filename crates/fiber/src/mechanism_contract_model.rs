//! Federated continual mechanism-exploration contract model.
//!
//! Atlas feature: `AFA-fiber-P08-F08`.  This module is the typed wire boundary
//! for mechanism candidates exchanged between institutions.  It canonicalizes
//! caller-provided attestations and makes missing, uncertain, contradictory, or
//! denied evidence explicit; it never discovers mechanisms, executes tools, or
//! exports raw observations.

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

pub const FEATURE_ID: &str = "AFA-fiber-P08-F08";
pub const CONTRACT_VERSION: &str =
    "fiber-federated-continual-mechanism-exploration-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "MechanismQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "MechanismPortfolio2@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.mechanism-contract-model-2+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismContractCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_only: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismQuestionContract {
    pub schema_version: String,
    pub question_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub candidates: Vec<MechanismContractCandidate>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismContractDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolioContract {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub question_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: MechanismContractDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub contract_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MechanismContractError {
    #[error("invalid mechanism contract: {0}")]
    Invalid(String),
    #[error("mechanism contract artifact failed: {0}")]
    Artifact(String),
}

fn invalid(value: impl Into<String>) -> MechanismContractError {
    MechanismContractError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl MechanismPortfolioContract {
    pub fn validate(&self) -> Result<(), MechanismContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.question_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
        {
            return Err(invalid(
                "mechanism contract identity, locality, or candidates are incomplete",
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unknown_order,
            &self.denied_order,
            &self.missing_candidate_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("mechanism contract ordering is not canonical"));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid(
                "mechanism contract states do not partition candidates",
            ));
        }
        if !self.effect_receipts.is_empty() {
            return Err(invalid(
                "mechanism contract model cannot claim an external effect",
            ));
        }
        for value in [
            &self.replay_identity,
            &self.contract_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("mechanism contract digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismContractError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("mechanism contract artifact type is invalid"));
        }
        Ok(())
    }
}

pub fn mechanism_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "fiber".into(),
        consumers: ["context compiler engineer".into(), "federation operator".into()].into(),
        behavior: "canonicalizes typed federated mechanism candidates and emits compatible, partial, unknown, or blocked contract receipts without discovering or executing mechanisms".into(),
        value: "prevents semantic drift and silent evidence loss when policy-separated institutions exchange mechanism metadata".into(),
        inputs: vec![TypedPort { name: "mechanism_question".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "mechanism_portfolio".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::<Effect>::new(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Model, ResearchSurface::Policy].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn model_mechanism_contract(
    question: &MechanismQuestionContract,
) -> Result<MechanismPortfolioContract, MechanismContractError> {
    validate_question(question)?;
    let mut candidates = question.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = candidates
        .iter()
        .map(|row| row.candidate_id.clone())
        .collect::<Vec<_>>();
    let required = question
        .required_candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let known = candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut missing = required
        .difference(&known)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for row in &candidates {
        if row.negative_result {
            negative.insert(format!("{}:negative-result", row.candidate_id));
        }
        omissions.extend(
            row.omissions
                .iter()
                .map(|item| format!("{}:{item}", row.candidate_id)),
        );
        uncertainty.extend(
            row.uncertainty
                .iter()
                .map(|item| format!("{}:{item}", row.candidate_id)),
        );
        if row.evidence_state == EvidenceState::Contradicted || !row.local_only || !row.permitted {
            denied.insert(row.candidate_id.clone());
        } else if row.semantic_profile != question.semantic_profile
            || !row.omissions.is_empty()
            || !row.uncertainty.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unknown.insert(row.candidate_id.clone());
        } else {
            selected.insert(row.candidate_id.clone());
        }
    }
    for id in &missing {
        omissions.insert(format!("{id}:required-candidate-missing"));
    }
    negative.extend(
        question
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !question.policy_allow
        || !question.protected_closure
        || !question.signed_approval
        || !question.federation_approved
        || !question.raw_data_local
        || !question.aggregate_only
        || !question.adversarial_events.is_empty();
    if !question.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !question.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !question.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !question.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    if global_block {
        denied.extend(candidate_order.iter().cloned());
        selected.clear();
        unknown.clear();
        missing.clear();
        omissions.insert("request:contract-release-blocked".into());
    }
    let disposition = if global_block {
        MechanismContractDisposition::Blocked
    } else if required.is_subset(&selected) && unknown.is_empty() && denied.is_empty() {
        MechanismContractDisposition::Compatible
    } else if selected.is_empty() || !missing.is_empty() {
        MechanismContractDisposition::Unknown
    } else {
        MechanismContractDisposition::Partial
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let denied_order = denied.into_iter().collect::<Vec<_>>();
    let missing_candidate_order = missing.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = Vec::<String>::new();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "question_id": question.question_id,
        "federation_id": question.federation_id,
        "semantic_profile": question.semantic_profile,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "unknown_order": unknown_order,
        "denied_order": denied_order,
        "missing_candidate_order": missing_candidate_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "replay_identity": question.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": question.raw_data_local,
        "aggregate_only": question.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let contract_digest = ContentHash::of_value(&payload)
        .map_err(|error| MechanismContractError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-contract:{}", question.question_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MechanismContractError::Artifact(error.to_string()))?;
    let contract = MechanismPortfolioContract {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        question_id: question.question_id.clone(),
        federation_id: question.federation_id.clone(),
        semantic_profile: question.semantic_profile.clone(),
        disposition,
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
        unknown_order: payload["unknown_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        denied_order: payload["denied_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_candidate_order: payload["missing_candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        replay_identity: question.replay_identity.clone(),
        contract_digest,
        artifact,
        effect_receipts,
        raw_data_local: question.raw_data_local,
        aggregate_only: question.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    contract.validate()?;
    Ok(contract)
}

fn validate_question(question: &MechanismQuestionContract) -> Result<(), MechanismContractError> {
    if question.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || question.question_id.trim().is_empty()
        || question.federation_id.trim().is_empty()
        || question.semantic_profile.trim().is_empty()
        || question.required_candidate_order.is_empty()
        || !canonical(&question.required_candidate_order)
        || question.candidates.is_empty()
        || !digest(&question.replay_identity)
        || !canonical(&question.adversarial_events)
        || !question.raw_data_local
        || !question.aggregate_only
        || question.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "mechanism question identity, closure, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for row in &question.candidates {
        if row.candidate_id.trim().is_empty()
            || !ids.insert(row.candidate_id.clone())
            || row.mechanism_id.trim().is_empty()
            || row.study_order.is_empty()
            || !canonical(&row.study_order)
            || row.modality_order.is_empty()
            || !canonical(&row.modality_order)
            || row.semantic_profile.trim().is_empty()
            || !digest(&row.artifact_digest)
            || !digest(&row.evidence_digest)
            || !digest(&row.provenance_digest)
            || !canonical(&row.omissions)
            || !canonical(&row.uncertainty)
        {
            return Err(invalid(format!(
                "candidate {} is malformed or duplicated",
                row.candidate_id
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
    fn question() -> MechanismQuestionContract {
        let d = hash("mechanism");
        let candidate = |id: &str| MechanismContractCandidate {
            candidate_id: id.into(),
            mechanism_id: format!("mechanism:{id}"),
            study_order: vec!["study:imaging".into(), "study:omics".into()],
            modality_order: vec!["modality:image".into(), "modality:single-cell".into()],
            semantic_profile: "preclinical-neural".into(),
            artifact_digest: d.clone(),
            evidence_digest: d.clone(),
            provenance_digest: d.clone(),
            evidence_state: EvidenceState::Supported,
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_only: true,
            permitted: true,
        };
        MechanismQuestionContract {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            question_id: "question:one".into(),
            federation_id: "fed:commons".into(),
            semantic_profile: "preclinical-neural".into(),
            required_candidate_order: vec!["candidate:a".into(), "candidate:b".into()],
            replay_identity: d.clone(),
            candidates: vec![candidate("candidate:a"), candidate("candidate:b")],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            mechanism_contract_model_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn compatible_contract() {
        assert_eq!(
            model_mechanism_contract(&question()).unwrap().disposition,
            MechanismContractDisposition::Compatible
        );
    }
    #[test]
    fn deterministic_contract() {
        let a = model_mechanism_contract(&question()).unwrap();
        let b = model_mechanism_contract(&question()).unwrap();
        assert_eq!(a.contract_digest, b.contract_digest);
    }
    #[test]
    fn missing_candidate_is_unknown() {
        let mut value = question();
        value.required_candidate_order = vec!["candidate:a".into(), "candidate:c".into()];
        assert_eq!(
            model_mechanism_contract(&value).unwrap().disposition,
            MechanismContractDisposition::Unknown
        );
    }
    #[test]
    fn uncertain_candidate_is_partial() {
        let mut value = question();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            model_mechanism_contract(&value).unwrap().disposition,
            MechanismContractDisposition::Partial
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = question();
        value.policy_allow = false;
        assert_eq!(
            model_mechanism_contract(&value).unwrap().disposition,
            MechanismContractDisposition::Blocked
        );
    }
    #[test]
    fn no_external_effects() {
        let value = model_mechanism_contract(&question()).unwrap();
        assert!(value.effect_receipts.is_empty());
    }
}
