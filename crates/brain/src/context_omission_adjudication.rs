//! Typed omission and conflict adjudication for compiled research context.
//!
//! Atlas feature: `AFA-brain-P03-F05`. This product makes every unresolved
//! evidence edge a consumable certificate instead of an implicit omission.

use crate::context_compilation::ContextCompilationDisposition;
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

pub const FEATURE_ID: &str = "AFA-brain-P03-F05";
pub const CONTRACT_VERSION: &str = "brain-context-omission-adjudication/1.0";
const ADJUDICATION_CONTENT_TYPE: &str = "application/vnd.aurora.context-omission-adjudication+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAdjudicationEvidence {
    pub evidence_id: String,
    pub state: EvidenceState,
    pub support_milli: u16,
    pub provenance_complete: bool,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextOmissionAdjudicationRequest {
    pub request_id: String,
    pub objective: String,
    pub required_evidence_ids: Vec<String>,
    pub evidence: Vec<ContextAdjudicationEvidence>,
    pub minimum_support_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextOmissionAdjudicationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub disposition: ContextCompilationDisposition,
    pub required_evidence_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub contested_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omission_certificate_order: Vec<String>,
    pub adjudication_digest: ContentHash,
    pub context_digest: ContentHash,
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
pub enum ContextOmissionAdjudicationError {
    #[error("invalid context omission adjudication request: {0}")]
    Invalid(String),
    #[error("context omission adjudication artifact failed: {0}")]
    Artifact(String),
}

impl ContextOmissionAdjudicationReceipt {
    pub fn validate(&self) -> Result<(), ContextOmissionAdjudicationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.required_evidence_order.is_empty()
            || self.omission_certificate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication identity, required evidence, certificates, locality, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.objective, "objective"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.required_evidence_order, "required_evidence_order"),
            (&self.admitted_order, "admitted_order"),
            (&self.contested_order, "contested_order"),
            (&self.missing_order, "missing_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (
                &self.omission_certificate_order,
                "omission_certificate_order",
            ),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let required = self
            .required_evidence_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = self.admitted_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.contested_order.iter().cloned());
        classified.extend(self.missing_order.iter().cloned());
        classified.extend(self.blocked_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != required
            || !identity_keys(&self.admitted_order)
                .is_disjoint(&identity_keys(&self.contested_order))
            || !identity_keys(&self.admitted_order).is_disjoint(&identity_keys(&self.missing_order))
            || !identity_keys(&self.admitted_order).is_disjoint(&identity_keys(&self.blocked_order))
            || !identity_keys(&self.admitted_order).is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.contested_order)
                .is_disjoint(&identity_keys(&self.missing_order))
            || !identity_keys(&self.contested_order)
                .is_disjoint(&identity_keys(&self.blocked_order))
            || !identity_keys(&self.contested_order)
                .is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.missing_order).is_disjoint(&identity_keys(&self.blocked_order))
            || !identity_keys(&self.missing_order).is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.blocked_order).is_disjoint(&identity_keys(&self.unknown_order))
        {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication states do not partition required evidence".into(),
            ));
        }
        for digest in [
            &self.adjudication_digest,
            &self.context_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextOmissionAdjudicationError::Invalid(
                    "omission adjudication digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("compile:local-omission-adjudication:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication effect is outside local compilation gate".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
        ) {
            vec![format!(
                "compile:local-omission-adjudication:{}",
                self.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication receipts must declare local emitted data".into(),
            ));
        }
        let expected_adjudication_digest = ContentHash::of_value(&json!({
            "required_evidence_order": self.required_evidence_order,
            "admitted_order": self.admitted_order,
            "contested_order": self.contested_order,
            "missing_order": self.missing_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "omission_certificate_order": self.omission_certificate_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
        if self.adjudication_digest != expected_adjudication_digest {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "adjudication digest is not bound to evidence outcomes".into(),
            ));
        }
        let expected_context_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "disposition": self.disposition,
            "adjudication_digest": self.adjudication_digest,
            "negative_evidence": self.negative_evidence,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
        if self.context_digest != expected_context_digest {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "adjudication context digest is not bound to result state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-context-omission-adjudication:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != ADJUDICATION_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextOmissionAdjudicationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), ContextOmissionAdjudicationError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ContextOmissionAdjudicationError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ContextOmissionAdjudicationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextOmissionAdjudicationError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ContextOmissionAdjudicationError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContextOmissionAdjudicationError::Invalid(format!(
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

fn receipt_payload(receipt: &ContextOmissionAdjudicationReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "objective": receipt.objective,
        "disposition": receipt.disposition,
        "required_evidence_order": receipt.required_evidence_order,
        "admitted_order": receipt.admitted_order,
        "contested_order": receipt.contested_order,
        "missing_order": receipt.missing_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "omission_certificate_order": receipt.omission_certificate_order,
        "adjudication_digest": receipt.adjudication_digest,
        "context_digest": receipt.context_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn context_omission_adjudication_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["decision-section compiler".into(), "researcher".into(), "evidence auditor".into()].into(), behavior: "adjudicates required context evidence into admitted, contested, missing, blocked, and unknown states with individually addressable omission certificates".into(), value: "prevents contradictory or unmeasured evidence from being silently dropped by downstream research workflows".into(), inputs: vec![TypedPort { name: "context_omission_adjudication_request".into(), schema: "ContextOmissionAdjudicationRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_omission_adjudication_receipt".into(), schema: "ContextOmissionAdjudicationReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-omission-adjudication".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn adjudicate_context_omissions(
    request: &ContextOmissionAdjudicationRequest,
) -> Result<ContextOmissionAdjudicationReceipt, ContextOmissionAdjudicationError> {
    if request.request_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.required_evidence_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ContextOmissionAdjudicationError::Invalid(
            "omission adjudication identity, required evidence, replay, or boundary is invalid"
                .into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.objective, "objective"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.required_evidence_ids, "required_evidence_ids")?;
    let required = request
        .required_evidence_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != request.required_evidence_ids.len() {
        return Err(ContextOmissionAdjudicationError::Invalid(
            "required evidence identifiers must be unique and non-empty".into(),
        ));
    }
    let mut evidence = std::collections::BTreeMap::new();
    for value in &request.evidence {
        validate_text(&value.evidence_id, "evidence.evidence_id")?;
        validate_text(&value.boundary, "evidence.boundary")?;
        if value.replay_identity.as_str().len() != 64 {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "evidence replay identity must be 64 characters".into(),
            ));
        }
        if evidence.insert(value.evidence_id.clone(), value).is_some() {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "evidence identifiers must be unique".into(),
            ));
        }
    }
    let mut admitted = BTreeSet::new();
    let mut contested = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let locality_failure =
        !request.raw_data_local || request.evidence.iter().any(|item| !item.raw_data_local);
    for id in &required {
        match evidence.get(id) {
            None => {
                missing.insert(id.clone());
                omissions.insert(format!("evidence:{}:missing", id));
            }
            Some(item)
                if !request.policy_allow
                    || !request.protected_closure
                    || !request.raw_data_local
                    || !item.raw_data_local
                    || !item.provenance_complete
                    || item.boundary != PRECLINICAL_BOUNDARY =>
            {
                blocked.insert(id.clone());
                omissions.insert(format!(
                    "evidence:{}:policy-provenance-locality-blocked",
                    id
                ));
            }
            Some(item) if item.replay_identity != request.replay_identity => {
                unknown.insert(id.clone());
                uncertainty.insert(format!("evidence:{}:replay-mismatch", id));
            }
            Some(item) if item.state == EvidenceState::Contradicted => {
                contested.insert(id.clone());
                negative.insert(format!("evidence:{}:contradicted", id));
            }
            Some(item)
                if item.state == EvidenceState::Supported
                    && item.support_milli >= request.minimum_support_milli =>
            {
                admitted.insert(id.clone());
            }
            Some(item)
                if matches!(
                    item.state,
                    EvidenceState::Unknown | EvidenceState::Speculative
                ) =>
            {
                unknown.insert(id.clone());
                uncertainty.insert(format!("evidence:{}:unresolved", id));
            }
            Some(item) => {
                blocked.insert(id.clone());
                omissions.insert(format!(
                    "evidence:{}:below-support-or-unproven",
                    item.evidence_id
                ));
            }
        }
    }
    let mut certificates = BTreeSet::new();
    for value in omissions
        .iter()
        .chain(uncertainty.iter())
        .chain(negative.iter())
    {
        certificates.insert(format!("certificate:{}", value));
    }
    if certificates.is_empty() {
        certificates.insert("certificate:none".into());
    }
    if locality_failure {
        omissions.insert("context:raw-data-locality-failed".into());
        certificates.insert("certificate:context:raw-data-locality-failed".into());
    }
    let disposition = if !request.policy_allow || !request.protected_closure || locality_failure {
        ContextCompilationDisposition::Blocked
    } else if admitted.is_empty() {
        ContextCompilationDisposition::Unknown
    } else if admitted.len() == required.len()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        ContextCompilationDisposition::Qualified
    } else {
        ContextCompilationDisposition::Partial
    };
    let required_evidence_order = required.into_iter().collect::<Vec<_>>();
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let contested_order = contested.into_iter().collect::<Vec<_>>();
    let missing_order = missing.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omission_certificate_order = certificates.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let raw_data_local = true;
    let adjudication_digest = ContentHash::of_value(&json!({"required_evidence_order": required_evidence_order, "admitted_order": admitted_order, "contested_order": contested_order, "missing_order": missing_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "omission_certificate_order": omission_certificate_order, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "disposition": disposition, "adjudication_digest": adjudication_digest, "negative_evidence": negative_evidence, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
    let effects = if matches!(
        disposition,
        ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial
    ) {
        vec![format!(
            "compile:local-omission-adjudication:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "disposition": disposition, "required_evidence_order": required_evidence_order, "admitted_order": admitted_order, "contested_order": contested_order, "missing_order": missing_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "omission_certificate_order": omission_certificate_order, "adjudication_digest": adjudication_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "effect_receipts": effects, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-omission-adjudication:{}", request.request_id),
        ADJUDICATION_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
    let receipt = ContextOmissionAdjudicationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective: request.objective.clone(),
        disposition,
        required_evidence_order,
        admitted_order,
        contested_order,
        missing_order,
        blocked_order,
        unknown_order,
        omission_certificate_order,
        adjudication_digest,
        context_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts: effects,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
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
    fn request() -> ContextOmissionAdjudicationRequest {
        ContextOmissionAdjudicationRequest {
            request_id: "request:omission".into(),
            objective: "adjudicate context closure".into(),
            required_evidence_ids: vec!["evidence:a".into(), "evidence:b".into()],
            evidence: vec![
                ContextAdjudicationEvidence {
                    evidence_id: "evidence:a".into(),
                    state: EvidenceState::Supported,
                    support_milli: 900,
                    provenance_complete: true,
                    replay_identity: hash("replay"),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                ContextAdjudicationEvidence {
                    evidence_id: "evidence:b".into(),
                    state: EvidenceState::Contradicted,
                    support_milli: 0,
                    provenance_complete: true,
                    replay_identity: hash("replay"),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
            ],
            minimum_support_milli: 700,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            context_omission_adjudication_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn contradiction_is_partial_with_certificate() {
        let receipt = adjudicate_context_omissions(&request()).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial);
        assert!(receipt.contested_order.contains(&"evidence:b".into()));
        assert!(!receipt.omission_certificate_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = adjudicate_context_omissions(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = adjudicate_context_omissions(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn non_local_evidence_is_blocked_and_retained() {
        let mut value = request();
        value.evidence[0].raw_data_local = false;
        let receipt = adjudicate_context_omissions(&value).unwrap();
        assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "context:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn adjudication_artifact_payload_is_bound() {
        let mut receipt = adjudicate_context_omissions(&request()).unwrap();
        receipt.objective = "tampered objective".into();
        assert!(receipt.validate().is_err());
    }
}
