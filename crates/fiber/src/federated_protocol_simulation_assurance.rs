//! Federated continual protocol-simulation assurance for FIBER.
//!
//! Atlas feature: `AFA-fiber-P10-F28`.
//!
//! The module verifies caller-supplied protocol state-machine summaries. It never starts a
//! protocol runner, instrument, external provider, or clinical workflow; raw study data remains
//! local and only an evidence-bearing release verdict may cross the federation boundary.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P10-F28";
pub const CONTRACT_VERSION: &str = "fiber-federated-continual-protocol-simulation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ProtocolDraft4@1";
pub const OUTPUT_SCHEMA: &str = "ProtocolSimulationReport7@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerProtocolSummary {
    pub institution_id: String,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub semantic_profile: String,
    pub observed_step_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub signed_approval: bool,
    pub permitted_artifact: String,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDraft {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub protocol_schema: String,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub required_step_order: Vec<String>,
    pub observed_step_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub provenance_digest: Option<ContentHash>,
    pub peer_institution_order: Vec<String>,
    pub required_peer_quorum: u32,
    pub peers: Vec<PeerProtocolSummary>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSimulationReport {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub protocol_schema: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub required_step_order: Vec<String>,
    pub observed_step_order: Vec<String>,
    pub missing_step_order: Vec<String>,
    pub violation_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub unresolved_peer_order: Vec<String>,
    pub blocked_peer_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub peer_envelope_digest: ContentHash,
    pub verdict_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolSimulationAssuranceError {
    #[error("invalid federated protocol-simulation request: {0}")]
    Invalid(String),
    #[error("federated protocol-simulation artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ProtocolSimulationReport {
    pub fn validate(&self) -> Result<(), ProtocolSimulationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.protocol_schema != INPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_step_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "identity, schema, locality, steps, peers, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.required_step_order,
            &self.observed_step_order,
            &self.missing_step_order,
            &self.violation_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.unresolved_peer_order,
            &self.blocked_peer_order,
            &self.adversarial_event_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(ProtocolSimulationAssuranceError::Invalid(
                    "protocol orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let required = self
            .required_step_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if required.len() != self.required_step_order.len()
            || self
                .observed_step_order
                .iter()
                .any(|step| !required.contains(step))
            || self
                .missing_step_order
                .iter()
                .any(|step| !required.contains(step))
            || self
                .observed_step_order
                .iter()
                .chain(self.missing_step_order.iter())
                .cloned()
                .collect::<BTreeSet<_>>()
                != required
        {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "step closure is not a disjoint required-step partition".into(),
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_partition = self
            .qualified_peer_order
            .iter()
            .chain(self.unresolved_peer_order.iter())
            .chain(self.blocked_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_partition.len() != peers.len()
            || peer_partition.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "peer disposition partition is incomplete".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:fiber-protocol-simulation:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "effect is outside protocol verification gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ProtocolSimulationAssuranceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "fiber".into(), consumers: BTreeSet::from(["downstream AURORA crate maintainer".into(), "federation evidence operator".into(), "protocol simulator reviewer".into()]), behavior: "verifies federated protocol state-machine summaries with peer quorum and emits replayable release evidence without executing protocols".into(), value: "prevents incomplete, adversarial, non-replayable, or semantically incomparable protocol evidence from appearing qualified across institutions".into(), inputs: vec![TypedPort { name: "protocol_draft".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "protocol_simulation_report".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]), permissions: BTreeSet::from(["evaluate:capability-runs".into(), "exchange:permitted-research-artifacts".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }, EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federation-protocol-reviewer".into(), reason: "approve permitted protocol evidence exchange".into() }], autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure(
    request: &ProtocolDraft,
) -> Result<ProtocolSimulationReport, ProtocolSimulationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.protocol_schema != INPUT_SCHEMA
        || request.semantic_profile.trim().is_empty()
        || request.required_step_order.is_empty()
        || request.peer_institution_order.is_empty()
        || request.peers.is_empty()
        || request.required_peer_quorum == 0
        || request.required_peer_quorum as usize > request.peer_institution_order.len()
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ProtocolSimulationAssuranceError::Invalid(
            "identity, schema, steps, peers, quorum, budget, locality, or boundary is invalid"
                .into(),
        ));
    }
    for values in [
        &request.required_step_order,
        &request.peer_institution_order,
    ] {
        if !canonical(values) || values.iter().any(|value| value.trim().is_empty()) {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "required protocol and peer orders must be unique and canonical".into(),
            ));
        }
    }
    let required = request
        .required_step_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed = request
        .observed_step_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request
        .observed_step_order
        .iter()
        .any(|step| !required.contains(step))
        || observed.len() != request.observed_step_order.len()
    {
        return Err(ProtocolSimulationAssuranceError::Invalid(
            "observed steps must be a canonical subset of required steps".into(),
        ));
    }
    let mut missing_steps = required.difference(&observed).cloned().collect::<Vec<_>>();
    missing_steps.sort();
    let peer_set = request
        .peer_institution_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for peer in &request.peers {
        let mut hard = !peer_set.contains(&peer.institution_id)
            || peer.protocol_digest != request.protocol_digest
            || peer.replay_identity != request.replay_identity
            || peer.provenance_digest.is_none()
            || !peer.signed_approval
            || peer.permitted_artifact != "protocol-simulation"
            || peer.semantic_profile != request.semantic_profile
            || matches!(peer.evidence_state, EvidenceState::Contradicted);
        if !peer_set.contains(&peer.institution_id) {
            omissions.insert(format!("peer:{}:not-declared", peer.institution_id));
        }
        if peer.provenance_digest.is_none() {
            omissions.insert(format!("peer:{}:provenance-missing", peer.institution_id));
        }
        if !peer.signed_approval {
            omissions.insert(format!(
                "peer:{}:signed-approval-missing",
                peer.institution_id
            ));
        }
        if peer.protocol_digest != request.protocol_digest {
            hard = true;
            omissions.insert(format!("peer:{}:protocol-mismatch", peer.institution_id));
        }
        if peer.replay_identity != request.replay_identity {
            hard = true;
            omissions.insert(format!("peer:{}:replay-mismatch", peer.institution_id));
        }
        if peer.semantic_profile != request.semantic_profile {
            hard = true;
            omissions.insert(format!(
                "peer:{}:semantic-profile-mismatch",
                peer.institution_id
            ));
        }
        let peer_required = required.iter().cloned().collect::<BTreeSet<_>>();
        let peer_observed = peer
            .observed_step_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let peer_observed_in_protocol = peer_observed
            .intersection(&peer_required)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peer_observed != peer_observed_in_protocol {
            hard = true;
            omissions.insert(format!(
                "peer:{}:step-outside-protocol",
                peer.institution_id
            ));
        }
        if matches!(
            peer.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            uncertainty.insert(format!("peer:{}:evidence-state", peer.institution_id));
        }
        for item in &peer.omissions {
            omissions.insert(format!("peer:{}:{item}", peer.institution_id));
        }
        for item in &peer.uncertainty {
            uncertainty.insert(format!("peer:{}:{item}", peer.institution_id));
        }
        negative.insert(format!(
            "peer:{}:{}",
            peer.institution_id,
            if peer.negative_result {
                "negative-result"
            } else {
                "negative-result-not-observed"
            }
        ));
        if hard {
            blocked.insert(peer.institution_id.clone());
        } else if matches!(
            peer.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            qualified.insert(peer.institution_id.clone());
        } else {
            unresolved.insert(peer.institution_id.clone());
        }
    }
    for peer in peer_set
        .difference(&qualified)
        .chain(blocked.difference(&qualified))
        .cloned()
        .collect::<BTreeSet<_>>()
    {
        if !blocked.contains(&peer) {
            unresolved.insert(peer);
        }
    }
    let quorum_met = qualified.len() >= request.required_peer_quorum as usize;
    if !quorum_met {
        omissions.insert(format!(
            "peer-quorum:{}/{}",
            qualified.len(),
            request.required_peer_quorum
        ));
        uncertainty.insert("federation:peer-quorum-incomplete".into());
    }
    let mut violations = BTreeSet::new();
    for (name, failed) in [
        ("policy", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("signed-approval", !request.signed_approval),
        ("federation-approval", !request.federation_approved),
        ("budget", request.budget_units > request.max_budget_units),
        ("provenance", request.provenance_digest.is_none()),
        ("raw-data-locality", !request.raw_data_local),
    ] {
        if failed {
            violations.insert(name.to_string());
        }
    }
    for event in &request.adversarial_events {
        violations.insert(format!("adversarial:{event}"));
    }
    if !missing_steps.is_empty() {
        omissions.insert(format!("missing-steps:{}", missing_steps.join(",")));
    }
    if matches!(
        request.evidence_state,
        EvidenceState::Unknown | EvidenceState::Speculative
    ) {
        uncertainty.insert("evidence-state-not-qualified".into());
    }
    if request.evidence_state == EvidenceState::Contradicted {
        violations.insert("contradicted-evidence".into());
        negative.insert("local:contradicted-evidence".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("workflow:signed-approval-missing".into());
    }
    if !request.federation_approved {
        omissions.insert("workflow:federation-approval-missing".into());
    }
    for event in &request.adversarial_events {
        omissions.insert(format!("workflow:adversarial:{event}"));
    }
    let global_block =
        !violations.is_empty() || !request.adversarial_events.is_empty() || !blocked.is_empty();
    let disposition = if global_block {
        "blocked"
    } else if !missing_steps.is_empty()
        || !quorum_met
        || !unresolved.is_empty()
        || !uncertainty.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    let peer_order = request.peer_institution_order.clone();
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let peer_envelope = ContentHash::of_value(&json!({"federation_id":request.federation_id,"purpose":request.purpose,"protocol_digest":request.protocol_digest,"peer_order":peer_order,"qualified_peer_order":qualified_order,"replay_identity":request.replay_identity,"semantic_profile":request.semantic_profile})).map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"purpose":request.purpose,"protocol_schema":request.protocol_schema,"semantic_profile":request.semantic_profile,"required_step_order":request.required_step_order,"observed_step_order":request.observed_step_order,"missing_step_order":missing_steps,"violation_order":violations,"peer_order":peer_order,"qualified_peer_order":qualified_order,"unresolved_peer_order":unresolved_order,"blocked_peer_order":blocked_order,"replay_identity":request.replay_identity,"peer_envelope_digest":peer_envelope,"disposition":disposition,"boundary":PRECLINICAL_BOUNDARY});
    let verdict_digest = ContentHash::of_value(&payload)
        .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("fiber-protocol-simulation:{}", request.request_id),
        "application/vnd.aurora.protocol-simulation-report+json",
        &payload,
        Vec::<SemanticLoss>::new(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "federated-protocol-simulation-assurance".into(),
            digest: verdict_digest.clone(),
        }],
    )
    .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "qualified" {
        vec![format!(
            "verify:fiber-protocol-simulation:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let mut checks: Vec<String> = vec![
        "schema-version".into(),
        "step-closure".into(),
        "peer-provenance".into(),
        "peer-semantic-profile".into(),
        "peer-replay-identity".into(),
        "peer-quorum".into(),
        "policy-boundary".into(),
        "negative-evidence-retention".into(),
    ];
    checks.sort();
    let report = ProtocolSimulationReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        protocol_schema: request.protocol_schema.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        required_step_order: request.required_step_order.clone(),
        observed_step_order: request.observed_step_order.clone(),
        missing_step_order: missing_steps,
        violation_order: violations.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_order,
        unresolved_peer_order: unresolved_order,
        blocked_peer_order: blocked_order,
        adversarial_event_order: request
            .adversarial_events
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        protocol_digest: request.protocol_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest: request.provenance_digest.clone(),
        peer_envelope_digest: peer_envelope,
        verdict_digest,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

pub fn assure_json(value: &Value) -> Result<Value, ProtocolSimulationAssuranceError> {
    let request: ProtocolDraft = serde_json::from_value(value.clone())
        .map_err(|error| ProtocolSimulationAssuranceError::Invalid(error.to_string()))?;
    serde_json::to_value(assure(&request)?)
        .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"fiber-federated-protocol")
    }
    fn peer(id: &str, state: EvidenceState) -> PeerProtocolSummary {
        PeerProtocolSummary {
            institution_id: id.into(),
            protocol_digest: hash(),
            replay_identity: hash(),
            provenance_digest: Some(hash()),
            semantic_profile: "protocol:v1".into(),
            observed_step_order: vec!["preflight".into(), "simulate".into(), "checkpoint".into()],
            evidence_state: state,
            signed_approval: true,
            permitted_artifact: "protocol-simulation".into(),
            negative_result: false,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }
    fn draft() -> ProtocolDraft {
        ProtocolDraft {
            request_id: "request:fiber-protocol".into(),
            federation_id: "federation:protocol".into(),
            purpose: "replication-benchmark".into(),
            protocol_schema: INPUT_SCHEMA.into(),
            protocol_digest: hash(),
            replay_identity: hash(),
            semantic_profile: "protocol:v1".into(),
            required_step_order: vec!["checkpoint".into(), "preflight".into(), "simulate".into()],
            observed_step_order: vec!["checkpoint".into(), "preflight".into(), "simulate".into()],
            evidence_state: EvidenceState::Supported,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            budget_units: 10,
            max_budget_units: 10,
            adversarial_events: Vec::new(),
            provenance_digest: Some(hash()),
            peer_institution_order: vec!["institution-b".into(), "institution-c".into()],
            required_peer_quorum: 2,
            peers: vec![
                peer("institution-b", EvidenceState::Supported),
                peer("institution-c", EvidenceState::Proven),
            ],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_quorum_emits_verification() {
        let report = assure(&draft()).unwrap();
        assert_eq!(report.disposition, "qualified");
        assert!(report.effect_receipts[0].starts_with("verify:fiber-protocol-simulation:"));
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }
    #[test]
    fn missing_step_is_unresolved() {
        let mut value = draft();
        value.observed_step_order.pop();
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.missing_step_order.contains(&"simulate".into()));
    }
    #[test]
    fn unknown_peer_is_unresolved() {
        let mut value = draft();
        value.peers[0].evidence_state = EvidenceState::Unknown;
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report
            .uncertainty
            .iter()
            .any(|item| item.contains("peer-quorum")));
    }
    #[test]
    fn peer_semantic_mismatch_blocks() {
        let mut value = draft();
        value.peers[0].semantic_profile = "other".into();
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert!(report.blocked_peer_order.contains(&"institution-b".into()));
    }
    #[test]
    fn contradiction_and_adversarial_input_block() {
        let mut value = draft();
        value.evidence_state = EvidenceState::Contradicted;
        value.adversarial_events = vec!["poisoned-summary".into()];
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
        assert!(report
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
    }
    #[test]
    fn manifest_is_a1_and_federated() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .effects
            .contains(&Effect::FederationExport));
    }
}
