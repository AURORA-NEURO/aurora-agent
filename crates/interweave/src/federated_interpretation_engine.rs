//! Federated, uncertainty-aware interpretation and visualization inference.
//!
//! Atlas feature: `AFA-interweave-P14-F04`.
//!
//! This module turns caller-supplied multimodal observations into a deterministic interpretation
//! receipt. It does not claim a biological mechanism, move raw data, or render a chart on its
//! own. Instead it selects only evidence that is locally available, provenance-backed, policy
//! permitted, and semantically comparable; every missing, contradictory, negative, or uncertain
//! observation is retained in the receipt so a downstream workbench can render an honest view.

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

pub const FEATURE_ID: &str = "AFA-interweave-P14-F04";
pub const CONTRACT_VERSION: &str =
    "interweave-federated-continual-interpretation-visualization-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "InterpretationInferenceRequest5@1";
pub const OUTPUT_SCHEMA: &str = "InterpretationInferenceReceipt7@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.interweave-interpretation-inference-receipt-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationEvidence {
    pub evidence_id: String,
    pub study_id: String,
    pub site_id: String,
    pub modality: String,
    pub measure: String,
    pub value_digest: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub evidence_state: EvidenceState,
    pub uncertainty_basis_points: u32,
    pub effect_direction: String,
    pub negative_result: bool,
    pub omissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationInferenceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub question: String,
    pub scope: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub evidence: Vec<InterpretationEvidence>,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub requested_views: Vec<String>,
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
pub struct InterpretationInferenceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub question: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub evidence_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub view_order: Vec<String>,
    pub interpretation_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub interpretation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InterpretationInferenceError {
    #[error("invalid interpretation inference request: {0}")]
    Invalid(String),
    #[error("interpretation inference artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InterpretationInferenceReceipt {
    pub fn validate(&self) -> Result<(), InterpretationInferenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.question.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.evidence_order.is_empty()
            || self.view_order.is_empty()
            || self.interpretation_order.is_empty()
            || self.effect_receipts.is_empty()
            || !self.raw_data_local
            || !self.aggregate_only
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(InterpretationInferenceError::Invalid(
                "interpretation identity, evidence, views, locality, aggregate boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.evidence_order,
            &self.selected_order,
            &self.missing_order,
            &self.contradictory_order,
            &self.uncertain_order,
            &self.negative_order,
            &self.study_order,
            &self.modality_order,
            &self.view_order,
            &self.interpretation_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(InterpretationInferenceError::Invalid(
                    "interpretation orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let partition = self
            .selected_order
            .iter()
            .chain(self.missing_order.iter())
            .chain(self.contradictory_order.iter())
            .chain(self.uncertain_order.iter())
            .chain(self.negative_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.evidence_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.evidence_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(InterpretationInferenceError::Invalid(
                "interpretation evidence states do not partition observations".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("interpret:local-results:") && effect != "block:unsafe-release"
        }) {
            return Err(InterpretationInferenceError::Invalid(
                "interpretation effect is outside the local-results gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationInferenceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationInferenceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| InterpretationInferenceError::Artifact(error.to_string()))?,
        )
        .map_err(|error| InterpretationInferenceError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "interweave".into(),
        consumers: BTreeSet::from([
            "multimodal interpretation researcher".into(),
            "research visualization workbench".into(),
            "federated evaluation operator".into(),
        ]),
        behavior: "compiles comparable multimodal observations into an uncertainty-aware interpretation and visualization receipt without moving raw data".into(),
        value: "makes cross-study interpretation auditable by retaining provenance, contradictions, negative results, omissions, and explicit federation boundaries".into(),
        inputs: vec![TypedPort { name: "interpretation_inference_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "interpretation_inference_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["interpret:local-results".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ome-ngff".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "federated-interpretation-reviewer".into(), reason: "approval is required before a federated interpretation is released".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Model, ResearchSurface::Protocol, ResearchSurface::Api, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &InterpretationInferenceRequest,
) -> Result<(), InterpretationInferenceError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.question.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.evidence.is_empty()
        || request.required_studies.is_empty()
        || request.required_modalities.is_empty()
        || request.requested_views.is_empty()
        || request.budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationInferenceError::Invalid(
            "request identity, evidence closure, budgets, locality, aggregate boundary, or schema is invalid".into(),
        ));
    }
    let ids = request
        .evidence
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.trim().is_empty())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(InterpretationInferenceError::Invalid(
            "evidence identifiers must be present and unique".into(),
        ));
    }
    Ok(())
}

pub fn compile_interpretation(
    request: &InterpretationInferenceRequest,
) -> Result<InterpretationInferenceReceipt, InterpretationInferenceError> {
    validate_request(request)?;
    let mut evidence = request.evidence.clone();
    evidence.sort_by(|left, right| {
        left.study_id
            .cmp(&right.study_id)
            .then(left.modality.cmp(&right.modality))
            .then(left.measure.cmp(&right.measure))
            .then(left.evidence_id.cmp(&right.evidence_id))
    });
    let evidence_order = evidence
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut missing = Vec::new();
    let mut contradictory = Vec::new();
    let mut uncertain = Vec::new();
    let mut negative = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for item in &evidence {
        if item.artifact_digest.is_none() || item.provenance_digest.is_none() {
            missing.push(item.evidence_id.clone());
            omissions.insert(format!(
                "{}:artifact-or-provenance-missing",
                item.evidence_id
            ));
            continue;
        }
        if item.evidence_state == EvidenceState::Contradicted {
            contradictory.push(item.evidence_id.clone());
            uncertainty.insert(format!("{}:contradicted-evidence", item.evidence_id));
            continue;
        }
        if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) || item.uncertainty_basis_points >= 5000
        {
            uncertain.push(item.evidence_id.clone());
            uncertainty.insert(format!("{}:uncertainty", item.evidence_id));
            continue;
        }
        if item.negative_result {
            negative.push(item.evidence_id.clone());
            negative_evidence.insert(format!("{}:null-or-negative-result", item.evidence_id));
        } else {
            selected.push(item.evidence_id.clone());
        }
        omissions.extend(
            item.omissions
                .iter()
                .map(|omission| format!("{}:{omission}", item.evidence_id)),
        );
    }
    let study_order = evidence
        .iter()
        .map(|item| item.study_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = evidence
        .iter()
        .map(|item| item.modality.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut view_order = request.requested_views.clone();
    view_order.sort();
    view_order.dedup();
    let mut interpretation_order = selected
        .iter()
        .map(|id| format!("interpretation:{id}"))
        .chain(negative.iter().map(|id| format!("negative:{id}")))
        .collect::<Vec<_>>();
    interpretation_order.sort();
    if interpretation_order.is_empty() {
        interpretation_order.push("interpretation:none-qualified".into());
    }
    let present_studies = study_order.iter().collect::<BTreeSet<_>>();
    for required in &request.required_studies {
        if !present_studies.contains(required) {
            omissions.insert(format!("study:{required}:missing"));
        }
    }
    let present_modalities = modality_order.iter().collect::<BTreeSet<_>>();
    for required in &request.required_modalities {
        if !present_modalities.contains(required) {
            omissions.insert(format!("modality:{required}:missing"));
        }
    }
    if !request.policy_allow {
        omissions.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !request.federation_approved {
        omissions.insert("request:federation-approval-missing".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-missing".into());
    }
    if request.budget_units > request.max_budget_units {
        omissions.insert("request:budget-exceeded".into());
    }
    omissions.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("request:adversarial:{event}")),
    );
    let required_studies_present = request
        .required_studies
        .iter()
        .all(|id| present_studies.contains(id));
    let required_modalities_present = request
        .required_modalities
        .iter()
        .all(|id| present_modalities.contains(id));
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || !required_studies_present
        || !required_modalities_present
    {
        "blocked"
    } else if !request.signed_approval {
        "approval_required"
    } else if !missing.is_empty() || !contradictory.is_empty() || !uncertain.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        uncertainty.clear();
    }
    let effect = if disposition == "qualified" {
        format!("interpret:local-results:{}", request.request_id)
    } else {
        "block:unsafe-release".into()
    };
    let payload = json!({
        "schema_version": OUTPUT_SCHEMA,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "evidence_order": evidence_order,
        "selected_order": selected,
        "missing_order": missing,
        "contradictory_order": contradictory,
        "uncertain_order": uncertain,
        "negative_order": negative,
        "view_order": view_order,
        "interpretation_order": interpretation_order,
        "disposition": disposition,
        "replay_identity": request.replay_identity,
    });
    let interpretation_digest = ContentHash::of_value(&payload)
        .map_err(|error| InterpretationInferenceError::Artifact(error.to_string()))?;
    let semantic_loss = omissions
        .iter()
        .map(|item| SemanticLoss {
            field: item.clone(),
            reason: "interpretation input was omitted or gated".into(),
            severity: LossSeverity::DecisionRelevant,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("interpretation-inference:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.workflow_id.clone(),
            relation: "interweave-interpretation-inference".into(),
            digest: interpretation_digest.clone(),
        }],
    )
    .map_err(|error| InterpretationInferenceError::Artifact(error.to_string()))?;
    let receipt = InterpretationInferenceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        question: request.question.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        evidence_order,
        selected_order: selected,
        missing_order: missing,
        contradictory_order: contradictory,
        uncertain_order: uncertain,
        negative_order: negative,
        study_order,
        modality_order,
        view_order,
        interpretation_order,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        interpretation_digest,
        artifact,
        effect_receipts: vec![effect],
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
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

    fn evidence(id: &str, modality: &str, negative_result: bool) -> InterpretationEvidence {
        InterpretationEvidence {
            evidence_id: id.into(),
            study_id: "study:one".into(),
            site_id: "site:a".into(),
            modality: modality.into(),
            measure: "signal".into(),
            value_digest: hash(id),
            artifact_digest: Some(hash(&format!("artifact:{id}"))),
            provenance_digest: Some(hash(&format!("prov:{id}"))),
            evidence_state: EvidenceState::Supported,
            uncertainty_basis_points: 100,
            effect_direction: "positive".into(),
            negative_result,
            omissions: Vec::new(),
        }
    }

    fn request() -> InterpretationInferenceRequest {
        InterpretationInferenceRequest {
            request_id: "request:interpretation".into(),
            workflow_id: "workflow:interpretation".into(),
            federation_id: "federation:alpha".into(),
            question: "Which multimodal signals replicate?".into(),
            scope: "organoid-study".into(),
            semantic_profile: "interpretation:v1".into(),
            schema_version: INPUT_SCHEMA.into(),
            evidence: vec![
                evidence("e:a", "imaging", false),
                evidence("e:b", "transcriptomics", true),
            ],
            required_studies: vec!["study:one".into()],
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            requested_views: vec!["forest".into(), "heatmap".into()],
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
    fn qualified_interpretation_preserves_negative_result() {
        let receipt = compile_interpretation(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert!(receipt.negative_order.contains(&"e:b".into()));
        assert!(receipt.effect_receipts[0].starts_with("interpret:local-results:"));
    }

    #[test]
    fn missing_provenance_is_unresolved_and_explicit() {
        let mut value = request();
        value.evidence[0].provenance_digest = None;
        let receipt = compile_interpretation(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt.missing_order.contains(&"e:a".into()));
        assert!(!receipt.omissions.is_empty());
    }

    #[test]
    fn contradiction_and_uncertainty_never_disappear() {
        let mut value = request();
        value.evidence[0].evidence_state = EvidenceState::Contradicted;
        value.evidence[1].uncertainty_basis_points = 9000;
        let receipt = compile_interpretation(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(!receipt.contradictory_order.is_empty());
        assert!(!receipt.uncertain_order.is_empty());
    }

    #[test]
    fn policy_and_federation_gates_block() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_interpretation(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        value.policy_allow = true;
        value.federation_approved = false;
        assert_eq!(
            compile_interpretation(&value).unwrap().disposition,
            "blocked"
        );
    }

    #[test]
    fn missing_required_modality_blocks_without_invention() {
        let mut value = request();
        value.required_modalities.push("proteomics".into());
        let receipt = compile_interpretation(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("proteomics")));
    }

    #[test]
    fn manifest_is_byte_stable_a2_and_preclinical() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
        assert_eq!(manifest.boundary, PRECLINICAL_BOUNDARY);
    }
}
