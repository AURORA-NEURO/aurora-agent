//! Prospective high-throughput context operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P03-F31`. Queue admission, concurrency, budget,
//! retries, checkpoints, telemetry, and recovery are independently observable.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F31";
pub const CONTRACT_VERSION: &str =
    "brain-throughput-context-compilation-federated-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextControlJob {
    pub job_id: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub ready: bool,
    pub retry_count: u16,
    pub telemetry_digest: Option<ContentHash>,
    pub cost_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextControlRequest {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub jobs: Vec<ThroughputContextControlJob>,
    pub max_concurrency: u16,
    pub max_retries: u16,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub signed_approval: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContextControlDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: ThroughputContextControlDisposition,
    pub candidate_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub degraded_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub exchange_order: Vec<ContentHash>,
    pub checkpoint_seq: u64,
    pub retry_count: u64,
    pub consumed_budget_units: u32,
    pub run_digest: ContentHash,
    pub telemetry_digest: ContentHash,
    pub federation_digest: ContentHash,
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
pub enum ThroughputContextControlError {
    #[error("invalid throughput context control request: {0}")]
    Invalid(String),
    #[error("throughput context control artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputContextControlReceipt {
    pub fn validate(&self) -> Result<(), ThroughputContextControlError> {
        let candidate_count = u64::try_from(self.candidate_order.len()).map_err(|_| {
            ThroughputContextControlError::Invalid(
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
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputContextControlError::Invalid(
                "throughput control identity, checkpoint, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.completed_order,
            &self.degraded_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ThroughputContextControlError::Invalid(
                    "throughput control ordering is not canonical".into(),
                ));
            }
        }
        if self
            .exchange_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ThroughputContextControlError::Invalid(
                "throughput control exchange ordering is not canonical".into(),
            ));
        }
        let classified = self
            .completed_order
            .iter()
            .chain(self.degraded_order.iter())
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified
                .iter()
                .any(|job| !self.candidate_order.contains(job))
        {
            return Err(ThroughputContextControlError::Invalid(
                "throughput control dispositions do not partition jobs".into(),
            ));
        }
        if self.exchange_order.len() != self.completed_order.len() {
            return Err(ThroughputContextControlError::Invalid(
                "throughput control exchange does not match completed jobs".into(),
            ));
        }
        for digest in self.exchange_order.iter().chain([
            &self.run_digest,
            &self.telemetry_digest,
            &self.federation_digest,
            &self.replay_identity,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(ThroughputContextControlError::Invalid(
                    "throughput control digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-throughput-summary:")
                && !effect.starts_with("manage:throughput-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputContextControlError::Invalid(
                "throughput control effect is outside the governed operations gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputContextControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))
    }
}

pub fn throughput_context_compilation_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "platform reliability engineer".into(), "federation administrator".into()].into(), behavior: "operates prospective high-throughput context queues with concurrency admission, checkpoints, retries, telemetry, budgets, recovery dispositions, and permitted summary exchange".into(), value: "prevents silent queue drops and distinguishes execution completion from scientific qualification under throughput pressure".into(), inputs: vec![TypedPort { name: "throughput_context_control_request".into(), schema: "ThroughputContextControlRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_context_control_receipt".into(), schema: "ThroughputContextControlResponse1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["operate:institution-node".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_throughput_context_compilation(
    request: &ThroughputContextControlRequest,
) -> Result<ThroughputContextControlReceipt, ThroughputContextControlError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.jobs.is_empty()
        || request.max_concurrency == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputContextControlError::Invalid("throughput control identity, queue, concurrency, budget, replay, or boundary is invalid".into()));
    }
    let mut jobs = request.jobs.clone();
    jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
    let candidate = jobs
        .iter()
        .map(|job| job.job_id.clone())
        .collect::<Vec<_>>();
    let checkpoint_seq = u64::try_from(candidate.len()).map_err(|_| {
        ThroughputContextControlError::Invalid(
            "throughput candidate count exceeds checkpoint sequence width".into(),
        )
    })?;
    if candidate.windows(2).any(|pair| pair[0] == pair[1])
        || candidate.iter().any(|value| value.trim().is_empty())
    {
        return Err(ThroughputContextControlError::Invalid(
            "throughput job identifiers must be unique and non-empty".into(),
        ));
    }
    let mut job_map = BTreeMap::new();
    for job in &jobs {
        job_map.insert(job.job_id.clone(), job);
    }
    let mut completed = BTreeSet::new();
    let mut degraded = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut exchanges = Vec::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-throughput-control-contract".to_string(),
        "gate:queue-checkpoint".to_string(),
        "gate:concurrency-window".to_string(),
        "gate:bounded-retry".to_string(),
        "gate:telemetry".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:permitted-summary".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.signed_approval;
    let mut consumed = 0u32;
    let mut retries = 0u64;
    for (index, job_id) in candidate.iter().enumerate() {
        let job = job_map[job_id];
        retries = retries.saturating_add(u64::from(job.retry_count));
        if !global_open || !job.raw_data_local || job.boundary != PRECLINICAL_BOUNDARY {
            denied.insert(job_id.clone());
            counterexamples.insert(format!(
                "counterexample:{}:policy-approval-locality",
                job_id
            ));
        } else if index >= usize::from(request.max_concurrency) {
            unresolved.insert(job_id.clone());
            uncertainty.insert(format!("job:{}:concurrency-window", job_id));
        } else if job.retry_count > request.max_retries {
            degraded.insert(job_id.clone());
            omissions.insert(format!("job:{}:retry-budget-exhausted", job_id));
        } else if consumed.saturating_add(job.cost_units) > request.budget_units {
            denied.insert(job_id.clone());
            omissions.insert(format!("job:{}:resource-budget-exhausted", job_id));
        } else if !job.ready {
            unresolved.insert(job_id.clone());
            uncertainty.insert(format!("job:{}:not-ready", job_id));
        } else if job.replay_identity != request.replay_identity {
            unresolved.insert(job_id.clone());
            uncertainty.insert(format!("job:{}:replay-mismatch", job_id));
        } else if job.telemetry_digest.is_none() {
            unresolved.insert(job_id.clone());
            omissions.insert(format!("job:{}:telemetry-missing", job_id));
        } else if job.evidence_digest.is_none() || job.provenance_digest.is_none() {
            unresolved.insert(job_id.clone());
            omissions.insert(format!("job:{}:evidence-or-provenance-missing", job_id));
        } else if matches!(
            job.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(job_id.clone());
            uncertainty.insert(format!("job:{}:evidence-uncertain", job_id));
        } else if matches!(job.state, EvidenceState::Contradicted) {
            denied.insert(job_id.clone());
            negative.insert(format!("job:{}:contradicted", job_id));
        } else {
            completed.insert(job_id.clone());
            consumed = consumed.saturating_add(job.cost_units);
            exchanges.push(ContentHash::of_value(&json!({"job_id": job.job_id, "context_digest": job.context_digest, "section_digest": job.section_digest, "evidence_digest": job.evidence_digest, "provenance_digest": job.provenance_digest, "telemetry_digest": job.telemetry_digest})).map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))?);
        }
    }
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        counterexamples.insert("counterexample:signed-approval-missing".into());
        omissions.insert("control:signed-approval-missing".into());
    }
    if !unresolved.is_empty() || !degraded.is_empty() {
        witnesses.insert("gate:degraded-or-unresolved-retained".into());
    }
    exchanges.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let disposition = if !global_open || !denied.is_empty() {
        ThroughputContextControlDisposition::Denied
    } else if !unresolved.is_empty() {
        ThroughputContextControlDisposition::Unresolved
    } else if !degraded.is_empty() {
        ThroughputContextControlDisposition::Degraded
    } else {
        ThroughputContextControlDisposition::Completed
    };
    let telemetry = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "batch_id": request.batch_id, "candidate_order": candidate, "retry_count": retries, "exchange_order": exchanges})).map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))?;
    let raw_data_local = true;
    let federation = ContentHash::of_value(&json!({"partition": request.partition, "batch_id": request.batch_id, "exchange_order": exchanges, "raw_data_local": raw_data_local, "replay_identity": request.replay_identity})).map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))?;
    let run = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "disposition": disposition, "completed_order": completed, "degraded_order": degraded, "unresolved_order": unresolved, "denied_order": denied, "checkpoint_seq": checkpoint_seq, "consumed_budget_units": consumed, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": request.replay_identity})).map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "disposition": disposition, "candidate_order": candidate, "completed_order": completed, "degraded_order": degraded, "unresolved_order": unresolved, "denied_order": denied, "exchange_order": exchanges, "checkpoint_seq": checkpoint_seq, "retry_count": retries, "consumed_budget_units": consumed, "run_digest": run, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": request.replay_identity, "witness_order": witnesses, "counterexample_order": counterexamples, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-context-compilation-federated-control-plane:{}",
            request.request_id
        ),
        "application/vnd.aurora.throughput-context-control+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputContextControlError::Artifact(error.to_string()))?;
    let receipt = ThroughputContextControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        disposition,
        candidate_order: candidate.clone(),
        completed_order: completed.into_iter().collect(),
        degraded_order: degraded.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        exchange_order: exchanges,
        checkpoint_seq,
        retry_count: retries,
        consumed_budget_units: consumed,
        run_digest: run,
        telemetry_digest: telemetry,
        federation_digest: federation,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(disposition, ThroughputContextControlDisposition::Completed) {
            vec![
                format!(
                    "exchange:permitted-throughput-summary:{}",
                    request.request_id
                ),
                format!("manage:throughput-context:{}", request.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ThroughputContextControlRequest {
        let replay = hash("throughput-control");
        let job = |id: &str| ThroughputContextControlJob {
            job_id: id.into(),
            context_digest: replay.clone(),
            section_digest: replay.clone(),
            evidence_digest: Some(replay.clone()),
            provenance_digest: Some(replay.clone()),
            replay_identity: replay.clone(),
            state: EvidenceState::Supported,
            ready: true,
            retry_count: 0,
            telemetry_digest: Some(replay.clone()),
            cost_units: 1,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        ThroughputContextControlRequest {
            request_id: "request:throughput-control".into(),
            batch_id: "batch:alpha".into(),
            partition: "partition:zero".into(),
            jobs: vec![job("job:a"), job("job:b")],
            max_concurrency: 2,
            max_retries: 2,
            budget_units: 2,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            signed_approval: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            throughput_context_compilation_federated_control_plane_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn complete_is_completed() {
        assert_eq!(
            operate_throughput_context_compilation(&request())
                .unwrap()
                .disposition,
            ThroughputContextControlDisposition::Completed
        );
    }
    #[test]
    fn concurrency_is_unresolved() {
        let mut value = request();
        value.max_concurrency = 1;
        assert_eq!(
            operate_throughput_context_compilation(&value)
                .unwrap()
                .disposition,
            ThroughputContextControlDisposition::Unresolved
        );
    }
    #[test]
    fn retry_is_degraded() {
        let mut value = request();
        value.jobs[0].retry_count = 3;
        assert_eq!(
            operate_throughput_context_compilation(&value)
                .unwrap()
                .disposition,
            ThroughputContextControlDisposition::Degraded
        );
    }
    #[test]
    fn budget_is_denied() {
        let mut value = request();
        value.budget_units = 1;
        assert_eq!(
            operate_throughput_context_compilation(&value)
                .unwrap()
                .disposition,
            ThroughputContextControlDisposition::Denied
        );
    }
    #[test]
    fn policy_is_denied() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            operate_throughput_context_compilation(&value)
                .unwrap()
                .disposition,
            ThroughputContextControlDisposition::Denied
        );
    }
    #[test]
    fn non_local_input_returns_denied_metadata_receipt() {
        let mut value = request();
        value.raw_data_local = false;
        let receipt = operate_throughput_context_compilation(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputContextControlDisposition::Denied
        );
        assert!(receipt.raw_data_local);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = operate_throughput_context_compilation(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
