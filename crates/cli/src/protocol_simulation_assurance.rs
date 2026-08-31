//! Federated continual protocol-simulation assurance for the CLI.
//!
//! This is a verification boundary, not a protocol runner. It checks a caller-supplied,
//! content-addressed protocol state-machine summary and emits a replayable release verdict.
//! Raw study data, instruments, and external workflow engines remain outside this crate.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P10-F28";
pub const CONTRACT_VERSION: &str = "cli-federated-continual-protocol-simulation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ProtocolDraft4@1";
pub const OUTPUT_SCHEMA: &str = "ProtocolSimulationReport7@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolSimulationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSimulationAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub protocol_schema: String,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub required_steps: Vec<String>,
    pub observed_steps: Vec<String>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSimulationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub protocol_schema: String,
    pub disposition: ProtocolSimulationDisposition,
    pub required_step_order: Vec<String>,
    pub observed_step_order: Vec<String>,
    pub missing_step_order: Vec<String>,
    pub violation_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub verdict_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolSimulationAssuranceError {
    #[error("invalid protocol-simulation assurance request: {0}")]
    Invalid(String),
    #[error("protocol-simulation assurance artifact failed: {0}")]
    Artifact(String),
}

impl ProtocolSimulationAssuranceReceipt {
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
            || self.required_step_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.violation_order.windows(2).any(|w| w[0] >= w[1])
            || self
                .adversarial_event_order
                .windows(2)
                .any(|w| w[0] >= w[1])
            || self.omissions.windows(2).any(|w| w[0] >= w[1])
            || self.uncertainty.windows(2).any(|w| w[0] >= w[1])
            || self.negative_evidence.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "identity, schema, locality, steps, ordering, or effects are incomplete".into(),
            ));
        }
        let required = BTreeSet::from_iter(self.required_step_order.iter().cloned());
        if required.len() != self.required_step_order.len()
            || self.observed_step_order.windows(2).any(|w| w[0] >= w[1])
            || self.missing_step_order.windows(2).any(|w| w[0] >= w[1])
            || self
                .observed_step_order
                .iter()
                .any(|step| !required.contains(step))
            || self
                .missing_step_order
                .iter()
                .any(|step| !required.contains(step))
        {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "step orders are not canonical subsets".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:protocol-simulation:") && effect != "block:unsafe-release"
        }) {
            return Err(ProtocolSimulationAssuranceError::Invalid(
                "effect is outside the verification/release gate".into(),
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
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "cli".into(),
        consumers: BTreeSet::from([
            "AURORA extension developer".into(),
            "release-evidence operator".into(),
        ]),
        behavior: "verifies a typed protocol simulation summary across federated continual safety and release gates".into(),
        value: "prevents incomplete, adversarial, non-replayable, or unauthorized protocol evidence from appearing qualified".into(),
        inputs: vec![TypedPort { name: "protocol_draft".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "protocol_simulation_report".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["evaluate:capability-runs".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
            EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "release-evidence-operator".into(), reason: "protocol simulation release verdict".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn verify(
    request: &ProtocolSimulationAssuranceRequest,
) -> Result<ProtocolSimulationAssuranceReceipt, ProtocolSimulationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.protocol_schema != INPUT_SCHEMA
        || request.required_steps.is_empty()
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
    {
        return Err(ProtocolSimulationAssuranceError::Invalid(
            "identity, schema, required steps, budget, or locality is invalid".into(),
        ));
    }
    let mut required = request.required_steps.clone();
    required.sort();
    if required
        .windows(2)
        .any(|window| window[0] == window[1] || window[0].trim().is_empty())
    {
        return Err(ProtocolSimulationAssuranceError::Invalid(
            "required steps must be unique and non-empty".into(),
        ));
    }
    let mut observed = request.observed_steps.clone();
    observed.sort();
    observed.dedup();
    if observed
        .iter()
        .any(|step| !required.binary_search(step).is_ok())
    {
        return Err(ProtocolSimulationAssuranceError::Invalid(
            "observed steps must be declared required steps".into(),
        ));
    }
    let missing = required
        .iter()
        .filter(|step| !observed.contains(step))
        .cloned()
        .collect::<Vec<_>>();
    let mut violations = BTreeSet::new();
    for (name, failed) in [
        ("policy", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("signed-approval", !request.signed_approval),
        ("federation-approval", !request.federation_approved),
        ("raw-data-locality", !request.raw_data_local),
        ("budget", request.budget_units > request.max_budget_units),
        ("provenance", request.provenance_digest.is_none()),
    ] {
        if failed {
            violations.insert(name.to_string());
        }
    }
    let adversarial = request
        .adversarial_events
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for event in &adversarial {
        violations.insert(format!("adversarial:{event}"));
    }
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if !missing.is_empty() {
        omissions.insert(format!("missing-steps:{}", missing.join(",")));
    }
    match request.evidence_state {
        EvidenceState::Unknown | EvidenceState::Speculative => {
            uncertainty.insert("evidence-state-not-qualified".into());
        }
        EvidenceState::Contradicted => {
            violations.insert("contradicted-evidence".into());
            negative.insert("contradicted-evidence".into());
        }
        EvidenceState::Proven | EvidenceState::Supported => {}
    }
    if !request.adversarial_events.is_empty() {
        negative.insert("adversarial-event-present".into());
    }
    let disposition = if !violations.is_empty() {
        ProtocolSimulationDisposition::Blocked
    } else if !missing.is_empty() || !uncertainty.is_empty() {
        ProtocolSimulationDisposition::Unresolved
    } else {
        ProtocolSimulationDisposition::Qualified
    };
    let violation_order = violations.into_iter().collect::<Vec<_>>();
    let adversarial_event_order = adversarial.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "protocol_schema": request.protocol_schema,
        "disposition": disposition,
        "required_step_order": required,
        "observed_step_order": observed,
        "missing_step_order": missing,
        "violation_order": violation_order,
        "adversarial_event_order": adversarial_event_order,
        "protocol_digest": request.protocol_digest,
        "replay_identity": request.replay_identity,
        "provenance_digest": request.provenance_digest,
    });
    let verdict_digest = ContentHash::of_value(&payload)
        .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))?;
    let semantic_loss = violation_order
        .iter()
        .map(|gate| SemanticLoss {
            field: format!("gate:{gate}"),
            reason: "protocol evidence cannot be promoted through a failed safety gate".into(),
            severity: LossSeverity::DecisionRelevant,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("protocol-simulation-assurance:{}", request.request_id),
        "application/vnd.aurora.protocol-simulation-report+json",
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.request_id.clone(),
            relation: "protocol-simulation-assurance".into(),
            digest: verdict_digest.clone(),
        }],
    )
    .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(disposition, ProtocolSimulationDisposition::Qualified) {
        vec![format!("verify:protocol-simulation:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ProtocolSimulationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        protocol_schema: request.protocol_schema.clone(),
        disposition,
        required_step_order: required,
        observed_step_order: observed,
        missing_step_order: missing,
        violation_order,
        adversarial_event_order,
        omissions,
        uncertainty,
        negative_evidence,
        protocol_digest: request.protocol_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest: request.provenance_digest.clone(),
        verdict_digest,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn verify_json(value: &Value) -> Result<Value, ProtocolSimulationAssuranceError> {
    let request: ProtocolSimulationAssuranceRequest = serde_json::from_value(value.clone())
        .map_err(|error| ProtocolSimulationAssuranceError::Invalid(error.to_string()))?;
    serde_json::to_value(verify(&request)?)
        .map_err(|error| ProtocolSimulationAssuranceError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"protocol-simulation-assurance")
    }
    fn request() -> ProtocolSimulationAssuranceRequest {
        ProtocolSimulationAssuranceRequest {
            request_id: "request:protocol".into(),
            federation_id: "federation:protocol".into(),
            purpose: "protocol-simulation-release".into(),
            protocol_schema: INPUT_SCHEMA.into(),
            protocol_digest: hash(),
            replay_identity: hash(),
            required_steps: vec!["acquire".into(), "simulate".into(), "verify".into()],
            observed_steps: vec!["verify".into(), "acquire".into(), "simulate".into()],
            evidence_state: EvidenceState::Supported,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            budget_units: 2,
            max_budget_units: 5,
            adversarial_events: Vec::new(),
            provenance_digest: Some(hash()),
        }
    }

    #[test]
    fn manifest_is_a1_and_cli_facing() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Cli));
    }
    #[test]
    fn complete_protocol_qualifies_and_is_replayable() {
        let receipt = verify(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            ProtocolSimulationDisposition::Qualified
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn missing_step_is_unresolved() {
        let mut value = request();
        value.observed_steps.pop();
        let receipt = verify(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            ProtocolSimulationDisposition::Unresolved
        );
        assert!(!receipt.missing_step_order.is_empty());
    }
    #[test]
    fn contradiction_and_adversary_block() {
        let mut value = request();
        value.evidence_state = EvidenceState::Contradicted;
        value.adversarial_events = vec!["prompt_injection".into()];
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, ProtocolSimulationDisposition::Blocked);
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }
    #[test]
    fn policy_or_provenance_gap_blocks() {
        let mut value = request();
        value.policy_allow = false;
        value.provenance_digest = None;
        let receipt = verify(&value).unwrap();
        assert_eq!(receipt.disposition, ProtocolSimulationDisposition::Blocked);
        assert!(receipt.violation_order.iter().any(|gate| gate == "policy"));
    }
}
