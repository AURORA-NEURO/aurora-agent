//! Federated continual context-to-Decision-Section projection.
//!
//! Atlas feature: `AFA-brain-P03-F12`. The projector combines signed, digest-only
//! section attestations from policy-separated institutions. It requires a fresh
//! quorum and preserves every missing, stale, contradictory, replay-mismatched,
//! or policy-blocked peer instead of manufacturing a consensus decision.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F12";
pub const CONTRACT_VERSION: &str = "brain-federated-decision-projection/1.0";
const FEDERATED_CONTENT_TYPE: &str = "application/vnd.aurora.federated-decision-projection+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerDecisionAttestation {
    pub institution_id: String,
    pub epoch: u64,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionProjectionRequest {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub goal: String,
    pub semantic_profile: String,
    pub required_institution_ids: Vec<String>,
    pub attestations: Vec<PeerDecisionAttestation>,
    pub minimum_quorum: u16,
    pub current_epoch: u64,
    pub max_epoch_lag: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionProjectionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub goal: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub institution_order: Vec<String>,
    pub qualified_institution_order: Vec<String>,
    pub stale_institution_order: Vec<String>,
    pub blocked_institution_order: Vec<String>,
    pub unknown_institution_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub quorum: u16,
    pub minimum_quorum: u16,
    pub current_epoch: u64,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub federation_envelope_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedDecisionProjectionError {
    #[error("invalid federated decision projection: {0}")]
    Invalid(String),
    #[error("federated decision projection artifact failed: {0}")]
    Artifact(String),
}

impl FederatedDecisionProjectionReceipt {
    pub fn validate(&self) -> Result<(), FederatedDecisionProjectionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.institution_order.len() < 2
            || self.aggregate_order.iter().any(|value| value.len() != 64)
            || self.effect_receipts.is_empty()
            || self.disposition.is_empty()
        {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated projection identity, quorum, aggregate-only locality, or effects are incomplete".into(),
            ));
        }
        if self.minimum_quorum == 0
            || self.institution_order.len() > usize::from(u16::MAX)
            || usize::from(self.quorum) > self.institution_order.len()
        {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated quorum is invalid".into(),
            ));
        }
        for (values, field) in [
            (&self.institution_order, "institution_order"),
            (
                &self.qualified_institution_order,
                "qualified_institution_order",
            ),
            (&self.stale_institution_order, "stale_institution_order"),
            (&self.blocked_institution_order, "blocked_institution_order"),
            (&self.unknown_institution_order, "unknown_institution_order"),
            (&self.aggregate_order, "aggregate_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let institutions = self
            .institution_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = self
            .qualified_institution_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.stale_institution_order.iter().cloned());
        classified.extend(self.blocked_institution_order.iter().cloned());
        classified.extend(self.unknown_institution_order.iter().cloned());
        if classified != institutions {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated peer states do not partition institutions".into(),
            ));
        }
        if usize::from(self.quorum) != self.qualified_institution_order.len() {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated quorum does not match qualified peers".into(),
            ));
        }
        let groups = [
            &self.qualified_institution_order,
            &self.stale_institution_order,
            &self.blocked_institution_order,
            &self.unknown_institution_order,
        ];
        for (index, left) in groups.iter().enumerate() {
            for right in groups.iter().skip(index + 1) {
                if !identity_keys(left).is_disjoint(&identity_keys(right)) {
                    return Err(FederatedDecisionProjectionError::Invalid(
                        "federated peer states overlap by identity".into(),
                    ));
                }
            }
        }
        for digest in [
            &self.context_digest,
            &self.section_digest,
            &self.federation_envelope_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedDecisionProjectionError::Invalid(
                    "federated projection digest is invalid".into(),
                ));
            }
        }
        let gate_blocked = self
            .omissions
            .iter()
            .any(|item| item.ends_with(":federation-gate-blocked"));
        let expected_disposition = if !self.raw_data_local || gate_blocked {
            "blocked"
        } else if self.quorum >= self.minimum_quorum {
            "admitted"
        } else {
            "refinement_required"
        };
        if self.disposition != expected_disposition {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated projection disposition does not match quorum or locality".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition == "admitted" {
            vec![format!(
                "project:federated-decision-section:{}",
                self.federation_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated projection effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated decision receipts must declare local emitted data".into(),
            ));
        }
        let expected_context_digest = ContentHash::of_value(&json!({
            "institution_order": self.institution_order,
            "qualified_order": self.qualified_institution_order,
            "stale_order": self.stale_institution_order,
            "blocked_order": self.blocked_institution_order,
            "unknown_order": self.unknown_institution_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
        }))
        .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
        if self.context_digest != expected_context_digest {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated context digest is not bound to peer state".into(),
            ));
        }
        let expected_section_digest = ContentHash::of_value(&json!({
            "query_id": self.query_id,
            "goal": self.goal,
            "semantic_profile": self.semantic_profile,
            "aggregate_order": self.aggregate_order,
            "context_digest": self.context_digest,
            "quorum": self.quorum,
            "minimum_quorum": self.minimum_quorum,
            "current_epoch": self.current_epoch,
        }))
        .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
        if self.section_digest != expected_section_digest {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated section digest is not bound to projection state".into(),
            ));
        }
        let expected_envelope_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.goal,
            "aggregate_order": self.aggregate_order,
            "section_digest": self.section_digest,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
        }))
        .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
        if self.federation_envelope_digest != expected_envelope_digest {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federation envelope digest is not bound to release state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-decision-projection:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != FEDERATED_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated projection artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedDecisionProjectionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedDecisionProjectionError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedDecisionProjectionError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedDecisionProjectionError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedDecisionProjectionError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedDecisionProjectionError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedDecisionProjectionError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &FederatedDecisionProjectionReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "semantic_profile": receipt.semantic_profile,
        "disposition": receipt.disposition,
        "institution_order": receipt.institution_order,
        "qualified_institution_order": receipt.qualified_institution_order,
        "stale_institution_order": receipt.stale_institution_order,
        "blocked_institution_order": receipt.blocked_institution_order,
        "unknown_institution_order": receipt.unknown_institution_order,
        "aggregate_order": receipt.aggregate_order,
        "quorum": receipt.quorum,
        "minimum_quorum": receipt.minimum_quorum,
        "current_epoch": receipt.current_epoch,
        "context_digest": receipt.context_digest,
        "section_digest": receipt.section_digest,
        "federation_envelope_digest": receipt.federation_envelope_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "aggregate_only": receipt.aggregate_only,
        "boundary": receipt.boundary,
    })
}

pub fn federated_decision_projection_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["research workflow operator".into(), "federation steward".into(), "decision-section compiler".into()].into(),
        behavior: "projects fresh, policy-authorized, digest-only peer attestations into a quorum-gated federated Decision Section".into(),
        value: "enables continual consortium context projection without moving raw preclinical observations or inventing consensus under missing evidence".into(),
        inputs: vec![TypedPort { name: "federated_decision_projection_request".into(), schema: "FederatedDecisionProjectionRequest1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_decision_projection_receipt".into(), schema: "FederatedDecisionProjectionReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::FederationExport, Effect::WriteLocalArtifact].into(),
        permissions: ["project:federated-decision-section".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ro-crate-specification".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federated decision approver".into(), reason: "authorize purpose-bound digest-only projection only after freshness, quorum, policy, closure, locality, and replay gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn project_federated_decision_section(
    request: &FederatedDecisionProjectionRequest,
) -> Result<FederatedDecisionProjectionReceipt, FederatedDecisionProjectionError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_institution_ids.len() < 2
        || request.required_institution_ids.len() > usize::from(u16::MAX)
        || request.minimum_quorum == 0
        || usize::from(request.minimum_quorum) > request.required_institution_ids.len()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
        || !request.aggregate_only
    {
        return Err(FederatedDecisionProjectionError::Invalid(
            "federated projection identity, quorum, replay, or boundary is invalid".into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.federation_id, "federation_id"),
        (&request.query_id, "query_id"),
        (&request.goal, "goal"),
        (&request.semantic_profile, "semantic_profile"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(
        &request.required_institution_ids,
        "required_institution_ids",
    )?;
    let institution_order = request
        .required_institution_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if institution_order.len() != request.required_institution_ids.len()
        || institution_order.iter().any(|id| id.trim().is_empty())
    {
        return Err(FederatedDecisionProjectionError::Invalid(
            "federated institutions must be unique and non-empty".into(),
        ));
    }
    let mut attestation_map = BTreeMap::new();
    for attestation in &request.attestations {
        validate_text(&attestation.institution_id, "attestation.institution_id")?;
        validate_text(&attestation.boundary, "attestation.boundary")?;
        for digest in [
            &attestation.context_digest,
            &attestation.section_digest,
            &attestation.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedDecisionProjectionError::Invalid(
                    "federated attestation digest is invalid".into(),
                ));
            }
        }
        if attestation_map
            .insert(attestation.institution_id.clone(), attestation)
            .is_some()
        {
            return Err(FederatedDecisionProjectionError::Invalid(
                "federated attestations must be unique".into(),
            ));
        }
    }
    if attestation_map
        .keys()
        .any(|institution_id| !institution_order.contains(institution_id))
    {
        return Err(FederatedDecisionProjectionError::Invalid(
            "federated attestations must target required institutions".into(),
        ));
    }
    let attestation_ids = request
        .attestations
        .iter()
        .map(|attestation| attestation.institution_id.clone())
        .collect::<Vec<_>>();
    validate_unique(&attestation_ids, "attestation.institution_ids")?;
    let locality_gate = request.raw_data_local
        && request
            .attestations
            .iter()
            .all(|attestation| attestation.raw_data_local);
    let mut qualified = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for institution_id in &institution_order {
        let Some(peer) = attestation_map.get(institution_id) else {
            unknown.insert(institution_id.clone());
            omissions.insert(format!(
                "institution:{}:missing-attestation",
                institution_id
            ));
            continue;
        };
        if !request.policy_allow
            || !request.protected_closure
            || !request.signed_approval
            || !locality_gate
            || !request.aggregate_only
            || !peer.policy_allow
            || !peer.protected_closure
            || !peer.raw_data_local
            || !peer.aggregate_only
            || peer.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(institution_id.clone());
            omissions.insert(format!(
                "institution:{}:federation-gate-blocked",
                institution_id
            ));
            continue;
        }
        if peer.replay_identity != request.replay_identity {
            unknown.insert(institution_id.clone());
            uncertainty.insert(format!("institution:{}:replay-mismatch", institution_id));
            continue;
        }
        if peer.epoch > request.current_epoch
            || request.current_epoch.saturating_sub(peer.epoch) > request.max_epoch_lag
        {
            stale.insert(institution_id.clone());
            omissions.insert(format!("institution:{}:stale-epoch", institution_id));
            continue;
        }
        match peer.evidence_state {
            EvidenceState::Proven | EvidenceState::Supported => {
                qualified.insert(institution_id.clone());
                let digest = ContentHash::of_value(&json!({"institution_id": institution_id, "epoch": peer.epoch, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "replay_identity": peer.replay_identity}))
                    .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
                aggregate.insert(digest.to_string());
            }
            EvidenceState::Speculative | EvidenceState::Unknown => {
                unknown.insert(institution_id.clone());
                uncertainty.insert(format!("institution:{}:evidence-uncertain", institution_id));
            }
            EvidenceState::Contradicted => {
                blocked.insert(institution_id.clone());
                negative.insert(format!(
                    "institution:{}:contradicted-attestation",
                    institution_id
                ));
            }
        }
    }
    let quorum = u16::try_from(qualified.len()).map_err(|_| {
        FederatedDecisionProjectionError::Invalid(
            "federated qualified institution count exceeds the receipt quorum width".into(),
        )
    })?;
    if !locality_gate {
        omissions.insert("federation:raw-data-locality-failed".into());
    }
    let gates_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && locality_gate
        && request.aggregate_only;
    let disposition = if !gates_open {
        "blocked"
    } else if quorum >= request.minimum_quorum {
        "admitted"
    } else {
        "refinement_required"
    };
    if disposition != "admitted" && stale.is_empty() && unknown.is_empty() && blocked.is_empty() {
        omissions.insert("federation:quorum-not-reached".into());
    }
    let institution_order = institution_order.into_iter().collect::<Vec<_>>();
    let qualified_institution_order = qualified.into_iter().collect::<Vec<_>>();
    let stale_institution_order = stale.into_iter().collect::<Vec<_>>();
    let blocked_institution_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_institution_order = unknown.into_iter().collect::<Vec<_>>();
    let aggregate_order = aggregate.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let raw_data_local = true;
    let context_digest = ContentHash::of_value(&json!({
        "institution_order": institution_order,
        "qualified_order": qualified_institution_order,
        "stale_order": stale_institution_order,
        "blocked_order": blocked_institution_order,
        "unknown_order": unknown_institution_order,
        "replay_identity": request.replay_identity,
        "raw_data_local": raw_data_local,
        "aggregate_only": request.aggregate_only,
    }))
    .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
    let section_digest = ContentHash::of_value(&json!({
        "query_id": request.query_id,
        "goal": request.goal,
        "semantic_profile": request.semantic_profile,
        "aggregate_order": aggregate_order,
        "context_digest": context_digest,
        "quorum": quorum,
        "minimum_quorum": request.minimum_quorum,
        "current_epoch": request.current_epoch,
    }))
    .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
    let federation_envelope_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "purpose": request.goal,
        "aggregate_order": aggregate_order,
        "section_digest": section_digest,
        "raw_data_local": raw_data_local,
        "aggregate_only": request.aggregate_only,
    }))
    .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "admitted" {
        vec![format!(
            "project:federated-decision-section:{}",
            request.federation_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt_without_artifact = FederatedDecisionProjectionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        institution_order,
        qualified_institution_order,
        stale_institution_order,
        blocked_institution_order,
        unknown_institution_order,
        aggregate_order,
        quorum,
        minimum_quorum: request.minimum_quorum,
        current_epoch: request.current_epoch,
        context_digest,
        section_digest,
        federation_envelope_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact: TypedResearchArtifact::from_payload(
            "placeholder",
            FEDERATED_CONTENT_TYPE,
            &json!({}),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?,
        raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = receipt_payload(&receipt_without_artifact);
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-decision-projection:{}", request.request_id),
        FEDERATED_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedDecisionProjectionError::Artifact(error.to_string()))?;
    let receipt = FederatedDecisionProjectionReceipt {
        artifact,
        ..receipt_without_artifact
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
    fn request() -> FederatedDecisionProjectionRequest {
        FederatedDecisionProjectionRequest {
            request_id: "request:federated-decision".into(),
            federation_id: "federation:one".into(),
            query_id: "query:mechanism".into(),
            goal: "compare preclinical mechanism context".into(),
            semantic_profile: "aurora:decision:v1".into(),
            required_institution_ids: vec!["institution:a".into(), "institution:b".into()],
            attestations: vec![
                PeerDecisionAttestation {
                    institution_id: "institution:a".into(),
                    epoch: 10,
                    context_digest: hash("context-a"),
                    section_digest: hash("section-a"),
                    evidence_state: EvidenceState::Supported,
                    replay_identity: hash("replay"),
                    policy_allow: true,
                    protected_closure: true,
                    raw_data_local: true,
                    aggregate_only: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                PeerDecisionAttestation {
                    institution_id: "institution:b".into(),
                    epoch: 10,
                    context_digest: hash("context-b"),
                    section_digest: hash("section-b"),
                    evidence_state: EvidenceState::Proven,
                    replay_identity: hash("replay"),
                    policy_allow: true,
                    protected_closure: true,
                    raw_data_local: true,
                    aggregate_only: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
            ],
            minimum_quorum: 2,
            current_epoch: 10,
            max_epoch_lag: 1,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_authorized() {
        assert_eq!(
            federated_decision_projection_manifest().autonomy_tier,
            AutonomyTier::A2
        );
        assert_eq!(
            federated_decision_projection_manifest()
                .authority_requirements
                .len(),
            1
        );
    }
    #[test]
    fn quorum_admits_digest_only_projection() {
        let receipt = project_federated_decision_section(&request()).unwrap();
        assert_eq!(receipt.disposition, "admitted");
        assert_eq!(receipt.quorum, 2);
        assert!(receipt
            .aggregate_order
            .iter()
            .all(|value| value.len() == 64));
    }
    #[test]
    fn stale_peer_requires_refinement() {
        let mut value = request();
        value.attestations[1].epoch = 1;
        let receipt = project_federated_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "refinement_required");
        assert_eq!(receipt.stale_institution_order, vec!["institution:b"]);
    }
    #[test]
    fn missing_peer_is_explicit_unknown() {
        let mut value = request();
        value.attestations.pop();
        let receipt = project_federated_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "refinement_required");
        assert!(receipt
            .unknown_institution_order
            .contains(&"institution:b".into()));
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("missing-attestation")));
    }
    #[test]
    fn policy_denial_blocks_without_export() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = project_federated_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn non_local_peer_is_blocked_and_retained() {
        let mut value = request();
        value.attestations[1].raw_data_local = false;
        let receipt = project_federated_decision_section(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .contains(&"federation:raw-data-locality-failed".into()));
    }

    #[test]
    fn projection_artifact_payload_is_bound() {
        let mut receipt = project_federated_decision_section(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered");
        assert!(matches!(
            receipt.validate(),
            Err(FederatedDecisionProjectionError::Artifact(_))
        ));
    }

    #[test]
    fn contradiction_is_negative_evidence() {
        let mut value = request();
        value.attestations[1].evidence_state = EvidenceState::Contradicted;
        let receipt = project_federated_decision_section(&value).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted-attestation")));
        assert_eq!(receipt.disposition, "refinement_required");
    }
    #[test]
    fn digest_is_stable() {
        let receipt = project_federated_decision_section(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn institution_count_cannot_overflow_receipt_quorum() {
        let mut value = request();
        value.required_institution_ids = (0..=usize::from(u16::MAX))
            .map(|index| format!("institution:{index}"))
            .collect();
        assert!(matches!(
            project_federated_decision_section(&value),
            Err(FederatedDecisionProjectionError::Invalid(_))
        ));
    }
}
