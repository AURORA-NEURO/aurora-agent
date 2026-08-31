//! Multimodal context-compilation assurance harness (`AFA-bioethics-P03-F26`).
//!
//! The harness verifies caller-supplied preclinical context facts across independent studies and
//! modalities. It records ethical, provenance, scope, replay, omission, and uncertainty gates;
//! it never ingests human-subject data, gives clinical advice, or claims a complete context when
//! protected closure is incomplete.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P03-F26";
pub const CONTRACT_VERSION: &str = "bioethics-multimodal-context-compilation-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery2@1";
pub const OUTPUT_SCHEMA: &str = "CertifiedDecisionSection7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-certified-decision-section-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact4 {
    pub fact_id: String,
    pub study_id: String,
    pub modality: String,
    pub scope: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub source_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub permitted: bool,
    pub local_only: bool,
    pub privacy_reviewed: bool,
    pub dual_use_reviewed: bool,
    pub representation_reviewed: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery2 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub institutional_authorized: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub facts: Vec<ContextFact4>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSectionArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub fact_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub ethical_gate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub context_digest: ContentHash,
    pub artifact: CertifiedDecisionSectionArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextCompilationAssuranceError {
    #[error("invalid multimodal context compilation request or receipt: {0}")]
    Invalid(String),
    #[error("context compilation artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn multimodal_context_compilation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioethics".into(), consumers: ["consortium operator".into(), "context compiler".into(), "ethics steward".into()].into(), behavior: "assure multimodal preclinical context compilation with ethical, evidence, scope, provenance, replay, omission, and locality gates".into(), value: "prevents incomplete or ethically unreviewed multi-study context from being presented as a certified decision section".into(), inputs: vec![TypedPort { name: "decision_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "certified_decision_section".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl CertifiedDecisionSection7 {
    pub fn validate(&self) -> Result<(), ContextCompilationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.fact_order.is_empty()
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
            return Err(ContextCompilationAssuranceError::Invalid(
                "context identity, locality, facts, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.fact_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.ethical_gate_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ContextCompilationAssuranceError::Invalid(
                    "context ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.fact_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.fact_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context fact states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.context_digest)
            || self.artifact.content_hash != self.context_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(ContextCompilationAssuranceError::Artifact(
                "context digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("observe:context:"))
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "context effect is outside assurance gate".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("observe:context:{}", self.request_id)]
        {
            return Err(ContextCompilationAssuranceError::Invalid(
                "qualified context effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(ContextCompilationAssuranceError::Invalid(
                "non-qualified context must block".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_multimodal_context_compilation(
    request: &DecisionQuery2,
) -> Result<CertifiedDecisionSection7, ContextCompilationAssuranceError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.target_scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.facts.is_empty()
        || !ordered(&request.required_study_order)
        || !ordered(&request.required_modality_order)
        || !digest(&request.replay_identity)
        || !request.aggregate_only
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextCompilationAssuranceError::Invalid(
            "context query identity, requirements, replay, locality, or boundary is invalid".into(),
        ));
    }
    let fact_order = request
        .facts
        .iter()
        .map(|f| f.fact_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if fact_order.len() != request.facts.len() || fact_order.iter().any(|id| id.trim().is_empty()) {
        return Err(ContextCompilationAssuranceError::Invalid(
            "fact ids must be unique and non-empty".into(),
        ));
    }
    let required_studies = request
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed_studies = request
        .facts
        .iter()
        .map(|f| f.study_id.clone())
        .collect::<BTreeSet<_>>();
    let observed_modalities = request
        .facts
        .iter()
        .map(|f| f.modality.clone())
        .collect::<BTreeSet<_>>();
    let missing_studies = required_studies
        .difference(&observed_studies)
        .cloned()
        .collect::<Vec<_>>();
    let missing_modalities = required_modalities
        .difference(&observed_modalities)
        .cloned()
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut ethical = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for fact in &request.facts {
        if fact.negative_result {
            negative.insert(fact.fact_id.clone());
        }
        let hard = !fact.permitted
            || !fact.local_only
            || !fact.privacy_reviewed
            || !fact.dual_use_reviewed
            || !fact.representation_reviewed
            || fact.scope != request.target_scope
            || fact.semantic_profile != request.semantic_profile
            || !digest(&fact.source_digest)
            || !digest(&fact.provenance_digest)
            || fact.replay_identity != request.replay_identity
            || !ordered(&fact.omission_order);
        if !fact.privacy_reviewed {
            ethical.insert(format!("{}:privacy-review-missing", fact.fact_id));
        }
        if !fact.dual_use_reviewed {
            ethical.insert(format!("{}:dual-use-review-missing", fact.fact_id));
        }
        if !fact.representation_reviewed {
            ethical.insert(format!("{}:representation-review-missing", fact.fact_id));
        }
        if !fact.omission_order.is_empty() {
            omissions.extend(
                fact.omission_order
                    .iter()
                    .map(|o| format!("{}:{}", fact.fact_id, o)),
            );
        }
        if hard {
            blocked.insert(fact.fact_id.clone());
            omissions.insert(format!("{}:ethical-or-integrity-gate", fact.fact_id));
        } else if matches!(
            fact.evidence_state,
            EvidenceState::Contradicted | EvidenceState::Unknown
        ) {
            unresolved.insert(fact.fact_id.clone());
            uncertainty.insert(format!("{}:evidence-state", fact.fact_id));
        } else {
            selected.insert(fact.fact_id.clone());
        }
    }
    omissions.extend(
        missing_studies
            .iter()
            .map(|s| format!("study-missing:{}", s)),
    );
    omissions.extend(
        missing_modalities
            .iter()
            .map(|m| format!("modality-missing:{}", m)),
    );
    for (flag, label) in [
        (request.policy_allow, "workflow:policy-denied"),
        (
            request.protected_closure,
            "workflow:protected-closure-incomplete",
        ),
        (
            request.institutional_authorized,
            "workflow:institutional-authorization-missing",
        ),
    ] {
        if !flag {
            ethical.insert(label.into());
        }
    }
    let global_block =
        !request.policy_allow || !request.protected_closure || !request.institutional_authorized;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if !missing_studies.is_empty()
        || !missing_modalities.is_empty()
        || !unresolved.is_empty()
    {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(fact_order.iter().cloned());
        selected.clear();
        unresolved.clear();
    }
    let checkpoint = ContentHash::of_value(&json!({"request_id":request.request_id,"target_scope":request.target_scope,"semantic_profile":request.semantic_profile,"replay_identity":request.replay_identity})).map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))?;
    let payload = json!({"fact_order":fact_order,"selected_order":selected,"unresolved_order":unresolved,"blocked_order":blocked,"missing_study_order":missing_studies,"missing_modality_order":missing_modalities,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"ethical_gate_order":ethical,"replay_identity":request.replay_identity,"checkpoint":checkpoint});
    let context_digest = ContentHash::of_value(&payload)
        .map_err(|e| ContextCompilationAssuranceError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = CertifiedDecisionSection7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        target_scope: request.target_scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        fact_order: strings("fact_order"),
        selected_order: strings("selected_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        missing_study_order: strings("missing_study_order"),
        missing_modality_order: strings("missing_modality_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        ethical_gate_order: strings("ethical_gate_order"),
        replay_identity: request.replay_identity.clone(),
        context_digest: context_digest.clone(),
        artifact: CertifiedDecisionSectionArtifact7 {
            artifact_id: format!("bioethics-context:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: context_digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["context-closure-incomplete".into()]
            },
            provenance_digests: request
                .facts
                .iter()
                .map(|f| f.provenance_digest.clone())
                .collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("observe:context:{}", request.request_id)]
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

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> DecisionQuery2 {
        DecisionQuery2 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "ctx-1".into(),
            consumer: "consortium".into(),
            purpose: "multimodal context".into(),
            target_scope: "organoid".into(),
            semantic_profile: "ctx:v1".into(),
            required_study_order: vec!["s1".into()],
            required_modality_order: vec!["imaging".into()],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            institutional_authorized: true,
            aggregate_only: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            facts: vec![ContextFact4 {
                fact_id: "f1".into(),
                study_id: "s1".into(),
                modality: "imaging".into(),
                scope: "organoid".into(),
                semantic_profile: "ctx:v1".into(),
                evidence_state: EvidenceState::Supported,
                source_digest: h("source"),
                provenance_digest: h("prov"),
                replay_identity: h("replay"),
                permitted: true,
                local_only: true,
                privacy_reviewed: true,
                dual_use_reviewed: true,
                representation_reviewed: true,
                negative_result: false,
                omission_order: vec![],
            }],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_context_compilation_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified_context() {
        assert_eq!(
            assure_multimodal_context_compilation(&request())
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn missing_modality_partial() {
        let mut r = request();
        r.required_modality_order.push("omics".into());
        let q = assure_multimodal_context_compilation(&r).unwrap();
        assert_eq!(q.disposition, "partial");
        assert!(q.missing_modality_order.iter().any(|v| v == "omics"))
    }
    #[test]
    fn privacy_blocks() {
        let mut r = request();
        r.facts[0].privacy_reviewed = false;
        assert_eq!(
            assure_multimodal_context_compilation(&r)
                .unwrap()
                .disposition,
            "blocked"
        )
    }
}
