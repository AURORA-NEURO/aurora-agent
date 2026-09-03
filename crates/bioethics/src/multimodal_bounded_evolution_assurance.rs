//! Multimodal bioethics bounded-evolution assurance (`AFA-bioethics-P32-F26`).
//!
//! The harness verifies caller-supplied capability-evolution attestations across imaging and
//! omics studies. It never mutates implementations, grants authority, exports raw data, or makes
//! a clinical decision; incomplete ethics and provenance closure fail closed.

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

pub const FEATURE_ID: &str = "AFA-bioethics-P32-F26";
pub const CONTRACT_VERSION: &str = "bioethics-multimodal-bounded-evolution-assurance/1.0";
pub const INPUT_SCHEMA: &str = "BioethicsEvolutionCandidate2@1";
pub const OUTPUT_SCHEMA: &str = "BioethicsEvolutionDecision7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-evolution-decision-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsEvolutionCandidate2 {
    pub candidate_id: String,
    pub from_version: String,
    pub to_version: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub benchmark_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub compatible: bool,
    pub benchmark_pass: bool,
    pub privacy_reviewed: bool,
    pub dual_use_reviewed: bool,
    pub human_subject_free: bool,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsEvolutionRequest3 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub current_version: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub candidates: Vec<BioethicsEvolutionCandidate2>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub institutional_authorized: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsEvolutionDecision7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub current_version: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub approved_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub incomparability_order: Vec<String>,
    pub benchmark_failed_order: Vec<String>,
    pub ethics_failed_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub decision_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BioethicsEvolutionError {
    #[error("invalid bioethics evolution request: {0}")]
    Invalid(String),
    #[error("bioethics evolution decision failed: {0}")]
    Decision(String),
    #[error("bioethics evolution artifact failed: {0}")]
    Artifact(String),
}

fn text(value: &str) -> bool {
    !value.trim().is_empty()
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn multimodal_bounded_evolution_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioethics".into(),
        consumers: ["release governance board".into(), "research governance specialist".into(), "verification engineer".into()].into(),
        behavior: "verify multimodal capability-evolution declarations with ethics, compatibility, benchmark, provenance, replay, policy, locality, and protected-closure gates without mutating a release".into(),
        value: "prevents unsafe or ethically incomplete evolution claims from becoming an implementation or release decision".into(),
        inputs: vec![TypedPort { name: "bioethics_evolution_candidate".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "bioethics_evolution_decision".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }],
        authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &BioethicsEvolutionRequest3) -> Result<(), BioethicsEvolutionError> {
    if request.schema_version != INPUT_SCHEMA
        || !text(&request.request_id)
        || !text(&request.consumer)
        || !text(&request.purpose)
        || !text(&request.current_version)
        || !text(&request.semantic_profile)
        || request.required_study_order.is_empty()
        || !ordered(&request.required_study_order)
        || request.required_modality_order.is_empty()
        || !ordered(&request.required_modality_order)
        || !digest(&request.replay_identity)
        || request.candidates.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(BioethicsEvolutionError::Invalid(
            "identity, study/modality closure, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if !text(&candidate.candidate_id)
            || !ids.insert(candidate.candidate_id.clone())
            || !text(&candidate.from_version)
            || !text(&candidate.to_version)
            || !ordered(&candidate.study_order)
            || !ordered(&candidate.modality_order)
            || !text(&candidate.semantic_profile)
            || !digest(&candidate.artifact_digest)
            || !digest(&candidate.benchmark_digest)
            || !digest(&candidate.provenance_digest)
            || candidate.replay_identity != request.replay_identity
            || !candidate.raw_data_local
            || !candidate.aggregate_only
        {
            return Err(BioethicsEvolutionError::Invalid(
                "candidate identity, ordering, digest, replay, or locality is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl BioethicsEvolutionDecision7 {
    pub fn validate(&self) -> Result<(), BioethicsEvolutionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "unresolved" | "blocked" | "unknown"
            )
        {
            return Err(BioethicsEvolutionError::Decision(
                "decision identity, locality, disposition, candidates, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.approved_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.incomparability_order,
            &self.benchmark_failed_order,
            &self.ethics_failed_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(BioethicsEvolutionError::Decision(
                    "decision ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let states = self
            .approved_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.unknown_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.candidate_order.len() || states != ids {
            return Err(BioethicsEvolutionError::Decision(
                "candidate dispositions do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.decision_digest)
            || self.artifact.content_hash != self.decision_digest
        {
            return Err(BioethicsEvolutionError::Decision(
                "decision digest or artifact metadata is invalid".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| BioethicsEvolutionError::Decision(error.to_string()))
    }
}

pub fn assure_multimodal_bounded_evolution(
    request: &BioethicsEvolutionRequest3,
) -> Result<BioethicsEvolutionDecision7, BioethicsEvolutionError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    let candidate_order = candidates
        .iter()
        .map(|c| c.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut approved = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut incomparability = BTreeSet::new();
    let mut benchmark_failed = BTreeSet::new();
    let mut ethics_failed = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for c in &candidates {
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.candidate_id));
        }
        let missing_studies = request
            .required_study_order
            .iter()
            .filter(|id| !c.study_order.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        let missing_modalities = request
            .required_modality_order
            .iter()
            .filter(|id| !c.modality_order.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        let ethics = !c.privacy_reviewed
            || !c.dual_use_reviewed
            || !c.human_subject_free
            || !c.policy_allowed
            || !c.protected_closure
            || !c.raw_data_local
            || !c.aggregate_only
            || c.semantic_profile != request.semantic_profile
            || !digest(&c.provenance_digest);
        if ethics {
            blocked.insert(c.candidate_id.clone());
            ethics_failed.insert(c.candidate_id.clone());
            omissions.insert(format!("{}:ethics-policy-locality-blocked", c.candidate_id));
        } else if c.from_version != request.current_version
            || !c.compatible
            || !missing_studies.is_empty()
            || !missing_modalities.is_empty()
        {
            unresolved.insert(c.candidate_id.clone());
            incomparability.insert(c.candidate_id.clone());
            for id in missing_studies {
                omissions.insert(format!("{}:missing-study:{id}", c.candidate_id));
            }
            for id in missing_modalities {
                omissions.insert(format!("{}:missing-modality:{id}", c.candidate_id));
            }
        } else if !c.benchmark_pass {
            unresolved.insert(c.candidate_id.clone());
            benchmark_failed.insert(c.candidate_id.clone());
            negative.insert(format!("{}:benchmark-failed", c.candidate_id));
        } else if matches!(
            c.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unknown.insert(c.candidate_id.clone());
            uncertainty.insert(format!("{}:evidence-unresolved", c.candidate_id));
        } else if c.evidence_state == EvidenceState::Contradicted {
            unknown.insert(c.candidate_id.clone());
            negative.insert(format!("{}:contradicted-evidence", c.candidate_id));
        } else {
            approved.insert(c.candidate_id.clone());
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.institutional_authorized
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(candidate_order.iter().cloned());
        approved.clear();
        unresolved.clear();
        unknown.clear();
        omissions.insert("request:governance-or-locality-blocked".into());
    }
    let disposition = if global || (!blocked.is_empty() && approved.is_empty()) {
        "blocked"
    } else if !unresolved.is_empty() || !unknown.is_empty() || !blocked.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:evolution-release-not-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"consumer":request.consumer,"purpose":request.purpose,"current_version":request.current_version,"disposition":disposition,"candidate_order":candidate_order,"approved_order":approved,"unresolved_order":unresolved,"blocked_order":blocked,"unknown_order":unknown,"incomparability_order":incomparability,"benchmark_failed_order":benchmark_failed,"ethics_failed_order":ethics_failed,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioethics-evolution-decision:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| BioethicsEvolutionError::Artifact(e.to_string()))?;
    let decision_digest = artifact.content_hash.clone();
    let effect_receipts = if disposition == "qualified" {
        vec![format!("observe:bounded-evolution:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let out = BioethicsEvolutionDecision7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        current_version: request.current_version.clone(),
        disposition: disposition.into(),
        candidate_order: serde_json::from_value(payload["candidate_order"].clone()).unwrap(),
        approved_order: serde_json::from_value(payload["approved_order"].clone()).unwrap(),
        unresolved_order: serde_json::from_value(payload["unresolved_order"].clone()).unwrap(),
        blocked_order: serde_json::from_value(payload["blocked_order"].clone()).unwrap(),
        unknown_order: serde_json::from_value(payload["unknown_order"].clone()).unwrap(),
        incomparability_order: serde_json::from_value(payload["incomparability_order"].clone())
            .unwrap(),
        benchmark_failed_order: serde_json::from_value(payload["benchmark_failed_order"].clone())
            .unwrap(),
        ethics_failed_order: serde_json::from_value(payload["ethics_failed_order"].clone())
            .unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        replay_identity: request.replay_identity.clone(),
        decision_digest,
        artifact,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

pub fn assure_multimodal_bounded_evolution_json(
    v: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let r: BioethicsEvolutionRequest3 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid bioethics evolution request: {e}"))?;
    serde_json::to_value(assure_multimodal_bounded_evolution(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_bioethics_evolution_json(
    v: &serde_json::Value,
) -> Result<BioethicsEvolutionDecision7, String> {
    let o: BioethicsEvolutionDecision7 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid bioethics evolution decision: {e}"))?;
    o.validate().map_err(|e| e.to_string())?;
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> BioethicsEvolutionRequest3 {
        BioethicsEvolutionRequest3 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "evo-1".into(),
            consumer: "release governance board".into(),
            purpose: "review multimodal capability evolution".into(),
            current_version: "v1".into(),
            required_study_order: vec!["study-a".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            semantic_profile: "profile:v1".into(),
            replay_identity: h("replay"),
            candidates: vec![BioethicsEvolutionCandidate2 {
                candidate_id: "c1".into(),
                from_version: "v1".into(),
                to_version: "v2".into(),
                study_order: vec!["study-a".into()],
                modality_order: vec!["imaging".into(), "omics".into()],
                semantic_profile: "profile:v1".into(),
                artifact_digest: h("a"),
                benchmark_digest: h("b"),
                provenance_digest: h("p"),
                replay_identity: h("replay"),
                evidence_state: EvidenceState::Supported,
                compatible: true,
                benchmark_pass: true,
                privacy_reviewed: true,
                dual_use_reviewed: true,
                human_subject_free: true,
                policy_allowed: true,
                protected_closure: true,
                raw_data_local: true,
                aggregate_only: true,
                negative_result: false,
            }],
            policy_allow: true,
            protected_closure: true,
            institutional_authorized: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_bounded_evolution_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified_candidate() {
        assert_eq!(
            assure_multimodal_bounded_evolution(&req())
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn ethics_blocks() {
        let mut r = req();
        r.candidates[0].dual_use_reviewed = false;
        assert_eq!(
            assure_multimodal_bounded_evolution(&r).unwrap().disposition,
            "blocked"
        )
    }
    #[test]
    fn missing_modality_is_incomparable() {
        let mut r = req();
        r.candidates[0].modality_order = vec!["imaging".into()];
        assert!(!assure_multimodal_bounded_evolution(&r)
            .unwrap()
            .incomparability_order
            .is_empty())
    }
    #[test]
    fn unknown_is_retained() {
        let mut r = req();
        r.candidates[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            assure_multimodal_bounded_evolution(&r).unwrap().disposition,
            "unresolved"
        )
    }
}
