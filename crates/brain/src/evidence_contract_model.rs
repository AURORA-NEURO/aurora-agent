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
            || !self.raw_data_local
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
        if self
            .missing_order
            .iter()
            .any(|field| !self.required_order.contains(field))
            || self
                .semantic_loss_order
                .iter()
                .any(|field| !self.provided_order.contains(field))
        {
            return Err(EvidenceContractModelError::Invalid(
                "contract loss state is outside declared fields".into(),
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
                return Err(EvidenceContractModelError::Invalid(
                    "contract ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(EvidenceContractModelError::Invalid(
                "effect is outside the contract-model gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
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
    let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": request.compatibility, "required_order": required, "provided_order": provided, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest})).map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "disposition": disposition, "compatibility": request.compatibility, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "required_order": required, "provided_order": provided, "missing_order": missing, "semantic_loss_order": semantic_loss, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-contract:{}", request.request_id),
        "application/vnd.aurora.evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceContractModelError::Artifact(error.to_string()))?;
    let has_qualified = disposition == ContractDisposition::Qualified;
    let receipt = EvidenceContractModelReceipt {
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
        effect_receipts: if has_qualified {
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
