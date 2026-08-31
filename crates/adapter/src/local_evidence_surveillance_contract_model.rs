//! Local single-study evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-adapter-P01-F05`. This is a typed schema/compatibility product surface,
//! distinct from the F01 inference engine: it proves that migrations retain uncertainty and
//! semantic loss instead of changing an evidence claim into a silent default.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F05";
pub const CONTRACT_VERSION: &str = "adapter-local-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractModelClaim {
    pub claim_id: String,
    pub semantic_type: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub claims: Vec<ContractModelClaim>,
    pub required_claim_ids: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractCompatibilityDisposition {
    Compatible,
    AdditiveMigration,
    Breaking,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractModelDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: LocalEvidenceSurveillanceContractRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibilityDisposition,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: ContractModelDisposition,
    pub candidate_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub required_order: Vec<String>,
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
pub enum LocalEvidenceSurveillanceContractError {
    #[error("invalid local evidence contract request: {0}")]
    Invalid(String),
    #[error("local evidence contract artifact failed: {0}")]
    Artifact(String),
}
fn validate_text(field: &str, value: &str) -> Result<(), LocalEvidenceSurveillanceContractError> {
    if value.is_empty() || value.trim() != value {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), LocalEvidenceSurveillanceContractError> {
    if values.len() > MAX_ITEMS {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), LocalEvidenceSurveillanceContractError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), LocalEvidenceSurveillanceContractError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn canonical_local_evidence_surveillance_request(
    request: &LocalEvidenceSurveillanceContractRequest,
) -> LocalEvidenceSurveillanceContractRequest {
    let mut canonical = request.clone();
    canonical
        .claims
        .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    canonical
}

fn contract_input_digest(
    request: &LocalEvidenceSurveillanceContractRequest,
) -> Result<ContentHash, LocalEvidenceSurveillanceContractError> {
    let canonical = canonical_local_evidence_surveillance_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))
}

impl LocalEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), LocalEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "contract identity, schemas, locality, candidates, or effects are incomplete"
                    .into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("input_schema", &self.input_schema)?;
        validate_text("output_schema", &self.output_schema)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("retained_order", &self.retained_order)?;
        validate_sorted_strings("unknown_order", &self.unknown_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("migration_order", &self.migration_order)?;
        validate_sorted_strings("semantic_loss", &self.semantic_loss)?;
        validate_sorted_strings("required_order", &self.required_order)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        let classified = self
            .retained_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "contract states do not partition candidates".into(),
            ));
        }
        let expected_compatibility =
            if self.input_schema == INPUT_SCHEMA && self.output_schema == OUTPUT_SCHEMA {
                ContractCompatibilityDisposition::AdditiveMigration
            } else if self.input_schema == self.output_schema {
                ContractCompatibilityDisposition::Compatible
            } else {
                ContractCompatibilityDisposition::Breaking
            };
        if self.compatibility != expected_compatibility {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "contract compatibility does not match its input and output schemas".into(),
            ));
        }
        let required_missing = self
            .required_order
            .iter()
            .any(|claim_id| !self.retained_order.contains(claim_id));
        if !self
            .required_order
            .iter()
            .all(|claim_id| self.candidate_order.contains(claim_id))
        {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "required claims must be represented in the candidate closure".into(),
            ));
        }
        for digest in [
            &self.contract_digest,
            &self.canonical_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("local contract receipt digest", digest)?;
        }
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        let expected_disposition = if should_block {
            ContractModelDisposition::Blocked
        } else if self.retained_order.is_empty() {
            ContractModelDisposition::Unknown
        } else if self.compatibility == ContractCompatibilityDisposition::Breaking
            || !self.unknown_order.is_empty()
            || !self.denied_order.is_empty()
            || required_missing
        {
            ContractModelDisposition::Partial
        } else {
            ContractModelDisposition::Compatible
        };
        if self.disposition != expected_disposition {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "contract disposition does not match migration and claim states".into(),
            ));
        }
        if matches!(
            self.disposition,
            ContractModelDisposition::Unknown | ContractModelDisposition::Blocked
        ) && !self.retained_order.is_empty()
        {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "unknown or blocked contract cannot retain claims".into(),
            ));
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!("read:local-contract:{}", self.request_id)]
        };
        if self.effect_receipts != expected_effect {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "contract effect does not match its release state".into(),
            ));
        }
        let expected_contract = ContentHash::of_value(&json!({
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "candidate_order": self.candidate_order,
            "required_order": self.required_order,
        }))
        .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "contract digest does not match schema and candidate closure".into(),
            ));
        }
        let expected_canonical = ContentHash::of_value(&json!({
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
        }))
        .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
        if self.canonical_digest != expected_canonical {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "canonical digest does not match migration and semantic-loss states".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
        }))
        .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
        if self.provenance_digest != expected_provenance {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "provenance digest does not match contract identity".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-local-evidence-contract:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.local-evidence-contract+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(LocalEvidenceSurveillanceContractError::Artifact(
                "contract artifact is not bound to the receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "disposition": self.disposition,
            "candidate_order": self.candidate_order,
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
            "required_order": self.required_order,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
        if self.input_digest != contract_input_digest(&self.input)? {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "local contract retained input digest mismatch".into(),
            ));
        }
        let expected = build_local_evidence_surveillance_contract(&self.input)?;
        if self != &expected {
            return Err(LocalEvidenceSurveillanceContractError::Invalid(
                "local contract receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, LocalEvidenceSurveillanceContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))
    }
}

pub fn local_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["integration engineer".into(), "schema steward".into()].into(), behavior: "validates EvidenceFeed1 into a versioned QualifiedEvidenceSet2 with deterministic compatibility and semantic-loss witnesses".into(), value: "makes additive migration, breaking schema changes, omissions, and unknown evidence machine-checkable".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "JSON Schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_local_evidence_surveillance_contract(
    request: &LocalEvidenceSurveillanceContractRequest,
) -> Result<LocalEvidenceSurveillanceContractReceipt, LocalEvidenceSurveillanceContractError> {
    let receipt = build_local_evidence_surveillance_contract(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_local_evidence_surveillance_contract(
    request: &LocalEvidenceSurveillanceContractRequest,
) -> Result<LocalEvidenceSurveillanceContractReceipt, LocalEvidenceSurveillanceContractError> {
    if request.request_id.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.claims.is_empty()
        || request.claims.len() > MAX_ITEMS
        || request.required_claim_ids.len() > MAX_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(
            "contract identity, schemas, claims, replay, locality, or boundary is invalid".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("input_schema", &request.input_schema)?;
    validate_text("output_schema", &request.output_schema)?;
    validate_text("boundary", &request.boundary)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    validate_sorted_strings("required_claim_ids", &request.required_claim_ids)?;
    for claim in &request.claims {
        validate_text("claim.claim_id", &claim.claim_id)?;
        validate_text("claim.semantic_type", &claim.semantic_type)?;
        validate_digest("claim.value_digest", &claim.value_digest)?;
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_ids = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if claim_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LocalEvidenceSurveillanceContractError::Invalid(
            "claim identities must be unique".into(),
        ));
    }
    let mut candidate_set = claim_ids.into_iter().collect::<BTreeSet<_>>();
    candidate_set.extend(request.required_claim_ids.iter().cloned());
    let candidate_order = candidate_set.into_iter().collect::<Vec<_>>();
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            ContractCompatibilityDisposition::AdditiveMigration
        } else if request.input_schema == request.output_schema {
            ContractCompatibilityDisposition::Compatible
        } else {
            ContractCompatibilityDisposition::Breaking
        };
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    let required = request
        .required_claim_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let global_release_blocked =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    for claim in &claims {
        if global_release_blocked {
            denied.insert(claim.claim_id.clone());
            semantic_loss.insert(format!("claim:{}:release-gate", claim.claim_id));
        } else if compatibility == ContractCompatibilityDisposition::Breaking {
            denied.insert(claim.claim_id.clone());
            semantic_loss.insert(format!("claim:{}:breaking-schema", claim.claim_id));
        } else if claim.omitted {
            unknown.insert(claim.claim_id.clone());
            semantic_loss.insert(format!("claim:{}:omitted-preserved", claim.claim_id));
        } else if matches!(
            claim.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unknown.insert(claim.claim_id.clone());
            semantic_loss.insert(format!("claim:{}:unknown-not-asserted", claim.claim_id));
        } else if claim.evidence_state == EvidenceState::Contradicted {
            denied.insert(claim.claim_id.clone());
            semantic_loss.insert(format!("claim:{}:contradicted-retained", claim.claim_id));
        } else {
            retained.insert(claim.claim_id.clone());
            if compatibility == ContractCompatibilityDisposition::AdditiveMigration {
                migration.insert(format!("claim:{}:evidence-state-preserved", claim.claim_id));
            }
            if claim.negative_result {
                semantic_loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    let known = candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    for claim_id in &required {
        if !known.contains(claim_id) || (!retained.contains(claim_id) && !denied.contains(claim_id))
        {
            unknown.insert(claim_id.clone());
            semantic_loss.insert(format!("claim:{}:required-unresolved", claim_id));
        }
    }
    if !request.policy_allow {
        semantic_loss.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        semantic_loss.insert("control:protected-closure-incomplete".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            ContractModelDisposition::Blocked
        } else if retained.is_empty() {
            ContractModelDisposition::Unknown
        } else if compatibility == ContractCompatibilityDisposition::Breaking
            || !unknown.is_empty()
            || !denied.is_empty()
        {
            ContractModelDisposition::Partial
        } else {
            ContractModelDisposition::Compatible
        };
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let migration_order = migration.iter().cloned().collect::<Vec<_>>();
    let semantic_loss = semantic_loss.iter().cloned().collect::<Vec<_>>();
    let required_order = required.iter().cloned().collect::<Vec<_>>();
    let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "candidate_order": candidate_order.clone(), "required_order": required_order.clone()})).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({"retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "migration_order": migration_order.clone(), "semantic_loss": semantic_loss.clone()})).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "contract_digest": contract_digest, "canonical_digest": canonical_digest})).map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == ContractModelDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!("read:local-contract:{}", request.request_id)]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "policy_allow": request.policy_allow, "protected_closure": request.protected_closure, "disposition": disposition, "candidate_order": candidate_order, "retained_order": retained_order, "unknown_order": unknown_order, "denied_order": denied_order, "migration_order": migration_order, "semantic_loss": semantic_loss, "required_order": required_order, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-local-evidence-contract:{}", request.request_id),
        "application/vnd.aurora.local-evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| LocalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_request = canonical_local_evidence_surveillance_request(request);
    let receipt = LocalEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: contract_input_digest(request)?,
        request_id: request.request_id.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        compatibility,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        candidate_order,
        retained_order,
        unknown_order,
        denied_order,
        migration_order,
        semantic_loss,
        required_order,
        contract_digest,
        canonical_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> LocalEvidenceSurveillanceContractRequest {
        let digest = hash("contract");
        let claim = |id: &str, state: EvidenceState| ContractModelClaim {
            claim_id: id.into(),
            semantic_type: "evidence".into(),
            value_digest: digest.clone(),
            evidence_state: state,
            omitted: false,
            negative_result: false,
        };
        LocalEvidenceSurveillanceContractRequest {
            request_id: "request:contract".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            claims: vec![
                claim("claim:a", EvidenceState::Supported),
                claim("claim:b", EvidenceState::Supported),
            ],
            required_claim_ids: vec!["claim:a".into()],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: digest,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            local_evidence_surveillance_contract_model_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn additive_model_is_compatible() {
        assert_eq!(
            model_local_evidence_surveillance_contract(&request())
                .unwrap()
                .compatibility,
            ContractCompatibilityDisposition::AdditiveMigration
        );
    }
    #[test]
    fn required_missing_is_unknown() {
        let mut value = request();
        value.required_claim_ids.push("claim:missing".into());
        assert!(model_local_evidence_surveillance_contract(&value)
            .unwrap()
            .unknown_order
            .contains(&"claim:missing".to_string()));
    }
    #[test]
    fn unknown_is_preserved() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert!(model_local_evidence_surveillance_contract(&value)
            .unwrap()
            .semantic_loss
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn breaking_schema_is_partial() {
        let mut value = request();
        value.input_schema = "EvidenceFeed9@1".into();
        let receipt = model_local_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(
            receipt.compatibility,
            ContractCompatibilityDisposition::Breaking
        );
        assert_eq!(receipt.input_schema, "EvidenceFeed9@1");
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            model_local_evidence_surveillance_contract(&value)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        );
    }
    #[test]
    fn policy_block_clears_retained_claims() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = model_local_evidence_surveillance_contract(&value).unwrap();
        assert!(receipt.retained_order.is_empty());
        assert_eq!(receipt.disposition, ContractModelDisposition::Blocked);
    }
    #[test]
    fn duplicate_required_claim_is_rejected() {
        let mut value = request();
        value.required_claim_ids.push("claim:a".into());
        assert!(model_local_evidence_surveillance_contract(&value).is_err());
    }
    #[test]
    fn tampered_contract_digest_is_rejected() {
        let mut receipt = model_local_evidence_surveillance_contract(&request()).unwrap();
        receipt.contract_digest = hash("tampered-contract");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_canonical_digest_is_rejected() {
        let mut receipt = model_local_evidence_surveillance_contract(&request()).unwrap();
        receipt.canonical_digest = hash("tampered-canonical");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt = model_local_evidence_surveillance_contract(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn digest_is_stable() {
        let first = model_local_evidence_surveillance_contract(&request()).unwrap();
        let second = model_local_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn reordered_claims_share_the_same_retained_input_identity() {
        let mut reordered = request();
        reordered.claims.reverse();
        let first = model_local_evidence_surveillance_contract(&request()).unwrap();
        let second = model_local_evidence_surveillance_contract(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn receipt_rejects_tampered_retained_claim() {
        let mut receipt = model_local_evidence_surveillance_contract(&request()).unwrap();
        receipt.input.claims[0].semantic_type = "tampered-type".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
