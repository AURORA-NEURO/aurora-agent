//! Deterministic high-throughput context compilation for study queues.
//!
//! Atlas feature: `AFA-brain-P03-F03`. This is a product boundary for running
//! many bounded context compilations without turning queue pressure or partial
//! evidence into a silent scientific pass.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F03";
pub const CONTRACT_VERSION: &str = "brain-throughput-context-compilation/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextItem {
    pub context_id: String,
    pub required_fact_count: u32,
    pub supported_fact_count: u32,
    pub priority: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextCompilationRequest {
    pub request_id: String,
    pub batch_id: String,
    pub objective: String,
    pub items: Vec<ThroughputContextItem>,
    pub max_items: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextCompilationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub objective: String,
    pub disposition: ContextCompilationDisposition,
    pub batch_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub queue_digest: ContentHash,
    pub throughput_digest: ContentHash,
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
pub enum ThroughputContextCompilationError {
    #[error("invalid throughput context compilation request: {0}")]
    Invalid(String),
    #[error("throughput context compilation artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputContextCompilationReceipt {
    pub fn validate(&self) -> Result<(), ThroughputContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.batch_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputContextCompilationError::Invalid(
                "throughput context identity, batch, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.batch_order,
            &self.accepted_order,
            &self.deferred_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ThroughputContextCompilationError::Invalid(
                    "throughput context vectors are not canonical".into(),
                ));
            }
        }
        let batch = self.batch_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.accepted_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.deferred_order.iter().cloned());
        classified.extend(self.blocked_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != batch {
            return Err(ThroughputContextCompilationError::Invalid(
                "throughput context queue states do not partition the batch".into(),
            ));
        }
        for digest in [&self.queue_digest, &self.throughput_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputContextCompilationError::Invalid(
                    "throughput context digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("compile:local-throughput-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputContextCompilationError::Invalid(
                "throughput context effect is outside local compilation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputContextCompilationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ThroughputContextCompilationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputContextCompilationError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputContextCompilationError::Artifact(error.to_string()))
    }
}

pub fn throughput_context_compilation_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["research queue operator".into(), "context compiler".into(), "federated scheduler".into()].into(),
        behavior: "compiles a bounded deterministic queue of local preclinical research contexts with explicit capacity and evidence dispositions".into(),
        value: "keeps high-throughput context production reproducible and honest under queue pressure, partial evidence, and policy denial".into(),
        inputs: vec![TypedPort { name: "throughput_context_compilation_request".into(), schema: "ThroughputContextCompilationRequest1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "throughput_context_compilation_receipt".into(), schema: "ThroughputContextCompilationReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["compile:local-throughput-context".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_throughput_context(
    request: &ThroughputContextCompilationRequest,
) -> Result<ThroughputContextCompilationReceipt, ThroughputContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.items.is_empty()
        || request.max_items == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ThroughputContextCompilationError::Invalid(
            "throughput context identity, capacity, replay, or boundary is invalid".into(),
        ));
    }
    let mut items = request.items.clone();
    items.sort_by(|left, right| left.context_id.cmp(&right.context_id));
    if items.windows(2).any(|pair| pair[0].context_id == pair[1].context_id)
        || items.iter().any(|item| item.context_id.trim().is_empty())
    {
        return Err(ThroughputContextCompilationError::Invalid(
            "throughput context identifiers must be unique and non-empty".into(),
        ));
    }
    let batch_order = items.iter().map(|item| item.context_id.clone()).collect::<Vec<_>>();
    let mut accepted = BTreeSet::new();
    let mut deferred = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut capacity = request.max_items as usize;
    for item in &items {
        if !request.policy_allow || !request.raw_data_local || !item.policy_allow || !item.raw_data_local {
            blocked.insert(item.context_id.clone());
            omissions.insert(format!("context:{}:scope-or-policy-blocked", item.context_id));
        } else if item.replay_identity != request.replay_identity {
            unknown.insert(item.context_id.clone());
            uncertainty.insert(format!("context:{}:replay-mismatch", item.context_id));
        } else if item.required_fact_count == 0 || item.supported_fact_count == 0 {
            unknown.insert(item.context_id.clone());
            uncertainty.insert(format!("context:{}:no-qualified-facts", item.context_id));
        } else if item.supported_fact_count < item.required_fact_count {
            deferred.insert(item.context_id.clone());
            omissions.insert(format!("context:{}:incomplete-fact-closure", item.context_id));
        } else if capacity == 0 {
            deferred.insert(item.context_id.clone());
            omissions.insert(format!("context:{}:capacity-deferred", item.context_id));
        } else {
            capacity -= 1;
            accepted.insert(item.context_id.clone());
        }
    }
    let disposition = if !request.policy_allow || !request.raw_data_local {
        ContextCompilationDisposition::Blocked
    } else if accepted.is_empty() && !deferred.is_empty() {
        ContextCompilationDisposition::Partial
    } else if accepted.is_empty() {
        ContextCompilationDisposition::Unknown
    } else if deferred.is_empty() && blocked.is_empty() && unknown.is_empty() {
        ContextCompilationDisposition::Qualified
    } else {
        ContextCompilationDisposition::Partial
    };
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "batch_order": batch_order, "replay_identity": request.replay_identity}))
        .map_err(|error| ThroughputContextCompilationError::Artifact(error.to_string()))?;
    let throughput_digest = ContentHash::of_value(&json!({"accepted": accepted, "deferred": deferred, "blocked": blocked, "unknown": unknown, "max_items": request.max_items}))
        .map_err(|error| ThroughputContextCompilationError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(disposition, ContextCompilationDisposition::Qualified | ContextCompilationDisposition::Partial) {
        vec![format!("compile:local-throughput-context:{}", request.batch_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "objective": request.objective, "disposition": disposition, "batch_order": batch_order, "accepted_order": accepted, "deferred_order": deferred, "blocked_order": blocked, "unknown_order": unknown, "queue_digest": queue_digest, "throughput_digest": throughput_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": [], "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-research-context:{}", request.batch_id),
        "application/vnd.aurora.throughput-context+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputContextCompilationError::Artifact(error.to_string()))?;
    let receipt = ThroughputContextCompilationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), batch_id: request.batch_id.clone(), objective: request.objective.clone(), disposition,
        batch_order, accepted_order: accepted.into_iter().collect(), deferred_order: deferred.into_iter().collect(), blocked_order: blocked.into_iter().collect(), unknown_order: unknown.into_iter().collect(), queue_digest, throughput_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: Vec::new(), effect_receipts, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> ThroughputContextCompilationRequest {
        ThroughputContextCompilationRequest { request_id: "request:throughput".into(), batch_id: "batch:one".into(), objective: "compile queued contexts".into(), max_items: 2, items: vec![
            ThroughputContextItem { context_id: "context:a".into(), required_fact_count: 2, supported_fact_count: 2, priority: 1, replay_identity: hash("replay"), policy_allow: true, raw_data_local: true },
            ThroughputContextItem { context_id: "context:b".into(), required_fact_count: 2, supported_fact_count: 1, priority: 2, replay_identity: hash("replay"), policy_allow: true, raw_data_local: true },
        ], replay_identity: hash("replay"), policy_allow: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn manifest_is_a1() { assert_eq!(throughput_context_compilation_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn bounded_queue_is_partial() { let receipt = compile_throughput_context(&request()).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Partial); assert_eq!(receipt.accepted_order, vec!["context:a"]); }
    #[test] fn policy_denial_blocks() { let mut value = request(); value.policy_allow = false; let receipt = compile_throughput_context(&value).unwrap(); assert_eq!(receipt.disposition, ContextCompilationDisposition::Blocked); }
    #[test] fn digest_is_stable() { let receipt = compile_throughput_context(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
