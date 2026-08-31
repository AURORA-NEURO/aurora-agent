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
const CONTRACT_CONTENT_TYPE: &str = "application/vnd.aurora.retrieval-contract+json";
const MAX_TEXT_BYTES: usize = 512;

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
            || self.required_order.is_empty()
            || self.provided_order.is_empty()
            || self.effect_receipts.len() != 1
        {
            return Err(RetrievalContractModelError::Invalid("retrieval contract identity, schemas, field closure, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.study_id, "study_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
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
            validate_sorted_unique(values, "retrieval contract collection")?;
        }
        let required = self.required_order.iter().cloned().collect::<BTreeSet<_>>();
        let provided = self.provided_order.iter().cloned().collect::<BTreeSet<_>>();
        let expected_missing = required.difference(&provided).cloned().collect::<Vec<_>>();
        let expected_semantic_loss = provided.difference(&required).cloned().collect::<Vec<_>>();
        if self.missing_order != expected_missing
            || self.semantic_loss_order != expected_semantic_loss
        {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract loss state is not bound to declared fields".into(),
            ));
        }
        let gate_blocked = self.negative_evidence.iter().any(|item| {
            matches!(
                item.as_str(),
                "request:policy-denied" | "request:raw-data-locality-failed"
            )
        }) || self
            .omissions
            .iter()
            .any(|item| item == "request:protected-closure-incomplete")
            || !self.missing_order.is_empty()
            || self.compatibility == ContractCompatibility::Breaking;
        let expected_disposition = if gate_blocked {
            ContractDisposition::Blocked
        } else if !self.semantic_loss_order.is_empty()
            || self.compatibility != ContractCompatibility::Additive
            || !self.negative_evidence.is_empty()
        {
            ContractDisposition::Partial
        } else {
            ContractDisposition::Qualified
        };
        if self.disposition != expected_disposition {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract disposition does not match retained gate evidence".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != ContractDisposition::Blocked
                || !self
                    .negative_evidence
                    .iter()
                    .any(|item| item == "request:raw-data-locality-failed"))
        {
            return Err(RetrievalContractModelError::Invalid(
                "non-local retrieval contracts must be blocked and retain locality evidence".into(),
            ));
        }
        let expected_effect = if matches!(
            self.disposition,
            ContractDisposition::Qualified | ContractDisposition::Partial
        ) {
            format!("read:local-research-artifacts:{}", self.request_id)
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract effect does not match disposition".into(),
            ));
        }
        for digest in [
            &self.semantic_digest,
            &self.artifact_digest,
            &self.provenance_digest,
            &self.contract_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(RetrievalContractModelError::Invalid(
                    "retrieval contract digest is invalid".into(),
                ));
            }
        }
        let expected_contract_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "required_order": self.required_order,
            "provided_order": self.provided_order,
            "compatibility": self.compatibility,
            "semantic_digest": self.semantic_digest,
            "artifact_digest": self.artifact_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "disposition": self.disposition,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract_digest {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract digest is not bound to contract state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-retrieval-contract:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTRACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
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

fn validate_text(value: &str, field: &str) -> Result<(), RetrievalContractModelError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(RetrievalContractModelError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), RetrievalContractModelError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(RetrievalContractModelError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), RetrievalContractModelError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RetrievalContractModelError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &RetrievalContractModelReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_id": receipt.study_id,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "compatibility": receipt.compatibility,
        "input_schema": receipt.input_schema,
        "output_schema": receipt.output_schema,
        "required_order": receipt.required_order,
        "provided_order": receipt.provided_order,
        "missing_order": receipt.missing_order,
        "semantic_loss_order": receipt.semantic_loss_order,
        "semantic_digest": receipt.semantic_digest,
        "artifact_digest": receipt.artifact_digest,
        "provenance_digest": receipt.provenance_digest,
        "contract_digest": receipt.contract_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
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
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !missing.is_empty()
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
    let contract_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "required_order": required, "provided_order": provided, "compatibility": request.compatibility, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "replay_identity": request.replay_identity, "disposition": disposition, "raw_data_local": true})).map_err(|error| RetrievalContractModelError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        ContractDisposition::Qualified | ContractDisposition::Partial
    ) {
        vec![format!(
            "read:local-research-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "disposition": disposition, "compatibility": request.compatibility, "input_schema": request.input_schema, "output_schema": request.output_schema, "required_order": required, "provided_order": provided, "missing_order": missing, "semantic_loss_order": semantic_loss, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-contract:{}", request.request_id),
        CONTRACT_CONTENT_TYPE,
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
        effect_receipts,
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
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.study_id, "study_id"),
        (&request.scope, "scope"),
        (&request.input_schema, "input_schema"),
        (&request.output_schema, "output_schema"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.required_fields, "required_fields")?;
    validate_unique(&request.provided_fields, "provided_fields")?;
    for digest in [
        &request.semantic_digest,
        &request.artifact_digest,
        &request.provenance_digest,
        &request.replay_identity,
    ] {
        if digest.as_str().len() != 64 {
            return Err(RetrievalContractModelError::Invalid(
                "retrieval contract digest is invalid".into(),
            ));
        }
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
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "evidence".into(), "provenance".into()],
        );
        q.raw_data_local = false;
        let r = model_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
        assert!(r.raw_data_local);
        assert!(r
            .negative_evidence
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        r.validate().unwrap();
    }
    #[test]
    fn digest_and_payload_drift_are_rejected() {
        let r = model_retrieval_contract(&request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "evidence".into(), "provenance".into()],
        ))
        .unwrap();
        let mut digest_drift = r.clone();
        digest_drift.compatibility = ContractCompatibility::MigrationRequired;
        assert!(digest_drift.validate().is_err());

        let mut payload_drift = r;
        payload_drift.scope = "organoid:other".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_contract_identity_is_rejected() {
        let mut q = request(
            ContractCompatibility::Additive,
            vec!["scope".into(), "evidence".into(), "provenance".into()],
        );
        q.request_id = " request:retrieval-contract".into();
        assert!(model_retrieval_contract(&q).is_err());
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
