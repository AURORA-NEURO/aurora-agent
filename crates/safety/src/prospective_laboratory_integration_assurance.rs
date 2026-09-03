//! Prospective high-throughput laboratory-integration assurance harness (`AFA-safety-P11-F27`).
//!
//! This A1 assurance contract verifies instrument and action attestations into a
//! fail-closed preflight. It never sends commands to hardware; any physical execution
//! requires a separate signed A3 gateway and institutional interlocks.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-safety-P11-F27";
pub const CONTRACT_VERSION: &str =
    "safety-prospective-high-throughput-laboratory-integration-assurance/1.0";
pub const INPUT_SCHEMA: &str = "InstrumentActionRequest3@1";
pub const OUTPUT_SCHEMA: &str = "InstrumentActionReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.instrument-action-receipt-7+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_INSTRUMENTS: usize = 512;
pub const MAX_ACTIONS: usize = 4096;
pub const MAX_PEERS: usize = 512;

/// Public contract aliases used by the cross-language research surfaces.
pub type InstrumentActionRequest3 = LaboratoryIntegrationRequest6;
pub type InstrumentActionReceipt7 = LaboratoryIntegrationReport9;
pub type InstrumentActionAssuranceError = LaboratoryIntegrationError;

pub fn safety_prospective_laboratory_integration_manifest() -> serde_json::Value {
    laboratory_integration_manifest()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaboratoryEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentEndpoint6 {
    pub instrument_id: String,
    pub site_id: String,
    pub protocol_version: String,
    pub calibration_digest: ContentHash,
    pub firmware_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: LaboratoryEvidenceState,
    pub interlock_ready: bool,
    pub emergency_stop_ready: bool,
    pub deterministic: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabAction6 {
    pub action_id: String,
    pub instrument_id: String,
    pub sequence: u32,
    pub effect_class: String,
    pub input_schema: String,
    pub output_schema: String,
    pub estimated_units: u64,
    pub evidence_state: LaboratoryEvidenceState,
    pub action_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed_approval: bool,
    pub deterministic: bool,
    pub local_only: bool,
    pub compensation: String,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaboratoryPeer6 {
    pub peer_id: String,
    pub origin: String,
    pub workflow_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub report_digest: ContentHash,
    pub evidence_state: LaboratoryEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaboratoryIntegrationRequest6 {
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_protocol_version: String,
    pub instruments: Vec<InstrumentEndpoint6>,
    pub actions: Vec<LabAction6>,
    pub peers: Vec<LaboratoryPeer6>,
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
pub struct LaboratoryIntegrationArtifact9 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaboratoryIntegrationReport9 {
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
    pub instrument_order: Vec<String>,
    pub ready_instrument_order: Vec<String>,
    pub unresolved_instrument_order: Vec<String>,
    pub blocked_instrument_order: Vec<String>,
    pub action_order: Vec<String>,
    pub ready_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub interlock_order: Vec<String>,
    pub emergency_stop_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub integration_digest: ContentHash,
    pub artifact: LaboratoryIntegrationArtifact9,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LaboratoryIntegrationError {
    #[error("invalid laboratory integration request: {0}")]
    Invalid(String),
    #[error("laboratory integration artifact failed: {0}")]
    Artifact(String),
}

pub fn laboratory_integration_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"safety","consumers":["release governance board","instrument safety officer","research workflow operator"],"behavior":"verifies prospective high-throughput instrument actions with fail-closed safety and release gates","value":"makes interlocks, emergency stops, compensation, permissions, evidence, replay, provenance, and federation closure auditable before any physical gateway","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["block:unsafe-release"],"permissions":["read:instrument-capability-attestations","verify:instrument-safety"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl LaboratoryIntegrationReport9 {
    pub fn validate(&self) -> Result<(), LaboratoryIntegrationError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.federation_id,
                &self.workflow_id,
                &self.requester,
                &self.purpose,
                &self.semantic_profile,
            ])
            || self.checkpoint == 0
            || self.instrument_order.is_empty()
            || self.action_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(LaboratoryIntegrationError::Invalid("identity, checkpoint, locality, instruments, actions, peers, or effects are incomplete".into()));
        }
        for values in [
            &self.instrument_order,
            &self.ready_instrument_order,
            &self.unresolved_instrument_order,
            &self.blocked_instrument_order,
            &self.action_order,
            &self.ready_action_order,
            &self.unresolved_action_order,
            &self.blocked_action_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.interlock_order,
            &self.emergency_stop_order,
            &self.compensation_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(LaboratoryIntegrationError::Invalid(
                    "laboratory integration ordering is not canonical".into(),
                ));
            }
        }
        let instruments = BTreeSet::from_iter(self.instrument_order.iter().cloned());
        let ip = self
            .ready_instrument_order
            .iter()
            .chain(&self.unresolved_instrument_order)
            .chain(&self.blocked_instrument_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if instruments != ip || instruments.len() != self.instrument_order.len() {
            return Err(LaboratoryIntegrationError::Invalid(
                "instrument states do not partition".into(),
            ));
        }
        let actions = BTreeSet::from_iter(self.action_order.iter().cloned());
        let ap = self
            .ready_action_order
            .iter()
            .chain(&self.unresolved_action_order)
            .chain(&self.blocked_action_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if actions != ap || actions.len() != self.action_order.len() {
            return Err(LaboratoryIntegrationError::Invalid(
                "action states do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let pp = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != pp || peers.len() != self.peer_order.len() {
            return Err(LaboratoryIntegrationError::Invalid(
                "peer states do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.integration_digest
        {
            return Err(LaboratoryIntegrationError::Artifact(
                "artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("verify:instrument-preflight:")
                && !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(LaboratoryIntegrationError::Invalid(
                "effect is outside laboratory integration gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, LaboratoryIntegrationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))?,
        )
        .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))
    }
}
fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

pub fn assure_prospective_laboratory_integration(
    request: &LaboratoryIntegrationRequest6,
) -> Result<LaboratoryIntegrationReport9, LaboratoryIntegrationError> {
    validate_request(request)?;
    let mut instruments = request.instruments.clone();
    instruments.sort_by(|a, b| a.instrument_id.cmp(&b.instrument_id));
    let instrument_order = instruments
        .iter()
        .map(|x| x.instrument_id.clone())
        .collect::<Vec<_>>();
    let mut actions = request.actions.clone();
    actions.sort_by(|a, b| {
        a.sequence
            .cmp(&b.sequence)
            .then(a.action_id.cmp(&b.action_id))
    });
    let action_order = actions
        .iter()
        .map(|x| x.action_id.clone())
        .collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|x| x.peer_id.clone()).collect::<Vec<_>>();
    let instrument_map = instruments
        .iter()
        .map(|i| (i.instrument_id.clone(), i))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut ready_i = BTreeSet::new();
    let mut unresolved_i = BTreeSet::new();
    let mut blocked_i = BTreeSet::new();
    let mut interlocks = BTreeSet::new();
    let mut emergency = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for i in &instruments {
        if i.interlock_ready {
            interlocks.insert(format!("{}:ready", i.instrument_id));
        } else {
            interlocks.insert(format!("{}:missing", i.instrument_id));
        }
        if i.emergency_stop_ready {
            emergency.insert(format!("{}:ready", i.instrument_id));
        } else {
            emergency.insert(format!("{}:missing", i.instrument_id));
        }
        let hard = !i.interlock_ready
            || !i.emergency_stop_ready
            || !i.deterministic
            || !i.local_only
            || !i.permitted
            || !i.signed
            || i.evidence_state == LaboratoryEvidenceState::Contradicted;
        if hard {
            if i.evidence_state == LaboratoryEvidenceState::Contradicted
                || !i.local_only
                || !i.interlock_ready
                || !i.emergency_stop_ready
            {
                blocked_i.insert(i.instrument_id.clone());
            } else {
                unresolved_i.insert(i.instrument_id.clone());
            }
            if i.evidence_state == LaboratoryEvidenceState::Contradicted {
                negative.insert(format!("instrument:{}:contradicted", i.instrument_id));
            }
        } else if !matches!(
            i.evidence_state,
            LaboratoryEvidenceState::Proven | LaboratoryEvidenceState::Supported
        ) {
            unresolved_i.insert(i.instrument_id.clone());
            uncertainty.insert(format!("instrument:{}:evidence-state", i.instrument_id));
        } else {
            ready_i.insert(i.instrument_id.clone());
        }
    }
    let mut ready_a = BTreeSet::new();
    let mut unresolved_a = BTreeSet::new();
    let mut blocked_a = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut total = 0u64;
    for a in &actions {
        total = total.saturating_add(a.estimated_units);
        if a.negative_result {
            negative.insert(format!("action:{}:negative-result", a.action_id));
        }
        if a.compensation.trim().is_empty() {
            omissions.insert(format!("action:{}:missing-compensation", a.action_id));
        }
        let instrument_ok = instrument_map
            .get(&a.instrument_id)
            .map(|i| ready_i.contains(&i.instrument_id))
            .unwrap_or(false);
        let hard = !instrument_ok
            || !a.local_only
            || !a.deterministic
            || !a.signed_approval
            || a.evidence_state == LaboratoryEvidenceState::Contradicted
            || a.replay_identity != request.replay_identity;
        if hard {
            if a.evidence_state == LaboratoryEvidenceState::Contradicted || !instrument_ok {
                blocked_a.insert(a.action_id.clone());
            } else {
                unresolved_a.insert(a.action_id.clone());
            }
            if !a.compensation.trim().is_empty() {
                compensation.insert(format!("{}:{}", a.action_id, a.compensation));
            }
            if a.evidence_state == LaboratoryEvidenceState::Contradicted {
                negative.insert(format!("action:{}:contradicted", a.action_id));
            }
        } else if !matches!(
            a.evidence_state,
            LaboratoryEvidenceState::Proven | LaboratoryEvidenceState::Supported
        ) {
            unresolved_a.insert(a.action_id.clone());
            uncertainty.insert(format!("action:{}:evidence-state", a.action_id));
        } else {
            ready_a.insert(a.action_id.clone());
            if !a.compensation.trim().is_empty() {
                compensation.insert(format!("{}:{}", a.action_id, a.compensation));
            }
        }
    }
    let mut qp = BTreeSet::new();
    let mut mp = BTreeSet::new();
    for p in &peers {
        let ok = p.workflow_id == request.workflow_id
            && p.semantic_profile == request.semantic_profile
            && p.checkpoint == request.checkpoint
            && p.signed
            && p.aggregate_only
            && p.raw_data_local
            && matches!(
                p.evidence_state,
                LaboratoryEvidenceState::Proven | LaboratoryEvidenceState::Supported
            );
        if ok {
            qp.insert(p.peer_id.clone());
        } else {
            mp.insert(p.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", p.peer_id));
        }
    }
    if total > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{}", total));
    }
    if qp.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
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
    let disposition = if global || !blocked_i.is_empty() || !blocked_a.is_empty() {
        "blocked"
    } else if qp.len() < request.minimum_peer_quorum
        || !unresolved_i.is_empty()
        || !unresolved_a.is_empty()
        || !omissions.is_empty()
        || ready_a.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked_i.extend(instrument_order.iter().cloned());
        blocked_a.extend(action_order.iter().cloned());
        ready_i.clear();
        unresolved_i.clear();
        ready_a.clear();
        unresolved_a.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:preflight-not-release-ready".into());
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"workflow_id":request.workflow_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"instrument_order":instrument_order,"ready_instrument_order":ready_i,"unresolved_instrument_order":unresolved_i,"blocked_instrument_order":blocked_i,"action_order":action_order,"ready_action_order":ready_a,"unresolved_action_order":unresolved_a,"blocked_action_order":blocked_a,"peer_order":peer_order,"qualified_peer_order":qp,"missing_peer_order":mp,"interlock_order":interlocks,"emergency_stop_order":emergency,"compensation_order":compensation,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"total_units":total,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))?;
    let artifact = LaboratoryIntegrationArtifact9 {
        artifact_id: format!("laboratory-integration-report-9:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: instruments
            .iter()
            .map(|x| x.provenance_digest.clone())
            .chain(actions.iter().map(|x| x.provenance_digest.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![format!("verify:instrument-preflight:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = LaboratoryIntegrationReport9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        workflow_id: request.workflow_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        instrument_order: payload["instrument_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        ready_instrument_order: payload["ready_instrument_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_instrument_order: payload["unresolved_instrument_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_instrument_order: payload["blocked_instrument_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        action_order: payload["action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        ready_action_order: payload["ready_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_action_order: payload["unresolved_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_action_order: payload["blocked_action_order"]
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
        interlock_order: payload["interlock_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        emergency_stop_order: payload["emergency_stop_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        compensation_order: payload["compensation_order"]
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
        total_units: total,
        replay_identity: request.replay_identity.clone(),
        integration_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(r: &LaboratoryIntegrationRequest6) -> Result<(), LaboratoryIntegrationError> {
    if !all_nonempty([
        &r.request_id,
        &r.federation_id,
        &r.workflow_id,
        &r.requester,
        &r.purpose,
        &r.semantic_profile,
        &r.required_protocol_version,
    ]) || r.instruments.is_empty()
        || r.instruments.len() > MAX_INSTRUMENTS
        || r.actions.is_empty()
        || r.actions.len() > MAX_ACTIONS
        || r.peers.is_empty()
        || r.peers.len() > MAX_PEERS
        || r.checkpoint == 0
        || r.max_budget_units == 0
        || r.minimum_peer_quorum == 0
        || r.replay_identity.as_str().len() != 64
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
    {
        return Err(LaboratoryIntegrationError::Invalid("request identity, bounds, instruments, actions, peers, budget, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for i in &r.instruments {
        if i.instrument_id.trim().is_empty()
            || !ids.insert(i.instrument_id.clone())
            || i.site_id.trim().is_empty()
            || i.protocol_version.trim().is_empty()
            || i.calibration_digest.as_str().len() != 64
            || i.firmware_digest.as_str().len() != 64
            || i.provenance_digest.as_str().len() != 64
        {
            return Err(LaboratoryIntegrationError::Invalid(
                "instrument identity, uniqueness, site, protocol, or digest is invalid".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for a in &r.actions {
        if a.action_id.trim().is_empty()
            || !ids.insert(a.action_id.clone())
            || a.instrument_id.trim().is_empty()
            || a.effect_class.trim().is_empty()
            || a.input_schema.trim().is_empty()
            || a.output_schema.trim().is_empty()
            || a.estimated_units == 0
            || a.action_digest.as_str().len() != 64
            || a.provenance_digest.as_str().len() != 64
            || a.replay_identity.as_str().len() != 64
        {
            return Err(LaboratoryIntegrationError::Invalid(
                "action identity, instrument, schemas, bounds, or digest is invalid".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for p in &r.peers {
        if p.peer_id.trim().is_empty()
            || !ids.insert(p.peer_id.clone())
            || p.origin.trim().is_empty()
            || p.workflow_id.trim().is_empty()
            || p.report_digest.as_str().len() != 64
        {
            return Err(LaboratoryIntegrationError::Invalid(
                "peer identity, uniqueness, origin, workflow, or report digest is invalid".into(),
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
    fn req() -> LaboratoryIntegrationRequest6 {
        let i = InstrumentEndpoint6 {
            instrument_id: "instrument:1".into(),
            site_id: "site:a".into(),
            protocol_version: "1.0".into(),
            calibration_digest: h("cal"),
            firmware_digest: h("fw"),
            provenance_digest: h("ip"),
            evidence_state: LaboratoryEvidenceState::Supported,
            interlock_ready: true,
            emergency_stop_ready: true,
            deterministic: true,
            local_only: true,
            permitted: true,
            signed: true,
        };
        let a = LabAction6 {
            action_id: "action:1".into(),
            instrument_id: "instrument:1".into(),
            sequence: 1,
            effect_class: "simulate".into(),
            input_schema: "Input@1".into(),
            output_schema: "Output@1".into(),
            estimated_units: 5,
            evidence_state: LaboratoryEvidenceState::Supported,
            action_digest: h("a"),
            provenance_digest: h("ap"),
            replay_identity: h("r"),
            signed_approval: true,
            deterministic: true,
            local_only: true,
            compensation: "halt".into(),
            negative_result: false,
        };
        let p = LaboratoryPeer6 {
            peer_id: "peer:a".into(),
            origin: "site:a".into(),
            workflow_id: "workflow:1".into(),
            semantic_profile: "neuro:v1".into(),
            checkpoint: 3,
            report_digest: h("pr"),
            evidence_state: LaboratoryEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        };
        LaboratoryIntegrationRequest6 {
            request_id: "request:lab".into(),
            federation_id: "federation:lab".into(),
            workflow_id: "workflow:1".into(),
            requester: "instrument-safety".into(),
            purpose: "federated-preflight".into(),
            semantic_profile: "neuro:v1".into(),
            required_protocol_version: "1.0".into(),
            instruments: vec![i],
            actions: vec![a],
            peers: vec![p],
            checkpoint: 3,
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
        assert_eq!(laboratory_integration_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_qualifies_without_hardware() {
        let r = assure_prospective_laboratory_integration(&req()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn interlock_blocks() {
        let mut r = req();
        r.instruments[0].interlock_ready = false;
        assert_eq!(
            assure_prospective_laboratory_integration(&r).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut r = req();
        r.actions[0].evidence_state = LaboratoryEvidenceState::Unknown;
        assert_eq!(
            assure_prospective_laboratory_integration(&r).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn budget_is_unresolved() {
        let mut r = req();
        r.max_budget_units = 1;
        assert_eq!(
            assure_prospective_laboratory_integration(&r).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn duplicate_instrument_rejected() {
        let mut r = req();
        r.instruments.push(r.instruments[0].clone());
        assert!(assure_prospective_laboratory_integration(&r).is_err());
    }
}
