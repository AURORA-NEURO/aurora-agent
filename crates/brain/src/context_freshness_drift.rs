//! Freshness and semantic-drift evaluation for compiled research context.
//!
//! Atlas feature: `AFA-brain-P03-F07`. A context digest can be perfectly
//! reproducible and still be too old or semantically changed for a new release;
//! this product makes those distinctions typed and replayable.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F07";
pub const CONTRACT_VERSION: &str = "brain-context-freshness-drift/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub snapshot_id: String,
    pub source_digest: ContentHash,
    pub schema_digest: ContentHash,
    pub semantics_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub observed_at_epoch: u64,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFreshnessDriftRequest {
    pub request_id: String,
    pub objective: String,
    pub baseline: ContextSnapshot,
    pub candidate: ContextSnapshot,
    pub now_epoch: u64,
    pub max_age_seconds: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFreshnessDriftReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub disposition: String,
    pub changed_dimension_order: Vec<String>,
    pub freshness_age_seconds: u64,
    pub baseline_digest: ContentHash,
    pub candidate_digest: ContentHash,
    pub drift_digest: ContentHash,
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
pub enum ContextFreshnessDriftError {
    #[error("invalid context freshness/drift request: {0}")]
    Invalid(String),
    #[error("context freshness/drift artifact failed: {0}")]
    Artifact(String),
}

impl ContextFreshnessDriftReceipt {
    pub fn validate(&self) -> Result<(), ContextFreshnessDriftError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(self.disposition.as_str(), "fresh" | "drifted" | "stale" | "unknown" | "blocked")
        {
            return Err(ContextFreshnessDriftError::Invalid("freshness/drift identity, locality, disposition, or effects are incomplete".into()));
        }
        for values in [&self.changed_dimension_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) { return Err(ContextFreshnessDriftError::Invalid("freshness/drift vectors are not canonical".into())); }
        }
        for digest in [&self.baseline_digest, &self.candidate_digest, &self.drift_digest, &self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 { return Err(ContextFreshnessDriftError::Invalid("freshness/drift digest is invalid".into())); }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("evaluate:local-context-freshness:") && effect != "block:unsafe-release") {
            return Err(ContextFreshnessDriftError::Invalid("freshness/drift effect is outside local evaluation gate".into()));
        }
        self.artifact.validate_metadata().map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextFreshnessDriftError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))
    }
}

pub fn context_freshness_drift_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["context compiler".into(), "decision-section compiler".into(), "researcher".into()].into(), behavior: "evaluates context age and source/schema/semantic/provenance drift with deterministic replay-bound evidence".into(), value: "prevents stale or semantically changed context from being reused as if it were current".into(), inputs: vec![TypedPort { name: "context_freshness_drift_request".into(), schema: "ContextFreshnessDriftRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_freshness_drift_receipt".into(), schema: "ContextFreshnessDriftReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:local-context-freshness".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn evaluate_context_freshness_drift(request: &ContextFreshnessDriftRequest) -> Result<ContextFreshnessDriftReceipt, ContextFreshnessDriftError> {
    if request.request_id.trim().is_empty() || request.objective.trim().is_empty() || request.max_age_seconds == 0 || request.boundary != PRECLINICAL_BOUNDARY || request.baseline.boundary != PRECLINICAL_BOUNDARY || request.candidate.boundary != PRECLINICAL_BOUNDARY || !request.baseline.raw_data_local || !request.candidate.raw_data_local || !request.raw_data_local {
        return Err(ContextFreshnessDriftError::Invalid("freshness/drift identity, age limit, locality, or boundary is invalid".into()));
    }
    let age = request.now_epoch.saturating_sub(request.candidate.observed_at_epoch);
    let mut changed = BTreeSet::new();
    if request.baseline.source_digest != request.candidate.source_digest { changed.insert("source".into()); }
    if request.baseline.schema_digest != request.candidate.schema_digest { changed.insert("schema".into()); }
    if request.baseline.semantics_digest != request.candidate.semantics_digest { changed.insert("semantics".into()); }
    if request.baseline.provenance_digest != request.candidate.provenance_digest { changed.insert("provenance".into()); }
    let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let negative = BTreeSet::new();
    if !request.policy_allow || !request.protected_closure { omissions.insert("context:policy-or-protected-closure-blocked".into()); }
    if request.baseline.replay_identity != request.candidate.replay_identity { uncertainty.insert("context:replay-identity-mismatch".into()); }
    if age > request.max_age_seconds { omissions.insert(format!("context:stale:{}", age)); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local || !request.baseline.raw_data_local || !request.candidate.raw_data_local { "blocked" } else if !uncertainty.is_empty() { "unknown" } else if age > request.max_age_seconds { "stale" } else if !changed.is_empty() { "drifted" } else { "fresh" };
    let baseline_digest = ContentHash::of_value(&json!({"snapshot_id": request.baseline.snapshot_id, "source_digest": request.baseline.source_digest, "schema_digest": request.baseline.schema_digest, "semantics_digest": request.baseline.semantics_digest, "provenance_digest": request.baseline.provenance_digest})).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))?;
    let candidate_digest = ContentHash::of_value(&json!({"snapshot_id": request.candidate.snapshot_id, "source_digest": request.candidate.source_digest, "schema_digest": request.candidate.schema_digest, "semantics_digest": request.candidate.semantics_digest, "provenance_digest": request.candidate.provenance_digest})).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))?;
    let drift_digest = ContentHash::of_value(&json!({"changed_dimension_order": changed, "freshness_age_seconds": age, "disposition": disposition})).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "baseline_digest": baseline_digest, "candidate_digest": candidate_digest, "drift_digest": drift_digest, "replay_identity": request.candidate.replay_identity})).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))?;
    let effects = if disposition == "fresh" { vec![format!("evaluate:local-context-freshness:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "disposition": disposition, "changed_dimension_order": changed, "freshness_age_seconds": age, "baseline_digest": baseline_digest, "candidate_digest": candidate_digest, "drift_digest": drift_digest, "context_digest": context_digest, "replay_identity": request.candidate.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-context-freshness-drift:{}", request.request_id), "application/vnd.aurora.context-freshness-drift+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextFreshnessDriftError::Artifact(error.to_string()))?;
    let receipt = ContextFreshnessDriftReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), objective: request.objective.clone(), disposition: disposition.into(), changed_dimension_order: changed.into_iter().collect(), freshness_age_seconds: age, baseline_digest, candidate_digest, drift_digest, context_digest, replay_identity: request.candidate.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: effects, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn snapshot(source: &str, epoch: u64) -> ContextSnapshot { ContextSnapshot { snapshot_id: format!("snapshot:{source}"), source_digest: hash(source), schema_digest: hash("schema"), semantics_digest: hash("semantics"), provenance_digest: hash("provenance"), replay_identity: hash("replay"), observed_at_epoch: epoch, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    fn request() -> ContextFreshnessDriftRequest { ContextFreshnessDriftRequest { request_id: "request:freshness".into(), objective: "check context freshness".into(), baseline: snapshot("source", 90), candidate: snapshot("source", 95), now_epoch: 100, max_age_seconds: 20, policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(context_freshness_drift_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn equal_snapshots_are_fresh() { let receipt = evaluate_context_freshness_drift(&request()).unwrap(); assert_eq!(receipt.disposition, "fresh"); }
    #[test] fn changed_source_is_drifted() { let mut value = request(); value.candidate = snapshot("changed", 95); let receipt = evaluate_context_freshness_drift(&value).unwrap(); assert_eq!(receipt.disposition, "drifted"); assert!(receipt.changed_dimension_order.contains(&"source".into())); }
    #[test] fn stale_snapshot_blocks_release() { let mut value = request(); value.candidate = snapshot("source", 1); let receipt = evaluate_context_freshness_drift(&value).unwrap(); assert_eq!(receipt.disposition, "stale"); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
    #[test] fn digest_is_stable() { let receipt = evaluate_context_freshness_drift(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
