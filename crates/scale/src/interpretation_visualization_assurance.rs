//! Federated continual interpretation and visualization assurance (`AFA-scale-P14-F28`).
//!
//! This boundary verifies caller-produced multimodal interpretation candidates before they are
//! rendered or shared. It is deliberately read-only: no model is fitted, no raw data is moved,
//! and no scientific conclusion is upgraded when evidence, comparability, policy, or replay
//! closure is incomplete.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-scale-P14-F28";
pub const CONTRACT_VERSION: &str =
    "scale-federated-continual-interpretation-visualization-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult4@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.scale-interactive-interpretation-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationCandidate4 {
    pub interpretation_id: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub comparability_milli: u16,
    pub visualization_ready: bool,
    pub policy_allowed: bool,
    pub local_only: bool,
    pub protected_closure: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBackedResult4 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub required_panel_order: Vec<String>,
    pub minimum_comparability_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub adversarial_clear: bool,
    pub boundary: String,
    pub candidates: Vec<InterpretationCandidate4>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveInterpretationArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveInterpretation7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub interpretation_digest: ContentHash,
    pub artifact: InteractiveInterpretationArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterpretationVisualizationAssuranceError {
    #[error("invalid interpretation assurance request or receipt: {0}")]
    Invalid(String),
    #[error("interpretation assurance artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn interpretation_visualization_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "scale".into(),
        consumers: ["laboratory automation engineer".into(), "research workflow operator".into(), "visualization steward".into()].into(),
        behavior: "verify federated continual multimodal interpretation and visualization candidates with comparability, evidence, provenance, replay, policy, and locality gates".into(),
        value: "prevents incomplete or non-comparable interpretations from being rendered or shared as qualified research artifacts".into(),
        inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) },
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
        ],
        authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &EvidenceBackedResult4,
) -> Result<(), InterpretationVisualizationAssuranceError> {
    if request.schema_version != INPUT_SCHEMA
        || [
            &request.request_id,
            &request.consumer,
            &request.purpose,
            &request.target_scope,
            &request.semantic_profile,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        || request.required_panel_order.is_empty()
        || !ordered(&request.required_panel_order)
        || request.minimum_comparability_milli == 0
        || !digest(&request.replay_identity)
        || !request.aggregate_only
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.candidates.is_empty()
    {
        return Err(InterpretationVisualizationAssuranceError::Invalid(
            "interpretation identity, panel, replay, locality, bounds, or boundary is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.interpretation_id.trim().is_empty()
            || !ids.insert(candidate.interpretation_id.clone())
            || candidate.target_scope.trim().is_empty()
            || candidate.semantic_profile.trim().is_empty()
            || !digest(&candidate.artifact_digest)
            || !digest(&candidate.provenance_digest)
            || candidate.replay_identity != request.replay_identity
            || candidate.comparability_milli > 1000
            || !ordered(&candidate.omission_order)
            || !ordered(&candidate.uncertainty_order)
        {
            return Err(InterpretationVisualizationAssuranceError::Invalid(
                "candidate identity, digest, replay, comparability, or ordering is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl InteractiveInterpretation7 {
    pub fn validate(&self) -> Result<(), InterpretationVisualizationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.candidate_order.is_empty()
            || self.panel_order.is_empty()
            || self.effect_receipts.is_empty()
            || [
                &self.request_id,
                &self.consumer,
                &self.purpose,
                &self.target_scope,
                &self.semantic_profile,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(InterpretationVisualizationAssuranceError::Invalid(
                "interpretation identity, locality, panel, disposition, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.panel_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(InterpretationVisualizationAssuranceError::Invalid(
                    "interpretation ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let states = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(InterpretationVisualizationAssuranceError::Invalid(
                "interpretation candidate states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.interpretation_digest)
            || self.artifact.content_hash != self.interpretation_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(InterpretationVisualizationAssuranceError::Artifact(
                "interpretation digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("render:interpretation:"))
        {
            return Err(InterpretationVisualizationAssuranceError::Invalid(
                "interpretation effect is outside assurance gate".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("render:interpretation:{}", self.request_id)]
        {
            return Err(InterpretationVisualizationAssuranceError::Invalid(
                "qualified interpretation effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(InterpretationVisualizationAssuranceError::Invalid(
                "non-qualified interpretation must block".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_interpretation_visualization(
    request: &EvidenceBackedResult4,
) -> Result<InteractiveInterpretation7, InterpretationVisualizationAssuranceError> {
    validate_request(request)?;
    let candidate_order = request
        .candidates
        .iter()
        .map(|c| c.interpretation_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for c in &request.candidates {
        provenance.insert(c.provenance_digest.clone());
        if c.negative_result {
            negative.insert(c.interpretation_id.clone());
        }
        omissions.extend(
            c.omission_order
                .iter()
                .map(|o| format!("{}:{}", c.interpretation_id, o)),
        );
        uncertainty.extend(
            c.uncertainty_order
                .iter()
                .map(|o| format!("{}:{}", c.interpretation_id, o)),
        );
        let hard = !c.policy_allowed
            || !c.local_only
            || !c.protected_closure
            || c.target_scope != request.target_scope
            || c.semantic_profile != request.semantic_profile
            || c.comparability_milli < request.minimum_comparability_milli
            || !c.visualization_ready
            || !digest(&c.artifact_digest)
            || !digest(&c.provenance_digest)
            || c.replay_identity != request.replay_identity;
        if hard {
            blocked.insert(c.interpretation_id.clone());
            omissions.insert(format!(
                "{}:interpretation-integrity-or-comparability",
                c.interpretation_id
            ));
        } else if matches!(
            c.evidence_state,
            EvidenceState::Contradicted | EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(c.interpretation_id.clone());
            uncertainty.insert(format!("{}:evidence-state", c.interpretation_id));
        } else {
            qualified.insert(c.interpretation_id.clone());
        }
    }
    for (ok, label) in [
        (request.policy_allowed, "workflow:policy-denied"),
        (
            request.protected_closure,
            "workflow:protected-closure-incomplete",
        ),
        (request.signed_approval, "workflow:signed-approval-missing"),
        (
            request.adversarial_clear,
            "workflow:adversarial-gate-failed",
        ),
    ] {
        if !ok {
            omissions.insert(label.into());
        }
    }
    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.adversarial_clear;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || qualified.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
    }
    if disposition != "qualified" {
        omissions.insert("workflow:interpretation-closure-not-ready".into());
    }
    let panel_order = request.required_panel_order.clone();
    let payload = json!({"candidate_order":candidate_order,"panel_order":panel_order,"qualified_order":qualified,"unresolved_order":unresolved,"blocked_order":blocked,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":request.replay_identity});
    let interpretation_digest = ContentHash::of_value(&payload)
        .map_err(|e| InterpretationVisualizationAssuranceError::Artifact(e.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = InteractiveInterpretation7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        target_scope: request.target_scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        candidate_order: strings("candidate_order"),
        panel_order: strings("panel_order"),
        qualified_order: strings("qualified_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: request.replay_identity.clone(),
        interpretation_digest: interpretation_digest.clone(),
        artifact: InteractiveInterpretationArtifact7 {
            artifact_id: format!("scale-interpretation:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: interpretation_digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["interpretation-not-released".into()]
            },
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("render:interpretation:{}", request.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn assure_interpretation_visualization_json(
    value: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request: EvidenceBackedResult4 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid interpretation assurance request: {e}"))?;
    serde_json::to_value(assure_interpretation_visualization(&request).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_interpretation_visualization_json(
    value: &serde_json::Value,
) -> Result<InteractiveInterpretation7, String> {
    let receipt: InteractiveInterpretation7 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid interpretation assurance receipt: {e}"))?;
    receipt.validate().map_err(|e| e.to_string())?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> EvidenceBackedResult4 {
        EvidenceBackedResult4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "interpret-1".into(),
            consumer: "operator".into(),
            purpose: "render multimodal evidence".into(),
            target_scope: "organoid".into(),
            semantic_profile: "ome-ngff+anndata".into(),
            required_panel_order: vec!["overview".into()],
            minimum_comparability_milli: 800,
            replay_identity: h("replay"),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            aggregate_only: true,
            raw_data_local: true,
            adversarial_clear: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates: vec![InterpretationCandidate4 {
                interpretation_id: "i1".into(),
                target_scope: "organoid".into(),
                semantic_profile: "ome-ngff+anndata".into(),
                artifact_digest: h("artifact"),
                provenance_digest: h("provenance"),
                replay_identity: h("replay"),
                evidence_state: EvidenceState::Supported,
                comparability_milli: 900,
                visualization_ready: true,
                policy_allowed: true,
                local_only: true,
                protected_closure: true,
                negative_result: false,
                omission_order: vec![],
                uncertainty_order: vec![],
            }],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            interpretation_visualization_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_interpretation() {
        assert_eq!(
            assure_interpretation_visualization(&request())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn comparability_blocks() {
        let mut r = request();
        r.candidates[0].comparability_milli = 100;
        assert_eq!(
            assure_interpretation_visualization(&r).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn approval_blocks() {
        let mut r = request();
        r.signed_approval = false;
        assert_eq!(
            assure_interpretation_visualization(&r).unwrap().disposition,
            "blocked"
        );
    }
}
