//! Prospective high-throughput protocol-simulation research workbench (`AFA-ids-P10-F19`).
//!
//! The workbench evaluates a bounded protocol state machine and declared fault scenarios
//! before a laboratory gateway. It produces only deterministic, content-addressed metadata;
//! it cannot schedule animals, enroll subjects, control instruments, or make clinical decisions.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P10-F19";
pub const CONTRACT_VERSION: &str =
    "ids-prospective-high-throughput-protocol-simulation-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ProtocolWorkbenchRequest5@1";
pub const OUTPUT_SCHEMA: &str = "ProtocolWorkbenchReport9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.protocol-workbench-report-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_STAGES: usize = 512;
pub const MAX_SCENARIOS: usize = 4096;
pub const MAX_PEERS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStage5 {
    pub stage_id: String,
    pub sequence: u32,
    pub input_schema: String,
    pub output_schema: String,
    pub required_capabilities: Vec<String>,
    pub effect_class: String,
    pub estimated_units: u64,
    pub evidence_state: ProtocolEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub deterministic: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScenario5 {
    pub scenario_id: String,
    pub fault_class: String,
    pub affected_stages: Vec<String>,
    pub observed_state: ProtocolEvidenceState,
    pub expected_recovery: String,
    pub budget_units: u64,
    pub replay_digest: ContentHash,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolPeer5 {
    pub peer_id: String,
    pub origin: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub report_digest: ContentHash,
    pub evidence_state: ProtocolEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolWorkbenchRequest5 {
    pub request_id: String,
    pub federation_id: String,
    pub protocol_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_protocol_version: String,
    pub stages: Vec<ProtocolStage5>,
    pub scenarios: Vec<ProtocolScenario5>,
    pub peers: Vec<ProtocolPeer5>,
    pub checkpoint: u64,
    pub batch_size: usize,
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
pub struct ProtocolWorkbenchArtifact9 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolWorkbenchReport9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub protocol_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub qualified_stage_order: Vec<String>,
    pub unresolved_stage_order: Vec<String>,
    pub blocked_stage_order: Vec<String>,
    pub scenario_order: Vec<String>,
    pub passed_scenario_order: Vec<String>,
    pub failed_scenario_order: Vec<String>,
    pub unknown_scenario_order: Vec<String>,
    pub negative_scenario_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub batch_order: Vec<String>,
    pub capacity_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub recovery_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub simulation_digest: ContentHash,
    pub artifact: ProtocolWorkbenchArtifact9,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolWorkbenchError {
    #[error("invalid protocol workbench request: {0}")]
    Invalid(String),
    #[error("protocol workbench artifact failed: {0}")]
    Artifact(String),
}

pub fn protocol_workbench_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["high-throughput protocol scientist","preclinical workbench operator","federation steward"],"behavior":"simulates a bounded prospective protocol state machine across fault scenarios and aggregate peer summaries","value":"exposes capacity, recovery, evidence, and release gates before any laboratory effect","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:permitted-summaries","manage:local-capability"],"permissions":["read:local-protocol-manifests"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl ProtocolWorkbenchReport9 {
    pub fn validate(&self) -> Result<(), ProtocolWorkbenchError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.federation_id,
                &self.protocol_id,
                &self.requester,
                &self.purpose,
                &self.semantic_profile,
            ])
            || self.checkpoint == 0
            || self.stage_order.is_empty()
            || self.scenario_order.is_empty()
            || self.peer_order.is_empty()
            || self.batch_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ProtocolWorkbenchError::Invalid("identity, checkpoint, locality, stages, scenarios, peers, batches, or effects are incomplete".into()));
        }
        for values in [
            &self.stage_order,
            &self.qualified_stage_order,
            &self.unresolved_stage_order,
            &self.blocked_stage_order,
            &self.scenario_order,
            &self.passed_scenario_order,
            &self.failed_scenario_order,
            &self.unknown_scenario_order,
            &self.negative_scenario_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.batch_order,
            &self.capacity_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.recovery_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ProtocolWorkbenchError::Invalid(
                    "protocol workbench ordering is not canonical".into(),
                ));
            }
        }
        let stages = BTreeSet::from_iter(self.stage_order.iter().cloned());
        let stage_parts = self
            .qualified_stage_order
            .iter()
            .chain(&self.unresolved_stage_order)
            .chain(&self.blocked_stage_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if stages != stage_parts || stages.len() != self.stage_order.len() {
            return Err(ProtocolWorkbenchError::Invalid(
                "stage dispositions do not partition stages".into(),
            ));
        }
        let scenarios = BTreeSet::from_iter(self.scenario_order.iter().cloned());
        let scenario_parts = self
            .passed_scenario_order
            .iter()
            .chain(&self.failed_scenario_order)
            .chain(&self.unknown_scenario_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if scenarios != scenario_parts || scenarios.len() != self.scenario_order.len() {
            return Err(ProtocolWorkbenchError::Invalid(
                "scenario dispositions do not partition scenarios".into(),
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
            return Err(ProtocolWorkbenchError::Invalid(
                "peer dispositions do not partition peers".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.simulation_digest
        {
            return Err(ProtocolWorkbenchError::Artifact(
                "artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(ProtocolWorkbenchError::Invalid(
                "effect is outside the protocol gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ProtocolWorkbenchError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ProtocolWorkbenchError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ProtocolWorkbenchError::Artifact(e.to_string()))
    }
}

fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

pub fn simulate_protocol_workbench(
    request: &ProtocolWorkbenchRequest5,
) -> Result<ProtocolWorkbenchReport9, ProtocolWorkbenchError> {
    validate_request(request)?;
    let mut stages = request.stages.clone();
    stages.sort_by(|a, b| {
        a.sequence
            .cmp(&b.sequence)
            .then(a.stage_id.cmp(&b.stage_id))
    });
    let stage_order = stages
        .iter()
        .map(|x| x.stage_id.clone())
        .collect::<Vec<_>>();
    let mut scenarios = request.scenarios.clone();
    scenarios.sort_by(|a, b| a.scenario_id.cmp(&b.scenario_id));
    let scenario_order = scenarios
        .iter()
        .map(|x| x.scenario_id.clone())
        .collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|x| x.peer_id.clone()).collect::<Vec<_>>();
    let batch_order = (0..((stage_order.len() + request.batch_size - 1) / request.batch_size))
        .map(|n| format!("batch:{n:04}"))
        .collect::<Vec<_>>();
    let mut qualified_stage = BTreeSet::new();
    let mut unresolved_stage = BTreeSet::new();
    let mut blocked_stage = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut total_units = 0u64;
    for stage in &stages {
        total_units = total_units.saturating_add(stage.estimated_units);
        let mut reasons = Vec::new();
        if stage.evidence_state == ProtocolEvidenceState::Contradicted {
            reasons.push("contradicted");
            negative_evidence.insert(format!("stage:{}:contradicted", stage.stage_id));
        }
        if !matches!(
            stage.evidence_state,
            ProtocolEvidenceState::Proven | ProtocolEvidenceState::Supported
        ) {
            reasons.push("evidence-unresolved");
            uncertainty.insert(format!("stage:{}:evidence-state", stage.stage_id));
        }
        if !stage.deterministic {
            reasons.push("nondeterministic");
        }
        if !stage.local_only {
            reasons.push("not-local");
        }
        if reasons
            .iter()
            .any(|x| *x == "contradicted" || *x == "not-local")
        {
            blocked_stage.insert(stage.stage_id.clone());
        } else if reasons.is_empty() {
            qualified_stage.insert(stage.stage_id.clone());
        } else {
            unresolved_stage.insert(stage.stage_id.clone());
        }
    }
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut recovery = BTreeSet::new();
    let mut capacity = BTreeSet::new();
    for scenario in &scenarios {
        if scenario.negative_result {
            negative.insert(scenario.scenario_id.clone());
            negative_evidence.insert(format!("scenario:{}:negative-result", scenario.scenario_id));
        }
        if scenario.budget_units > request.max_budget_units {
            failed.insert(scenario.scenario_id.clone());
            capacity.insert(format!("scenario:{}:budget-exceeded", scenario.scenario_id));
            continue;
        }
        if scenario.expected_recovery.trim().is_empty() {
            omissions.insert(format!(
                "scenario:{}:missing-recovery-plan",
                scenario.scenario_id
            ));
        }
        match scenario.observed_state {
            ProtocolEvidenceState::Proven | ProtocolEvidenceState::Supported => {
                if scenario
                    .affected_stages
                    .iter()
                    .all(|id| qualified_stage.contains(id))
                {
                    passed.insert(scenario.scenario_id.clone());
                } else {
                    failed.insert(scenario.scenario_id.clone());
                    recovery.insert(format!("{}:blocked-stage-recovery", scenario.scenario_id));
                }
            }
            ProtocolEvidenceState::Contradicted => {
                failed.insert(scenario.scenario_id.clone());
                negative_evidence.insert(format!("scenario:{}:contradicted", scenario.scenario_id));
            }
            ProtocolEvidenceState::Unknown | ProtocolEvidenceState::Unmeasured => {
                unknown.insert(scenario.scenario_id.clone());
                uncertainty.insert(format!("scenario:{}:evidence-state", scenario.scenario_id));
            }
        }
    }
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        let ok = peer.protocol_id == request.protocol_id
            && peer.semantic_profile == request.semantic_profile
            && peer.checkpoint == request.checkpoint
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                ProtocolEvidenceState::Proven | ProtocolEvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    if total_units > request.max_budget_units {
        capacity.insert(format!("request:total-budget-exceeded:{}", total_units));
    }
    if qualified_peers.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative_evidence.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let disposition = if global || !blocked_stage.is_empty() {
        "blocked"
    } else if qualified_peers.len() < request.minimum_peer_quorum
        || !failed.is_empty()
        || !unknown.is_empty()
        || qualified_stage.is_empty()
        || !capacity.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:simulation-not-release-ready".into());
    }
    if global {
        blocked_stage.extend(stage_order.iter().cloned());
        qualified_stage.clear();
        unresolved_stage.clear();
        passed.clear();
        failed.clear();
        unknown.clear();
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"protocol_id":request.protocol_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"stage_order":stage_order,"qualified_stage_order":qualified_stage,"unresolved_stage_order":unresolved_stage,"blocked_stage_order":blocked_stage,"scenario_order":scenario_order,"passed_scenario_order":passed,"failed_scenario_order":failed,"unknown_scenario_order":unknown,"negative_scenario_order":negative,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"batch_order":batch_order,"capacity_order":capacity,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative_evidence,"recovery_order":recovery,"total_units":total_units,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let simulation_digest = ContentHash::of_value(&payload)
        .map_err(|e| ProtocolWorkbenchError::Artifact(e.to_string()))?;
    let artifact = ProtocolWorkbenchArtifact9 {
        artifact_id: format!("protocol-workbench-report-9:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: simulation_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: stages
            .iter()
            .map(|x| x.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ProtocolWorkbenchReport9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        protocol_id: request.protocol_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        stage_order: payload["stage_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        qualified_stage_order: payload["qualified_stage_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_stage_order: payload["unresolved_stage_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_stage_order: payload["blocked_stage_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        scenario_order: payload["scenario_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        passed_scenario_order: payload["passed_scenario_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        failed_scenario_order: payload["failed_scenario_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unknown_scenario_order: payload["unknown_scenario_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_scenario_order: payload["negative_scenario_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        batch_order: payload["batch_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        capacity_order: payload["capacity_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        recovery_order: payload["recovery_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        total_units,
        replay_identity: request.replay_identity.clone(),
        simulation_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ProtocolWorkbenchRequest5) -> Result<(), ProtocolWorkbenchError> {
    if !all_nonempty([
        &request.request_id,
        &request.federation_id,
        &request.protocol_id,
        &request.requester,
        &request.purpose,
        &request.semantic_profile,
        &request.required_protocol_version,
    ]) || request.checkpoint == 0
        || request.batch_size == 0
        || request.stages.is_empty()
        || request.stages.len() > MAX_STAGES
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.max_budget_units == 0
        || request.minimum_peer_quorum == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(ProtocolWorkbenchError::Invalid("request identity, bounds, stages, scenarios, peers, budget, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for s in &request.stages {
        if s.stage_id.trim().is_empty()
            || !ids.insert(s.stage_id.clone())
            || s.input_schema.trim().is_empty()
            || s.output_schema.trim().is_empty()
            || s.effect_class.trim().is_empty()
            || s.estimated_units == 0
            || s.artifact_digest.as_str().len() != 64
            || s.provenance_digest.as_str().len() != 64
        {
            return Err(ProtocolWorkbenchError::Invalid(
                "stage identity, schemas, bounds, or digests are invalid".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for s in &request.scenarios {
        if s.scenario_id.trim().is_empty()
            || !ids.insert(s.scenario_id.clone())
            || s.fault_class.trim().is_empty()
            || s.replay_digest.as_str().len() != 64
        {
            return Err(ProtocolWorkbenchError::Invalid(
                "scenario identity, fault class, uniqueness, or replay digest is invalid".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for p in &request.peers {
        if p.peer_id.trim().is_empty()
            || !ids.insert(p.peer_id.clone())
            || p.origin.trim().is_empty()
            || p.report_digest.as_str().len() != 64
        {
            return Err(ProtocolWorkbenchError::Invalid(
                "peer identity, uniqueness, origin, or report digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn stage(id: &str, state: ProtocolEvidenceState) -> ProtocolStage5 {
        ProtocolStage5 {
            stage_id: id.into(),
            sequence: 1,
            input_schema: "Input@1".into(),
            output_schema: "Output@1".into(),
            required_capabilities: vec!["simulate".into()],
            effect_class: "local-simulation".into(),
            estimated_units: 5,
            evidence_state: state,
            artifact_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            deterministic: true,
            local_only: true,
        }
    }
    fn request() -> ProtocolWorkbenchRequest5 {
        ProtocolWorkbenchRequest5 {
            request_id: "request:workbench".into(),
            federation_id: "federation:workbench".into(),
            protocol_id: "protocol:1".into(),
            requester: "protocol-scientist".into(),
            purpose: "throughput-preflight".into(),
            semantic_profile: "neuro:v1".into(),
            required_protocol_version: "1.0".into(),
            stages: vec![stage("stage:a", ProtocolEvidenceState::Supported)],
            scenarios: vec![ProtocolScenario5 {
                scenario_id: "scenario:nominal".into(),
                fault_class: "none".into(),
                affected_stages: vec!["stage:a".into()],
                observed_state: ProtocolEvidenceState::Supported,
                expected_recovery: "continue".into(),
                budget_units: 10,
                replay_digest: h("s"),
                negative_result: false,
            }],
            peers: vec![ProtocolPeer5 {
                peer_id: "peer:a".into(),
                origin: "site-a".into(),
                protocol_id: "protocol:1".into(),
                semantic_profile: "neuro:v1".into(),
                checkpoint: 2,
                report_digest: h("peer"),
                evidence_state: ProtocolEvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            batch_size: 1,
            max_budget_units: 20,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: h("r"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(protocol_workbench_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_is_qualified() {
        let r = simulate_protocol_workbench(&request()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.batch_order, vec!["batch:0000"]);
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut r = request();
        r.scenarios[0].observed_state = ProtocolEvidenceState::Unknown;
        assert_eq!(
            simulate_protocol_workbench(&r).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn contradiction_blocks() {
        let mut r = request();
        r.stages[0].evidence_state = ProtocolEvidenceState::Contradicted;
        assert_eq!(
            simulate_protocol_workbench(&r).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn capacity_is_unresolved() {
        let mut r = request();
        r.max_budget_units = 1;
        assert_eq!(
            simulate_protocol_workbench(&r).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn duplicate_stage_rejected() {
        let mut r = request();
        r.stages
            .push(stage("stage:a", ProtocolEvidenceState::Supported));
        assert!(simulate_protocol_workbench(&r).is_err());
    }
}
