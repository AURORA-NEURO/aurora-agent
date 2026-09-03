//! Worldgen P22 interoperability/extensibility contract.
//!
//! This support layer negotiates version-pinned capabilities and additive extension points
//! without executing a connector or exporting raw research data. Every missing extension,
//! migration loss, approval failure, and incomplete workflow stage is retained in the receipt.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P22-F01";
pub const CONTRACT_VERSION: &str = "worldgen-local-interoperability-extensibility/1.0";
pub const TARGET_VERSION: &str = "1.0.0";
pub const COMPATIBLE_VERSION: &str = "0.9.0";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.interoperability-extensibility-receipt-1+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensibilityRequest4 {
    pub request_id: String,
    pub source_contract_version: String,
    pub supported_contract_versions: Vec<String>,
    pub target_contract_version: String,
    pub offered_capability_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub extension_order: Vec<String>,
    pub schema_digest: ContentHash,
    pub artifact_digest_order: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub action_budget: u32,
    pub action_count: u32,
    pub stage_order: Vec<String>,
    pub completed_stage_order: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensibilityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensibilityReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub mode: String,
    pub scale: String,
    pub request_id: String,
    pub negotiated_version: String,
    pub disposition: String,
    pub capability_order: Vec<String>,
    pub extension_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub unsupported_extension_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub stage_order: Vec<String>,
    pub completed_stage_order: Vec<String>,
    pub pending_stage_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub schema_digest: ContentHash,
    pub artifact_digest_order: Vec<ContentHash>,
    pub receipt_digest: ContentHash,
    pub artifact: ExtensibilityArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InteroperabilityExtensibilityError {
    #[error("invalid interoperability/extensibility request: {0}")]
    Invalid(String),
    #[error("interoperability/extensibility receipt failed validation: {0}")]
    Output(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn ordered_hash(v: &[ContentHash]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn dig(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn nonempty(v: &str) -> bool {
    !v.trim().is_empty()
}
pub fn manifest(
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["schema steward","connector developer","research workflow orchestrator","federation operator"],"behavior":format!("negotiate version-pinned capabilities and extensibility for {scale} at {mode} scale"),"value":"prevents incompatible schemas, undeclared extensions, migration loss, and unsafe effects from crossing a research boundary","input_schema":"ExtensibilityRequest4@1","output_schema":"ExtensibilityReceipt7@1","effects":["exchange:capability-manifest","block:unsafe-release"],"permissions":["negotiate:declared-extension"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}
impl ExtensibilityReceipt7 {
    pub fn validate(&self) -> Result<(), InteroperabilityExtensibilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || !nonempty(&self.contract_version)
            || !nonempty(&self.feature_id)
            || !nonempty(&self.mode)
            || !nonempty(&self.scale)
            || !nonempty(&self.request_id)
            || !nonempty(&self.negotiated_version)
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.capability_order.is_empty()
            || !ordered(&self.capability_order)
            || !ordered(&self.extension_order)
            || !ordered(&self.missing_capability_order)
            || !ordered(&self.unsupported_extension_order)
            || !ordered(&self.omission_order)
            || !ordered(&self.uncertainty_order)
            || !ordered(&self.semantic_loss_order)
            || !ordered(&self.stage_order)
            || !ordered(&self.completed_stage_order)
            || !ordered(&self.pending_stage_order)
            || !ordered_hash(&self.artifact_digest_order)
            || !ordered(&self.effect_receipts)
            || !dig(&self.replay_identity)
            || !dig(&self.schema_digest)
            || !dig(&self.receipt_digest)
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.receipt_digest
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.semantic_loss != self.semantic_loss_order
            || self.effect_receipts.is_empty()
        {
            return Err(InteroperabilityExtensibilityError::Output("identity, canonical ordering, locality, digest, artifact, or effects are incomplete".into()));
        }
        if self.stage_order.len()
            != self.completed_stage_order.len() + self.pending_stage_order.len()
            || self.stage_order.iter().any(|s| {
                !self.completed_stage_order.contains(s) && !self.pending_stage_order.contains(s)
            })
        {
            return Err(InteroperabilityExtensibilityError::Output(
                "workflow stages do not partition".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, InteroperabilityExtensibilityError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|e| InteroperabilityExtensibilityError::Output(e.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|e| InteroperabilityExtensibilityError::Output(e.to_string()))
    }
}
pub fn negotiate(
    request: &ExtensibilityRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<ExtensibilityReceipt7, InteroperabilityExtensibilityError> {
    if !nonempty(&request.request_id)
        || !nonempty(&request.source_contract_version)
        || !nonempty(&request.target_contract_version)
        || request.supported_contract_versions.is_empty()
        || request.offered_capability_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.extension_order.is_empty()
        || request.artifact_digest_order.is_empty()
        || !dig(&request.schema_digest)
        || !dig(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || !ordered(&request.offered_capability_order)
        || !ordered(&request.required_capability_order)
        || !ordered(&request.extension_order)
        || !ordered(&request.supported_contract_versions)
    {
        return Err(InteroperabilityExtensibilityError::Invalid("request identity, versions, capability/extension order, digests, locality, or boundary is invalid".into()));
    }
    let mut capabilities = BTreeSet::from_iter(request.offered_capability_order.iter().cloned());
    capabilities.extend(request.required_capability_order.iter().cloned());
    let capability_order = capabilities.into_iter().collect::<Vec<_>>();
    let missing = request
        .required_capability_order
        .iter()
        .filter(|c| !request.offered_capability_order.contains(c))
        .cloned()
        .collect::<Vec<_>>();
    let unsupported = request
        .extension_order
        .iter()
        .filter(|e| {
            !request.offered_capability_order.contains(e)
                && !request.required_capability_order.contains(e)
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut omission: Vec<String> = Vec::new();
    let mut uncertainty: Vec<String> = Vec::new();
    let mut loss: Vec<String> = Vec::new();
    let (negotiated_version, mut disposition) = if request.target_contract_version == TARGET_VERSION
        && request.source_contract_version == TARGET_VERSION
    {
        (TARGET_VERSION.into(), "accepted")
    } else if request.target_contract_version == TARGET_VERSION
        && request.source_contract_version == COMPATIBLE_VERSION
        && request
            .supported_contract_versions
            .contains(&TARGET_VERSION.to_string())
    {
        loss.push("legacy-extension-semantics".into());
        omission.push("migration:legacy-fields-not-inferred".into());
        (TARGET_VERSION.into(), "migrated")
    } else {
        uncertainty.push("contract-version-outside-compatibility-window".into());
        (request.target_contract_version.clone(), "incompatible")
    };
    if !missing.is_empty() {
        omission.push("required-capability-missing".into());
        disposition = "unknown"
    }
    if !unsupported.is_empty() {
        loss.push("undeclared-extension-rejected".into());
        uncertainty.push("extension-not-offered-by-source".into());
        if disposition == "accepted" {
            disposition = "unknown"
        }
    }
    if !request.policy_allowed || !request.raw_data_local || !request.aggregate_only {
        omission.push("policy-or-locality-denied".into());
        disposition = "blocked"
    }
    if !request.protected_closure || !request.signed_approval {
        omission.push("protected-closure-or-approval-missing".into());
        if mode == "copilot" || mode == "workflow" {
            disposition = "approval_required"
        }
    }
    if mode == "copilot"
        && (request.action_count > request.action_budget || request.action_budget == 0)
    {
        omission.push("copilot:action-budget-exceeded".into());
        disposition = "blocked"
    }
    let completed = request.completed_stage_order.clone();
    let pending = request
        .stage_order
        .iter()
        .filter(|s| !completed.contains(s))
        .cloned()
        .collect::<Vec<_>>();
    if mode == "workflow"
        && !request.stage_order.is_empty()
        && !pending.is_empty()
        && disposition == "accepted"
    {
        disposition = "partial"
    }
    if disposition != "accepted" && disposition != "migrated" {
        omission.push("release:unsafe-interoperability-state".into())
    }
    // Every exported order is canonical, including multi-cause omission lists whose insertion
    // order depends on which compatibility gate fired first.
    omission.sort();
    omission.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    loss.sort();
    loss.dedup();
    let mut payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"mode":mode,"scale":scale,"request_id":request.request_id,"negotiated_version":negotiated_version,"disposition":disposition,"capability_order":capability_order,"extension_order":request.extension_order,"missing_capability_order":missing,"unsupported_extension_order":unsupported,"omission_order":omission,"uncertainty_order":uncertainty,"semantic_loss_order":loss,"stage_order":request.stage_order,"completed_stage_order":completed,"pending_stage_order":pending,"replay_identity":request.replay_identity,"schema_digest":request.schema_digest,"artifact_digest_order":request.artifact_digest_order,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| InteroperabilityExtensibilityError::Output(e.to_string()))?;
    payload["receipt_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id":format!("worldgen-interoperability-extensibility:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":loss,"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "accepted" || disposition == "migrated" {
        vec![format!(
            "exchange:capability-manifest:{}",
            request.request_id
        )]
    } else if disposition == "approval_required" {
        vec!["approval-required:interoperability".to_string()]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let out: ExtensibilityReceipt7 = serde_json::from_value(payload)
        .map_err(|e| InteroperabilityExtensibilityError::Output(e.to_string()))?;
    out.validate()?;
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn q() -> ExtensibilityRequest4 {
        ExtensibilityRequest4 {
            request_id: "interop:req".into(),
            source_contract_version: TARGET_VERSION.into(),
            supported_contract_versions: vec![TARGET_VERSION.into()],
            target_contract_version: TARGET_VERSION.into(),
            offered_capability_order: vec!["artifact-digest".into(), "schema-v1".into()],
            required_capability_order: vec!["artifact-digest".into()],
            extension_order: vec!["artifact-digest".into()],
            schema_digest: h("schema"),
            artifact_digest_order: vec![h("artifact")],
            replay_identity: h("replay"),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            action_budget: 4,
            action_count: 1,
            stage_order: vec!["negotiate".into()],
            completed_stage_order: vec!["negotiate".into()],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn accepted() {
        assert_eq!(
            negotiate(
                &q(),
                FEATURE_ID,
                CONTRACT_VERSION,
                "local single-study",
                "inference"
            )
            .unwrap()
            .disposition,
            "accepted"
        )
    }
    #[test]
    fn missing_is_unknown() {
        let mut x = q();
        x.required_capability_order.push("missing".into());
        x.required_capability_order.sort();
        assert_eq!(
            negotiate(
                &x,
                FEATURE_ID,
                CONTRACT_VERSION,
                "local single-study",
                "inference"
            )
            .unwrap()
            .disposition,
            "unknown"
        )
    }
    #[test]
    fn migration_retains_loss() {
        let mut x = q();
        x.source_contract_version = COMPATIBLE_VERSION.into();
        x.supported_contract_versions = vec![COMPATIBLE_VERSION.into(), TARGET_VERSION.into()];
        let r = negotiate(
            &x,
            FEATURE_ID,
            CONTRACT_VERSION,
            "local single-study",
            "inference",
        )
        .unwrap();
        assert_eq!(r.disposition, "migrated");
        assert!(!r.semantic_loss_order.is_empty())
    }
}
