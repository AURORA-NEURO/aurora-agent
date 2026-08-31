//! Local single-study evidence operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P01-F29`. This capability owns the operator-facing lifecycle around
//! evidence surveillance: bounded budget, checkpoint, retry, telemetry, and recovery receipts.
//! It does not turn operational completion into scientific qualification.

use crate::evidence_surveillance::{
    surveil_evidence, EvidenceFeedRequest, EvidenceSurveillanceDisposition,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F29";
pub const CONTRACT_VERSION: &str = "brain-evidence-operations-control-plane/1.0";
const OPERATIONS_CONTENT_TYPE: &str = "application/vnd.aurora.evidence-operations+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceOperationsRequest {
    pub request: EvidenceFeedRequest,
    pub operation_id: String,
    pub actor_id: String,
    pub budget_units: u32,
    pub retry_budget: u16,
    pub checkpoint_interval: u16,
    pub telemetry_enabled: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub operation_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub disposition: OperationsDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub attempts: u16,
    pub recovered: bool,
    pub budget_units: u32,
    pub retry_budget: u16,
    pub checkpoint_interval: u16,
    pub telemetry_enabled: bool,
    pub telemetry_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub operations_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceOperationsError {
    #[error("invalid evidence operations request: {0}")]
    Invalid(String),
    #[error("evidence operations artifact failed: {0}")]
    Artifact(String),
    #[error("evidence operations engine failed: {0}")]
    Engine(String),
}

impl EvidenceOperationsReceipt {
    pub fn validate(&self) -> Result<(), EvidenceOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.operation_id.trim().is_empty()
            || self.actor_id.trim().is_empty()
            || self.request_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.attempts == 0
            || self.budget_units == 0
            || self.checkpoint_interval == 0
            || !self.telemetry_enabled
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceOperationsError::Invalid(
                "operations identity, run budget, candidate state, or effects are incomplete"
                    .into(),
            ));
        }
        for (value, field) in [
            (&self.operation_id, "operation_id"),
            (&self.actor_id, "actor_id"),
            (&self.request_id, "request_id"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.candidate_order, "candidate_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        let classified = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut admitted_or_blocked = classified.clone();
        admitted_or_blocked.extend(self.blocked_order.iter().cloned());
        if admitted_or_blocked
            != self
                .candidate_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
        {
            return Err(EvidenceOperationsError::Invalid(
                "operations states must partition candidates without overlap".into(),
            ));
        }
        for digest in [
            &self.telemetry_digest,
            &self.evidence_digest,
            &self.operations_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(EvidenceOperationsError::Invalid(
                    "operations digest is invalid".into(),
                ));
            }
        }
        let gate_denied = self
            .omissions
            .iter()
            .any(|omission| omission.starts_with("ops:"));
        let expected_disposition = if gate_denied || !self.raw_data_local {
            OperationsDisposition::Denied
        } else if self.qualified_order.is_empty() {
            OperationsDisposition::Unresolved
        } else if self.blocked_order.is_empty()
            && self.omissions.is_empty()
            && self.uncertainty.is_empty()
            && self.negative_evidence.is_empty()
        {
            OperationsDisposition::Completed
        } else {
            OperationsDisposition::Degraded
        };
        if self.disposition != expected_disposition {
            return Err(EvidenceOperationsError::Invalid(
                "operations disposition does not match candidate state or gates".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(EvidenceOperationsError::Invalid(
                "operations receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition != OperationsDisposition::Denied {
            vec![format!("ops:local-evidence:{}", self.operation_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(EvidenceOperationsError::Invalid(
                "operations effect does not match disposition".into(),
            ));
        }
        let expected_telemetry_digest = ContentHash::of_value(&json!({
            "operation_id": self.operation_id,
            "actor_id": self.actor_id,
            "attempts": self.attempts,
            "checkpoint_seq": self.checkpoint_seq,
            "recovered": self.recovered,
            "disposition": self.disposition,
        }))
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
        if self.telemetry_digest != expected_telemetry_digest {
            return Err(EvidenceOperationsError::Invalid(
                "operations telemetry digest is not bound to run state".into(),
            ));
        }
        let expected_operations_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "operation_id": self.operation_id,
            "request_id": self.request_id,
            "budget_units": self.budget_units,
            "retry_budget": self.retry_budget,
            "checkpoint_interval": self.checkpoint_interval,
            "telemetry_digest": self.telemetry_digest,
            "evidence_digest": self.evidence_digest,
            "replay_identity": self.replay_identity,
            "disposition": self.disposition,
        }))
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
        if self.operations_digest != expected_operations_digest {
            return Err(EvidenceOperationsError::Invalid(
                "operations digest is not bound to operational state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-evidence-operations:{}", self.operation_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != OPERATIONS_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(EvidenceOperationsError::Invalid(
                "operations artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), EvidenceOperationsError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(EvidenceOperationsError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), EvidenceOperationsError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(EvidenceOperationsError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), EvidenceOperationsError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceOperationsError::Invalid(format!(
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

fn receipt_payload(receipt: &EvidenceOperationsReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "operation_id": receipt.operation_id,
        "actor_id": receipt.actor_id,
        "request_id": receipt.request_id,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "checkpoint_seq": receipt.checkpoint_seq,
        "attempts": receipt.attempts,
        "recovered": receipt.recovered,
        "budget_units": receipt.budget_units,
        "retry_budget": receipt.retry_budget,
        "checkpoint_interval": receipt.checkpoint_interval,
        "telemetry_enabled": receipt.telemetry_enabled,
        "telemetry_digest": receipt.telemetry_digest,
        "evidence_digest": receipt.evidence_digest,
        "operations_digest": receipt.operations_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn evidence_operations_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "institution-local operations service".into()].into(), behavior: "operates one local evidence-surveillance run with budget, retry, checkpoint, telemetry, recovery, and explicit scientific disposition".into(), value: "makes evidence operations observable and recoverable without promoting operational completion into a scientific pass".into(), inputs: vec![TypedPort { name: "evidence_operations_request".into(), schema: "EvidenceOperationsRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "evidence_operations_receipt".into(), schema: "QualifiedEvidenceSet8@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["operate:local-evidence".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_evidence(
    request: &EvidenceOperationsRequest,
) -> Result<EvidenceOperationsReceipt, EvidenceOperationsError> {
    validate_request(request)?;
    let evidence = surveil_evidence(&request.request)
        .map_err(|error| EvidenceOperationsError::Engine(error.to_string()))?;
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let policy_allow = request.policy_allow && request.request.policy_allow;
    let protected_closure = request.protected_closure && request.request.protected_closure;
    let locality_gate = request.raw_data_local
        && request.request.raw_data_local
        && request
            .request
            .observations
            .iter()
            .all(|observation| observation.raw_data_local);
    let allowed = policy_allow && protected_closure && locality_gate;
    if !policy_allow {
        omissions.insert("ops:policy-denied".into());
    }
    if !protected_closure {
        omissions.insert("ops:protected-closure-incomplete".into());
    }
    if !locality_gate {
        omissions.insert("ops:raw-data-locality-failed".into());
    }
    let disposition = if !allowed {
        OperationsDisposition::Denied
    } else {
        match evidence.disposition {
            EvidenceSurveillanceDisposition::Qualified => OperationsDisposition::Completed,
            EvidenceSurveillanceDisposition::Partial => OperationsDisposition::Degraded,
            EvidenceSurveillanceDisposition::Unknown => OperationsDisposition::Unresolved,
            EvidenceSurveillanceDisposition::Blocked => OperationsDisposition::Denied,
        }
    };
    let checkpoint_seq = if evidence.candidate_order.is_empty() {
        0
    } else {
        1
    };
    let attempts = 1;
    let recovered = false;
    let telemetry_digest = ContentHash::of_value(&json!({"operation_id": request.operation_id, "actor_id": request.actor_id, "attempts": attempts, "checkpoint_seq": checkpoint_seq, "recovered": recovered, "disposition": disposition}))
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let evidence_digest = evidence
        .digest()
        .map_err(|error| EvidenceOperationsError::Engine(error.to_string()))?;
    let operations_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "operation_id": request.operation_id, "request_id": request.request.request_id, "budget_units": request.budget_units, "retry_budget": request.retry_budget, "checkpoint_interval": request.checkpoint_interval, "telemetry_digest": telemetry_digest, "evidence_digest": evidence_digest, "replay_identity": request.request.replay_identity, "disposition": disposition}))
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let effect_receipts = if allowed {
        vec![format!("ops:local-evidence:{}", request.operation_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let raw_data_local = true;
    let receipt_without_artifact = EvidenceOperationsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        operation_id: request.operation_id.clone(),
        actor_id: request.actor_id.clone(),
        request_id: request.request.request_id.clone(),
        disposition,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        checkpoint_seq,
        attempts,
        recovered,
        budget_units: request.budget_units,
        retry_budget: request.retry_budget,
        checkpoint_interval: request.checkpoint_interval,
        telemetry_enabled: request.telemetry_enabled,
        telemetry_digest,
        evidence_digest,
        operations_digest,
        replay_identity: request.request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact: TypedResearchArtifact::from_payload(
            "placeholder",
            OPERATIONS_CONTENT_TYPE,
            &json!({}),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = receipt_payload(&receipt_without_artifact);
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-operations:{}", request.operation_id),
        OPERATIONS_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let receipt = EvidenceOperationsReceipt {
        artifact,
        ..receipt_without_artifact
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &EvidenceOperationsRequest) -> Result<(), EvidenceOperationsError> {
    if request.operation_id.trim().is_empty()
        || request.actor_id.trim().is_empty()
        || request.budget_units == 0
        || request.checkpoint_interval == 0
        || !request.telemetry_enabled
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceOperationsError::Invalid(
            "operations actor, budget, checkpoint, telemetry, or boundary is incomplete".into(),
        ));
    }
    for (value, field) in [
        (&request.operation_id, "operation_id"),
        (&request.actor_id, "actor_id"),
        (&request.boundary, "boundary"),
        (&request.request.request_id, "request_id"),
        (&request.request.study_id, "study_id"),
        (&request.request.scope, "scope"),
        (&request.request.query, "query"),
        (&request.request.boundary, "request.boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(
        &request
            .request
            .observations
            .iter()
            .map(|observation| observation.evidence_id.clone())
            .collect::<Vec<_>>(),
        "observation.evidence_ids",
    )?;
    for observation in &request.request.observations {
        for (value, field) in [
            (&observation.evidence_id, "observation.evidence_id"),
            (&observation.source_id, "observation.source_id"),
            (&observation.study_id, "observation.study_id"),
            (&observation.modality, "observation.modality"),
            (&observation.scope, "observation.scope"),
            (&observation.boundary, "observation.boundary"),
        ] {
            validate_text(value, field)?;
        }
        for digest in [
            &observation.semantic_digest,
            &observation.artifact_digest,
            &observation.provenance_digest,
            &observation.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(EvidenceOperationsError::Invalid(
                    "observation digest is invalid".into(),
                ));
            }
        }
    }
    if request.request.replay_identity.as_str().len() != 64 {
        return Err(EvidenceOperationsError::Invalid(
            "operations replay identity is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> EvidenceOperationsRequest {
        EvidenceOperationsRequest {
            request: EvidenceFeedRequest {
                request_id: "request:ops".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic morphology".into(),
                minimum_relevance_milli: 700,
                observations: vec![EvidenceObservation {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:organoid".into(),
                    modality: "imaging".into(),
                    scope: "organoid:neural".into(),
                    relevance_milli: 900,
                    state,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operation_id: "operation:local-evidence".into(),
            actor_id: "actor:operator".into(),
            budget_units: 100,
            retry_budget: 2,
            checkpoint_interval: 1,
            telemetry_enabled: true,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = evidence_operations_control_plane_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_completes() {
        let r = operate_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Completed);
        assert_eq!(r.checkpoint_seq, 1);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_evidence(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Unresolved);
    }
    #[test]
    fn policy_denies() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = operate_evidence(&q).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Denied);
    }
    #[test]
    fn budget_is_required() {
        let mut q = request(EvidenceState::Supported);
        q.budget_units = 0;
        assert!(operate_evidence(&q).is_err());
    }
    #[test]
    fn nested_locality_failure_is_denied_and_retained() {
        let mut q = request(EvidenceState::Supported);
        q.request.observations[0].raw_data_local = false;
        let r = operate_evidence(&q).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Denied);
        assert!(r.raw_data_local);
        assert!(r.omissions.contains(&"ops:raw-data-locality-failed".into()));
    }

    #[test]
    fn artifact_payload_is_bound() {
        let mut r = operate_evidence(&request(EvidenceState::Supported)).unwrap();
        r.artifact.content_hash = hash("tampered");
        assert!(matches!(
            r.validate(),
            Err(EvidenceOperationsError::Artifact(_))
        ));
    }

    #[test]
    fn digest_is_stable() {
        let r = operate_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
