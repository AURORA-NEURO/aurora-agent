//! Federated continual experiment-design contract model (`AFA-fabric-P09-F08`).
//!
//! This typed primitive negotiates versioned experiment-design envelopes. It classifies exact,
//! additive, and breaking schema compatibility and records migration/semantic-loss witnesses.
//! It does not generate designs, execute protocols, contact instruments, move raw data, or make
//! clinical decisions.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fabric-P09-F08";
pub const CONTRACT_VERSION: &str =
    "fabric-federated-continual-experiment-design-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective4@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.fabric-experiment-design-contract-2+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignContractCandidate4 {
    pub candidate_id: String,
    pub source_schema: String,
    pub target_schema: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub permitted: bool,
    pub local_only: bool,
    pub signed: bool,
    pub migration_available: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FabricExperimentDesignContractRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_schema: String,
    pub replay_identity: ContentHash,
    pub candidates: Vec<DesignContractCandidate4>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesignArtifact2 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesign2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_schema: String,
    pub compatibility: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub compatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub contract_digest: ContentHash,
    pub artifact: ExecutableExperimentDesignArtifact2,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DesignContractError {
    #[error("invalid experiment-design contract request or receipt: {0}")]
    Invalid(String),
    #[error("experiment-design contract artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn experiment_design_contract_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "fabric".into(),
        consumers: ["agent developer".into(), "schema migration steward".into(), "experiment workflow compiler".into()].into(),
        behavior: "negotiate federated continual experiment-design schemas with deterministic compatibility and semantic-loss witnesses".into(),
        value: "gives downstream agents a typed, replayable design contract without pretending a migrated envelope is an executable protocol".into(),
        inputs: vec![TypedPort { name: "experiment_design_contract_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "executable_experiment_design_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::new(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl ExecutableExperimentDesign2 {
    pub fn validate(&self) -> Result<(), DesignContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.compatibility.as_str(),
                "exact" | "additive" | "breaking"
            )
            || !matches!(
                self.disposition.as_str(),
                "compatible" | "partial" | "blocked"
            )
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || [
                &self.request_id,
                &self.consumer,
                &self.purpose,
                &self.semantic_profile,
                &self.required_schema,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(DesignContractError::Invalid(
                "contract identity, compatibility, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.compatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.omitted_order,
            &self.migration_order,
            &self.semantic_loss_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(DesignContractError::Invalid(
                    "contract ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .compatible_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(DesignContractError::Invalid(
                "contract candidate states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.contract_digest)
            || self.artifact.content_hash != self.contract_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(DesignContractError::Artifact(
                "contract digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| !e.starts_with("observe:design-contract:"))
        {
            return Err(DesignContractError::Invalid(
                "contract effect is outside observation gate".into(),
            ));
        }
        Ok(())
    }
}

pub fn negotiate_experiment_design_contract(
    request: &FabricExperimentDesignContractRequest4,
) -> Result<ExecutableExperimentDesign2, DesignContractError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_schema.trim().is_empty()
        || request.candidates.is_empty()
        || !digest(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(DesignContractError::Invalid(
            "contract request identity, candidates, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    let candidate_order = request
        .candidates
        .iter()
        .map(|c| c.candidate_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if candidate_order.len() != request.candidates.len()
        || candidate_order.iter().any(|v| v.trim().is_empty())
    {
        return Err(DesignContractError::Invalid(
            "candidate ids must be unique and non-empty".into(),
        ));
    }
    let mut compatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.negative_result {
            negative.insert(candidate.candidate_id.clone());
        }
        let exact = candidate.source_schema == request.required_schema
            && candidate.target_schema == request.required_schema;
        let additive =
            candidate.target_schema == request.required_schema && candidate.migration_available;
        if !candidate.permitted
            || !candidate.signed
            || !candidate.local_only
            || !digest(&candidate.artifact_digest)
            || !digest(&candidate.provenance_digest)
            || candidate.replay_identity != request.replay_identity
            || !request.policy_allow
            || !request.protected_closure
        {
            blocked.insert(candidate.candidate_id.clone());
        } else if matches!(
            candidate.evidence_state,
            EvidenceState::Contradicted | EvidenceState::Unknown
        ) {
            unresolved.insert(candidate.candidate_id.clone());
            semantic_loss.insert(format!("{}:evidence-state", candidate.candidate_id));
        } else if exact {
            compatible.insert(candidate.candidate_id.clone());
        } else if additive {
            compatible.insert(candidate.candidate_id.clone());
            migration.insert(format!("{}:additive-schema", candidate.candidate_id));
            semantic_loss.insert(format!("{}:bounded-migration", candidate.candidate_id));
        } else {
            omitted.insert(candidate.candidate_id.clone());
            migration.insert(format!("{}:breaking-schema", candidate.candidate_id));
        }
    }
    let compatibility = if request.candidates.iter().all(|c| {
        c.source_schema == request.required_schema && c.target_schema == request.required_schema
    }) {
        "exact"
    } else if !migration.is_empty() {
        "additive"
    } else {
        "breaking"
    };
    let global_block = !request.policy_allow || !request.protected_closure;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "compatible"
    };
    let payload = json!({"candidate_order":candidate_order,"compatible_order":compatible,"unresolved_order":unresolved,"blocked_order":blocked,"omitted_order":omitted,"migration_order":migration,"semantic_loss_order":semantic_loss,"negative_evidence_order":negative,"replay_identity":request.replay_identity,"compatibility":compatibility,"disposition":disposition});
    let contract_digest = ContentHash::of_value(&payload)
        .map_err(|e| DesignContractError::Artifact(e.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = ExecutableExperimentDesign2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        required_schema: request.required_schema.clone(),
        compatibility: compatibility.into(),
        disposition: disposition.into(),
        candidate_order: strings("candidate_order"),
        compatible_order: strings("compatible_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        omitted_order: strings("omitted_order"),
        migration_order: strings("migration_order"),
        semantic_loss_order: strings("semantic_loss_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: request.replay_identity.clone(),
        contract_digest: contract_digest.clone(),
        artifact: ExecutableExperimentDesignArtifact2 {
            artifact_id: format!("fabric-experiment-design-contract:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: contract_digest,
            semantic_loss: vec!["contract-only; no executable dispatch".into()],
            provenance_digests: request
                .candidates
                .iter()
                .map(|c| c.provenance_digest.clone())
                .collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: vec![format!("observe:design-contract:{}", request.request_id)],
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> FabricExperimentDesignContractRequest4 {
        FabricExperimentDesignContractRequest4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "contract-1".into(),
            consumer: "agent".into(),
            purpose: "design compatibility".into(),
            semantic_profile: "design:v1".into(),
            required_schema: "ExperimentObjective4@1".into(),
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates: vec![DesignContractCandidate4 {
                candidate_id: "d1".into(),
                source_schema: INPUT_SCHEMA.into(),
                target_schema: INPUT_SCHEMA.into(),
                semantic_profile: "design:v1".into(),
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: h("replay"),
                evidence_state: EvidenceState::Supported,
                permitted: true,
                local_only: true,
                signed: true,
                migration_available: false,
                negative_result: false,
            }],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            experiment_design_contract_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn exact_is_compatible() {
        assert_eq!(
            negotiate_experiment_design_contract(&request())
                .unwrap()
                .disposition,
            "compatible"
        )
    }
    #[test]
    fn breaking_is_partial() {
        let mut r = request();
        r.candidates[0].target_schema = "Other@1".into();
        assert_eq!(
            negotiate_experiment_design_contract(&r)
                .unwrap()
                .disposition,
            "partial"
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = request();
        r.policy_allow = false;
        assert_eq!(
            negotiate_experiment_design_contract(&r)
                .unwrap()
                .disposition,
            "blocked"
        )
    }
}
