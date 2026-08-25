//! Contradiction-resolution planning for compiled research context.
//!
//! Atlas feature: `AFA-brain-P03-F09`. Competing supported claims are retained
//! as a resolution plan; the compiler never selects a winner merely because a
//! claim appears first or has a larger score.

use crate::context_compilation::ContextCompilationDisposition;
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F09";
pub const CONTRACT_VERSION: &str = "brain-context-contradiction-resolution/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextContradictionClaim {
    pub claim_id: String,
    pub conflict_group: String,
    pub polarity: String,
    pub support_milli: u16,
    pub state: EvidenceState,
    pub provenance_complete: bool,
    pub evidence_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextContradictionResolutionRequest {
    pub request_id: String,
    pub objective: String,
    pub required_group_ids: Vec<String>,
    pub claims: Vec<ContextContradictionClaim>,
    pub minimum_support_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextContradictionResolutionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub disposition: ContextCompilationDisposition,
    pub group_order: Vec<String>,
    pub resolved_group_order: Vec<String>,
    pub contested_group_order: Vec<String>,
    pub missing_group_order: Vec<String>,
    pub blocked_group_order: Vec<String>,
    pub unknown_group_order: Vec<String>,
    pub resolution_plan_order: Vec<String>,
    pub conflict_digest: ContentHash,
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
pub enum ContextContradictionResolutionError {
    #[error("invalid context contradiction-resolution request: {0}")]
    Invalid(String),
    #[error("context contradiction-resolution artifact failed: {0}")]
    Artifact(String),
}

impl ContextContradictionResolutionReceipt {
    pub fn validate(&self) -> Result<(), ContextContradictionResolutionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.group_order.is_empty()
            || self.resolution_plan_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContextContradictionResolutionError::Invalid(
                "contradiction-resolution identity, groups, plan, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.group_order,
            &self.resolved_group_order,
            &self.contested_group_order,
            &self.missing_group_order,
            &self.blocked_group_order,
            &self.unknown_group_order,
            &self.resolution_plan_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextContradictionResolutionError::Invalid(
                    "contradiction-resolution vectors are not canonical".into(),
                ));
            }
        }
        let groups = self.group_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.resolved_group_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.contested_group_order.iter().cloned());
        classified.extend(self.missing_group_order.iter().cloned());
        classified.extend(self.blocked_group_order.iter().cloned());
        classified.extend(self.unknown_group_order.iter().cloned());
        if classified != groups {
            return Err(ContextContradictionResolutionError::Invalid(
                "contradiction-resolution group states do not partition groups".into(),
            ));
        }
        for digest in [&self.conflict_digest, &self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ContextContradictionResolutionError::Invalid("contradiction-resolution digest is invalid".into()));
            }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("compile:local-contradiction-resolution:") && effect != "block:unsafe-release") {
            return Err(ContextContradictionResolutionError::Invalid("contradiction-resolution effect is outside local compilation gate".into()));
        }
        self.artifact.validate_metadata().map_err(|error| ContextContradictionResolutionError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextContradictionResolutionError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextContradictionResolutionError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextContradictionResolutionError::Artifact(error.to_string()))
    }
}

pub fn context_contradiction_resolution_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["decision-section compiler".into(), "replication planner".into(), "researcher".into()].into(), behavior: "groups competing typed claims into resolved, contested, missing, blocked, and unknown states and emits a deterministic resolution plan without winner selection".into(), value: "prevents contradictory evidence from being silently overwritten and routes contested context toward replication or adjudication".into(), inputs: vec![TypedPort { name: "context_contradiction_resolution_request".into(), schema: "ContextContradictionResolutionRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_contradiction_resolution_receipt".into(), schema: "ContextContradictionResolutionReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-contradiction-resolution".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_context_contradiction_resolution(request: &ContextContradictionResolutionRequest) -> Result<ContextContradictionResolutionReceipt, ContextContradictionResolutionError> {
    if request.request_id.trim().is_empty() || request.objective.trim().is_empty() || request.required_group_ids.is_empty() || request.boundary != PRECLINICAL_BOUNDARY || request.replay_identity.as_str().len() != 64 {
        return Err(ContextContradictionResolutionError::Invalid("contradiction-resolution identity, groups, replay, or boundary is invalid".into()));
    }
    let groups = request.required_group_ids.iter().cloned().collect::<BTreeSet<_>>();
    if groups.len() != request.required_group_ids.len() || groups.iter().any(|value| value.trim().is_empty()) { return Err(ContextContradictionResolutionError::Invalid("group identifiers must be unique and non-empty".into())); }
    let mut by_group: BTreeMap<String, Vec<&ContextContradictionClaim>> = BTreeMap::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty() || claim.conflict_group.trim().is_empty() || claim.polarity.trim().is_empty() || claim.support_milli > 1000 || claim.boundary != PRECLINICAL_BOUNDARY { return Err(ContextContradictionResolutionError::Invalid(format!("claim {} has invalid identity, support, polarity, or boundary", claim.claim_id))); }
        by_group.entry(claim.conflict_group.clone()).or_default().push(claim);
    }
    let mut resolved = BTreeSet::new(); let mut contested = BTreeSet::new(); let mut missing = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut unknown = BTreeSet::new(); let mut plans = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    for group in &groups {
        let claims = by_group.get(group).cloned().unwrap_or_default();
        if claims.is_empty() { missing.insert(group.clone()); omissions.insert(format!("group:{}:missing-claims", group)); continue; }
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local || claims.iter().any(|claim| !claim.provenance_complete || !claim.raw_data_local) { blocked.insert(group.clone()); omissions.insert(format!("group:{}:policy-provenance-locality-blocked", group)); continue; }
        if claims.iter().any(|claim| claim.replay_identity != request.replay_identity) { unknown.insert(group.clone()); uncertainty.insert(format!("group:{}:replay-mismatch", group)); continue; }
        let supported = claims.iter().filter(|claim| claim.state == EvidenceState::Supported && claim.support_milli >= request.minimum_support_milli).collect::<Vec<_>>();
        let polarities = supported.iter().map(|claim| claim.polarity.as_str()).collect::<BTreeSet<_>>();
        if polarities.len() > 1 { contested.insert(group.clone()); plans.insert(format!("group:{}:retain-competing-and-replicate", group)); negative.insert(format!("group:{}:contradictory-supported-claims", group)); }
        else if supported.len() == 1 { resolved.insert(group.clone()); plans.insert(format!("group:{}:retain-supported-claim", group)); }
        else if claims.iter().any(|claim| matches!(claim.state, EvidenceState::Unknown | EvidenceState::Speculative)) { unknown.insert(group.clone()); plans.insert(format!("group:{}:acquire-discriminating-evidence", group)); uncertainty.insert(format!("group:{}:unresolved-claim-state", group)); }
        else { blocked.insert(group.clone()); plans.insert(format!("group:{}:below-support-or-unproven", group)); omissions.insert(format!("group:{}:no-supported-claim", group)); }
    }
    if plans.is_empty() { plans.insert("plan:none".into()); }
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ContextCompilationDisposition::Blocked } else if resolved.is_empty() { ContextCompilationDisposition::Unknown } else if resolved.len() == groups.len() && contested.is_empty() && missing.is_empty() && blocked.is_empty() && unknown.is_empty() { ContextCompilationDisposition::Qualified } else { ContextCompilationDisposition::Partial };
    let conflict_digest = ContentHash::of_value(&json!({"group_order": groups, "resolved": resolved, "contested": contested, "missing": missing, "blocked": blocked, "unknown": unknown, "plan": plans, "replay_identity": request.replay_identity})).map_err(|error| ContextContradictionResolutionError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "conflict_digest": conflict_digest, "negative": negative})).map_err(|error| ContextContradictionResolutionError::Artifact(error.to_string()))?;
    let effects = if matches!(disposition, ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial) { vec![format!("compile:local-contradiction-resolution:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "disposition": disposition, "group_order": groups, "resolved_group_order": resolved, "contested_group_order": contested, "missing_group_order": missing, "blocked_group_order": blocked, "unknown_group_order": unknown, "resolution_plan_order": plans, "conflict_digest": conflict_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-context-contradiction-resolution:{}", request.request_id), "application/vnd.aurora.context-contradiction-resolution+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextContradictionResolutionError::Artifact(error.to_string()))?;
    let receipt = ContextContradictionResolutionReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), objective: request.objective.clone(), disposition, group_order: groups.into_iter().collect(), resolved_group_order: resolved.into_iter().collect(), contested_group_order: contested.into_iter().collect(), missing_group_order: missing.into_iter().collect(), blocked_group_order: blocked.into_iter().collect(), unknown_group_order: unknown.into_iter().collect(), resolution_plan_order: plans.into_iter().collect(), conflict_digest, context_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: effects, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn claim(id: &str, polarity: &str, state: EvidenceState) -> ContextContradictionClaim { ContextContradictionClaim { claim_id: id.into(), conflict_group: "group:mechanism".into(), polarity: polarity.into(), support_milli: 900, state, provenance_complete: true, evidence_digest: hash(id), replay_identity: hash("replay"), raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    fn request() -> ContextContradictionResolutionRequest { ContextContradictionResolutionRequest { request_id: "request:contradiction".into(), objective: "plan contradiction resolution".into(), required_group_ids: vec!["group:mechanism".into()], claims: vec![claim("claim:a", "supports", EvidenceState::Supported), claim("claim:b", "refutes", EvidenceState::Supported)], minimum_support_milli: 700, replay_identity: hash("replay"), policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(context_contradiction_resolution_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn competing_claims_are_contested() { let receipt = compile_context_contradiction_resolution(&request()).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Unknown); assert!(receipt.contested_group_order.contains(&"group:mechanism".into())); assert!(receipt.resolution_plan_order.iter().any(|value| value.contains("replicate"))); }
    #[test] fn missing_group_is_unknown() { let mut value = request(); value.claims.clear(); let receipt = compile_context_contradiction_resolution(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Unknown); }
    #[test] fn policy_denial_blocks() { let mut value = request(); value.policy_allow = false; let receipt = compile_context_contradiction_resolution(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked); }
    #[test] fn digest_is_stable() { let receipt = compile_context_contradiction_resolution(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
