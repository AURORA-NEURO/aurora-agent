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
            || !self.raw_data_local
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
        for values in [
            &self.required_evidence_order,
            &self.admitted_order,
            &self.contested_order,
            &self.missing_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omission_certificate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextOmissionAdjudicationError::Invalid(
                    "omission adjudication vectors are not canonical".into(),
                ));
            }
        }
        let required = self.required_evidence_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.admitted_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.contested_order.iter().cloned());
        classified.extend(self.missing_order.iter().cloned());
        classified.extend(self.blocked_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != required {
            return Err(ContextOmissionAdjudicationError::Invalid(
                "omission adjudication states do not partition required evidence".into(),
            ));
        }
        for digest in [&self.adjudication_digest, &self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ContextOmissionAdjudicationError::Invalid("omission adjudication digest is invalid".into()));
            }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("compile:local-omission-adjudication:") && effect != "block:unsafe-release") {
            return Err(ContextOmissionAdjudicationError::Invalid("omission adjudication effect is outside local compilation gate".into()));
        }
        self.artifact.validate_metadata().map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextOmissionAdjudicationError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))
    }
}

pub fn context_omission_adjudication_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["decision-section compiler".into(), "researcher".into(), "evidence auditor".into()].into(), behavior: "adjudicates required context evidence into admitted, contested, missing, blocked, and unknown states with individually addressable omission certificates".into(), value: "prevents contradictory or unmeasured evidence from being silently dropped by downstream research workflows".into(), inputs: vec![TypedPort { name: "context_omission_adjudication_request".into(), schema: "ContextOmissionAdjudicationRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_omission_adjudication_receipt".into(), schema: "ContextOmissionAdjudicationReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-omission-adjudication".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn adjudicate_context_omissions(request: &ContextOmissionAdjudicationRequest) -> Result<ContextOmissionAdjudicationReceipt, ContextOmissionAdjudicationError> {
    if request.request_id.trim().is_empty() || request.objective.trim().is_empty() || request.required_evidence_ids.is_empty() || request.boundary != PRECLINICAL_BOUNDARY || request.replay_identity.as_str().len() != 64 {
        return Err(ContextOmissionAdjudicationError::Invalid("omission adjudication identity, required evidence, replay, or boundary is invalid".into()));
    }
    let required = request.required_evidence_ids.iter().cloned().collect::<BTreeSet<_>>();
    if required.len() != request.required_evidence_ids.len() || required.iter().any(|value| value.trim().is_empty()) {
        return Err(ContextOmissionAdjudicationError::Invalid("required evidence identifiers must be unique and non-empty".into()));
    }
    let mut evidence = std::collections::BTreeMap::new();
    for value in &request.evidence {
        if evidence.insert(value.evidence_id.clone(), value).is_some() { return Err(ContextOmissionAdjudicationError::Invalid("evidence identifiers must be unique".into())); }
    }
    let mut admitted = BTreeSet::new(); let mut contested = BTreeSet::new(); let mut missing = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for id in &required {
        match evidence.get(id) {
            None => { missing.insert(id.clone()); omissions.insert(format!("evidence:{}:missing", id)); }
            Some(item) if !request.policy_allow || !request.protected_closure || !request.raw_data_local || !item.raw_data_local || !item.provenance_complete || item.boundary != PRECLINICAL_BOUNDARY => { blocked.insert(id.clone()); omissions.insert(format!("evidence:{}:policy-provenance-locality-blocked", id)); }
            Some(item) if item.replay_identity != request.replay_identity => { unknown.insert(id.clone()); uncertainty.insert(format!("evidence:{}:replay-mismatch", id)); }
            Some(item) if item.state == EvidenceState::Contradicted => { contested.insert(id.clone()); negative.insert(format!("evidence:{}:contradicted", id)); }
            Some(item) if item.state == EvidenceState::Supported && item.support_milli >= request.minimum_support_milli => { admitted.insert(id.clone()); }
            Some(item) if matches!(item.state, EvidenceState::Unknown | EvidenceState::Speculative) => { unknown.insert(id.clone()); uncertainty.insert(format!("evidence:{}:unresolved", id)); }
            Some(item) => { blocked.insert(id.clone()); omissions.insert(format!("evidence:{}:below-support-or-unproven", item.evidence_id)); }
        }
    }
    let mut certificates = BTreeSet::new();
    for value in omissions.iter().chain(uncertainty.iter()).chain(negative.iter()) { certificates.insert(format!("certificate:{}", value)); }
    if certificates.is_empty() { certificates.insert("certificate:none".into()); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ContextCompilationDisposition::Blocked } else if admitted.is_empty() { ContextCompilationDisposition::Unknown } else if admitted.len() == required.len() && omissions.is_empty() && uncertainty.is_empty() && negative.is_empty() { ContextCompilationDisposition::Qualified } else { ContextCompilationDisposition::Partial };
    let adjudication_digest = ContentHash::of_value(&json!({"required": required, "admitted": admitted, "contested": contested, "missing": missing, "blocked": blocked, "unknown": unknown, "certificates": certificates, "replay_identity": request.replay_identity})).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "adjudication_digest": adjudication_digest, "negative": negative})).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
    let effects = if matches!(disposition, ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial) { vec![format!("compile:local-omission-adjudication:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "disposition": disposition, "required_evidence_order": required, "admitted_order": admitted, "contested_order": contested, "missing_order": missing, "blocked_order": blocked, "unknown_order": unknown, "omission_certificate_order": certificates, "adjudication_digest": adjudication_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-context-omission-adjudication:{}", request.request_id), "application/vnd.aurora.context-omission-adjudication+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextOmissionAdjudicationError::Artifact(error.to_string()))?;
    let receipt = ContextOmissionAdjudicationReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), objective: request.objective.clone(), disposition, required_evidence_order: required.into_iter().collect(), admitted_order: admitted.into_iter().collect(), contested_order: contested.into_iter().collect(), missing_order: missing.into_iter().collect(), blocked_order: blocked.into_iter().collect(), unknown_order: unknown.into_iter().collect(), omission_certificate_order: certificates.into_iter().collect(), adjudication_digest, context_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: effects, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> ContextOmissionAdjudicationRequest { ContextOmissionAdjudicationRequest { request_id: "request:omission".into(), objective: "adjudicate context closure".into(), required_evidence_ids: vec!["evidence:a".into(), "evidence:b".into()], evidence: vec![ContextAdjudicationEvidence { evidence_id: "evidence:a".into(), state: EvidenceState::Supported, support_milli: 900, provenance_complete: true, replay_identity: hash("replay"), raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }, ContextAdjudicationEvidence { evidence_id: "evidence:b".into(), state: EvidenceState::Contradicted, support_milli: 0, provenance_complete: true, replay_identity: hash("replay"), raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }], minimum_support_milli: 700, replay_identity: hash("replay"), policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(context_omission_adjudication_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn contradiction_is_partial_with_certificate() { let receipt = adjudicate_context_omissions(&request()).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial); assert!(receipt.contested_order.contains(&"evidence:b".into())); assert!(!receipt.omission_certificate_order.is_empty()); }
    #[test] fn policy_denial_blocks() { let mut value = request(); value.policy_allow = false; let receipt = adjudicate_context_omissions(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked); }
    #[test] fn digest_is_stable() { let receipt = adjudicate_context_omissions(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
