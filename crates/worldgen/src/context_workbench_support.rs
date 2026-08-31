//! Read-only, omission-aware context-compilation researcher workbenches (P03 F17-F20).
//!
//! A workbench is deliberately a product surface rather than a UI mock: it has a versioned
//! request/receipt contract, deterministic panels, replay identity, and an explicit effect
//! receipt.  It never executes a tool or upgrades an incomplete context into a conclusion.

use std::collections::BTreeSet;

use super::context_copilot_support::{self, ContextCopilotRequest};
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.context-workbench-receipt+json";
pub const VIEWS: [&str; 5] = [
    "view:context",
    "view:evidence",
    "view:omissions",
    "view:negative",
    "view:provenance",
];
pub const PANELS: [&str; 5] = [
    "panel:qualified",
    "panel:partial",
    "panel:unknown",
    "panel:blocked",
    "panel:negative",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkbenchRequest {
    pub copilot: ContextCopilotRequest,
    pub workspace_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub disposition: String,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub required_fact_order: Vec<String>,
    pub resolved_fact_order: Vec<String>,
    pub unknown_fact_order: Vec<String>,
    pub blocked_fact_order: Vec<String>,
    pub denied_action_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_digest: ContentHash,
    pub workbench_digest: ContentHash,
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
pub enum ContextWorkbenchError {
    #[error("invalid context workbench request: {0}")]
    Invalid(String),
    #[error("context workbench copilot failed: {0}")]
    Copilot(String),
    #[error("context workbench artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn canonical(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

impl ContextWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ContextWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|value| value.as_str())
                != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|value| value.as_str())
                != Some(CONTENT_TYPE)
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.view_order != canonical(&VIEWS)
            || self.panel_order != canonical(&PANELS)
            || self.required_fact_order.is_empty()
            || self.effect_receipts.is_empty()
            || ![&self.replay_identity, &self.copilot_digest, &self.workbench_digest]
                .into_iter()
                .all(digest)
        {
            return Err(ContextWorkbenchError::Invalid(
                "workbench identity, panels, facts, locality, digests, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.required_fact_order,
            &self.resolved_fact_order,
            &self.unknown_fact_order,
            &self.blocked_fact_order,
            &self.denied_action_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ContextWorkbenchError::Invalid(
                    "workbench ordering is not canonical".into(),
                ));
            }
        }
        let required = self
            .required_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let classified = self
            .resolved_fact_order
            .iter()
            .chain(&self.unknown_fact_order)
            .chain(&self.blocked_fact_order)
            .cloned()
            .collect::<Vec<_>>();
        if required.len() != self.required_fact_order.len()
            || classified.len() != required.len()
            || classified.iter().cloned().collect::<BTreeSet<_>>() != required
        {
            return Err(ContextWorkbenchError::Invalid(
                "workbench facts do not form a complete partition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("view:context-workbench:")
        }) {
            return Err(ContextWorkbenchError::Invalid(
                "workbench effect is outside the read-only gate".into(),
            ));
        }
        if self.artifact.get("content_hash").and_then(|value| value.as_str())
            != Some(self.workbench_digest.as_str())
        {
            return Err(ContextWorkbenchError::Invalid(
                "workbench artifact digest is inconsistent".into(),
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
        "consumers": ["preclinical researcher", "research program lead", "imaging core scientist", "downstream context consumer"],
        "behavior": format!("render deterministic omission-aware context workbench panels for {scale}"),
        "value": "makes qualified, partial, unknown, negative, and blocked context state inspectable without hidden effects",
        "input_schema": input_schema,
        "output_schema": "ContextWorkbenchReceipt1@1",
        "effects": ["view:context-workbench", "block:unsafe-release"],
        "permissions": ["read:local-context-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy,
        "boundary": PRECLINICAL_BOUNDARY,
        "contract_version": version
    })
}

pub fn render(
    request: &ContextWorkbenchRequest,
    feature_id: &str,
    contract_version: &str,
    require_approval: bool,
    require_federation: bool,
) -> Result<ContextWorkbenchReceipt, ContextWorkbenchError> {
    if request.workspace_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.copilot.dry_run
        || !request.copilot.context_request.raw_data_local
        || !request.copilot.context_request.aggregate_only
        || request.requested_view_order != canonical(&VIEWS)
        || request.requested_panel_order != canonical(&PANELS)
        || !digest(&request.replay_identity)
        || request.replay_identity != request.copilot.context_request.replay_identity
    {
        return Err(ContextWorkbenchError::Invalid(
            "workbench identity, read-only, budget, locality, views, panels, or replay is invalid"
                .into(),
        ));
    }
    let copilot = context_copilot_support::run(
        &request.copilot,
        feature_id,
        contract_version,
        "context workbench",
        require_approval,
        require_federation,
    )
    .map_err(|error| ContextWorkbenchError::Copilot(error.to_string()))?;
    let view_order = canonical(&VIEWS);
    let panel_order = canonical(&PANELS);
    let required_fact_order = request
        .copilot
        .context_request
        .required_fact_order
        .clone();
    let resolved_fact_order = request
        .copilot
        .context_request
        .required_fact_order
        .iter()
        .filter(|id| copilot.context_disposition == "qualified" && copilot.denied_action_order.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let (unknown_fact_order, blocked_fact_order) = if resolved_fact_order.len() == required_fact_order.len() {
        (Vec::new(), Vec::new())
    } else if copilot.disposition == "blocked" {
        (Vec::new(), required_fact_order.clone())
    } else {
        (required_fact_order.clone(), Vec::new())
    };
    let copilot_value = serde_json::to_value(&copilot)
        .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))?;
    let copilot_digest = ContentHash::of_value(&copilot_value)
        .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))?;
    let workbench_payload = json!({
        "feature_id": feature_id,
        "workspace_id": request.workspace_id,
        "scope": request.scope,
        "view_order": view_order,
        "panel_order": panel_order,
        "required_fact_order": required_fact_order,
        "resolved_fact_order": resolved_fact_order,
        "unknown_fact_order": unknown_fact_order,
        "blocked_fact_order": blocked_fact_order,
        "replay_identity": request.replay_identity,
        "copilot_digest": copilot_digest,
    });
    let workbench_digest = ContentHash::of_value(&workbench_payload)
        .map_err(|error| ContextWorkbenchError::Artifact(error.to_string()))?;
    let mut omissions = copilot.omissions.clone();
    omissions.push("workbench:read-only-local-view".into());
    omissions.sort();
    omissions.dedup();
    let mut denied_action_order = copilot.denied_action_order.clone();
    denied_action_order.sort();
    let effect_receipts = if copilot.disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!("view:context-workbench:{}", request.workspace_id)]
    };
    let artifact = json!({
        "artifact_id": format!("worldgen-context-workbench:{}", request.workspace_id),
        "content_type": CONTENT_TYPE,
        "content_hash": workbench_digest,
        "boundary": PRECLINICAL_BOUNDARY,
        "views": view_order,
        "panels": panel_order,
        "copilot": copilot,
    });
    let receipt = ContextWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.copilot.context_request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        scope: request.scope.clone(),
        disposition: copilot.disposition.clone(),
        view_order,
        panel_order,
        required_fact_order,
        resolved_fact_order,
        unknown_fact_order,
        blocked_fact_order,
        denied_action_order,
        replay_identity: request.replay_identity.clone(),
        copilot_digest,
        workbench_digest,
        omissions,
        uncertainty: copilot.uncertainty.clone(),
        negative_evidence: copilot.negative_evidence.clone(),
        effect_receipts,
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
    fn request() -> ContextWorkbenchRequest {
        let replay = hash("replay");
        let fact = ContextFact { fact_id: "fact:workbench".into(), statement: "supported".into(), support_milli: 900, state: EvidenceState::Supported, evidence_digest: hash("e"), provenance_digest: hash("p"), artifact_digest: hash("a"), replay_identity: replay.clone(), negative_result: false, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
        let context = ContextCompilationRequest { request_id: "workbench:req".into(), objective: "inspect context".into(), scope: "study:workbench".into(), required_fact_order: vec!["fact:workbench".into()], minimum_support_milli: 500, facts: vec![fact], replay_identity: replay.clone(), policy_allow: true, protected_closure: true, federation_approved: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() };
        ContextWorkbenchRequest { copilot: ContextCopilotRequest { context_request: context, action_order: vec!["read:context".into()], action_budget: 2, dry_run: true, signed_approval: true, federation_approved: true, boundary: PRECLINICAL_BOUNDARY.into() }, workspace_id: "workspace:workbench".into(), scope: "study:workbench".into(), requested_view_order: canonical(&VIEWS), requested_panel_order: canonical(&PANELS), budget_units: 4, replay_identity: replay, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn read_only_panels_are_qualified_and_replayable() { let receipt = render(&request(), "AFA-worldgen-P03-F17", "worldgen-local-context-workbench/1.0", false, false).unwrap(); assert_eq!(receipt.disposition, "qualified"); assert!(receipt.effect_receipts[0].starts_with("view:context-workbench:")); assert!(receipt.validate().is_ok()); }
    #[test] fn incomplete_context_is_retained_as_unknown() { let mut req = request(); req.copilot.context_request.facts.clear(); let receipt = render(&req, "AFA-worldgen-P03-F18", "worldgen-multimodal-context-workbench/1.0", false, false).unwrap(); assert_eq!(receipt.disposition, "unknown"); assert_eq!(receipt.unknown_fact_order, vec!["fact:workbench"]); }
    #[test] fn workbench_requires_dry_run() { let mut req = request(); req.copilot.dry_run = false; assert!(render(&req, "AFA-worldgen-P03-F17", "worldgen-local-context-workbench/1.0", false, false).is_err()); }
}
