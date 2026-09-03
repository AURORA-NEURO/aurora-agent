//! Typed context-compilation interoperability gateways (P03 F21-F24).
//!
//! Gateways negotiate a versioned semantic profile and export only aggregate, content-addressed
//! context metadata.  They are intentionally incapable of moving raw facts or invoking a remote
//! provider; denied and incomplete exchanges remain explicit receipts.

use std::collections::BTreeSet;

use super::context_compilation_support::{self, ContextCompilationRequest};
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.context-interoperability-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextInteroperabilityRequest {
    pub context_request: ContextCompilationRequest,
    pub partner_id: String,
    pub semantic_profile: String,
    pub expected_contract_version: String,
    pub requested_export_order: Vec<String>,
    pub permitted_export_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextInteroperabilityReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub partner_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub requested_export_order: Vec<String>,
    pub permitted_export_order: Vec<String>,
    pub exported_order: Vec<String>,
    pub denied_export_order: Vec<String>,
    pub context_disposition: String,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub envelope_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextInteroperabilityError {
    #[error("invalid context interoperability request: {0}")]
    Invalid(String),
    #[error("context interoperability compilation failed: {0}")]
    Compilation(String),
    #[error("context interoperability artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn sorted(values: &[String]) -> Vec<String> {
    let mut output = values.to_vec();
    output.sort();
    output.dedup();
    output
}

impl ContextInteroperabilityReceipt {
    pub fn validate(&self) -> Result<(), ContextInteroperabilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|value| value.as_str())
                != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|value| value.as_str())
                != Some(CONTENT_TYPE)
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.partner_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.requested_export_order.is_empty()
            || self.permitted_export_order.is_empty()
            || !ordered(&self.requested_export_order)
            || !ordered(&self.permitted_export_order)
            || !ordered(&self.exported_order)
            || !ordered(&self.denied_export_order)
            || self.effect_receipts.is_empty()
            || ![&self.context_digest, &self.replay_identity, &self.envelope_digest]
                .into_iter()
                .all(digest)
        {
            return Err(ContextInteroperabilityError::Invalid(
                "gateway identity, export contract, locality, ordering, digests, or effects are incomplete".into(),
            ));
        }
        let requested = self.requested_export_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .exported_order
            .iter()
            .chain(&self.denied_export_order)
            .cloned()
            .collect::<Vec<_>>();
        if requested.len() != self.requested_export_order.len()
            || parts.len() != requested.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != requested
        {
            return Err(ContextInteroperabilityError::Invalid(
                "requested exports do not partition into permitted and denied state".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("export:worldgen-context:")
        }) {
            return Err(ContextInteroperabilityError::Invalid(
                "gateway effect is outside the aggregate export gate".into(),
            ));
        }
        if self.artifact.get("content_hash").and_then(|value| value.as_str())
            != Some(self.envelope_digest.as_str())
            || self.artifact.get("raw_facts").and_then(|value| value.as_bool()) != Some(false)
        {
            return Err(ContextInteroperabilityError::Invalid(
                "gateway artifact digest or raw-data boundary is inconsistent".into(),
            ));
        }
        Ok(())
    }
}

pub fn manifest(
    feature_id: &str,
    version: &str,
    input_schema: &str,
    scale: &str,
    autonomy: &str,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": version,
        "owner_crate": "worldgen",
        "consumers": ["schema steward", "federation operator", "downstream context consumer"],
        "behavior": format!("negotiate a typed aggregate-only context exchange for {scale}"),
        "value": "prevents semantic or policy-incompatible context artifacts from crossing a research boundary",
        "input_schema": input_schema,
        "output_schema": "FederationEnvelopeContext1@1",
        "effects": ["export:worldgen-context", "block:unsafe-release"],
        "permissions": ["export:aggregate-context-metadata"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy,
        "boundary": PRECLINICAL_BOUNDARY,
        "contract_version": version
    })
}

pub fn negotiate(
    request: &ContextInteroperabilityRequest,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    require_approval: bool,
    require_federation: bool,
) -> Result<ContextInteroperabilityReceipt, ContextInteroperabilityError> {
    if request.partner_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.expected_contract_version.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.context_request.boundary != PRECLINICAL_BOUNDARY
        || !request.context_request.raw_data_local
        || !request.context_request.aggregate_only
        || !digest(&request.replay_identity)
        || request.replay_identity != request.context_request.replay_identity
        || request.requested_export_order.is_empty()
        || sorted(&request.requested_export_order) != request.requested_export_order
        || request.permitted_export_order.is_empty()
        || sorted(&request.permitted_export_order) != request.permitted_export_order
    {
        return Err(ContextInteroperabilityError::Invalid(
            "gateway identity, semantic profile, export order, locality, boundary, or replay is invalid".into(),
        ));
    }
    let context = context_compilation_support::compile(
        &request.context_request,
        feature_id,
        contract_version,
        scale,
        require_federation,
    )
    .map_err(|error| ContextInteroperabilityError::Compilation(error.to_string()))?;
    let approval_ok = !require_approval || request.signed_approval;
    let federation_ok = !require_federation || request.federation_approved;
    let contract_ok = request.expected_contract_version == contract_version;
    let permitted = request
        .requested_export_order
        .iter()
        .filter(|field| request.permitted_export_order.contains(field))
        .cloned()
        .collect::<Vec<_>>();
    let denied = request
        .requested_export_order
        .iter()
        .filter(|field| !request.permitted_export_order.contains(field))
        .cloned()
        .collect::<Vec<_>>();
    let safe = context.disposition == "qualified"
        && approval_ok
        && federation_ok
        && contract_ok
        && denied.is_empty();
    let disposition = if !approval_ok || !federation_ok || !contract_ok || context.disposition == "blocked" {
        "blocked"
    } else if safe {
        "qualified"
    } else {
        "partial"
    };
    let mut omissions = context.omissions.clone();
    if !approval_ok { omissions.push("gateway:signed-approval-missing".into()); }
    if !federation_ok { omissions.push("gateway:federation-approval-missing".into()); }
    if !contract_ok { omissions.push("gateway:contract-version-mismatch".into()); }
    if !denied.is_empty() { omissions.push("gateway:requested-field-not-permitted".into()); }
    omissions.sort(); omissions.dedup();
    let effect_receipts = if disposition == "qualified" {
        vec![format!("export:worldgen-context:{}", request.partner_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request.context_request.request_id,
        "partner_id": request.partner_id,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "requested_export_order": request.requested_export_order,
        "permitted_export_order": request.permitted_export_order,
        "exported_order": if safe { permitted.clone() } else { Vec::<String>::new() },
        "denied_export_order": if safe { denied.clone() } else { request.requested_export_order.clone() },
        "context_disposition": context.disposition,
        "context_digest": context.context_digest,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": context.uncertainty,
        "negative_evidence": context.negative_evidence,
        "raw_facts": false,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let envelope_digest = ContentHash::of_value(&payload)
        .map_err(|error| ContextInteroperabilityError::Artifact(error.to_string()))?;
    let exported_order = if safe { permitted } else { Vec::new() };
    let denied_export_order = if safe { denied } else { request.requested_export_order.clone() };
    let artifact = json!({
        "artifact_id": format!("worldgen-context-envelope:{}", request.partner_id),
        "content_type": CONTENT_TYPE,
        "content_hash": envelope_digest,
        "raw_facts": false,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
        "exported_order": exported_order,
    });
    let receipt = ContextInteroperabilityReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.context_request.request_id.clone(),
        partner_id: request.partner_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        requested_export_order: request.requested_export_order.clone(),
        permitted_export_order: request.permitted_export_order.clone(),
        exported_order,
        denied_export_order,
        context_disposition: context.disposition,
        context_digest: context.context_digest,
        replay_identity: request.replay_identity.clone(),
        envelope_digest,
        omissions: sorted(&payload["omissions"].as_array().unwrap().iter().map(|value| value.as_str().unwrap().to_owned()).collect::<Vec<_>>()),
        uncertainty: sorted(&context.uncertainty),
        negative_evidence: sorted(&context.negative_evidence),
        effect_receipts: sorted(&effect_receipts),
        artifact,
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
    use crate::context_compilation_support::{ContextCompilationRequest, ContextFact};
    use bioprism_foundation::EvidenceState;
    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn request() -> ContextInteroperabilityRequest {
        let replay = hash("replay");
        let fact = ContextFact { fact_id: "fact:interop".into(), statement: "supported".into(), support_milli: 950, state: EvidenceState::Supported, evidence_digest: hash("e"), provenance_digest: hash("p"), artifact_digest: hash("a"), replay_identity: replay.clone(), negative_result: false, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
        ContextInteroperabilityRequest { context_request: ContextCompilationRequest { request_id: "interop:req".into(), objective: "exchange context".into(), scope: "study:interop".into(), required_fact_order: vec!["fact:interop".into()], minimum_support_milli: 500, facts: vec![fact], replay_identity: replay.clone(), policy_allow: true, protected_closure: true, federation_approved: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() }, partner_id: "partner:one".into(), semantic_profile: "context-v1".into(), expected_contract_version: "worldgen-local-context-compilation-gateway/1.0".into(), requested_export_order: vec!["context_digest".into(), "provenance_digest".into()], permitted_export_order: vec!["context_digest".into(), "provenance_digest".into()], replay_identity: replay, signed_approval: true, federation_approved: true, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn aggregate_exchange_is_qualified() { let r = negotiate(&request(), "AFA-worldgen-P03-F21", "worldgen-local-context-compilation-gateway/1.0", "local single-study", false, false).unwrap(); assert_eq!(r.disposition, "qualified"); assert!(r.artifact["raw_facts"] == false); }
    #[test] fn unpermitted_field_blocks() { let mut q = request(); q.requested_export_order.push("raw_fact".into()); q.requested_export_order.sort(); let r = negotiate(&q, "AFA-worldgen-P03-F21", "worldgen-local-context-compilation-gateway/1.0", "local single-study", false, false).unwrap(); assert_eq!(r.disposition, "partial"); assert!(r.denied_export_order.contains(&"raw_fact".into())); }
    #[test] fn federation_approval_is_required() { let mut q = request(); q.federation_approved = false; let r = negotiate(&q, "AFA-worldgen-P03-F24", "worldgen-federated-continual-context-compilation-gateway/1.0", "federated continual autonomous", true, true).unwrap(); assert_eq!(r.disposition, "blocked"); }
}
