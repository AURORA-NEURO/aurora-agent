//! Dependency-closure compilation for typed research context graphs.
//!
//! Atlas feature: `AFA-brain-P03-F10`. Context dependencies are compiled with
//! deterministic topological ordering; missing prerequisites and cycles remain
//! explicit release evidence.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F10";
pub const CONTRACT_VERSION: &str = "brain-context-dependency-closure/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDependencyEdge {
    pub context_id: String,
    pub dependency_id: String,
    pub relation: String,
    pub evidence_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDependencyClosureRequest {
    pub request_id: String,
    pub objective: String,
    pub required_context_ids: Vec<String>,
    pub available_context_ids: Vec<String>,
    pub edges: Vec<ContextDependencyEdge>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDependencyClosureReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective: String,
    pub disposition: ContextCompilationDisposition,
    pub context_order: Vec<String>,
    pub resolved_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub closure_digest: ContentHash,
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
pub enum ContextDependencyClosureError {
    #[error("invalid context dependency closure request: {0}")]
    Invalid(String),
    #[error("context dependency closure artifact failed: {0}")]
    Artifact(String),
}

impl ContextDependencyClosureReceipt {
    pub fn validate(&self) -> Result<(), ContextDependencyClosureError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.context_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContextDependencyClosureError::Invalid("dependency closure identity, context graph, locality, or effects are incomplete".into()));
        }
        for values in [&self.context_order, &self.resolved_order, &self.missing_dependency_order, &self.cycle_order, &self.blocked_order, &self.dependency_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) { return Err(ContextDependencyClosureError::Invalid("dependency closure vectors are not canonical".into())); }
        }
        let contexts = self.context_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.resolved_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.missing_dependency_order.iter().cloned()); classified.extend(self.cycle_order.iter().cloned()); classified.extend(self.blocked_order.iter().cloned());
        if classified != contexts { return Err(ContextDependencyClosureError::Invalid("dependency closure context states do not partition contexts".into())); }
        for digest in [&self.closure_digest, &self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 { return Err(ContextDependencyClosureError::Invalid("dependency closure digest is invalid".into())); }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("compile:local-dependency-closure:") && effect != "block:unsafe-release") { return Err(ContextDependencyClosureError::Invalid("dependency closure effect is outside local compilation gate".into())); }
        self.artifact.validate_metadata().map_err(|error| ContextDependencyClosureError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextDependencyClosureError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextDependencyClosureError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextDependencyClosureError::Artifact(error.to_string()))
    }
}

pub fn context_dependency_closure_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["context compiler".into(), "workflow planner".into(), "decision-section compiler".into()].into(), behavior: "compiles context dependency graphs into deterministic topological order while preserving missing prerequisites and cycles".into(), value: "prevents incomplete or cyclic context dependencies from appearing as a complete research context".into(), inputs: vec![TypedPort { name: "context_dependency_closure_request".into(), schema: "ContextDependencyClosureRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_dependency_closure_receipt".into(), schema: "ContextDependencyClosureReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["compile:local-dependency-closure".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_context_dependency_closure(request: &ContextDependencyClosureRequest) -> Result<ContextDependencyClosureReceipt, ContextDependencyClosureError> {
    if request.request_id.trim().is_empty() || request.objective.trim().is_empty() || request.required_context_ids.is_empty() || request.boundary != PRECLINICAL_BOUNDARY || request.replay_identity.as_str().len() != 64 { return Err(ContextDependencyClosureError::Invalid("dependency closure identity, contexts, replay, or boundary is invalid".into())); }
    let contexts = request.required_context_ids.iter().cloned().collect::<BTreeSet<_>>(); let available = request.available_context_ids.iter().cloned().collect::<BTreeSet<_>>();
    if contexts.len() != request.required_context_ids.len() || available.len() != request.available_context_ids.len() || contexts.iter().any(|value| value.trim().is_empty()) { return Err(ContextDependencyClosureError::Invalid("context identifiers must be unique and non-empty".into())); }
    let mut indegree = contexts.iter().map(|id| (id.clone(), 0usize)).collect::<BTreeMap<_, _>>(); let mut outgoing = contexts.iter().map(|id| (id.clone(), BTreeSet::new())).collect::<BTreeMap<_, _>>(); let mut missing = contexts.difference(&available).cloned().collect::<BTreeSet<_>>();
    for edge in &request.edges {
        if edge.context_id.trim().is_empty() || edge.dependency_id.trim().is_empty() || edge.relation.trim().is_empty() || edge.boundary != PRECLINICAL_BOUNDARY || !edge.raw_data_local { return Err(ContextDependencyClosureError::Invalid("dependency edge identity, relation, locality, or boundary is invalid".into())); }
        if !contexts.contains(&edge.context_id) { continue; }
        if !available.contains(&edge.dependency_id) || !contexts.contains(&edge.dependency_id) { missing.insert(edge.context_id.clone()); continue; }
        if outgoing.get_mut(&edge.dependency_id).expect("context initialized").insert(edge.context_id.clone()) { *indegree.get_mut(&edge.context_id).expect("context initialized") += 1; }
    }
    let mut ready = indegree.iter().filter_map(|(id, degree)| (*degree == 0).then_some(id.clone())).collect::<BTreeSet<_>>(); let mut topo = Vec::new();
    while let Some(id) = ready.pop_first() { topo.push(id.clone()); if let Some(children) = outgoing.get(&id) { for child in children { let degree = indegree.get_mut(child).expect("context initialized"); *degree -= 1; if *degree == 0 { ready.insert(child.clone()); } } } }
    let cycle = indegree.iter().filter_map(|(id, degree)| (*degree > 0).then_some(id.clone())).collect::<BTreeSet<_>>();
    let mut resolved = topo.iter().filter(|id| !missing.contains(*id) && !cycle.contains(*id)).cloned().collect::<BTreeSet<_>>();
    let mut blocked = BTreeSet::new(); if !request.policy_allow || !request.protected_closure || !request.raw_data_local { blocked.extend(contexts.iter().cloned()); resolved.clear(); }
    let mut missing_only = missing.clone(); missing_only.retain(|id| !blocked.contains(id)); let mut cycle_only = cycle.clone(); cycle_only.retain(|id| !blocked.contains(id) && !missing_only.contains(id));
    let mut omissions = BTreeSet::new(); for id in &missing_only { omissions.insert(format!("context:{}:missing-dependency", id)); } for id in &cycle_only { omissions.insert(format!("context:{}:dependency-cycle", id)); } if !blocked.is_empty() { omissions.insert("context:policy-protected-closure-locality-blocked".into()); }
    let mut uncertainty = BTreeSet::new(); if request.edges.iter().any(|edge| edge.replay_identity != request.replay_identity) { uncertainty.insert("context:dependency-replay-mismatch".into()); }
    let negative = BTreeSet::new();
    let disposition = if !request.policy_allow || !request.protected_closure || !request.raw_data_local { ContextCompilationDisposition::Blocked } else if resolved.is_empty() { ContextCompilationDisposition::Unknown } else if resolved.len() == contexts.len() && missing_only.is_empty() && cycle_only.is_empty() && uncertainty.is_empty() { ContextCompilationDisposition::Qualified } else { ContextCompilationDisposition::Partial };
    let closure_digest = ContentHash::of_value(&json!({"context_order": contexts, "dependency_order": topo, "missing": missing_only, "cycle": cycle_only, "replay_identity": request.replay_identity})).map_err(|error| ContextDependencyClosureError::Artifact(error.to_string()))?; let context_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "closure_digest": closure_digest})).map_err(|error| ContextDependencyClosureError::Artifact(error.to_string()))?;
    let effects = if matches!(disposition, ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial) { vec![format!("compile:local-dependency-closure:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "objective": request.objective, "disposition": disposition, "context_order": contexts, "resolved_order": resolved, "missing_dependency_order": missing_only, "cycle_order": cycle_only, "blocked_order": blocked, "dependency_order": topo, "closure_digest": closure_digest, "context_digest": context_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY}); let artifact = TypedResearchArtifact::from_payload(format!("brain-context-dependency-closure:{}", request.request_id), "application/vnd.aurora.context-dependency-closure+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextDependencyClosureError::Artifact(error.to_string()))?;
    let receipt = ContextDependencyClosureReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), objective: request.objective.clone(), disposition, context_order: contexts.into_iter().collect(), resolved_order: resolved.into_iter().collect(), missing_dependency_order: missing_only.into_iter().collect(), cycle_order: cycle_only.into_iter().collect(), blocked_order: blocked.into_iter().collect(), dependency_order: topo, closure_digest, context_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: effects, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn edge(context_id: &str, dependency_id: &str) -> ContextDependencyEdge { ContextDependencyEdge { context_id: context_id.into(), dependency_id: dependency_id.into(), relation: "requires".into(), evidence_digest: hash(context_id), replay_identity: hash("replay"), raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    fn request() -> ContextDependencyClosureRequest { ContextDependencyClosureRequest { request_id: "request:closure".into(), objective: "compile context dependencies".into(), required_context_ids: vec!["context:a".into(), "context:b".into(), "context:c".into()], available_context_ids: vec!["context:a".into(), "context:b".into(), "context:c".into()], edges: vec![edge("context:b", "context:a"), edge("context:c", "context:b")], replay_identity: hash("replay"), policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(context_dependency_closure_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn closure_is_topologically_qualified() { let receipt = compile_context_dependency_closure(&request()).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Qualified); assert_eq!(receipt.dependency_order, vec!["context:a", "context:b", "context:c"]); }
    #[test] fn missing_dependency_is_explicit() { let mut value = request(); value.available_context_ids.pop(); let receipt = compile_context_dependency_closure(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial); assert!(!receipt.missing_dependency_order.is_empty()); }
    #[test] fn cycle_is_unknown() { let mut value = request(); value.edges.push(edge("context:a", "context:c")); let receipt = compile_context_dependency_closure(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Unknown); assert!(!receipt.cycle_order.is_empty()); }
    #[test] fn policy_denial_blocks() { let mut value = request(); value.policy_allow = false; let receipt = compile_context_dependency_closure(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked); }
    #[test] fn digest_is_stable() { let receipt = compile_context_dependency_closure(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
