//! Uncertainty-envelope compilation for typed research context.
//!
//! Atlas feature: `AFA-brain-P03-F08`. Confidence and interval width are
//! release predicates; the compiler never turns an unknown or wide interval
//! into a qualified conclusion.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F08";
pub const CONTRACT_VERSION: &str = "brain-context-uncertainty-envelope/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUncertaintyObservation {
    pub evidence_id: String,
    pub estimate_milli: u16,
    pub lower_milli: u16,
    pub upper_milli: u16,
    pub confidence_milli: u16,
    pub state: EvidenceState,
    pub provenance_complete: bool,
    pub evidence_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUncertaintyEnvelopeRequest {
    pub request_id: String,
    pub objective: String,
    pub required_evidence_ids: Vec<String>,
    pub observations: Vec<ContextUncertaintyObservation>,
    pub minimum_confidence_milli: u16,
    pub maximum_interval_width_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUncertaintyEnvelopeReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub disposition: ContextCompilationDisposition,
    pub required_evidence_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub interval_width_order: Vec<String>,
    pub uncertainty_digest: ContentHash,
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
pub enum ContextUncertaintyEnvelopeError {
    #[error("invalid context uncertainty envelope request: {0}")]
    Invalid(String),
    #[error("context uncertainty envelope artifact failed: {0}")]
    Artifact(String),
}

impl ContextUncertaintyEnvelopeReceipt {
    pub fn validate(&self) -> Result<(), ContextUncertaintyEnvelopeError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.required_evidence_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContextUncertaintyEnvelopeError::Invalid(
                "uncertainty envelope identity, required evidence, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.required_evidence_order,
            &self.qualified_order,
            &self.uncertain_order,
            &self.missing_order,
            &self.blocked_order,
            &self.interval_width_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextUncertaintyEnvelopeError::Invalid(
                    "uncertainty envelope vectors are not canonical".into(),
                ));
            }
        }
        let required = self.required_evidence_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.qualified_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.uncertain_order.iter().cloned());
        classified.extend(self.missing_order.iter().cloned());
        classified.extend(self.blocked_order.iter().cloned());
        if classified != required {
            return Err(ContextUncertaintyEnvelopeError::Invalid(
                "uncertainty envelope states do not partition required evidence".into(),
            ));
        }
        for digest in [&self.uncertainty_digest, &self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ContextUncertaintyEnvelopeError::Invalid("uncertainty envelope digest is invalid".into()));
            }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("compile:local-uncertainty-envelope:") && effect != "block:unsafe-release") {
            return Err(ContextUncertaintyEnvelopeError::Invalid("uncertainty envelope effect is outside local compilation gate".into()));
        }
        self.artifact.validate_metadata().map_err(|error| ContextUncertaintyEnvelopeError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextUncertaintyEnvelopeError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextUncertaintyEnvelopeError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextUncertaintyEnvelopeError::Artifact(error.to_string()))
    }
}

pub fn context_uncertainty_envelope_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["decision-section compiler".into(), "statistical analyst".into(), "researcher".into()].into(), behavior: "compiles confidence and interval-width envelopes for required context evidence with deterministic replay-bound uncertainty receipts".into(), value: "keeps wide, low-confidence, unknown, and contradicted evidence visible instead of converting uncertainty into a confident context conclusion".into(), inputs: vec![TypedPort { name: "context_uncertainty_envelope_request".into(), schema: "ContextUncertaintyEnvelopeRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_uncertainty_envelope_receipt".into(), schema: "ContextUncertaintyEnvelopeReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-uncertainty-envelope".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_context_uncertainty_envelope(request: &ContextUncertaintyEnvelopeRequest) -> Result<ContextUncertaintyEnvelopeReceipt, ContextUncertaintyEnvelopeError> {
    if request.request_id.trim().is_empty() || request.objective.trim().is_empty() || request.required_evidence_ids.is_empty() || request.maximum_interval_width_milli == 0 || request.boundary != PRECLINICAL_BOUNDARY || request.replay_identity.as_str().len() != 64 {
        return Err(ContextUncertaintyEnvelopeError::Invalid("uncertainty envelope identity, thresholds, replay, or boundary is invalid".into()));
    }
    let required = request.required_evidence_ids.iter().cloned().collect::<BTreeSet<_>>();
    if required.len() != request.required_evidence_ids.len() || required.iter().any(|value| value.trim().is_empty()) { return Err(ContextUncertaintyEnvelopeError::Invalid("required evidence identifiers must be unique and non-empty".into())); }
    let mut observations = std::collections::BTreeMap::new();
    for item in &request.observations {
        if item.lower_milli > item.estimate_milli || item.estimate_milli > item.upper_milli || item.upper_milli - item.lower_milli > 1000 || item.confidence_milli > 1000 { return Err(ContextUncertaintyEnvelopeError::Invalid(format!("observation {} has an invalid interval or confidence", item.evidence_id))); }
        if observations.insert(item.evidence_id.clone(), item).is_some() { return Err(ContextUncertaintyEnvelopeError::Invalid("uncertainty evidence identifiers must be unique".into())); }
    }
    let mut qualified = BTreeSet::new(); let mut uncertain = BTreeSet::new(); let mut missing = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut widths = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for id in &required {
        match observations.get(id) {
            None => { missing.insert(id.clone()); omissions.insert(format!("evidence:{}:missing", id)); }
            Some(item) if !request.policy_allow || !request.protected_closure || !request.raw_data_local || !item.raw_data_local || !item.provenance_complete || item.boundary != PRECLINICAL_BOUNDARY => { blocked.insert(id.clone()); omissions.insert(format!("evidence:{}:policy-provenance-locality-blocked", id)); }
            Some(item) if item.replay_identity != request.replay_identity => { uncertain.insert(id.clone()); uncertainty.insert(format!("evidence:{}:replay-mismatch", id)); }
            Some(item) if item.state == EvidenceState::Contradicted => { uncertain.insert(id.clone()); negative.insert(format!("evidence:{}:contradicted", id)); }
            Some(item) if item.state == EvidenceState::Supported && item.confidence_milli >= request.minimum_confidence_milli && item.upper_milli - item.lower_milli <= request.maximum_interval_width_milli => { qualified.insert(id.clone()); widths.insert(format!("{}:{}", id, item.upper_milli - item.lower_milli)); }
            Some(item) if matches!(item.state, EvidenceState::Unknown | EvidenceState::Speculative) => { uncertain.insert(id.clone()); uncertainty.insert(format!("evidence:{}:unresolved", id)); widths.insert(format!("{}:{}", id, item.upper_milli - item.lower_milli)); }
            Some(item) => { uncertain.insert(id.clone()); uncertainty.insert(format!("evidence:{}:confidence-or-interval-too-wide", id)); widths.insert(format!("{}:{}", id, item.upper_milli - item.lower_milli)); }
        }
    }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ContextCompilationDisposition::Blocked } else if qualified.is_empty() { ContextCompilationDisposition::Unknown } else if qualified.len() == required.len() && uncertain.is_empty() && missing.is_empty() && blocked.is_empty() && omissions.is_empty() && uncertainty.is_empty() && negative.is_empty() { ContextCompilationDisposition::Qualified } else { ContextCompilationDisposition::Partial };
    let uncertainty_digest = ContentHash::of_value(&json!({"required": required, "qualified": qualified, "uncertain": uncertain, "missing": missing, "blocked": blocked, "interval_width_order": widths, "replay_identity": request.replay_identity})).map_err(|error| ContextUncertaintyEnvelopeError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "uncertainty_digest": uncertainty_digest, "negative": negative})).map_err(|error| ContextUncertaintyEnvelopeError::Artifact(error.to_string()))?;
    let effects = if matches!(disposition, ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial) { vec![format!("compile:local-uncertainty-envelope:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "disposition": disposition, "required_evidence_order": required, "qualified_order": qualified, "uncertain_order": uncertain, "missing_order": missing, "blocked_order": blocked, "interval_width_order": widths, "uncertainty_digest": uncertainty_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-context-uncertainty-envelope:{}", request.request_id), "application/vnd.aurora.context-uncertainty-envelope+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextUncertaintyEnvelopeError::Artifact(error.to_string()))?;
    let receipt = ContextUncertaintyEnvelopeReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), objective: request.objective.clone(), disposition, required_evidence_order: required.into_iter().collect(), qualified_order: qualified.into_iter().collect(), uncertain_order: uncertain.into_iter().collect(), missing_order: missing.into_iter().collect(), blocked_order: blocked.into_iter().collect(), interval_width_order: widths.into_iter().collect(), uncertainty_digest, context_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: effects, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> ContextUncertaintyEnvelopeRequest { ContextUncertaintyEnvelopeRequest { request_id: "request:uncertainty".into(), objective: "compile uncertainty envelope".into(), required_evidence_ids: vec!["evidence:a".into()], observations: vec![ContextUncertaintyObservation { evidence_id: "evidence:a".into(), estimate_milli: 700, lower_milli: 650, upper_milli: 750, confidence_milli: 900, state: EvidenceState::Supported, provenance_complete: true, evidence_digest: hash("evidence"), replay_identity: hash("replay"), raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }], minimum_confidence_milli: 700, maximum_interval_width_milli: 200, replay_identity: hash("replay"), policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(context_uncertainty_envelope_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn narrow_supported_interval_qualifies() { let receipt = compile_context_uncertainty_envelope(&request()).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Qualified); }
    #[test] fn wide_interval_is_partial() { let mut value = request(); value.observations[0].upper_milli = 1000; let receipt = compile_context_uncertainty_envelope(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial); assert!(!receipt.uncertain_order.is_empty()); }
    #[test] fn policy_denial_blocks() { let mut value = request(); value.policy_allow = false; let receipt = compile_context_uncertainty_envelope(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked); }
    #[test] fn digest_is_stable() { let receipt = compile_context_uncertainty_envelope(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
