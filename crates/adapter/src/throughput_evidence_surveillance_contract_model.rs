//! Prospective high-throughput evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-adapter-P01-F07`. This typed data primitive binds schema migration to
//! queue capacity, checkpoint identity, and explicit overflow so high-throughput evidence cannot
//! disappear between ingestion and qualification.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F07";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContractClaim {
    pub claim_id: String,
    pub sequence: u64,
    pub semantic_type: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub max_claims: usize,
    pub budget_units: usize,
    pub claims: Vec<ThroughputContractClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContractCompatibility {
    Compatible,
    AdditiveMigration,
    Breaking,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContractDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub compatibility: ThroughputContractCompatibility,
    pub disposition: ThroughputContractDisposition,
    pub candidate_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub contract_digest: ContentHash,
    pub canonical_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceContractError {
    #[error("invalid throughput contract request: {0}")]
    Invalid(String),
    #[error("throughput contract artifact failed: {0}")]
    Artifact(String),
}
fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ThroughputEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid("throughput contract identity, schemas, checkpoint, locality, candidates, or effects are incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.retained_order,
            &self.unknown_order,
            &self.denied_order,
            &self.overflow_order,
            &self.migration_order,
            &self.semantic_loss,
            &self.effect_receipts,
        ] {
            if !sorted_unique(values) {
                return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                    "throughput contract ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .retained_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .chain(self.overflow_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.contract_digest,
            &self.canonical_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                    "throughput contract digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-throughput-contract:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract effect is outside local-read gate".into(),
            ));
        }
        if self.disposition == ThroughputContractDisposition::Blocked
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "blocked throughput contract must be explicitly blocked".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })
    }
}

pub fn throughput_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["preclinical researcher".into(), "queue schema steward".into()].into(), behavior: "models EvidenceFeed3 into QualifiedEvidenceSet2 with bounded queue, checkpoint, migration, and overflow witnesses".into(), value: "makes high-throughput capacity loss and replay identity part of the scientific data contract".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OpenTelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_throughput_evidence_surveillance_contract(
    request: &ThroughputEvidenceSurveillanceContractRequest,
) -> Result<
    ThroughputEvidenceSurveillanceContractReceipt,
    ThroughputEvidenceSurveillanceContractError,
> {
    if request.request_id.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.max_claims == 0
        || request.budget_units == 0
        || request.claims.is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid("throughput contract identity, schemas, batch/checkpoint, capacity, budget, claims, replay, locality, or boundary is invalid".into()));
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    let claim_ids = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if claim_ids.windows(2).any(|pair| pair[0] == pair[1])
        || claim_ids.iter().any(|value| value.trim().is_empty())
    {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            "throughput claim identities must be unique and non-empty".into(),
        ));
    }
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            ThroughputContractCompatibility::AdditiveMigration
        } else if request.input_schema == request.output_schema {
            ThroughputContractCompatibility::Compatible
        } else {
            ThroughputContractCompatibility::Breaking
        };
    let admission = request
        .max_claims
        .min(request.budget_units)
        .min(claims.len());
    let (admitted, overflow) = claims.split_at(admission);
    let mut candidate_order = claim_ids.clone();
    candidate_order.sort();
    let overflow_order = overflow
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<BTreeSet<_>>();
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss = BTreeSet::new();
    if claims.len() > request.max_claims {
        loss.insert(format!(
            "queue:capacity-overflow:{}",
            claims.len() - request.max_claims
        ));
    }
    if request.budget_units < request.max_claims {
        loss.insert(format!(
            "queue:budget-bounded:{}",
            request.max_claims - request.budget_units
        ));
    }
    for claim in admitted {
        if compatibility == ThroughputContractCompatibility::Breaking {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:breaking-schema", claim.claim_id));
        } else if claim.omitted
            || matches!(
                claim.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
        {
            unknown.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:unknown-not-asserted", claim.claim_id));
        } else if claim.evidence_state == EvidenceState::Contradicted {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:contradicted-retained", claim.claim_id));
        } else {
            retained.insert(claim.claim_id.clone());
            if compatibility == ThroughputContractCompatibility::AdditiveMigration {
                migration.insert(format!("claim:{}:sequence-preserved", claim.claim_id));
            }
            if claim.negative_result {
                loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    if !request.policy_allow {
        loss.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        loss.insert("control:protected-closure-incomplete".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            ThroughputContractDisposition::Blocked
        } else if retained.is_empty() {
            ThroughputContractDisposition::Unknown
        } else if !unknown.is_empty() || !denied.is_empty() || !overflow_order.is_empty() {
            ThroughputContractDisposition::Partial
        } else {
            ThroughputContractDisposition::Compatible
        };
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let overflow_order = overflow_order.into_iter().collect::<Vec<_>>();
    let migration_order = migration.iter().cloned().collect::<Vec<_>>();
    let semantic_loss = loss.iter().cloned().collect::<Vec<_>>();
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "candidate_order": candidate_order.clone(), "overflow_order": overflow_order.clone()})).map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "previous_checkpoint": request.previous_checkpoint, "queue_digest": queue_digest})).map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "candidate_order": candidate_order.clone()})).map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({"retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "overflow_order": overflow_order.clone(), "migration_order": migration_order.clone(), "semantic_loss": semantic_loss.clone()})).map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "checkpoint_digest": checkpoint_digest, "contract_digest": contract_digest})).map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "compatibility": compatibility, "disposition": disposition, "candidate_order": candidate_order, "retained_order": retained_order, "unknown_order": unknown_order, "denied_order": denied_order, "overflow_order": overflow_order, "migration_order": migration_order, "semantic_loss": semantic_loss, "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": request.replay_identity, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-throughput-contract:{}", request.request_id),
        "application/vnd.aurora.qualified-throughput-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let receipt = ThroughputEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        compatibility,
        disposition,
        candidate_order,
        retained_order,
        unknown_order,
        denied_order,
        overflow_order,
        migration_order,
        semantic_loss,
        queue_digest,
        checkpoint_digest,
        contract_digest,
        canonical_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if disposition == ThroughputContractDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "read:local-throughput-contract:{}",
                request.request_id
            )]
        },
        artifact,
        raw_data_local: request.raw_data_local,
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
    fn request() -> ThroughputEvidenceSurveillanceContractRequest {
        let digest = hash("throughput-contract");
        let claim = |id: &str, sequence: u64, state: EvidenceState| ThroughputContractClaim {
            claim_id: id.into(),
            sequence,
            semantic_type: "evidence".into(),
            value_digest: digest.clone(),
            evidence_state: state,
            omitted: false,
            negative_result: false,
        };
        ThroughputEvidenceSurveillanceContractRequest {
            request_id: "request:throughput-contract".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            batch_id: "batch:one".into(),
            checkpoint_seq: 8,
            previous_checkpoint: Some(digest.clone()),
            max_claims: 4,
            budget_units: 4,
            claims: vec![
                claim("claim:a", 1, EvidenceState::Supported),
                claim("claim:b", 2, EvidenceState::Supported),
            ],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: digest,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_evidence_surveillance_contract_model_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn bounded_contract_is_compatible() {
        assert_eq!(
            model_throughput_evidence_surveillance_contract(&request())
                .unwrap()
                .disposition,
            ThroughputContractDisposition::Compatible
        );
    }
    #[test]
    fn overflow_is_partial() {
        let mut value = request();
        value.max_claims = 1;
        let receipt = model_throughput_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Partial);
        assert_eq!(receipt.overflow_order.len(), 1);
    }
    #[test]
    fn unknown_is_preserved() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert!(model_throughput_evidence_surveillance_contract(&value)
            .unwrap()
            .semantic_loss
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn breaking_is_partial() {
        let mut value = request();
        value.input_schema = "EvidenceFeed9@1".into();
        assert_eq!(
            model_throughput_evidence_surveillance_contract(&value)
                .unwrap()
                .compatibility,
            ThroughputContractCompatibility::Breaking
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            model_throughput_evidence_surveillance_contract(&value)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        );
    }
    #[test]
    fn checkpoint_is_stable() {
        let first = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        let second = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.checkpoint_digest, second.checkpoint_digest);
    }
}
