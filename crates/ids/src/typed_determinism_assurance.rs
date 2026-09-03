//! Federated continual typed-determinism assurance (`AFA-ids-P17-F28`).
//!
//! This assurance harness is a product boundary for formal methods researchers: it verifies
//! caller-supplied canonical capability outputs across independent implementations. It records
//! every mismatch and omission, preserves negative evidence, and emits only digest-bound local
//! release evidence. It never executes a provider, moves raw data, or asserts scientific truth.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P17-F28";
pub const CONTRACT_VERSION: &str =
    "ids-federated-continual-typed-determinism-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "TypedCapabilityInput4@1";
pub const OUTPUT_SCHEMA: &str = "CanonicalCapabilityOutput7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.canonical-capability-output-7+json";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityImplementation5 {
    pub implementation_id: String,
    pub origin: String,
    pub semantic_profile: String,
    pub canonical_field_order: Vec<String>,
    pub input_digest: ContentHash,
    pub output_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: CapabilityEvidenceState,
    pub local: bool,
    pub aggregate_only: bool,
    pub signed: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedCapabilityInput4 {
    pub schema_version: String,
    pub request_id: String,
    pub capability_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub canonical_field_order: Vec<String>,
    pub input_digest: ContentHash,
    pub expected_output_digest: ContentHash,
    pub implementations: Vec<CapabilityImplementation5>,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalCapabilityArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalCapabilityOutput7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub capability_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub implementation_order: Vec<String>,
    pub verified_order: Vec<String>,
    pub mismatch_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub canonical_field_order: Vec<String>,
    pub input_digest: ContentHash,
    pub canonical_output_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub receipt_digest: ContentHash,
    pub artifact: CanonicalCapabilityArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TypedDeterminismAssuranceError {
    #[error("invalid typed-capability input: {0}")]
    Invalid(String),
    #[error("canonical capability output failed validation: {0}")]
    Output(String),
}

fn nonempty(value: &str) -> bool { !value.trim().is_empty() }
fn canonical(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn typed_determinism_assurance_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["formal methods researcher", "verification engineer", "release auditor"],
        "behavior": "verify typed canonical capability outputs across federated continual implementations",
        "value": "turns cross-language parity, omissions, and negative evidence into release-gated receipts",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["verify:canonical-parity", "block:unsafe-release"],
        "permissions": ["evaluate:capability-runs"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl CanonicalCapabilityOutput7 {
    pub fn validate(&self) -> Result<(), TypedDeterminismAssuranceError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || !nonempty(&self.request_id)
            || !nonempty(&self.capability_id)
            || !nonempty(&self.scope)
            || !nonempty(&self.semantic_profile)
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.implementation_order.is_empty()
            || self.canonical_field_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(TypedDeterminismAssuranceError::Output("identity, closure, locality, or effect metadata is incomplete".into()));
        }
        for values in [
            &self.implementation_order,
            &self.verified_order,
            &self.mismatch_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.canonical_field_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(TypedDeterminismAssuranceError::Output("output ordering is not canonical".into()));
            }
        }
        let all = self.implementation_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self.verified_order.iter().chain(&self.mismatch_order).chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<BTreeSet<_>>();
        if all.len() != self.implementation_order.len() || parts != all {
            return Err(TypedDeterminismAssuranceError::Output("implementation states do not partition the input".into()));
        }
        if !digest(&self.input_digest) || !digest(&self.canonical_output_digest) || !digest(&self.replay_identity) || !digest(&self.receipt_digest) || self.artifact.content_hash != self.receipt_digest {
            return Err(TypedDeterminismAssuranceError::Output("content digests are invalid".into()));
        }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("verify:canonical-parity:")) {
            return Err(TypedDeterminismAssuranceError::Output("effect is outside the parity gate".into()));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, TypedDeterminismAssuranceError> {
        self.validate()?;
        ContentHash::of_value(&serde_json::to_value(self).map_err(|error| TypedDeterminismAssuranceError::Output(error.to_string()))?).map_err(|error| TypedDeterminismAssuranceError::Output(error.to_string()))
    }
}

fn validate_input(input: &TypedCapabilityInput4) -> Result<(), TypedDeterminismAssuranceError> {
    if input.schema_version != INPUT_SCHEMA
        || !nonempty(&input.request_id)
        || !nonempty(&input.capability_id)
        || !nonempty(&input.scope)
        || !nonempty(&input.semantic_profile)
        || input.canonical_field_order.is_empty()
        || !canonical(&input.canonical_field_order)
        || input.implementations.is_empty()
        || !digest(&input.input_digest)
        || !digest(&input.expected_output_digest)
        || !digest(&input.replay_identity)
        || !canonical(&input.adversarial_events)
        || input.boundary != PRECLINICAL_BOUNDARY
        || !input.raw_data_local
        || !input.aggregate_only
    {
        return Err(TypedDeterminismAssuranceError::Invalid("input identity, fields, digests, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for implementation in &input.implementations {
        if !nonempty(&implementation.implementation_id)
            || !nonempty(&implementation.origin)
            || !nonempty(&implementation.semantic_profile)
            || implementation.canonical_field_order.is_empty()
            || !canonical(&implementation.canonical_field_order)
            || !digest(&implementation.input_digest)
            || !digest(&implementation.output_digest)
            || !digest(&implementation.provenance_digest)
            || !digest(&implementation.replay_identity)
            || !canonical(&implementation.omission_order)
            || !ids.insert(implementation.implementation_id.clone())
        {
            return Err(TypedDeterminismAssuranceError::Invalid("implementation identity, ordering, digests, or uniqueness is invalid".into()));
        }
    }
    Ok(())
}

pub fn assure_typed_determinism(input: &TypedCapabilityInput4) -> Result<CanonicalCapabilityOutput7, TypedDeterminismAssuranceError> {
    validate_input(input)?;
    let mut implementations = input.implementations.clone();
    implementations.sort_by(|left, right| left.implementation_id.cmp(&right.implementation_id));
    let implementation_order = implementations.iter().map(|item| item.implementation_id.clone()).collect::<Vec<_>>();
    let mut verified = BTreeSet::new();
    let mut mismatch = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for item in &implementations {
        let id = item.implementation_id.clone();
        provenance.insert(item.provenance_digest.clone());
        omission.extend(item.omission_order.iter().map(|value| format!("{id}:{value}")));
        if item.negative_result || item.evidence_state == CapabilityEvidenceState::Negative {
            negative.insert(format!("{id}:negative-result"));
        }
        if item.semantic_profile != input.semantic_profile || !item.local || !item.aggregate_only {
            blocked.insert(id.clone());
            omission.insert(format!("{id}:semantic-profile-or-locality-mismatch"));
        } else if item.canonical_field_order != input.canonical_field_order || item.input_digest != input.input_digest || item.output_digest != input.expected_output_digest {
            mismatch.insert(id.clone());
            omission.insert(format!("{id}:canonical-input-or-output-mismatch"));
        } else if item.replay_identity != input.replay_identity || !item.signed {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:replay-or-signature-unresolved"));
        } else if !matches!(item.evidence_state, CapabilityEvidenceState::Proven | CapabilityEvidenceState::Supported) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-not-proven"));
        } else {
            verified.insert(id);
        }
    }
    let global_block = !input.policy_allowed || !input.protected_closure || !input.signed_approval || !input.raw_data_local || !input.aggregate_only || !input.adversarial_events.is_empty();
    if global_block {
        blocked.extend(implementation_order.iter().cloned());
        verified.clear();
        mismatch.clear();
        unresolved.clear();
        omission.insert("request:governance-or-adversarial-gate-blocked".into());
    }
    uncertainty.extend(input.adversarial_events.iter().map(|event| format!("adversarial:{event}")));
    let verified_order = verified.iter().cloned().collect::<Vec<_>>();
    let mismatch_order = mismatch.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let disposition = if global_block || verified_order.is_empty() && unresolved_order.is_empty() { "blocked" } else if !mismatch_order.is_empty() || !unresolved_order.is_empty() || !blocked_order.is_empty() { "unresolved" } else { "qualified" };
    if disposition != "qualified" { omission.insert("request:canonical-parity-not-closed".into()); }
    let mut payload = json!({
        "schema_version": "aurora-research-contract/1.0", "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": input.request_id, "capability_id": input.capability_id, "scope": input.scope, "semantic_profile": input.semantic_profile,
        "disposition": disposition, "implementation_order": implementation_order, "verified_order": verified_order,
        "mismatch_order": mismatch_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order,
        "omission_order": omission.iter().cloned().collect::<Vec<_>>(), "uncertainty_order": uncertainty.iter().cloned().collect::<Vec<_>>(),
        "negative_evidence_order": negative.iter().cloned().collect::<Vec<_>>(), "canonical_field_order": input.canonical_field_order,
        "input_digest": input.input_digest, "canonical_output_digest": input.expected_output_digest, "replay_identity": input.replay_identity,
        "raw_data_local": true, "aggregate_only": true, "boundary": PRECLINICAL_BOUNDARY,
    });
    let receipt_digest = ContentHash::of_value(&payload).map_err(|error| TypedDeterminismAssuranceError::Output(error.to_string()))?;
    payload["receipt_digest"] = json!(receipt_digest);
    payload["artifact"] = json!({"artifact_id": format!("canonical-capability-output-7:{}", input.request_id), "content_type": CONTENT_TYPE, "content_hash": receipt_digest, "semantic_loss": omission.iter().cloned().collect::<Vec<_>>(), "provenance_digests": provenance.iter().cloned().collect::<Vec<_>>(), "boundary": PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" { vec![format!("verify:canonical-parity:{}", input.request_id)] } else { vec!["block:unsafe-release".to_string()] });
    let output: CanonicalCapabilityOutput7 = serde_json::from_value(payload).map_err(|error| TypedDeterminismAssuranceError::Output(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn implementation(id: &str) -> CapabilityImplementation5 { CapabilityImplementation5 { implementation_id: id.into(), origin: format!("origin:{id}"), semantic_profile: "canonical-json".into(), canonical_field_order: vec!["alpha".into(), "beta".into()], input_digest: hash("input"), output_digest: hash("output"), provenance_digest: hash("provenance"), replay_identity: hash("replay"), evidence_state: CapabilityEvidenceState::Proven, local: true, aggregate_only: true, signed: true, omission_order: vec![], negative_result: false } }
    fn input() -> TypedCapabilityInput4 { TypedCapabilityInput4 { schema_version: INPUT_SCHEMA.into(), request_id: "typed:req".into(), capability_id: "cap:determinism".into(), scope: "study:local".into(), semantic_profile: "canonical-json".into(), canonical_field_order: vec!["alpha".into(), "beta".into()], input_digest: hash("input"), expected_output_digest: hash("output"), implementations: vec![implementation("b"), implementation("a")], replay_identity: hash("replay"), policy_allowed: true, protected_closure: true, signed_approval: true, raw_data_local: true, aggregate_only: true, adversarial_events: vec![], boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(typed_determinism_assurance_manifest()["autonomy_tier"], "A1"); }
    #[test] fn qualified_parity_is_sorted() { let output = assure_typed_determinism(&input()).unwrap(); assert_eq!(output.disposition, "qualified"); assert_eq!(output.verified_order, vec!["a", "b"]); }
    #[test] fn mismatch_is_preserved() { let mut request = input(); request.implementations[0].output_digest = hash("different"); let output = assure_typed_determinism(&request).unwrap(); assert_eq!(output.disposition, "unresolved"); assert!(!output.mismatch_order.is_empty()); }
    #[test] fn policy_blocks() { let mut request = input(); request.policy_allowed = false; let output = assure_typed_determinism(&request).unwrap(); assert_eq!(output.disposition, "blocked"); assert_eq!(output.effect_receipts, vec!["block:unsafe-release"]); }
}
