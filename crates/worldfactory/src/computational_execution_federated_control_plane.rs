//! Federated continual computational-execution control plane for `AFA-worldfactory-P12-F32`.
//!
//! This contract admits a declared computational workflow for institution-local execution.
//! It evaluates task closure, replay/provenance identity, peer attestations, budgets, and
//! policy gates, but never starts a process, sends data, or controls an instrument. A qualified
//! result is an auditable authorization receipt that a separately governed runtime may consume.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldfactory-P12-F32";
pub const CONTRACT_VERSION: &str =
    "worldfactory-federated-continual-computational-execution-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ComputationalExecutionPlan4@1";
pub const OUTPUT_SCHEMA: &str = "ComputationalExecutionRun9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.computational-execution-run-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_TASKS: usize = 4096;
pub const MAX_PEERS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTask5 {
    pub task_id: String,
    pub sequence: u32,
    pub input_schema: String,
    pub output_schema: String,
    pub required_capabilities: Vec<String>,
    pub effect_class: String,
    pub estimated_units: u64,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_digest: ContentHash,
    pub deterministic: bool,
    pub local_only: bool,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPeerSummary5 {
    pub peer_id: String,
    pub origin: String,
    pub workflow_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub run_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationalExecutionPlan4 {
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_runtime_version: String,
    pub tasks: Vec<ExecutionTask5>,
    pub peers: Vec<ExecutionPeerSummary5>,
    pub checkpoint: u64,
    pub max_budget_units: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationalExecutionArtifact9 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationalExecutionRun9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub task_order: Vec<String>,
    pub admitted_task_order: Vec<String>,
    pub unresolved_task_order: Vec<String>,
    pub blocked_task_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub execution_digest: ContentHash,
    pub artifact: ComputationalExecutionArtifact9,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationalExecutionError {
    #[error("invalid computational execution request: {0}")]
    Invalid(String),
    #[error("computational execution artifact failed: {0}")]
    Artifact(String),
}

pub fn computational_execution_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "worldfactory",
        "consumers": ["computational researcher", "workflow operator", "federation steward", "runtime executor"],
        "behavior": "qualifies a declared computational workflow for bounded institution-local execution",
        "value": "turns execution readiness, provenance, replay, quorum, and policy into an auditable authorization receipt",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["authorize:local-computation", "exchange:permitted-execution-summaries"],
        "permissions": ["operate:institution-node", "authorize:research-computation"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl ComputationalExecutionRun9 {
    pub fn validate(&self) -> Result<(), ComputationalExecutionError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.task_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ComputationalExecutionError::Invalid(
                "execution identity, checkpoint, locality, tasks, peers, disposition, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.task_order,
            &self.admitted_task_order,
            &self.unresolved_task_order,
            &self.blocked_task_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ComputationalExecutionError::Invalid(
                    "execution ordering is not canonical".into(),
                ));
            }
        }
        let tasks = BTreeSet::from_iter(self.task_order.iter().cloned());
        let task_parts = self
            .admitted_task_order
            .iter()
            .chain(&self.unresolved_task_order)
            .chain(&self.blocked_task_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if tasks != task_parts || tasks.len() != self.task_order.len() {
            return Err(ComputationalExecutionError::Invalid(
                "task dispositions do not partition tasks".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != peer_parts || peers.len() != self.peer_order.len() {
            return Err(ComputationalExecutionError::Invalid(
                "peer dispositions do not partition peers".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.execution_digest
        {
            return Err(ComputationalExecutionError::Artifact(
                "execution artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("authorize:local-computation:")
                && !effect.starts_with("exchange:permitted-execution-summaries:")
                && effect != "block:unsafe-execution"
        }) {
            return Err(ComputationalExecutionError::Invalid(
                "execution effect is outside the governed gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ComputationalExecutionError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ComputationalExecutionError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ComputationalExecutionError::Artifact(error.to_string()))
    }
}

pub fn authorize_computational_execution(
    plan: &ComputationalExecutionPlan4,
) -> Result<ComputationalExecutionRun9, ComputationalExecutionError> {
    validate_plan(plan)?;
    let mut tasks = plan.tasks.clone();
    tasks.sort_by(|a, b| a.sequence.cmp(&b.sequence).then(a.task_id.cmp(&b.task_id)));
    let task_order = tasks.iter().map(|t| t.task_id.clone()).collect::<Vec<_>>();
    let mut admitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut adversarial = BTreeSet::new();
    let mut total_units = 0u64;
    for task in &tasks {
        total_units = total_units.saturating_add(task.estimated_units);
        let mut reasons = Vec::new();
        if task.evidence_state == EvidenceState::Contradicted {
            reasons.push("contradicted-evidence");
            negative.insert(format!("task:{}:contradicted", task.task_id));
        }
        if !matches!(
            task.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            reasons.push("evidence-state-unresolved");
            uncertainty.insert(format!("task:{}:evidence-state", task.task_id));
        }
        if !task.deterministic {
            reasons.push("nondeterministic-task");
            adversarial.insert(format!("task:{}:nondeterministic", task.task_id));
        }
        if !task.local_only {
            reasons.push("task-not-local");
        }
        if task.requires_approval && !plan.signed_approval {
            reasons.push("task-approval-missing");
            uncertainty.insert(format!("task:{}:approval-missing", task.task_id));
        }
        if task.required_capabilities.is_empty() {
            omissions.insert(format!("task:{}:capability-closure-missing", task.task_id));
            reasons.push("capability-closure-missing");
        }
        if reasons
            .iter()
            .any(|r| matches!(*r, "contradicted-evidence" | "task-not-local"))
        {
            blocked.insert(task.task_id.clone());
        } else if reasons.is_empty() {
            admitted.insert(task.task_id.clone());
        } else {
            unresolved.insert(task.task_id.clone());
        }
    }
    let mut peers = plan.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        let qualified = peer.workflow_id == plan.workflow_id
            && peer.semantic_profile == plan.semantic_profile
            && peer.checkpoint == plan.checkpoint
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            );
        if qualified {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
        if peer.evidence_state == EvidenceState::Contradicted {
            negative.insert(format!("peer:{}:contradicted", peer.peer_id));
        }
    }
    let global_block = !plan.policy_allow
        || !plan.protected_closure
        || !plan.signed_approval
        || !plan.federation_approved
        || !plan.raw_data_local
        || !plan.aggregate_only;
    if !plan.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !plan.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !plan.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !plan.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    if total_units > plan.max_budget_units {
        omissions.insert("request:budget-exceeded".into());
    }
    if qualified_peers.len() < plan.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    let disposition = if global_block || !blocked.is_empty() || total_units > plan.max_budget_units
    {
        "blocked"
    } else if qualified_peers.len() < plan.minimum_peer_quorum
        || !unresolved.is_empty()
        || admitted.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:execution-not-release-ready".into());
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":plan.request_id,"federation_id":plan.federation_id,"workflow_id":plan.workflow_id,"requester":plan.requester,"purpose":plan.purpose,"semantic_profile":plan.semantic_profile,"checkpoint":plan.checkpoint,"disposition":disposition,"task_order":task_order,"admitted_task_order":admitted,"unresolved_task_order":unresolved,"blocked_task_order":blocked,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"adversarial_event_order":adversarial,"total_units":total_units,"replay_identity":plan.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let execution_digest = ContentHash::of_value(&payload)
        .map_err(|error| ComputationalExecutionError::Artifact(error.to_string()))?;
    let artifact = ComputationalExecutionArtifact9 {
        artifact_id: format!("computational-execution-run-9:{}", plan.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: execution_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: tasks
            .iter()
            .map(|t| t.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effect_receipts = if disposition == "qualified" {
        vec![
            format!("authorize:local-computation:{}", plan.request_id),
            format!("exchange:permitted-execution-summaries:{}", plan.request_id),
        ]
    } else {
        vec!["block:unsafe-execution".into()]
    };
    let receipt = ComputationalExecutionRun9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: plan.request_id.clone(),
        federation_id: plan.federation_id.clone(),
        workflow_id: plan.workflow_id.clone(),
        requester: plan.requester.clone(),
        purpose: plan.purpose.clone(),
        semantic_profile: plan.semantic_profile.clone(),
        checkpoint: plan.checkpoint,
        disposition: disposition.into(),
        task_order,
        admitted_task_order: admitted.into_iter().collect(),
        unresolved_task_order: unresolved.into_iter().collect(),
        blocked_task_order: blocked.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        adversarial_event_order: adversarial.into_iter().collect(),
        total_units,
        replay_identity: plan.replay_identity.clone(),
        execution_digest,
        artifact,
        effect_receipts,
        raw_data_local: plan.raw_data_local,
        aggregate_only: plan.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_plan(plan: &ComputationalExecutionPlan4) -> Result<(), ComputationalExecutionError> {
    if ![
        &plan.request_id,
        &plan.federation_id,
        &plan.workflow_id,
        &plan.requester,
        &plan.purpose,
        &plan.semantic_profile,
        &plan.required_runtime_version,
    ]
    .iter()
    .all(|v| !v.trim().is_empty())
        || plan.checkpoint == 0
        || plan.tasks.is_empty()
        || plan.tasks.len() > MAX_TASKS
        || plan.peers.is_empty()
        || plan.peers.len() > MAX_PEERS
        || plan.max_budget_units == 0
        || plan.minimum_peer_quorum == 0
        || plan.boundary != PRECLINICAL_BOUNDARY
        || !plan.raw_data_local
        || !plan.aggregate_only
        || plan.replay_identity.as_str().len() != 64
    {
        return Err(ComputationalExecutionError::Invalid("execution request identity, bounds, tasks, peers, budget, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for task in &plan.tasks {
        if task.task_id.trim().is_empty()
            || !ids.insert(task.task_id.clone())
            || task.input_schema.trim().is_empty()
            || task.output_schema.trim().is_empty()
            || task.effect_class.trim().is_empty()
            || task.estimated_units == 0
            || task.artifact_digest.as_str().len() != 64
            || task.provenance_digest.as_str().len() != 64
            || task.replay_digest.as_str().len() != 64
        {
            return Err(ComputationalExecutionError::Invalid(
                "task identity, schemas, bounds, or digests are invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &plan.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.origin.trim().is_empty()
            || peer.workflow_id.trim().is_empty()
            || peer.run_digest.as_str().len() != 64
        {
            return Err(ComputationalExecutionError::Invalid(
                "peer identity, uniqueness, origin, workflow, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn task(id: &str, state: EvidenceState) -> ExecutionTask5 {
        ExecutionTask5 {
            task_id: id.into(),
            sequence: 1,
            input_schema: "Input@1".into(),
            output_schema: "Output@1".into(),
            required_capabilities: vec!["python".into()],
            effect_class: "local-compute".into(),
            estimated_units: 5,
            evidence_state: state,
            artifact_digest: hash(id),
            provenance_digest: hash(&format!("p:{id}")),
            replay_digest: hash(&format!("r:{id}")),
            deterministic: true,
            local_only: true,
            requires_approval: false,
        }
    }
    fn plan() -> ComputationalExecutionPlan4 {
        ComputationalExecutionPlan4 {
            request_id: "request:execution".into(),
            federation_id: "federation:execution".into(),
            workflow_id: "workflow:1".into(),
            requester: "researcher".into(),
            purpose: "multimodal-compute".into(),
            semantic_profile: "neuro:v1".into(),
            required_runtime_version: "runtime:1".into(),
            tasks: vec![task("task:a", EvidenceState::Supported)],
            peers: vec![ExecutionPeerSummary5 {
                peer_id: "peer:a".into(),
                origin: "site:a".into(),
                workflow_id: "workflow:1".into(),
                semantic_profile: "neuro:v1".into(),
                checkpoint: 2,
                run_digest: hash("run"),
                evidence_state: EvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            max_budget_units: 20,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash("replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(computational_execution_manifest()["autonomy_tier"], "A2");
    }
    #[test]
    fn nominal_authorization_is_deterministic() {
        let r = authorize_computational_execution(&plan()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(
            r.effect_receipts[0],
            "authorize:local-computation:request:execution"
        );
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_are_retained() {
        let mut p = plan();
        p.tasks[0].evidence_state = EvidenceState::Unknown;
        let r = authorize_computational_execution(&p).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.uncertainty_order.is_empty());
        let mut p = plan();
        p.tasks[0].evidence_state = EvidenceState::Contradicted;
        let r = authorize_computational_execution(&p).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(!r.negative_evidence_order.is_empty());
    }
    #[test]
    fn policy_and_duplicate_task_fail_closed() {
        let mut p = plan();
        p.policy_allow = false;
        assert_eq!(
            authorize_computational_execution(&p).unwrap().disposition,
            "blocked"
        );
        let mut p = plan();
        p.tasks.push(task("task:a", EvidenceState::Supported));
        assert!(authorize_computational_execution(&p).is_err());
    }
}
