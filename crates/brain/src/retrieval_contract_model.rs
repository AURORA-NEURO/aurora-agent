//! Typed retrieval-and-synthesis contract model.
//!
//! Atlas feature: `AFA-brain-P02-F05`. This capability validates a retrieval contract before
//! execution, making schema drift, required-field omissions, semantic loss, and replay identity
//! observable product state rather than an implicit implementation detail.

use crate::evidence_contract_model::{ContractCompatibility, ContractDisposition};
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F05";
pub const CONTRACT_VERSION: &str = "brain-retrieval-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery1@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalContractModelRequest {
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub required_fields: Vec<String>,
    pub provided_fields: Vec<String>,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalContractModelReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: ContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub required_order: Vec<String>,
    pub provided_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub contract_digest: ContentHash,
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
pub enum RetrievalContractModelError {
    #[error("invalid retrieval contract model: {0}")]
    Invalid(String),
    #[error("retrieval contract artifact failed: {0}")]
    Artifact(String),
}

impl RetrievalContractModelReceipt {
    pub fn validate(&self) -> Result<(), RetrievalContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.required_order.is_empty()
            || self.provided_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(RetrievalContractModelError::Invalid("retrieval contract identity, schemas, field closure, locality, or effects are incomplete".into()));
        }
        if self
            .missing_order
            .iter()
            .any(|field| !self.required_order.contains(field))
            || self
                .semantic_loss_order
                .iter()
                .any(|field| !self.provided_order.contains(field))
        {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract loss state is outside declared fields".into(),
            ));
        }
        for values in [
            &self.required_order,
            &self.provided_order,
            &self.missing_order,
            &self.semantic_loss_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RetrievalContractModelError::Invalid(
                    "retrieval contract ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalContractModelError::Invalid(
                "effect is outside retrieval contract gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, RetrievalContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))
    }
}

pub fn retrieval_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research software engineer".into(), "retrieval workflow operator".into()].into(), behavior: "validates a ScopedRetrievalQuery contract for schema compatibility, required-field closure, semantic loss, and canonical identity before local synthesis".into(), value: "prevents incompatible or incomplete retrieval data from entering a qualified evidence workflow".into(), inputs: vec![TypedPort { name: "retrieval_query_contract".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "retrieval_synthesis_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_retrieval_contract(
    request: &RetrievalContractModelRequest,
) -> Result<RetrievalContractModelReceipt, RetrievalContractModelError> {
    validate_request(request)?;
    let required = request
        .required_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let provided = request
        .provided_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&provided).cloned().collect::<Vec<_>>();
    let semantic_loss = provided.difference(&required).cloned().collect::<Vec<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|field| format!("field:{field}:required-missing")),
        );
    }
    if !semantic_loss.is_empty() {
        uncertainty.extend(
            semantic_loss
                .iter()
                .map(|field| format!("field:{field}:provided-not-declared")),
        );
    }
    if !matches!(request.compatibility, ContractCompatibility::Additive) {
        uncertainty.insert(format!(
            "contract:compatibility-{}",
            compatibility_label(request.compatibility)
        ));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    let disposition = if !request.policy_allow || !request.protected_closure {
        ContractDisposition::Blocked
    } else if !missing.is_empty()
        || matches!(request.compatibility, ContractCompatibility::Breaking)
    {
        ContractDisposition::Blocked
    } else if !semantic_loss.is_empty()
        || !matches!(request.compatibility, ContractCompatibility::Additive)
        || !negative.is_empty()
    {
        ContractDisposition::Partial
    } else {
        ContractDisposition::Qualified
    };
    let contract_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "required_order": required, "provided_order": provided, "compatibility": request.compatibility, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "replay_identity": request.replay_identity, "disposition": disposition})).map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "disposition": disposition, "compatibility": request.compatibility, "input_schema": request.input_schema, "output_schema": request.output_schema, "required_order": required, "provided_order": provided, "missing_order": missing, "semantic_loss_order": semantic_loss, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-contract:{}", request.request_id),
        "application/vnd.aurora.retrieval-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))?;
    let receipt = RetrievalContractModelReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        disposition,
        compatibility: request.compatibility,
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        required_order: required.into_iter().collect(),
        provided_order: provided.into_iter().collect(),
        missing_order: missing,
        semantic_loss_order: semantic_loss,
        semantic_digest: request.semantic_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        provenance_digest: request.provenance_digest.clone(),
        contract_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(
            disposition,
            ContractDisposition::Qualified | ContractDisposition::Partial
        ) {
            vec![format!(
                "read:local-research-artifacts:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &RetrievalContractModelRequest,
) -> Result<(), RetrievalContractModelError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.required_fields.is_empty()
        || request.provided_fields.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalContractModelError::Invalid(
            "retrieval contract identity, schemas, fields, or boundary is incomplete".into(),
        ));
    }
    Ok(())
}
fn compatibility_label(value: ContractCompatibility) -> &'static str {
    match value {
        ContractCompatibility::Additive => "additive",
        ContractCompatibility::MigrationRequired => "migration-required",
        ContractCompatibility::Breaking => "breaking",
        ContractCompatibility::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(
        compatibility: ContractCompatibility,
        provided_fields: Vec<String>,
    ) -> RetrievalContractModelRequest {
        RetrievalContractModelRequest {
            request_id: "request:retrieval-contract".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility,
            required_fields: vec!["scope".into(), "evidence".into(), "provenance".into()],
            provided_fields,
            semantic_digest: hash("semantic"),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        let m = retrieval_contract_model_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn complete_contract_is_qualified() {
        let r = model_retrieval_contract(&request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "evidence".into(), "provenance".into()],
        ))
        .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Qualified);
    }
    #[test]
    fn missing_field_blocks() {
        let r = model_retrieval_contract(&request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "provenance".into()],
        ))
        .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
        assert!(!r.omissions.is_empty());
    }
    #[test]
    fn semantic_loss_is_partial() {
        let r = model_retrieval_contract(&request(
            ContractCompatibility::Additive,
            vec![
                "scope".into(),
                "evidence".into(),
                "provenance".into(),
                "legacy".into(),
            ],
        ))
        .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Partial);
    }
    #[test]
    fn policy_blocks() {
        let mut q = request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "evidence".into(), "provenance".into()],
        );
        q.policy_allow = false;
        let r = model_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = model_retrieval_contract(&request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "evidence".into(), "provenance".into()],
        ))
        .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
