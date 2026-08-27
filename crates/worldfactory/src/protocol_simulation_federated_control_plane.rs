//! Federated continual protocol-simulation control plane for `AFA-worldfactory-P10-F32`.
//!
//! The control plane operates a declared protocol state machine against caller-supplied
//! deterministic scenario summaries. It never dispatches laboratory actions: the only possible
//! effects are retaining a local simulation artifact and exchanging an aggregate digest after
//! policy, replay, quorum, provenance, and locality gates close.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldfactory-P10-F32";
pub const CONTRACT_VERSION: &str =
    "worldfactory-federated-continual-protocol-simulation-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ProtocolDraft4@1";
pub const OUTPUT_SCHEMA: &str = "ProtocolSimulationReport8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.protocol-simulation-report-8+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_STAGES: usize = 512;
pub const MAX_SCENARIOS: usize = 4096;
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
pub struct ProtocolStage4 {
    pub stage_id: String,
    pub sequence: u32,
    pub input_schema: String,
    pub output_schema: String,
    pub required_capabilities: Vec<String>,
    pub effect_class: String,
    pub estimated_units: u64,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub deterministic: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScenario4 {
    pub scenario_id: String,
    pub fault_class: String,
    pub affected_stages: Vec<String>,
    pub observed_state: EvidenceState,
    pub expected_recovery: String,
    pub budget_units: u64,
    pub replay_digest: ContentHash,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerProtocolSummary4 {
    pub peer_id: String,
    pub origin: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub report_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDraft4 {
    pub request_id: String,
    pub federation_id: String,
    pub protocol_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_protocol_version: String,
    pub stages: Vec<ProtocolStage4>,
    pub scenarios: Vec<ProtocolScenario4>,
    pub peers: Vec<PeerProtocolSummary4>,
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
pub struct ProtocolSimulationReport8 {
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
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub recovery_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub simulation_digest: ContentHash,
    pub artifact: ProtocolSimulationArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSimulationArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolSimulationError {
    #[error("invalid protocol simulation request: {0}")]
    Invalid(String),
    #[error("protocol simulation artifact failed: {0}")]
    Artifact(String),
}

pub fn protocol_simulation_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "worldfactory",
        "consumers": ["preclinical neuroscientist", "protocol operator", "federation steward"],
        "behavior": "simulates a declared protocol state machine across bounded fault scenarios and peer summaries",
        "value": "makes protocol robustness, recovery, and federation gates auditable before any laboratory effect",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["manage:local-capability", "exchange:permitted-summaries"],
        "permissions": ["operate:institution-node"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl ProtocolSimulationReport8 {
    pub fn validate(&self) -> Result<(), ProtocolSimulationError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.protocol_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.stage_order.is_empty()
            || self.scenario_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ProtocolSimulationError::Invalid("identity, checkpoint, locality, stages, scenarios, peers, or effects are incomplete".into()));
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
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.recovery_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|window| window[0] >= window[1]) {
                return Err(ProtocolSimulationError::Invalid(
                    "protocol simulation ordering is not canonical".into(),
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
            return Err(ProtocolSimulationError::Invalid(
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
            return Err(ProtocolSimulationError::Invalid(
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
            return Err(ProtocolSimulationError::Invalid(
                "peer dispositions do not partition peers".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.simulation_digest
        {
            return Err(ProtocolSimulationError::Artifact(
                "artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("manage:local-capability:")
                && !effect.starts_with("exchange:permitted-summaries:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ProtocolSimulationError::Invalid(
                "effect is outside the simulation gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ProtocolSimulationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ProtocolSimulationError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ProtocolSimulationError::Artifact(error.to_string()))
    }
}

pub fn simulate_protocol(
    draft: &ProtocolDraft4,
) -> Result<ProtocolSimulationReport8, ProtocolSimulationError> {
    validate_draft(draft)?;
    let mut stages = draft.stages.clone();
    stages.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.stage_id.cmp(&right.stage_id))
    });
    let stage_order = stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<Vec<_>>();
    let mut peers = draft.peers.clone();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    let peer_order = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for peer in &peers {
        let qualified = peer.protocol_id == draft.protocol_id
            && peer.semantic_profile == draft.semantic_profile
            && peer.checkpoint == draft.checkpoint
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
            uncertainty.insert(format!("peer:{}:contradicted", peer.peer_id));
        }
    }
    let mut qualified_stage = BTreeSet::new();
    let mut unresolved_stage = BTreeSet::new();
    let mut blocked_stage = BTreeSet::new();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut recovery = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut total_units = 0u64;
    for stage in &stages {
        total_units = total_units.saturating_add(stage.estimated_units);
        let mut reasons = Vec::new();
        if stage.evidence_state == EvidenceState::Contradicted {
            reasons.push("contradicted-evidence");
            negative_evidence.insert(format!("stage:{}:contradicted", stage.stage_id));
        }
        if !matches!(
            stage.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            reasons.push("evidence-state-unresolved");
            uncertainty.insert(format!("stage:{}:evidence-state", stage.stage_id));
        }
        if !stage.deterministic {
            reasons.push("nondeterministic-stage");
        }
        if !stage.local_only {
            reasons.push("stage-not-local");
        }
        if reasons
            .iter()
            .any(|reason| *reason == "contradicted-evidence" || *reason == "stage-not-local")
        {
            blocked_stage.insert(stage.stage_id.clone());
        } else if reasons.is_empty() {
            qualified_stage.insert(stage.stage_id.clone());
        } else {
            unresolved_stage.insert(stage.stage_id.clone());
        }
    }
    let mut scenarios = draft.scenarios.clone();
    scenarios.sort_by(|left, right| left.scenario_id.cmp(&right.scenario_id));
    let scenario_order = scenarios
        .iter()
        .map(|scenario| scenario.scenario_id.clone())
        .collect::<Vec<_>>();
    for scenario in &scenarios {
        if scenario.negative_result {
            negative.insert(scenario.scenario_id.clone());
            negative_evidence.insert(format!("scenario:{}:negative-result", scenario.scenario_id));
        }
        if scenario.budget_units > draft.max_budget_units {
            failed.insert(scenario.scenario_id.clone());
            omissions.insert(format!("scenario:{}:budget-exceeded", scenario.scenario_id));
            continue;
        }
        match scenario.observed_state {
            EvidenceState::Proven | EvidenceState::Supported => {
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
            EvidenceState::Contradicted => {
                failed.insert(scenario.scenario_id.clone());
                negative_evidence.insert(format!("scenario:{}:contradicted", scenario.scenario_id));
            }
            EvidenceState::Unknown | EvidenceState::Unmeasured => {
                unknown.insert(scenario.scenario_id.clone());
                uncertainty.insert(format!("scenario:{}:evidence-state", scenario.scenario_id));
            }
        }
        if scenario.expected_recovery.trim().is_empty() {
            omissions.insert(format!(
                "scenario:{}:missing-recovery-plan",
                scenario.scenario_id
            ));
        }
    }
    if qualified_peers.len() < draft.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    let global_block = !draft.policy_allow
        || !draft.protected_closure
        || !draft.signed_approval
        || !draft.federation_approved
        || !draft.raw_data_local
        || !draft.aggregate_only;
    if !draft.policy_allow {
        negative_evidence.insert("request:policy-denied".into());
    }
    if !draft.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !draft.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !draft.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let disposition = if global_block || !blocked_stage.is_empty() {
        "blocked"
    } else if qualified_peers.len() < draft.minimum_peer_quorum
        || !failed.is_empty()
        || !unknown.is_empty()
        || qualified_stage.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:simulation-not-release-ready".into());
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":draft.request_id,"federation_id":draft.federation_id,"protocol_id":draft.protocol_id,"requester":draft.requester,"purpose":draft.purpose,"semantic_profile":draft.semantic_profile,"checkpoint":draft.checkpoint,"disposition":disposition,"stage_order":stage_order,"qualified_stage_order":qualified_stage,"unresolved_stage_order":unresolved_stage,"blocked_stage_order":blocked_stage,"scenario_order":scenario_order,"passed_scenario_order":passed,"failed_scenario_order":failed,"unknown_scenario_order":unknown,"negative_scenario_order":negative,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative_evidence,"recovery_order":recovery,"total_units":total_units,"replay_identity":draft.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let simulation_digest = ContentHash::of_value(&payload)
        .map_err(|error| ProtocolSimulationError::Artifact(error.to_string()))?;
    let artifact = ProtocolSimulationArtifact8 {
        artifact_id: format!("protocol-simulation-report-8:{}", draft.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: simulation_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: stages
            .iter()
            .map(|stage| stage.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", draft.request_id),
            format!("manage:local-capability:{}", draft.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ProtocolSimulationReport8 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: draft.request_id.clone(),
        federation_id: draft.federation_id.clone(),
        protocol_id: draft.protocol_id.clone(),
        requester: draft.requester.clone(),
        purpose: draft.purpose.clone(),
        semantic_profile: draft.semantic_profile.clone(),
        checkpoint: draft.checkpoint,
        disposition: disposition.into(),
        stage_order,
        qualified_stage_order: qualified_stage.into_iter().collect(),
        unresolved_stage_order: unresolved_stage.into_iter().collect(),
        blocked_stage_order: blocked_stage.into_iter().collect(),
        scenario_order,
        passed_scenario_order: passed.into_iter().collect(),
        failed_scenario_order: failed.into_iter().collect(),
        unknown_scenario_order: unknown.into_iter().collect(),
        negative_scenario_order: negative.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative_evidence.into_iter().collect(),
        recovery_order: recovery.into_iter().collect(),
        total_units,
        replay_identity: draft.replay_identity.clone(),
        simulation_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: draft.raw_data_local,
        aggregate_only: draft.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_draft(draft: &ProtocolDraft4) -> Result<(), ProtocolSimulationError> {
    if ![
        &draft.request_id,
        &draft.federation_id,
        &draft.protocol_id,
        &draft.requester,
        &draft.purpose,
        &draft.semantic_profile,
        &draft.required_protocol_version,
    ]
    .iter()
    .all(|value| !value.trim().is_empty())
        || draft.checkpoint == 0
        || draft.stages.is_empty()
        || draft.stages.len() > MAX_STAGES
        || draft.scenarios.is_empty()
        || draft.scenarios.len() > MAX_SCENARIOS
        || draft.peers.is_empty()
        || draft.peers.len() > MAX_PEERS
        || draft.max_budget_units == 0
        || draft.minimum_peer_quorum == 0
        || draft.boundary != PRECLINICAL_BOUNDARY
        || !draft.raw_data_local
        || !draft.aggregate_only
    {
        return Err(ProtocolSimulationError::Invalid("request identity, bounds, stages, scenarios, peers, budget, locality, or boundary is invalid".into()));
    }
    let mut stage_ids = BTreeSet::new();
    for stage in &draft.stages {
        if stage.stage_id.trim().is_empty()
            || !stage_ids.insert(stage.stage_id.clone())
            || stage.input_schema.trim().is_empty()
            || stage.output_schema.trim().is_empty()
            || stage.effect_class.trim().is_empty()
            || stage.estimated_units == 0
            || stage.artifact_digest.as_str().len() != 64
            || stage.provenance_digest.as_str().len() != 64
        {
            return Err(ProtocolSimulationError::Invalid(
                "stage identity, schemas, bounds, or digests are invalid".into(),
            ));
        }
    }
    let mut scenario_ids = BTreeSet::new();
    for scenario in &draft.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !scenario_ids.insert(scenario.scenario_id.clone())
            || scenario.replay_digest.as_str().len() != 64
        {
            return Err(ProtocolSimulationError::Invalid(
                "scenario identity, uniqueness, or replay digest is invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &draft.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.origin.trim().is_empty()
            || peer.report_digest.as_str().len() != 64
        {
            return Err(ProtocolSimulationError::Invalid(
                "peer identity, uniqueness, origin, or report digest is invalid".into(),
            ));
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
    fn stage(id: &str, state: EvidenceState) -> ProtocolStage4 {
        ProtocolStage4 {
            stage_id: id.into(),
            sequence: 1,
            input_schema: "Input1@1".into(),
            output_schema: "Output1@1".into(),
            required_capabilities: vec!["simulate".into()],
            effect_class: "local-simulation".into(),
            estimated_units: 5,
            evidence_state: state,
            artifact_digest: hash(id),
            provenance_digest: hash(&format!("p:{id}")),
            deterministic: true,
            local_only: true,
        }
    }
    fn peer(id: &str, state: EvidenceState) -> PeerProtocolSummary4 {
        PeerProtocolSummary4 {
            peer_id: id.into(),
            origin: id.into(),
            protocol_id: "protocol:1".into(),
            semantic_profile: "neuro:v1".into(),
            checkpoint: 3,
            report_digest: hash(id),
            evidence_state: state,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    fn draft() -> ProtocolDraft4 {
        ProtocolDraft4 {
            request_id: "request:protocol".into(),
            federation_id: "federation:protocol".into(),
            protocol_id: "protocol:1".into(),
            requester: "preclinical-neuroscientist".into(),
            purpose: "protocol-simulation".into(),
            semantic_profile: "neuro:v1".into(),
            required_protocol_version: "1.0".into(),
            stages: vec![stage("stage:a", EvidenceState::Supported)],
            scenarios: vec![ProtocolScenario4 {
                scenario_id: "scenario:nominal".into(),
                fault_class: "none".into(),
                affected_stages: vec!["stage:a".into()],
                observed_state: EvidenceState::Supported,
                expected_recovery: "continue".into(),
                budget_units: 10,
                replay_digest: hash("scenario"),
                negative_result: false,
            }],
            peers: vec![peer("peer:a", EvidenceState::Supported)],
            checkpoint: 3,
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
    fn manifest_is_a2_and_no_hardware_effect() {
        let m = protocol_simulation_manifest();
        assert_eq!(m["autonomy_tier"], "A2");
        assert_eq!(m["effects"][0], "manage:local-capability");
    }
    #[test]
    fn nominal_simulation_qualifies_and_is_deterministic() {
        let r = simulate_protocol(&draft()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(
            r.effect_receipts,
            vec![
                "exchange:permitted-summaries:request:protocol",
                "manage:local-capability:request:protocol"
            ]
        );
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_scenario_is_unresolved() {
        let mut d = draft();
        d.scenarios[0].observed_state = EvidenceState::Unknown;
        let r = simulate_protocol(&d).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(r.uncertainty_order.iter().any(|v| v.contains("scenario")));
    }
    #[test]
    fn contradicted_stage_blocks() {
        let mut d = draft();
        d.stages[0].evidence_state = EvidenceState::Contradicted;
        let r = simulate_protocol(&d).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(r
            .negative_evidence_order
            .iter()
            .any(|v| v.contains("contradicted")));
    }
    #[test]
    fn quorum_and_policy_are_fail_closed() {
        let mut d = draft();
        d.minimum_peer_quorum = 2;
        let r = simulate_protocol(&d).unwrap();
        assert_eq!(r.disposition, "unresolved");
        let mut d = draft();
        d.policy_allow = false;
        let r = simulate_protocol(&d).unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn duplicate_stage_is_rejected() {
        let mut d = draft();
        d.stages.push(stage("stage:a", EvidenceState::Supported));
        assert!(simulate_protocol(&d).is_err());
    }
}
