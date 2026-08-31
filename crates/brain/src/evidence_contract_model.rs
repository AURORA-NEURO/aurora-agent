//! Typed evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-brain-P01-F05`. The model makes schema compatibility, required-field
//! closure, semantic loss, and canonical contract identity observable before a local study is
//! handed to an evidence engine.

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

pub const FEATURE_ID: &str = "AFA-brain-P01-F05";
pub const CONTRACT_VERSION: &str = "brain-evidence-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
const CONTRACT_CONTENT_TYPE: &str = "application/vnd.aurora.evidence-contract+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractCompatibility {
    Additive,
    MigrationRequired,
    Breaking,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceContractModelRequest {
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
pub struct EvidenceContractModelReceipt {
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
pub enum EvidenceContractModelError {
    #[error("invalid evidence contract model: {0}")]
    Invalid(String),
    #[error("evidence contract artifact failed: {0}")]
    Artifact(String),
}

impl EvidenceContractModelReceipt {
    pub fn validate(&self) -> Result<(), EvidenceContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.required_order.is_empty()
            || self.provided_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceContractModelError::Invalid(
                "contract identity, schemas, field closure, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.study_id, "study_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.required_order, "required_order"),
            (&self.provided_order, "provided_order"),
            (&self.missing_order, "missing_order"),
            (&self.semantic_loss_order, "semantic_loss_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        if self
            .missing_order
            .iter()
            .any(|field| !self.required_order.contains(field))
            || self
                .semantic_loss_order
                .iter()
                .any(|field| !self.provided_order.contains(field))
            || !identity_keys(&self.missing_order)
                .is_disjoint(&identity_keys(&self.semantic_loss_order))
        {
            return Err(EvidenceContractModelError::Invalid(
                "contract loss state is outside declared fields".into(),
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
                return Err(EvidenceContractModelError::Invalid(
                    "contract digest is invalid".into(),
                ));
            }
        }
        let gate_blocked = self.negative_evidence.iter().any(|item| {
            item == "request:policy-denied" || item == "request:raw-data-locality-failed"
        }) || self
            .uncertainty
            .iter()
            .any(|item| item == "request:protected-closure-incomplete");
        let expected_disposition = if gate_blocked || !self.raw_data_local {
            ContractDisposition::Blocked
        } else if self.missing_order.is_empty()
            && self.semantic_loss_order.is_empty()
            && self.compatibility == ContractCompatibility::Additive
        {
            ContractDisposition::Qualified
        } else {
            ContractDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(EvidenceContractModelError::Invalid(
                "contract disposition does not match field loss, compatibility, or gates".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(EvidenceContractModelError::Invalid(
                "evidence contract receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition == ContractDisposition::Qualified {
            vec![format!("read:local-research-artifacts:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(EvidenceContractModelError::Invalid(
                "contract effect does not match disposition".into(),
            ));
        }
        let expected_contract_digest = ContentHash::of_value(&json!({
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "required_order": self.required_order,
            "provided_order": self.provided_order,
            "semantic_digest": self.semantic_digest,
            "artifact_digest": self.artifact_digest,
            "provenance_digest": self.provenance_digest,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract_digest {
            return Err(EvidenceContractModelError::Invalid(
                "contract digest is not bound to declared fields".into(),
            ));
        }
        let expected_artifact_id = format!("brain-evidence-contract:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTRACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(EvidenceContractModelError::Invalid(
                "contract artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), EvidenceContractModelError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(EvidenceContractModelError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), EvidenceContractModelError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(EvidenceContractModelError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), EvidenceContractModelError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceContractModelError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &EvidenceContractModelReceipt) -> serde_json::Value {
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

pub fn evidence_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["agent developer".into(), "research software engineer".into()].into(),
        behavior: "validates a local EvidenceFeed contract for schema compatibility, required-field closure, semantic loss, and canonical identity".into(),
        value: "prevents incompatible or incomplete evidence data from entering a qualified evidence workflow".into(),
        inputs: vec![TypedPort { name: "evidence_feed_contract".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_evidence_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn model_evidence_contract(
    request: &EvidenceContractModelRequest,
) -> Result<EvidenceContractModelReceipt, EvidenceContractModelError> {
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
    if matches!(
        request.compatibility,
        ContractCompatibility::MigrationRequired
            | ContractCompatibility::Breaking
            | ContractCompatibility::Unknown
    ) {
        uncertainty.insert(format!(
            "contract:compatibility-{}",
            compatibility_label(request.compatibility)
        ));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let blocked_by_gate =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    let disposition = if blocked_by_gate {
        ContractDisposition::Blocked
    } else if provided.is_empty() {
        ContractDisposition::Unknown
    } else if missing.is_empty()
        && semantic_loss.is_empty()
        && request.compatibility == ContractCompatibility::Additive
    {
        ContractDisposition::Qualified
    } else {
        ContractDisposition::Partial
    };
    let required_order = required.into_iter().collect::<Vec<_>>();
    let provided_order = provided.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let raw_data_local = true;
    let contract_digest = ContentHash::of_value(&json!({
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "compatibility": request.compatibility,
        "required_order": required_order,
        "provided_order": provided_order,
        "semantic_digest": request.semantic_digest,
        "artifact_digest": request.artifact_digest,
        "provenance_digest": request.provenance_digest,
        "raw_data_local": raw_data_local,
    }))
    .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == ContractDisposition::Qualified {
        vec![format!(
            "read:local-research-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt_without_artifact = EvidenceContractModelReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        disposition,
        compatibility: request.compatibility,
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        required_order,
        provided_order,
        missing_order: missing,
        semantic_loss_order: semantic_loss,
        semantic_digest: request.semantic_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        provenance_digest: request.provenance_digest.clone(),
        contract_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact: TypedResearchArtifact::from_payload(
            "placeholder",
            CONTRACT_CONTENT_TYPE,
            &json!({}),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-contract:{}", request.request_id),
        CONTRACT_CONTENT_TYPE,
        &receipt_payload(&receipt_without_artifact),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
    let receipt = EvidenceContractModelReceipt {
        artifact,
        ..receipt_without_artifact
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &EvidenceContractModelRequest,
) -> Result<(), EvidenceContractModelError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.required_fields.is_empty()
        || request.provided_fields.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceContractModelError::Invalid(
            "contract identity, schemas, fields, or boundary is incomplete".into(),
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
    if request
        .required_fields
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || request
            .provided_fields
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(EvidenceContractModelError::Invalid(
            "contract fields must be unique and canonical".into(),
        ));
    }
    for digest in [
        &request.semantic_digest,
        &request.artifact_digest,
        &request.provenance_digest,
        &request.replay_identity,
    ] {
        if digest.as_str().len() != 64 {
            return Err(EvidenceContractModelError::Invalid(
                "contract request digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn compatibility_label(value: ContractCompatibility) -> &'static str {
    match value {
        ContractCompatibility::Additive => "additive",
        ContractCompatibility::MigrationRequired => "migration_required",
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
    fn request(provided_fields: Vec<String>) -> EvidenceContractModelRequest {
        EvidenceContractModelRequest {
            request_id: "request:contract".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            required_fields: vec![
                "artifact_digest".into(),
                "provenance_digest".into(),
                "scope".into(),
            ],
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
    fn manifest_is_a0_and_typed() {
        let manifest = evidence_contract_model_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn additive_complete_contract_qualifies() {
        let receipt = model_evidence_contract(&request(vec![
            "artifact_digest".into(),
            "provenance_digest".into(),
            "scope".into(),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ContractDisposition::Qualified);
        assert!(receipt.missing_order.is_empty());
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn missing_required_field_is_partial() {
        let receipt =
            model_evidence_contract(&request(vec!["provenance_digest".into(), "scope".into()]))
                .unwrap();
        assert_eq!(receipt.disposition, ContractDisposition::Partial);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("artifact_digest")));
    }
    #[test]
    fn migration_required_is_explicit() {
        let mut input = request(vec![
            "artifact_digest".into(),
            "provenance_digest".into(),
            "scope".into(),
        ]);
        input.compatibility = ContractCompatibility::MigrationRequired;
        let receipt = model_evidence_contract(&input).unwrap();
        assert_eq!(receipt.disposition, ContractDisposition::Partial);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("migration_required")));
    }
    #[test]
    fn non_local_contract_is_blocked_and_retained() {
        let mut input = request(vec![
            "artifact_digest".into(),
            "provenance_digest".into(),
            "scope".into(),
        ]);
        input.raw_data_local = false;
        let receipt = model_evidence_contract(&input).unwrap();
        assert_eq!(receipt.disposition, ContractDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .negative_evidence
            .contains(&"request:raw-data-locality-failed".into()));
    }

    #[test]
    fn artifact_payload_is_bound() {
        let mut receipt = model_evidence_contract(&request(vec![
            "artifact_digest".into(),
            "provenance_digest".into(),
            "scope".into(),
        ]))
        .unwrap();
        receipt.artifact.content_hash = hash("tampered");
        assert!(matches!(
            receipt.validate(),
            Err(EvidenceContractModelError::Artifact(_))
        ));
    }

    #[test]
    fn policy_denial_blocks_contract() {
        let mut input = request(vec![
            "artifact_digest".into(),
            "provenance_digest".into(),
            "scope".into(),
        ]);
        input.policy_allow = false;
        let receipt = model_evidence_contract(&input).unwrap();
        assert_eq!(receipt.disposition, ContractDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn schema_mismatch_is_rejected() {
        let mut input = request(vec![
            "artifact_digest".into(),
            "provenance_digest".into(),
            "scope".into(),
        ]);
        input.input_schema = "EvidenceFeed0@1".into();
        assert!(model_evidence_contract(&input).is_err());
    }
}
