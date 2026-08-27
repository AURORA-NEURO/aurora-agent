//! Federated-commons interoperability gateway.
//!
//! Atlas feature: `AFA-policy-P31-F24`.
//!
//! The gateway admits only purpose-bound, signed, digest-only research artifacts. It is a policy
//! protocol boundary: it never moves raw data, contacts a remote institution, or decides anything
//! clinical. Every denied, unknown, contradictory, stale, or quorum-incomplete contribution is
//! represented in the returned `FederationEnvelope` rather than hidden behind a boolean.

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

pub const FEATURE_ID: &str = "AFA-policy-P31-F24";
pub const CONTRACT_VERSION: &str =
    "policy-federated-continual-commons-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "PolicyFederationRequest4@1";
pub const OUTPUT_SCHEMA: &str = "PolicyFederationEnvelope6@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationAdmission {
    Admitted,
    Partial,
    ApprovalRequired,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationArtifactCandidate {
    pub artifact_id: String,
    pub origin_institution: String,
    pub artifact_type: String,
    pub content_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub purpose: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFederationRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub allowed_artifact_types: Vec<String>,
    pub candidates: Vec<FederationArtifactCandidate>,
    pub required_origin_quorum: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub purpose_bound: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub network_permitted: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFederationEnvelope {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub admission: FederationAdmission,
    pub origin_order: Vec<String>,
    pub accepted_origin_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<Value>,
    pub replay_identity: ContentHash,
    pub envelope_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub federation_export: String,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyFederationError {
    #[error("invalid policy federation request: {0}")]
    Invalid(String),
    #[error("policy federation artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl PolicyFederationEnvelope {
    pub fn validate(&self) -> Result<(), PolicyFederationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.federation_export != "aggregate-digest-only"
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.decisions.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(PolicyFederationError::Invalid(
                "federation identity, locality, candidate decisions, export mode, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.origin_order,
            &self.accepted_origin_order,
            &self.candidate_order,
            &self.admitted_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(PolicyFederationError::Invalid(
                    "federation orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partition = self
            .admitted_order
            .iter()
            .chain(self.conditional_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if partition != candidates
            || partition.len()
                != self.admitted_order.len()
                    + self.conditional_order.len()
                    + self.blocked_order.len()
                    + self.unknown_order.len()
        {
            return Err(PolicyFederationError::Invalid(
                "federation admission states do not partition candidates".into(),
            ));
        }
        if self.decisions.iter().enumerate().any(|(index, decision)| {
            decision.get("artifact_id").and_then(Value::as_str)
                != Some(self.candidate_order[index].as_str())
        }) {
            return Err(PolicyFederationError::Invalid(
                "federation decisions do not match candidate order".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:")
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(PolicyFederationError::Invalid(
                "federation effect is outside the permitted-artifact gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| PolicyFederationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, PolicyFederationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| PolicyFederationError::Artifact(error.to_string()))?,
        )
        .map_err(|error| PolicyFederationError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "policy".into(),
        consumers: BTreeSet::from([
            "federation steward".into(),
            "institution-local research gateway".into(),
            "research-object release service".into(),
        ]),
        behavior: "admits purpose-bound digest-only research artifact exchange across policy-separated institutions".into(),
        value: "enables federated research commons without moving raw preclinical data or hiding policy and quorum failures".into(),
        inputs: vec![TypedPort { name: "policy_federation_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "policy_federation_envelope".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["connect:approved-endpoints".into(), "exchange:permitted-artifacts".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "slsa-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "federation-steward".into(), reason: "permitted artifact exchange".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &PolicyFederationRequest) -> Result<(), PolicyFederationError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.allowed_artifact_types.is_empty()
        || request.candidates.is_empty()
        || request.required_origin_quorum == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(PolicyFederationError::Invalid(
            "federation identity, allowed artifacts, candidates, quorum, locality, or boundary is invalid".into(),
        ));
    }
    if !canonical(&request.allowed_artifact_types)
        || request
            .allowed_artifact_types
            .iter()
            .any(|item| item.trim().is_empty())
    {
        return Err(PolicyFederationError::Invalid(
            "allowed artifact types must be unique, non-empty, and canonical".into(),
        ));
    }
    let mut ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.artifact_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1])
        || request.candidates.iter().any(|candidate| {
            candidate.artifact_id.trim().is_empty()
                || candidate.origin_institution.trim().is_empty()
                || candidate.artifact_type.trim().is_empty()
                || candidate.purpose.trim().is_empty()
                || candidate.semantic_profile.trim().is_empty()
        })
    {
        return Err(PolicyFederationError::Invalid(
            "artifact identifiers, origins, types, purpose, or semantic profile are invalid".into(),
        ));
    }
    Ok(())
}

pub fn admit(
    request: &PolicyFederationRequest,
) -> Result<PolicyFederationEnvelope, PolicyFederationError> {
    validate_request(request)?;
    let allowed_types = request
        .allowed_artifact_types
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.artifact_id.clone())
        .collect::<Vec<_>>();
    let origins = candidates
        .iter()
        .map(|candidate| candidate.origin_institution.clone())
        .collect::<BTreeSet<_>>();
    let global_failed = [
        ("policy", !request.policy_allow),
        ("purpose-bound", !request.purpose_bound),
        ("protected-closure", !request.protected_closure),
        ("raw-data-locality", !request.raw_data_local),
    ]
    .into_iter()
    .filter_map(|(gate, failed)| failed.then_some(gate.to_string()))
    .collect::<BTreeSet<_>>();
    let mut admitted = Vec::new();
    let mut conditional = Vec::new();
    let mut blocked = Vec::new();
    let mut unknown = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut decisions = Vec::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone();
        let mut pending = BTreeSet::<String>::new();
        let mut unknown_state = false;
        if !allowed_types.contains(&candidate.artifact_type) {
            failed.insert("artifact-type-not-allowed".into());
            omissions.insert(format!(
                "{}:artifact-type-not-allowed",
                candidate.artifact_id
            ));
        }
        if candidate.purpose != request.purpose {
            failed.insert("purpose-mismatch".into());
            omissions.insert(format!("{}:purpose-mismatch", candidate.artifact_id));
        }
        if candidate.semantic_profile != request.semantic_profile {
            pending.insert("semantic-profile-mismatch".into());
            omissions.insert(format!(
                "{}:semantic-profile-mismatch",
                candidate.artifact_id
            ));
        }
        if !candidate.permitted {
            failed.insert("candidate-not-permitted".into());
        }
        if !candidate.raw_data_local {
            failed.insert("candidate-locality".into());
        }
        if candidate.content_digest.is_none() {
            pending.insert("content-digest-missing".into());
            omissions.insert(format!("{}:content-digest-missing", candidate.artifact_id));
        }
        if candidate.provenance_digest.is_none() {
            pending.insert("provenance-missing".into());
            omissions.insert(format!("{}:provenance-missing", candidate.artifact_id));
        }
        if !candidate.omissions.is_empty() {
            pending.insert("candidate-omissions".into());
            omissions.extend(
                candidate
                    .omissions
                    .iter()
                    .map(|item| format!("{}:{item}", candidate.artifact_id)),
            );
        }
        if !candidate.uncertainty.is_empty() {
            pending.insert("candidate-uncertainty".into());
            uncertainty.extend(
                candidate
                    .uncertainty
                    .iter()
                    .map(|item| format!("{}:{item}", candidate.artifact_id)),
            );
        }
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
                blocked.push(candidate.artifact_id.clone());
                negative.insert(format!("{}:contradicted", candidate.artifact_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                pending.insert("evidence-state-not-qualified".into());
                unknown_state = true;
                uncertainty.insert(format!("{}:evidence-state", candidate.artifact_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        negative.insert(format!(
            "{}:{}",
            candidate.artifact_id,
            if candidate.negative_result {
                "negative-result"
            } else {
                "negative-result-not-observed"
            }
        ));
        let disposition = if !failed.is_empty() {
            blocked.push(candidate.artifact_id.clone());
            "blocked"
        } else if !pending.is_empty() {
            if unknown_state {
                unknown.push(candidate.artifact_id.clone());
            } else {
                conditional.push(candidate.artifact_id.clone());
            }
            "conditional"
        } else {
            admitted.push(candidate.artifact_id.clone());
            "admitted"
        };
        decisions.push(json!({
            "artifact_id": candidate.artifact_id,
            "origin": candidate.origin_institution,
            "disposition": disposition,
            "failed_gates": failed.clone().into_iter().collect::<Vec<_>>(),
            "conditional_gates": pending.into_iter().collect::<Vec<_>>(),
            "negative_result": candidate.negative_result,
        }));
        if !failed.is_empty() {
            semantic_loss.push(SemanticLoss {
                field: format!("artifact:{}", candidate.artifact_id),
                reason:
                    "artifact cannot cross the federation gate after a policy or evidence failure"
                        .into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    admitted.sort();
    conditional.sort();
    blocked.sort();
    blocked.dedup();
    unknown.sort();
    unknown.dedup();
    let accepted_origins = candidates
        .iter()
        .filter(|candidate| admitted.contains(&candidate.artifact_id))
        .map(|candidate| candidate.origin_institution.clone())
        .collect::<BTreeSet<_>>();
    if accepted_origins.len() < request.required_origin_quorum as usize {
        omissions.insert(format!(
            "origin-quorum:{}/{}",
            accepted_origins.len(),
            request.required_origin_quorum
        ));
    }
    let admission = if !global_failed.is_empty() || !blocked.is_empty() {
        FederationAdmission::Blocked
    } else if !request.signed_approval || !request.network_permitted {
        FederationAdmission::ApprovalRequired
    } else if !conditional.is_empty()
        || !unknown.is_empty()
        || accepted_origins.len() < request.required_origin_quorum as usize
    {
        FederationAdmission::Partial
    } else if admitted.is_empty() {
        FederationAdmission::Unknown
    } else {
        FederationAdmission::Admitted
    };
    let mut checks = [
        "artifact-digest",
        "artifact-permission",
        "candidate-locality",
        "evidence-state",
        "origin-quorum",
        "policy-boundary",
        "provenance-closure",
        "purpose-binding",
        "replay-identity",
        "semantic-profile",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    checks.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "institution_id": request.institution_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "admission": admission,
        "candidate_order": candidate_order,
        "admitted_order": admitted,
        "conditional_order": conditional,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "decisions": decisions,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let envelope_digest = ContentHash::of_value(&payload)
        .map_err(|error| PolicyFederationError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("federation-envelope:{}", request.request_id),
        "application/vnd.aurora.federation-envelope+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "policy-federation-gateway".into(),
            digest: envelope_digest.clone(),
        }],
    )
    .map_err(|error| PolicyFederationError::Artifact(error.to_string()))?;
    let effect_receipts = match admission {
        FederationAdmission::Admitted => vec![format!(
            "exchange:permitted-artifacts:{}",
            request.federation_id
        )],
        FederationAdmission::ApprovalRequired => {
            vec![format!("approval-required:{}", request.federation_id)]
        }
        _ => vec!["block:unsafe-release".into()],
    };
    let receipt = PolicyFederationEnvelope {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        admission,
        origin_order: origins.into_iter().collect(),
        accepted_origin_order: accepted_origins.into_iter().collect(),
        candidate_order,
        admitted_order: admitted,
        conditional_order: conditional,
        blocked_order: blocked,
        unknown_order: unknown,
        decisions,
        replay_identity: request.replay_identity.clone(),
        envelope_digest,
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        federation_export: "aggregate-digest-only".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn admit_json(value: &Value) -> Result<Value, PolicyFederationError> {
    let request: PolicyFederationRequest = serde_json::from_value(value.clone())
        .map_err(|error| PolicyFederationError::Invalid(error.to_string()))?;
    serde_json::to_value(admit(&request)?)
        .map_err(|error| PolicyFederationError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"policy-federation")
    }

    fn candidate(id: &str, origin: &str, state: EvidenceState) -> FederationArtifactCandidate {
        FederationArtifactCandidate {
            artifact_id: id.into(),
            origin_institution: origin.into(),
            artifact_type: "application/vnd.aurora.research-summary+json".into(),
            content_digest: Some(hash()),
            provenance_digest: Some(hash()),
            purpose: "preclinical-consortium-benchmark".into(),
            semantic_profile: "profile:v1".into(),
            evidence_state: state,
            permitted: true,
            raw_data_local: true,
            negative_result: false,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }

    fn request() -> PolicyFederationRequest {
        PolicyFederationRequest {
            request_id: "request:federation".into(),
            federation_id: "federation:commons".into(),
            institution_id: "institution:a".into(),
            purpose: "preclinical-consortium-benchmark".into(),
            semantic_profile: "profile:v1".into(),
            allowed_artifact_types: vec!["application/vnd.aurora.research-summary+json".into()],
            candidates: vec![
                candidate("a-1", "institution:a", EvidenceState::Supported),
                candidate("b-1", "institution:b", EvidenceState::Proven),
            ],
            required_origin_quorum: 2,
            replay_identity: hash(),
            policy_allow: true,
            purpose_bound: true,
            protected_closure: true,
            signed_approval: true,
            network_permitted: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn complete_exchange_is_admitted_and_replayable() {
        let envelope = admit(&request()).unwrap();
        assert_eq!(envelope.admission, FederationAdmission::Admitted);
        assert_eq!(envelope.digest().unwrap(), envelope.digest().unwrap());
        assert_eq!(envelope.accepted_origin_order.len(), 2);
    }

    #[test]
    fn missing_approval_is_explicit() {
        let mut value = request();
        value.signed_approval = false;
        let envelope = admit(&value).unwrap();
        assert_eq!(envelope.admission, FederationAdmission::ApprovalRequired);
        assert!(envelope.effect_receipts[0].starts_with("approval-required:"));
    }

    #[test]
    fn contradiction_and_policy_block_exchange() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        value.policy_allow = false;
        let envelope = admit(&value).unwrap();
        assert_eq!(envelope.admission, FederationAdmission::Blocked);
        assert!(envelope
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn unknown_candidate_is_partial_and_retained() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        let envelope = admit(&value).unwrap();
        assert_eq!(envelope.admission, FederationAdmission::Partial);
        assert!(envelope.unknown_order.contains(&"a-1".into()));
    }

    #[test]
    fn manifest_is_a2_and_digest_only() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A2);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Protocol));
    }
}
