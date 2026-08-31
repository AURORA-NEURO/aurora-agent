//! Federated continual reliability copilot.
//!
//! Atlas feature: `AFA-adapter-P21-F12`.
//!
//! This bounded agent automation plans declared tool invocations and produces replayable
//! reliability receipts. It never invokes a connector itself: approval, dry-run, retry, timeout,
//! revocation, budget, and locality state are evaluated before a downstream executor is allowed
//! to act.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P21-F12";
pub const CONTRACT_VERSION: &str = "federated-reliability-copilot/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolManifest {
    pub tool_id: String,
    pub version: String,
    pub effect: String,
    pub deterministic: bool,
    pub approved: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub invocation_id: String,
    pub tool_id: String,
    pub input_digest: ContentHash,
    pub estimated_cost_units: u64,
    pub timeout_ms: u64,
    pub max_attempts: u32,
    pub simulated_failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWorkload {
    pub schema_version: String,
    pub workload_id: String,
    pub capability_id: String,
    pub manifests: Vec<ToolManifest>,
    pub invocations: Vec<ToolInvocation>,
    pub budget_units: u64,
    pub policy_allow: bool,
    pub dry_run: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityDecision {
    Completed,
    DryRun,
    Partial,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliableCapabilityResult {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub workload_id: String,
    pub workload: CapabilityWorkload,
    pub decision: ReliabilityDecision,
    pub invocation_order: Vec<String>,
    pub retry_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub budget_used_units: u64,
    pub timeout_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub failure_reasons: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ReliableCapabilityResult {
    pub fn validate(&self) -> Result<(), ReliabilityCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ReliabilityCopilotError::Contract(
                "reliability copilot identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.workload_id.trim().is_empty()
            || self.invocation_order.is_empty()
            || self.tool_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ReliabilityCopilotError::InvalidRequest("reliability identity, invocations, tools, effects, locality, and boundary are required".into()));
        }
        for values in [
            &self.invocation_order,
            &self.retry_order,
            &self.tool_order,
            &self.timeout_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ReliabilityCopilotError::InvalidRequest(
                    "reliability output ordering is not canonical".into(),
                ));
            }
        }
        let expected_provenance = reliability_provenance(&self.workload);
        if self.artifact.artifact_id != format!("reliability-copilot:{}", self.workload_id)
            || self.artifact.content_type
                != "application/vnd.aurora.reliable-capability-result+json"
            || !self.artifact.semantic_loss.is_empty()
            || self.artifact.provenance != expected_provenance
        {
            return Err(ReliabilityCopilotError::Contract(
                "reliability artifact is not bound to the retained workload".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReliabilityCopilotError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&reliability_payload(self))
            .map_err(|error| ReliabilityCopilotError::Contract(error.to_string()))?;
        let expected = plan_reliable_capability_internal(&self.workload, false)?;
        if self != &expected {
            return Err(ReliabilityCopilotError::Contract(
                "reliability result is not derived from its retained workload".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ReliabilityCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReliabilityCopilotError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReliabilityCopilotError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReliabilityCopilotError {
    #[error("invalid reliability copilot workload: {0}")]
    InvalidRequest(String),
    #[error("reliability copilot contract rejected: {0}")]
    Contract(String),
    #[error("reliability copilot serialization failed: {0}")]
    Serialization(String),
}

fn canonical_workload(workload: &CapabilityWorkload) -> CapabilityWorkload {
    let mut workload = workload.clone();
    workload
        .manifests
        .sort_by(|left, right| left.tool_id.cmp(&right.tool_id));
    workload
        .invocations
        .sort_by(|left, right| left.invocation_id.cmp(&right.invocation_id));
    workload
}

fn reliability_provenance(workload: &CapabilityWorkload) -> Vec<ProvenanceLink> {
    workload
        .invocations
        .iter()
        .map(|invocation| ProvenanceLink {
            source_id: invocation.invocation_id.clone(),
            relation: "reliability-invocation-input".into(),
            digest: invocation.input_digest.clone(),
        })
        .collect()
}

fn reliability_payload(receipt: &ReliableCapabilityResult) -> serde_json::Value {
    reliability_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.workload,
        receipt.decision,
        &receipt.invocation_order,
        &receipt.retry_order,
        &receipt.tool_order,
        receipt.budget_used_units,
        &receipt.timeout_order,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.failure_reasons,
        &receipt.effect_receipts,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn reliability_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    workload: &CapabilityWorkload,
    decision: ReliabilityDecision,
    invocation_order: &[String],
    retry_order: &[String],
    tool_order: &[String],
    budget_used_units: u64,
    timeout_order: &[String],
    omissions: &[String],
    uncertainty: &[String],
    failure_reasons: &[String],
    effect_receipts: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "workload": workload,
        "decision": decision,
        "invocation_order": invocation_order,
        "retry_order": retry_order,
        "tool_order": tool_order,
        "budget_used_units": budget_used_units,
        "timeout_order": timeout_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "failure_reasons": failure_reasons,
        "effect_receipts": effect_receipts,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

pub fn plan_reliable_capability(
    workload: &CapabilityWorkload,
) -> Result<ReliableCapabilityResult, ReliabilityCopilotError> {
    plan_reliable_capability_internal(workload, true)
}

fn plan_reliable_capability_internal(
    workload: &CapabilityWorkload,
    validate_output: bool,
) -> Result<ReliableCapabilityResult, ReliabilityCopilotError> {
    validate_workload(workload)?;
    let workload = canonical_workload(workload);
    let manifests = workload
        .manifests
        .iter()
        .map(|manifest| (manifest.tool_id.clone(), manifest))
        .collect::<BTreeMap<_, _>>();
    let invocations = workload.invocations.clone();
    let invocation_order = invocations
        .iter()
        .map(|invocation| invocation.invocation_id.clone())
        .collect::<Vec<_>>();
    let tool_order = invocations
        .iter()
        .map(|invocation| invocation.tool_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let retry_order = invocations
        .iter()
        .filter(|invocation| invocation.max_attempts > 1)
        .map(|invocation| invocation.invocation_id.clone())
        .collect::<Vec<_>>();
    let timeout_order = invocations
        .iter()
        .filter(|invocation| invocation.timeout_ms > 0)
        .map(|invocation| invocation.invocation_id.clone())
        .collect::<Vec<_>>();
    let budget_used_units = invocations
        .iter()
        .map(|invocation| invocation.estimated_cost_units)
        .sum::<u64>();
    let non_deterministic = invocations.iter().try_fold(false, |found, invocation| {
        let manifest = manifests.get(&invocation.tool_id).ok_or_else(|| {
            ReliabilityCopilotError::InvalidRequest(format!(
                "invocation {} references an undeclared tool",
                invocation.invocation_id
            ))
        })?;
        Ok(found || !manifest.deterministic)
    })?;
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut failure_reasons = Vec::new();
    let mut effect_receipts = Vec::new();
    for invocation in &invocations {
        let manifest = manifests.get(&invocation.tool_id).ok_or_else(|| {
            ReliabilityCopilotError::InvalidRequest(format!(
                "invocation {} references an undeclared tool",
                invocation.invocation_id
            ))
        })?;
        if let Some(failure) = &invocation.simulated_failure {
            failure_reasons.push(format!("{}: {}", invocation.invocation_id, failure));
        }
        if !manifest.deterministic {
            uncertainty.push(format!(
                "{}: tool is bounded-nondeterministic",
                invocation.tool_id
            ));
        }
    }
    let decision = if !workload.policy_allow {
        ReliabilityDecision::Blocked
    } else if workload.dry_run {
        ReliabilityDecision::DryRun
    } else if invocations
        .iter()
        .any(|invocation| invocation.simulated_failure.is_some())
    {
        ReliabilityDecision::Partial
    } else if non_deterministic {
        ReliabilityDecision::Degraded
    } else {
        ReliabilityDecision::Completed
    };
    if decision == ReliabilityDecision::DryRun {
        omissions.push("no tool effects were executed in dry-run mode".into());
        effect_receipts.push("dry_run_no_tool_effect".into());
    } else if decision == ReliabilityDecision::Blocked {
        omissions.push("policy denied bounded tool invocation".into());
        effect_receipts.push("blocked_no_tool_effect".into());
    } else {
        effect_receipts.extend(invocations.iter().map(|invocation| {
            format!(
                "bounded-tool-invocation:{}:not-executed",
                invocation.invocation_id
            )
        }));
    }
    if !failure_reasons.is_empty() {
        omissions.push("failed or timed-out invocations remain unresolved".into());
    }
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    failure_reasons.sort();
    failure_reasons.dedup();
    let provenance = reliability_provenance(&workload);
    let payload = reliability_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &workload,
        decision,
        &invocation_order,
        &retry_order,
        &tool_order,
        budget_used_units,
        &timeout_order,
        &omissions,
        &uncertainty,
        &failure_reasons,
        &effect_receipts,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("reliability-copilot:{}", workload.workload_id),
        "application/vnd.aurora.reliable-capability-result+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| ReliabilityCopilotError::Contract(error.to_string()))?;
    let result = ReliableCapabilityResult {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        workload_id: workload.workload_id.clone(),
        workload,
        decision,
        invocation_order,
        retry_order,
        tool_order,
        budget_used_units,
        timeout_order,
        omissions,
        uncertainty,
        failure_reasons,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        result.validate()?;
    }
    Ok(result)
}

fn validate_workload(workload: &CapabilityWorkload) -> Result<(), ReliabilityCopilotError> {
    if workload.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || workload.workload_id.trim().is_empty()
        || workload.capability_id.trim().is_empty()
        || workload.manifests.is_empty()
        || workload.invocations.is_empty()
        || workload.budget_units == 0
        || !workload.raw_data_local
        || workload.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ReliabilityCopilotError::InvalidRequest("workload identity, manifests, invocations, budget, locality, and boundary are required".into()));
    }
    let mut tools = BTreeSet::new();
    for manifest in &workload.manifests {
        if manifest.tool_id.trim().is_empty()
            || manifest.version.trim().is_empty()
            || manifest.effect.trim().is_empty()
            || !manifest.approved
            || manifest.revoked
        {
            return Err(ReliabilityCopilotError::InvalidRequest(format!(
                "tool {} is unapproved or revoked",
                manifest.tool_id
            )));
        }
        if !tools.insert(manifest.tool_id.clone()) {
            return Err(ReliabilityCopilotError::InvalidRequest(format!(
                "duplicate tool {}",
                manifest.tool_id
            )));
        }
    }
    let mut ids = BTreeSet::new();
    let mut total = 0u64;
    for invocation in &workload.invocations {
        if invocation.invocation_id.trim().is_empty()
            || invocation.tool_id.trim().is_empty()
            || invocation.estimated_cost_units == 0
            || invocation.timeout_ms == 0
            || invocation.max_attempts == 0
            || !tools.contains(&invocation.tool_id)
        {
            return Err(ReliabilityCopilotError::InvalidRequest(format!(
                "invocation {} is incomplete or undeclared",
                invocation.invocation_id
            )));
        }
        if !ids.insert(invocation.invocation_id.clone()) {
            return Err(ReliabilityCopilotError::InvalidRequest(format!(
                "duplicate invocation {}",
                invocation.invocation_id
            )));
        }
        total = total
            .checked_add(invocation.estimated_cost_units)
            .ok_or_else(|| {
                ReliabilityCopilotError::InvalidRequest("workload budget overflow".into())
            })?;
    }
    if total > workload.budget_units {
        return Err(ReliabilityCopilotError::InvalidRequest(
            "workload cost exceeds budget".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workload() -> CapabilityWorkload {
        CapabilityWorkload {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workload_id: "workload:qc".into(),
            capability_id: "capability:qc".into(),
            manifests: vec![ToolManifest {
                tool_id: "tool:qc".into(),
                version: "1.0".into(),
                effect: "read-local".into(),
                deterministic: true,
                approved: true,
                revoked: false,
            }],
            invocations: vec![
                ToolInvocation {
                    invocation_id: "invoke:b".into(),
                    tool_id: "tool:qc".into(),
                    input_digest: ContentHash::of_bytes(b"b"),
                    estimated_cost_units: 10,
                    timeout_ms: 1000,
                    max_attempts: 2,
                    simulated_failure: None,
                },
                ToolInvocation {
                    invocation_id: "invoke:a".into(),
                    tool_id: "tool:qc".into(),
                    input_digest: ContentHash::of_bytes(b"a"),
                    estimated_cost_units: 5,
                    timeout_ms: 1000,
                    max_attempts: 1,
                    simulated_failure: None,
                },
            ],
            budget_units: 100,
            policy_allow: true,
            dry_run: false,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn copilot_is_deterministic_under_invocation_order() {
        let mut reversed = workload();
        reversed.invocations.reverse();
        let first = plan_reliable_capability(&workload()).unwrap();
        let second = plan_reliable_capability(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.decision, ReliabilityDecision::Completed);
    }
    #[test]
    fn dry_run_has_no_effects() {
        let mut workload = workload();
        workload.dry_run = true;
        let result = plan_reliable_capability(&workload).unwrap();
        assert_eq!(result.decision, ReliabilityDecision::DryRun);
        assert!(result
            .effect_receipts
            .iter()
            .any(|receipt| receipt == "dry_run_no_tool_effect"));
    }
    #[test]
    fn simulated_failure_is_partial_and_retained() {
        let mut workload = workload();
        workload.invocations[0].simulated_failure = Some("timeout".into());
        let result = plan_reliable_capability(&workload).unwrap();
        assert_eq!(result.decision, ReliabilityDecision::Partial);
        assert!(!result.failure_reasons.is_empty());
    }
    #[test]
    fn revoked_tool_is_rejected() {
        let mut workload = workload();
        workload.manifests[0].revoked = true;
        assert!(plan_reliable_capability(&workload).is_err());
    }

    #[test]
    fn retained_invocation_tampering_is_rejected() {
        let mut result = plan_reliable_capability(&workload()).unwrap();
        result.workload.invocations[0].estimated_cost_units = 99;
        assert!(result.validate().is_err());
    }

    #[test]
    fn reliability_provenance_tampering_is_rejected() {
        let mut result = plan_reliable_capability(&workload()).unwrap();
        result.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(result.validate().is_err());
    }

    #[test]
    fn retained_dry_run_gate_tampering_is_rejected() {
        let mut result = plan_reliable_capability(&workload()).unwrap();
        result.workload.dry_run = true;
        assert!(result.validate().is_err());
    }
}
