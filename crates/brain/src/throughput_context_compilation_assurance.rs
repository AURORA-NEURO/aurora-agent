//! Prospective high-throughput context-compilation verification and safety harness.
//!
//! Atlas feature: `AFA-brain-P03-F27`. Queue, checkpoint, concurrency, budget,
//! replay, and evidence gates are explicit product behavior.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F27";
pub const CONTRACT_VERSION: &str = "brain-throughput-context-compilation-assurance/1.0";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextAssuranceJob {
    pub job_id: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub ready: bool,
    pub cost_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextAssuranceRequest {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub jobs: Vec<ThroughputContextAssuranceJob>,
    pub max_concurrency: u16,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContextAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub verdict: ThroughputContextAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub verification_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputContextAssuranceError {
    #[error("invalid throughput context assurance request: {0}")]
    Invalid(String),
    #[error("throughput context assurance artifact failed: {0}")]
    Artifact(String),
}
impl ThroughputContextAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ThroughputContextAssuranceError> {
        let candidate_count = u64::try_from(self.candidate_order.len()).map_err(|_| {
            ThroughputContextAssuranceError::Invalid(
                "throughput candidate count exceeds checkpoint sequence width".into(),
            )
        })?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.checkpoint_seq != candidate_count
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputContextAssuranceError::Invalid("throughput assurance identity, queue, witnesses, locality, or effects are incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(ThroughputContextAssuranceError::Invalid(
                    "throughput assurance ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified.iter().any(|v| !self.candidate_order.contains(v))
        {
            return Err(ThroughputContextAssuranceError::Invalid(
                "throughput assurance outcomes do not partition candidates".into(),
            ));
        }
        for d in [
            &self.queue_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if d.as_str().len() != 64 {
                return Err(ThroughputContextAssuranceError::Invalid(
                    "throughput assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("assurance:local-throughput-context:") && e != "block:unsafe-release"
        }) {
            return Err(ThroughputContextAssuranceError::Invalid(
                "throughput assurance effect is outside the local release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| ThroughputContextAssuranceError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputContextAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|e| ThroughputContextAssuranceError::Artifact(e.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|e| ThroughputContextAssuranceError::Artifact(e.to_string()))
    }
}
pub fn throughput_context_compilation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"brain".into(),consumers:["laboratory automation engineer".into(),"batch release gate".into(),"workflow operator".into()].into(),behavior:"verifies bounded high-throughput context queues with readiness, replay, provenance, concurrency, budget, and fail-closed evidence predicates".into(),value:"prevents silent queue drops or unsafe promotion of incomplete prospective context batches".into(),inputs:vec![TypedPort{name:"throughput_context_assurance_request".into(),schema:"ThroughputContextAssuranceRequest1@1".into(),required:true}],outputs:vec![TypedPort{name:"throughput_context_assurance_receipt".into(),schema:"ThroughputContextAssuranceResponse1@1".into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["evaluate:throughput-context-compilation".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"slsa-provenance-1.2".into(),state:EvidenceState::Supported,locator:Some("https://slsa.dev/spec/v1.2/provenance".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn assure_throughput_context_compilation(
    request: &ThroughputContextAssuranceRequest,
) -> Result<ThroughputContextAssuranceReceipt, ThroughputContextAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.jobs.is_empty()
        || request.max_concurrency == 0
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ThroughputContextAssuranceError::Invalid(
            "throughput assurance identity, queue, budget, replay, or boundary is invalid".into(),
        ));
    }
    let mut jobs = request.jobs.clone();
    jobs.sort_by(|a, b| a.job_id.cmp(&b.job_id));
    let candidate = jobs.iter().map(|j| j.job_id.clone()).collect::<Vec<_>>();
    let checkpoint_seq = u64::try_from(candidate.len()).map_err(|_| {
        ThroughputContextAssuranceError::Invalid(
            "throughput candidate count exceeds checkpoint sequence width".into(),
        )
    })?;
    if candidate.windows(2).any(|p| p[0] == p[1]) || candidate.iter().any(|v| v.trim().is_empty()) {
        return Err(ThroughputContextAssuranceError::Invalid(
            "throughput job identifiers must be unique and non-empty".into(),
        ));
    }
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-throughput-contract".to_string(),
        "gate:queue-checkpoint".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:concurrency-window".to_string(),
        "gate:budget".to_string(),
        "gate:locality".to_string(),
    ]);
    let mut counter = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let open = request.policy_allow && request.protected_closure && request.raw_data_local;
    let mut consumed = 0u32;
    for job in &jobs {
        if !open || !job.raw_data_local || job.boundary != PRECLINICAL_BOUNDARY {
            blocked.insert(job.job_id.clone());
            counter.insert(format!(
                "counterexample:{}:policy-protected-closure-locality",
                job.job_id
            ));
        } else if job.replay_identity != request.replay_identity {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:replay-mismatch", job.job_id));
        } else if !job.ready {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:not-ready", job.job_id));
        } else if job.evidence_digest.is_none() || job.provenance_digest.is_none() {
            unknown.insert(job.job_id.clone());
            omissions.insert(format!("job:{}:evidence-or-provenance-missing", job.job_id));
        } else if matches!(
            job.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:evidence-uncertain", job.job_id));
        } else if matches!(job.state, EvidenceState::Contradicted) {
            blocked.insert(job.job_id.clone());
            negative.insert(format!("job:{}:contradicted", job.job_id));
        } else if qualified.len() >= usize::from(request.max_concurrency) {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:concurrency-window", job.job_id));
        } else if consumed.saturating_add(job.cost_units) > request.budget_units {
            blocked.insert(job.job_id.clone());
            omissions.insert(format!("job:{}:budget-exhausted", job.job_id));
        } else {
            qualified.insert(job.job_id.clone());
            consumed = consumed.saturating_add(job.cost_units);
        }
    }
    if !request.policy_allow {
        counter.insert("counterexample:policy-denied".into());
        omissions.insert("assurance:policy-denied".into());
    }
    if !request.protected_closure {
        counter.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("assurance:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        counter.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    if !unknown.is_empty() {
        witnesses.insert("gate:unresolved-batch-retained".into());
    }
    let verdict = if !open || !blocked.is_empty() {
        ThroughputContextAssuranceVerdict::Blocked
    } else if !unknown.is_empty() {
        ThroughputContextAssuranceVerdict::Unresolved
    } else {
        ThroughputContextAssuranceVerdict::Qualified
    };
    let queue=ContentHash::of_value(&json!({"candidate_order":candidate,"qualified_order":qualified,"blocked_order":blocked,"unknown_order":unknown,"max_concurrency":request.max_concurrency,"budget_units":request.budget_units,"consumed":consumed,"replay_identity":request.replay_identity})).map_err(|e|ThroughputContextAssuranceError::Artifact(e.to_string()))?;
    let verification=ContentHash::of_value(&json!({"feature_id":FEATURE_ID,"request_id":request.request_id,"batch_id":request.batch_id,"queue_digest":queue,"witness_order":witnesses,"counterexample_order":counter,"verdict":verdict,"replay_identity":request.replay_identity})).map_err(|e|ThroughputContextAssuranceError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"batch_id":request.batch_id,"partition":request.partition,"verdict":verdict,"candidate_order":candidate,"qualified_order":qualified,"blocked_order":blocked,"unknown_order":unknown,"checkpoint_seq":checkpoint_seq,"queue_digest":queue,"verification_digest":verification,"replay_identity":request.replay_identity,"witness_order":witnesses,"counterexample_order":counter,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-context-compilation-assurance:{}",
            request.request_id
        ),
        "application/vnd.aurora.throughput-context-compilation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ThroughputContextAssuranceError::Artifact(e.to_string()))?;
    let receipt = ThroughputContextAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        verdict,
        candidate_order: candidate,
        qualified_order: qualified.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        checkpoint_seq,
        queue_digest: queue,
        verification_digest: verification,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counter.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(verdict, ThroughputContextAssuranceVerdict::Qualified) {
            vec![format!(
                "assurance:local-throughput-context:{}",
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
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request(state: EvidenceState) -> ThroughputContextAssuranceRequest {
        let r = h("throughput-assurance");
        let jobs = vec!["job:a", "job:b"]
            .into_iter()
            .map(|id| ThroughputContextAssuranceJob {
                job_id: id.into(),
                context_digest: r.clone(),
                section_digest: r.clone(),
                evidence_digest: Some(r.clone()),
                provenance_digest: Some(r.clone()),
                replay_identity: r.clone(),
                state: state.clone(),
                ready: true,
                cost_units: 1,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            })
            .collect();
        ThroughputContextAssuranceRequest {
            request_id: "request:throughput-assurance".into(),
            batch_id: "batch:one".into(),
            partition: "partition:zero".into(),
            jobs,
            max_concurrency: 2,
            budget_units: 2,
            replay_identity: r,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_context_compilation_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn complete_is_qualified() {
        assert_eq!(
            assure_throughput_context_compilation(&request(EvidenceState::Supported))
                .unwrap()
                .verdict,
            ThroughputContextAssuranceVerdict::Qualified
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        assert_eq!(
            assure_throughput_context_compilation(&request(EvidenceState::Unknown))
                .unwrap()
                .verdict,
            ThroughputContextAssuranceVerdict::Unresolved
        )
    }
    #[test]
    fn concurrency_is_unresolved() {
        let mut x = request(EvidenceState::Supported);
        x.max_concurrency = 1;
        assert_eq!(
            assure_throughput_context_compilation(&x).unwrap().verdict,
            ThroughputContextAssuranceVerdict::Unresolved
        )
    }
    #[test]
    fn budget_is_blocked() {
        let mut x = request(EvidenceState::Supported);
        x.budget_units = 1;
        assert_eq!(
            assure_throughput_context_compilation(&x).unwrap().verdict,
            ThroughputContextAssuranceVerdict::Blocked
        )
    }
    #[test]
    fn not_ready_is_unresolved() {
        let mut x = request(EvidenceState::Supported);
        x.jobs[0].ready = false;
        assert_eq!(
            assure_throughput_context_compilation(&x).unwrap().verdict,
            ThroughputContextAssuranceVerdict::Unresolved
        )
    }
    #[test]
    fn non_local_input_returns_blocked_metadata_receipt() {
        let mut value = request(EvidenceState::Supported);
        value.raw_data_local = false;
        let receipt = assure_throughput_context_compilation(&value).unwrap();
        assert_eq!(receipt.verdict, ThroughputContextAssuranceVerdict::Blocked);
        assert!(receipt.raw_data_local);
    }
    #[test]
    fn digest_is_stable() {
        let r = assure_throughput_context_compilation(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap())
    }
}
