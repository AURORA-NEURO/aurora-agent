//! Prospective high-throughput mechanism-exploration assurance.
//! Atlas feature `AFA-atlashub-P08-F27`.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlashub-P08-F27";
pub const CONTRACT_VERSION: &str = "atlashub-prospective-high-throughput-mechanism-exploration-assurance/1.0";
pub const INPUT_SCHEMA: &str = "MechanismCandidateBatch1@1";
pub const OUTPUT_SCHEMA: &str = "MechanismAssuranceReceipt1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub mechanism_id: String,
    pub study_id: String,
    pub support_score: u16,
    pub evidence_state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismExplorationAssuranceRequest {
    pub request_id: String,
    pub consumer: String,
    pub scope: String,
    pub batch_id: String,
    pub baseline_id: String,
    pub algorithm_version: String,
    pub min_support_score: u16,
    pub capacity: u32,
    pub active_jobs: u32,
    pub candidates: Vec<MechanismCandidate>,
    pub required_mechanism_ids: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismExplorationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub scope: String,
    pub batch_id: String,
    pub baseline_id: String,
    pub algorithm_version: String,
    pub capacity: u32,
    pub active_jobs: u32,
    pub verdict: String,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub required_order: Vec<String>,
    pub check_order: Vec<String>,
    pub passed_checks: Vec<String>,
    pub counterexamples: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub assurance_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MechanismExplorationAssuranceError {
    #[error("invalid mechanism assurance request: {0}")]
    Invalid(String),
    #[error("mechanism assurance artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl MechanismExplorationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), MechanismExplorationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.baseline_id.trim().is_empty()
            || self.algorithm_version.trim().is_empty()
            || self.capacity == 0
            || self.active_jobs > self.capacity
            || !matches!(
                self.verdict.as_str(),
                "qualified" | "conditional" | "unknown" | "blocked"
            )
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid("mechanism assurance identity, capacity, candidates, verdict, locality, or effects are incomplete"));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.required_order,
            &self.check_order,
            &self.passed_checks,
            &self.counterexamples,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(Self::invalid(
                    "mechanism assurance ordering is not canonical",
                ));
            }
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
            return Err(Self::invalid(
                "mechanism assurance classifications do not reference candidates",
            ));
        }
        for value in [
            &self.replay_identity,
            &self.assurance_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(Self::invalid("mechanism assurance digest is invalid"));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "assure:atlashub-mechanism-exploration" && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "mechanism assurance effect is outside release gate",
            ));
        }
        if self.verdict != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(Self::invalid(
                "non-qualified mechanism assurance must block release",
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismExplorationAssuranceError::Artifact(error.to_string()))
    }
    fn invalid(message: &str) -> MechanismExplorationAssuranceError {
        MechanismExplorationAssuranceError::Invalid(message.into())
    }
}

pub fn mechanism_exploration_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id:FEATURE_ID.into(), version:CONTRACT_VERSION.into(), owner_crate:"atlashub".into(), consumers:["bioinformatician".into(),"research program lead".into(),"release governance board".into()].into(), behavior:"assures prospective high-throughput mechanism portfolios against baseline, evidence, provenance, comparability, closure, policy, approval, replay, and adversarial safety gates".into(), value:"prevents unsupported mechanistic explanations from entering research release while preserving unknown, contradictory, negative, and omitted evidence".into(), inputs:vec![TypedPort{name:"mechanism_candidate_batch".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs:vec![TypedPort{name:"mechanism_assurance_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}], effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(), permissions:["assure:atlashub-mechanism-exploration".into()].into(), determinism:Determinism::ByteStable, evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())},EvidenceReference{source_id:"cwl".into(),state:EvidenceState::Supported,locator:Some("https://www.commonwl.org/specification/".into())}], authority_requirements:Vec::new(), autonomy_tier:AutonomyTier::A1, surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(), boundary:PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_mechanism_exploration(
    request: &MechanismExplorationAssuranceRequest,
) -> Result<MechanismExplorationAssuranceReceipt, MechanismExplorationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.baseline_id.trim().is_empty()
        || request.algorithm_version.trim().is_empty()
        || request.capacity == 0
        || request.active_jobs > request.capacity
        || request.candidates.is_empty()
        || request
            .required_mechanism_ids
            .windows(2)
            .any(|p| p[0] >= p[1])
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest(&request.replay_identity)
    {
        return Err(MechanismExplorationAssuranceError::Invalid("mechanism assurance identity, batch, capacity, candidates, locality, replay, or boundary is invalid".into()));
    }
    let mut sorted = request.candidates.clone();
    sorted.sort_by(|a, b| {
        b.support_score
            .cmp(&a.support_score)
            .then(a.mechanism_id.cmp(&b.mechanism_id))
    });
    let candidate_order = sorted
        .iter()
        .map(|c| c.mechanism_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = Vec::new();
    let mut unknown = Vec::new();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative = Vec::new();
    for c in &sorted {
        if c.mechanism_id.trim().is_empty()
            || c.study_id.trim().is_empty()
            || !digest(&c.evidence_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.artifact_digest)
            || !digest(&c.comparability_digest)
        {
            blocked.push(c.mechanism_id.clone());
            omissions.push(format!("{}:typed-evidence-or-provenance", c.mechanism_id));
            continue;
        }
        if c.negative_result {
            negative.push(format!("{}:negative-result", c.mechanism_id));
        }
        match c.evidence_state {
            EvidenceState::Supported if c.support_score >= request.min_support_score => {
                admitted.push(c.mechanism_id.clone())
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unknown.push(c.mechanism_id.clone());
                uncertainty.push(format!("{}:evidence-unknown-or-unmeasured", c.mechanism_id));
            }
            EvidenceState::Contradicted => {
                blocked.push(c.mechanism_id.clone());
                omissions.push(format!("{}:contradicted-evidence", c.mechanism_id));
            }
            _ => {
                blocked.push(c.mechanism_id.clone());
                omissions.push(format!("{}:below-support-threshold", c.mechanism_id));
            }
        }
    }
    admitted.sort();
    admitted.dedup();
    blocked.sort();
    blocked.dedup();
    unknown.sort();
    unknown.dedup();
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    negative.sort();
    negative.dedup();
    let missing = request
        .required_mechanism_ids
        .iter()
        .filter(|id| !admitted.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let mut counter = missing
        .iter()
        .map(|id| format!("required mechanism not admitted: {id}"))
        .collect::<Vec<_>>();
    counter.sort();
    let mut checks = vec![
        "candidate identities and canonical ordering".into(),
        "typed evidence, provenance, artifact, and comparability digests".into(),
        "baseline and algorithm binding".into(),
        "replay identity binding".into(),
        "negative and unknown evidence retention".into(),
    ];
    let mut passed = checks.clone();
    let mut verdict = "qualified";
    if !request.policy_allow || !request.protected_closure || !request.signed_approval {
        verdict = "blocked";
        counter.push("policy, protected closure, or signed approval gate denied".into());
    } else if !missing.is_empty() || admitted.is_empty() {
        verdict = if admitted.is_empty() {
            "unknown"
        } else {
            "conditional"
        };
    }
    if request.active_jobs.saturating_mul(100) >= request.capacity.saturating_mul(90) {
        verdict = if verdict == "qualified" {
            "conditional"
        } else {
            verdict
        };
        uncertainty.push("capacity headroom is exhausted".into());
        checks.push("capacity headroom".into());
    }
    if verdict != "qualified" {
        passed.clear();
    }
    checks.sort();
    checks.dedup();
    passed.sort();
    passed.dedup();
    counter.sort();
    counter.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let effect = if verdict == "qualified" {
        "assure:atlashub-mechanism-exploration"
    } else {
        "block:unsafe-release"
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"consumer":request.consumer,"scope":request.scope,"batch_id":request.batch_id,"baseline_id":request.baseline_id,"algorithm_version":request.algorithm_version,"capacity":request.capacity,"active_jobs":request.active_jobs,"verdict":verdict,"candidate_order":candidate_order,"admitted_order":admitted,"blocked_order":blocked,"unknown_order":unknown,"required_order":request.required_mechanism_ids,"check_order":checks,"passed_checks":passed,"counterexamples":counter,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"replay_identity":request.replay_identity,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let assurance_digest = ContentHash::of_value(&payload)
        .map_err(|error| MechanismExplorationAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("atlashub-mechanism-assurance:{}", request.batch_id),
        "application/vnd.aurora.atlashub-mechanism-exploration-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MechanismExplorationAssuranceError::Artifact(error.to_string()))?;
    let receipt = MechanismExplorationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        scope: request.scope.clone(),
        batch_id: request.batch_id.clone(),
        baseline_id: request.baseline_id.clone(),
        algorithm_version: request.algorithm_version.clone(),
        capacity: request.capacity,
        active_jobs: request.active_jobs,
        verdict: verdict.into(),
        candidate_order: candidate_order.clone(),
        admitted_order: admitted.clone(),
        blocked_order: blocked.clone(),
        unknown_order: unknown.clone(),
        required_order: request.required_mechanism_ids.clone(),
        check_order: checks,
        passed_checks: passed,
        counterexamples: counter,
        omissions,
        uncertainty,
        negative_evidence: negative,
        replay_identity: request.replay_identity.clone(),
        assurance_digest,
        effect_receipts: vec![effect.into()],
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            mechanism_exploration_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
}


