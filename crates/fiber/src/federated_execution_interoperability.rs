//! Federated continual computational-execution interoperability for FIBER.
//!
//! Atlas feature: `AFA-fiber-P12-F24`.
//!
//! This gateway admits only typed execution-artifact manifests. It never dispatches a job,
//! opens a provider connection, moves raw experimental bytes, or makes a clinical decision.
//! Federation receives a digest-only envelope after capability, provenance, replay, policy,
//! locality, and authority gates close.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P12-F24";
pub const CONTRACT_VERSION: &str =
    "fiber-federated-continual-computational-execution-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ExecutionRun8@1";
pub const OUTPUT_SCHEMA: &str = "FederationEnvelope8@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionArtifactCandidate {
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub schema_version: String,
    pub effect_scope: String,
    pub evidence_state: EvidenceState,
    pub raw_data_local: bool,
    pub permitted: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionInteroperabilityRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub source_institution: String,
    pub target_institution: String,
    pub workflow_schema: String,
    pub semantic_profile: String,
    pub protocol_version: String,
    pub required_capability_order: Vec<String>,
    pub offered_capability_order: Vec<String>,
    pub artifact: ExecutionArtifactCandidate,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionInteroperabilityEnvelope {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub source_institution: String,
    pub target_institution: String,
    pub purpose: String,
    pub workflow_schema: String,
    pub semantic_profile: String,
    pub protocol_version: String,
    pub disposition: String,
    pub required_capability_order: Vec<String>,
    pub offered_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub violation_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub envelope_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutionInteroperabilityError {
    #[error("invalid federated execution interoperability request: {0}")]
    Invalid(String),
    #[error("federated execution interoperability artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ExecutionInteroperabilityEnvelope {
    pub fn validate(&self) -> Result<(), ExecutionInteroperabilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.workflow_schema != INPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.source_institution.trim().is_empty()
            || self.target_institution.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.protocol_version.trim().is_empty()
            || self.required_capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ExecutionInteroperabilityError::Invalid(
                "envelope identity, schema, locality, aggregate boundary, capabilities, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.required_capability_order,
            &self.offered_capability_order,
            &self.missing_capability_order,
            &self.violation_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(ExecutionInteroperabilityError::Invalid(
                    "envelope orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let required = self.required_capability_order.iter().collect::<BTreeSet<_>>();
        let offered = self.offered_capability_order.iter().collect::<BTreeSet<_>>();
        let missing = self.missing_capability_order.iter().collect::<BTreeSet<_>>();
        if required.len() != self.required_capability_order.len()
            || offered.len() != self.offered_capability_order.len()
            || missing != required.difference(&offered).cloned().collect::<BTreeSet<_>>()
        {
            return Err(ExecutionInteroperabilityError::Invalid(
                "capability closure is not a disjoint required/offered/missing partition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:execution-envelope:") && effect != "block:unsafe-release"
        }) {
            return Err(ExecutionInteroperabilityError::Invalid(
                "effect is outside the digest-only execution-envelope gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExecutionInteroperabilityError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ExecutionInteroperabilityError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ExecutionInteroperabilityError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ExecutionInteroperabilityError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "fiber".into(),
        consumers: BTreeSet::from([
            "context compiler engineer".into(),
            "federation execution operator".into(),
            "downstream workflow gateway".into(),
        ]),
        behavior: "validates digest-only execution artifact manifests and emits capability-negotiated federation envelopes without dispatching jobs".into(),
        value: "prevents incompatible, non-replayable, unauthorized, or locality-violating execution artifacts from crossing institution boundaries".into(),
        inputs: vec![TypedPort {
            name: "execution_run".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "federation_envelope".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]),
        permissions: BTreeSet::from([
            "negotiate:execution-capabilities".into(),
            "exchange:permitted-research-artifacts".into(),
        ]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "ga4gh-wes".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()),
            },
            EvidenceReference {
                source_id: "ga4gh-drs".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()),
            },
            EvidenceReference {
                source_id: "slsa-provenance-1.2".into(),
                state: EvidenceState::Supported,
                locator: Some("https://slsa.dev/spec/v1.2/provenance".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "federated execution gateway operator".into(),
            reason: "approve capability-negotiated execution artifact exchange".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure(
    request: &ExecutionInteroperabilityRequest,
) -> Result<ExecutionInteroperabilityEnvelope, ExecutionInteroperabilityError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.source_institution.trim().is_empty()
        || request.target_institution.trim().is_empty()
        || request.source_institution == request.target_institution
        || request.purpose.trim().is_empty()
        || request.workflow_schema != INPUT_SCHEMA
        || request.semantic_profile.trim().is_empty()
        || request.protocol_version.trim().is_empty()
        || request.required_capability_order.is_empty()
        || !canonical(&request.required_capability_order)
        || !canonical(&request.offered_capability_order)
        || !canonical(&request.artifact.omissions)
        || !canonical(&request.artifact.uncertainty)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ExecutionInteroperabilityError::Invalid(
            "request identity, schema, capability orders, locality, aggregate boundary, budget, or artifact annotations are invalid".into(),
        ));
    }
    let required = request
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let offered = request
        .offered_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != request.required_capability_order.len()
        || offered.len() != request.offered_capability_order.len()
    {
        return Err(ExecutionInteroperabilityError::Invalid(
            "required and offered capabilities must be unique".into(),
        ));
    }
    if request.artifact.artifact_id.trim().is_empty()
        || request.artifact.schema_version != "ExecutionArtifact7@1"
        || request.artifact.semantic_profile != request.semantic_profile
        || request.artifact.replay_identity != request.replay_identity
        || request.artifact.provenance_digest.is_none()
        || !request.artifact.raw_data_local
        || !request.artifact.permitted
        || request.artifact.effect_scope != "permitted-artifact"
    {
        return Err(ExecutionInteroperabilityError::Invalid(
            "artifact identity, schema, semantic/replay binding, provenance, locality, permission, or effect scope is invalid".into(),
        ));
    }
    let missing = required
        .difference(&offered)
        .cloned()
        .collect::<Vec<_>>();
    let mut violations = BTreeSet::new();
    let mut omissions = request
        .artifact
        .omissions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut uncertainty = request
        .artifact
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = BTreeSet::from([format!(
        "artifact:{}",
        if request.artifact.negative_result {
            "negative-result"
        } else {
            "negative-result-not-observed"
        }
    )]);
    if !missing.is_empty() {
        omissions.insert(format!("missing-capabilities:{}", missing.join(",")));
        uncertainty.insert("capability-closure-incomplete".into());
    }
    if !request.policy_allow {
        violations.insert("policy".into());
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        violations.insert("protected-closure".into());
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        violations.insert("signed-approval".into());
        omissions.insert("workflow:signed-approval-missing".into());
    }
    if !request.federation_approved {
        violations.insert("federation-approval".into());
        omissions.insert("workflow:federation-approval-missing".into());
    }
    if request.budget_units > request.max_budget_units {
        violations.insert("budget".into());
    }
    if !request.artifact.raw_data_local || !request.raw_data_local || !request.aggregate_only {
        violations.insert("raw-data-locality-or-aggregate-boundary".into());
    }
    if request.artifact.evidence_state == EvidenceState::Contradicted {
        violations.insert("contradicted-evidence".into());
        negative.insert("artifact:contradicted-evidence".into());
    }
    if matches!(
        request.artifact.evidence_state,
        EvidenceState::Unknown | EvidenceState::Speculative
    ) {
        uncertainty.insert("artifact:evidence-state-not-qualified".into());
    }
    for event in &request.adversarial_events {
        violations.insert(format!("adversarial:{event}"));
        omissions.insert(format!("workflow:adversarial:{event}"));
    }
    let global_block = !violations.is_empty() || !request.adversarial_events.is_empty();
    let disposition = if global_block {
        "blocked"
    } else if !missing.is_empty() || !uncertainty.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let missing_capability_order = missing;
    let offered_capability_order = request.offered_capability_order.clone();
    let artifact_digest = request.artifact.content_hash.clone();
    let envelope_payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "source_institution": request.source_institution,
        "target_institution": request.target_institution,
        "purpose": request.purpose,
        "workflow_schema": request.workflow_schema,
        "semantic_profile": request.semantic_profile,
        "protocol_version": request.protocol_version,
        "required_capability_order": request.required_capability_order,
        "offered_capability_order": offered_capability_order,
        "missing_capability_order": missing_capability_order,
        "artifact_digest": artifact_digest,
        "replay_identity": request.replay_identity,
        "disposition": disposition,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let envelope_digest = ContentHash::of_value(&envelope_payload)
        .map_err(|error| ExecutionInteroperabilityError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("fiber-execution-envelope:{}", request.request_id),
        "application/vnd.aurora.execution-federation-envelope+json",
        &envelope_payload,
        Vec::<SemanticLoss>::new(),
        vec![ProvenanceLink {
            source_id: request.source_institution.clone(),
            relation: "federated-execution-interoperability".into(),
            digest: artifact_digest.clone(),
        }],
    )
    .map_err(|error| ExecutionInteroperabilityError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "qualified" {
        vec![format!(
            "exchange:execution-envelope:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let report = ExecutionInteroperabilityEnvelope {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        source_institution: request.source_institution.clone(),
        target_institution: request.target_institution.clone(),
        purpose: request.purpose.clone(),
        workflow_schema: request.workflow_schema.clone(),
        semantic_profile: request.semantic_profile.clone(),
        protocol_version: request.protocol_version.clone(),
        disposition: disposition.into(),
        required_capability_order: request.required_capability_order.clone(),
        offered_capability_order,
        missing_capability_order,
        violation_order: violations.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact_digest,
        replay_identity: request.replay_identity.clone(),
        envelope_digest,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"fiber-execution-interoperability")
    }

    fn request() -> ExecutionInteroperabilityRequest {
        ExecutionInteroperabilityRequest {
            request_id: "request:fiber-execution".into(),
            federation_id: "federation:execution".into(),
            purpose: "replication-workflow".into(),
            source_institution: "institution-a".into(),
            target_institution: "institution-b".into(),
            workflow_schema: INPUT_SCHEMA.into(),
            semantic_profile: "execution:v1".into(),
            protocol_version: "wes:1.1".into(),
            required_capability_order: vec!["artifact-read".into(), "replay-verify".into()],
            offered_capability_order: vec!["artifact-read".into(), "replay-verify".into()],
            artifact: ExecutionArtifactCandidate {
                artifact_id: "execution:run-1".into(),
                content_hash: hash(),
                provenance_digest: Some(hash()),
                replay_identity: hash(),
                semantic_profile: "execution:v1".into(),
                schema_version: "ExecutionArtifact7@1".into(),
                effect_scope: "permitted-artifact".into(),
                evidence_state: EvidenceState::Supported,
                raw_data_local: true,
                permitted: true,
                omissions: Vec::new(),
                uncertainty: Vec::new(),
                negative_result: false,
            },
            replay_identity: hash(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            budget_units: 10,
            max_budget_units: 10,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn qualified_capability_negotiation_emits_exchange() {
        let report = assure(&request()).unwrap();
        assert_eq!(report.disposition, "qualified");
        assert!(report.effect_receipts[0].starts_with("exchange:execution-envelope:"));
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }

    #[test]
    fn missing_capability_is_unresolved() {
        let mut value = request();
        value.offered_capability_order.pop();
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.missing_capability_order.contains(&"replay-verify".into()));
    }

    #[test]
    fn unknown_artifact_retains_uncertainty() {
        let mut value = request();
        value.artifact.evidence_state = EvidenceState::Unknown;
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.uncertainty.iter().any(|item| item.contains("evidence-state")));
    }

    #[test]
    fn policy_and_contradiction_block() {
        let mut value = request();
        value.policy_allow = false;
        value.artifact.evidence_state = EvidenceState::Contradicted;
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
        assert!(report.negative_evidence.iter().any(|item| item.contains("contradicted")));
    }

    #[test]
    fn adversarial_event_blocks_without_execution() {
        let mut value = request();
        value.adversarial_events = vec!["poisoned-artifact".into()];
        let report = assure(&value).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert!(report.omissions.iter().any(|item| item.contains("adversarial")));
    }

    #[test]
    fn manifest_is_a2_and_federated() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
}
