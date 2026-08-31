//! Federated continual evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-foundation-P01-F08`. This contract keeps federation policy,
//! semantic compatibility, quorum, and aggregate-only locality as typed data.

use crate::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-foundation-P01-F08";
pub const CONTRACT_VERSION: &str =
    "foundation-federated-continual-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualContractClaim {
    pub claim_id: String,
    pub peer_id: String,
    pub institution_id: String,
    pub artifact_kind: String,
    pub semantic_profile: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub permitted_artifact: bool,
    pub aggregate_only: bool,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub claims: Vec<FederatedContinualContractClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualContractCompatibility {
    Compatible,
    AdditiveMigration,
    Breaking,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualContractDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub compatibility: FederatedContinualContractCompatibility,
    pub disposition: FederatedContinualContractDisposition,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
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
pub enum FederatedContinualEvidenceSurveillanceContractError {
    #[error("invalid federated continual contract request: {0}")]
    Invalid(String),
    #[error("federated continual contract artifact failed: {0}")]
    Artifact(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl FederatedContinualEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContinualEvidenceSurveillanceContractError::Invalid("federated contract identity, schema, locality, candidates, or effects are incomplete".into()));
        }
        for values in [
            &self.peer_order,
            &self.candidate_order,
            &self.retained_order,
            &self.unknown_order,
            &self.denied_order,
            &self.aggregate_order,
            &self.migration_order,
            &self.semantic_loss,
            &self.effect_receipts,
        ] {
            if !sorted_unique(values) {
                return Err(
                    FederatedContinualEvidenceSurveillanceContractError::Invalid(
                        "federated contract ordering is not canonical".into(),
                    ),
                );
            }
        }
        let classified = self
            .retained_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self
                .aggregate_order
                .iter()
                .any(|id| !self.retained_order.contains(id))
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract states do not partition candidates".into(),
                ),
            );
        }
        for digest in [
            &self.federation_digest,
            &self.envelope_digest,
            &self.contract_digest,
            &self.canonical_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(
                    FederatedContinualEvidenceSurveillanceContractError::Invalid(
                        "federated contract digest is invalid".into(),
                    ),
                );
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:aggregate-evidence-contract:")
                && effect != "block:unsafe-release"
        }) {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract effect is outside aggregate exchange gate".into(),
                ),
            );
        }
        if self.disposition == FederatedContinualContractDisposition::Blocked
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "blocked federated contract must be explicitly blocked".into(),
                ),
            );
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })
    }
}

pub fn federated_continual_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "foundation".into(), consumers: ["context compiler engineer".into(), "consortium administrator".into(), "federation schema steward".into()].into(), behavior: "models EvidenceFeed4 into a versioned QualifiedEvidenceSet2 contract with foundation-owned policy-bound aggregate-only federation, quorum, omission, and migration witnesses".into(), value: "provides a core deterministic evidence contract that downstream crates can embed without hiding unknown evidence or moving raw observations".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-aggregate-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "W3C PROV-O".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_federated_continual_evidence_surveillance_contract(
    request: &FederatedContinualEvidenceSurveillanceContractRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceContractReceipt,
    FederatedContinualEvidenceSurveillanceContractError,
> {
    if request.request_id.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.allowed_artifacts.is_empty()
        || request.min_peer_quorum == 0
        || request.claims.is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(FederatedContinualEvidenceSurveillanceContractError::Invalid("federated contract identity, schema, purpose, allow-list, quorum, claims, replay, locality, or boundary is invalid".into()));
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let candidate_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1])
        || claims.iter().any(|claim| {
            claim.claim_id.trim().is_empty()
                || claim.peer_id.trim().is_empty()
                || claim.institution_id.trim().is_empty()
        })
    {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(
                "federated claim identities must be unique and non-empty".into(),
            ),
        );
    }
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            FederatedContinualContractCompatibility::AdditiveMigration
        } else if request.input_schema == request.output_schema {
            FederatedContinualContractCompatibility::Compatible
        } else {
            FederatedContinualContractCompatibility::Breaking
        };
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut peers = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss = BTreeSet::new();
    for claim in &claims {
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:policy-closure-locality", claim.claim_id));
        } else if !claim.signed {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:signature-missing", claim.claim_id));
        } else if !claim.permitted_artifact
            || !request.allowed_artifacts.contains(&claim.artifact_kind)
        {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:artifact-not-permitted", claim.claim_id));
        } else if !claim.aggregate_only {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!(
                "claim:{}:raw-observation-export-denied",
                claim.claim_id
            ));
        } else if claim.semantic_profile != request.semantic_profile {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!(
                "claim:{}:semantic-profile-mismatch",
                claim.claim_id
            ));
        } else if compatibility == FederatedContinualContractCompatibility::Breaking {
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
            loss.insert(format!("claim:{}:contradicted", claim.claim_id));
        } else {
            retained.insert(claim.claim_id.clone());
            peers.insert(claim.peer_id.clone());
            aggregate.insert(claim.claim_id.clone());
            if compatibility == FederatedContinualContractCompatibility::AdditiveMigration {
                migration.insert(format!("claim:{}:aggregate-only-migration", claim.claim_id));
            }
            if claim.negative_result {
                loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    if peers.len() < request.min_peer_quorum {
        loss.insert(format!(
            "federation:quorum-incomplete:{}<{}",
            peers.len(),
            request.min_peer_quorum
        ));
    }
    if !request.policy_allow {
        loss.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        loss.insert("control:protected-closure-incomplete".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            FederatedContinualContractDisposition::Blocked
        } else if retained.is_empty() {
            FederatedContinualContractDisposition::Unknown
        } else if !unknown.is_empty() || !denied.is_empty() || peers.len() < request.min_peer_quorum
        {
            FederatedContinualContractDisposition::Partial
        } else {
            FederatedContinualContractDisposition::Compatible
        };
    let peer_order = peers.iter().cloned().collect::<Vec<_>>();
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let aggregate_order = aggregate.iter().cloned().collect::<Vec<_>>();
    let migration_order = migration.iter().cloned().collect::<Vec<_>>();
    let semantic_loss = loss.iter().cloned().collect::<Vec<_>>();
    let federation_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "purpose": request.purpose, "endpoint": request.endpoint, "peer_order": peer_order.clone(), "semantic_profile": request.semantic_profile})).map_err(|error| FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let envelope_digest = ContentHash::of_value(&json!({"aggregate_order": aggregate_order.clone(), "allowed_artifacts": request.allowed_artifacts.clone(), "aggregate_only": true, "federation_digest": federation_digest})).map_err(|error| FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "candidate_order": candidate_order.clone()})).map_err(|error| FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({"retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "aggregate_order": aggregate_order.clone(), "migration_order": migration_order.clone(), "semantic_loss": semantic_loss.clone()})).map_err(|error| FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "envelope_digest": envelope_digest, "contract_digest": contract_digest})).map_err(|error| FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "federation_id": request.federation_id, "purpose": request.purpose, "endpoint": request.endpoint, "semantic_profile": request.semantic_profile, "compatibility": compatibility, "disposition": disposition, "peer_order": peer_order, "candidate_order": candidate_order, "retained_order": retained_order, "unknown_order": unknown_order, "denied_order": denied_order, "aggregate_order": aggregate_order, "migration_order": migration_order, "semantic_loss": semantic_loss, "federation_digest": federation_digest, "envelope_digest": envelope_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": request.replay_identity, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "foundation-federated-continual-contract:{}",
            request.request_id
        ),
        "application/vnd.aurora.foundation.qualified-federated-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let receipt = FederatedContinualEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        semantic_profile: request.semantic_profile.clone(),
        compatibility,
        disposition,
        peer_order,
        candidate_order,
        retained_order,
        unknown_order,
        denied_order,
        aggregate_order,
        migration_order,
        semantic_loss,
        federation_digest,
        envelope_digest,
        contract_digest,
        canonical_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if disposition == FederatedContinualContractDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "exchange:aggregate-evidence-contract:{}",
                request.federation_id
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
    fn request() -> FederatedContinualEvidenceSurveillanceContractRequest {
        let digest = hash("federated-contract");
        let claim = |id: &str, peer: &str| FederatedContinualContractClaim {
            claim_id: id.into(),
            peer_id: peer.into(),
            institution_id: format!("institution:{peer}"),
            artifact_kind: "aggregate-evidence".into(),
            semantic_profile: "profile:v1".into(),
            value_digest: digest.clone(),
            evidence_state: EvidenceState::Supported,
            signed: true,
            permitted_artifact: true,
            aggregate_only: true,
            omitted: false,
            negative_result: false,
        };
        FederatedContinualEvidenceSurveillanceContractRequest {
            request_id: "request:federated-contract".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            federation_id: "federation:one".into(),
            purpose: "compare preclinical evidence".into(),
            endpoint: "local://federation".into(),
            semantic_profile: "profile:v1".into(),
            allowed_artifacts: vec!["aggregate-evidence".into()],
            min_peer_quorum: 2,
            claims: vec![claim("claim:a", "peer:a"), claim("claim:b", "peer:b")],
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
            federated_continual_evidence_surveillance_contract_model_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn quorum_contract_is_compatible() {
        assert_eq!(
            model_federated_continual_evidence_surveillance_contract(&request())
                .unwrap()
                .disposition,
            FederatedContinualContractDisposition::Compatible
        );
    }
    #[test]
    fn quorum_gap_is_partial() {
        let mut value = request();
        value.min_peer_quorum = 3;
        assert_eq!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .disposition,
            FederatedContinualContractDisposition::Partial
        );
    }
    #[test]
    fn unsigned_is_denied() {
        let mut value = request();
        value.claims[0].signed = false;
        assert!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .denied_order
                .iter()
                .any(|item| item == "claim:a")
        );
    }
    #[test]
    fn raw_export_is_denied() {
        let mut value = request();
        value.claims[0].aggregate_only = false;
        assert!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .semantic_loss
                .iter()
                .any(|item| item.contains("raw-observation-export-denied"))
        );
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .semantic_loss
                .iter()
                .any(|item| item.contains("unknown-not-asserted"))
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        );
    }
    #[test]
    fn replay_is_stable() {
        let first = model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        let second = model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.envelope_digest, second.envelope_digest);
    }
}
