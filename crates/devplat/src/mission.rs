//! Bounded, evidence-preserving mission planning for cross-domain agent workflows.
//!
//! The workspace already contains useful domain tools, but a capable agent needs a safe way to
//! compose them: ingest a world, audit provenance, compare measurements, run an evidence gate, and
//! prepare a release review without turning that sequence into an untyped prompt convention. This
//! module defines the composition contract. It validates a dependency DAG, produces deterministic
//! parallel waves, binds the plan to a content digest, and makes execution policy explicit.
//!
//! The module does not know how to call an MCP tool. The server owns that boundary and uses the
//! plan here before every call. That separation matters: a plan can be inspected without running
//! anything, while an executed mission is still restricted by an explicit tool allow-list, output
//! budgets, refusal propagation, and a side-effect confirmation policy.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

use crate::evaluator::MissionEvaluatorCatalogue;

/// Schema version for mission requests, plans, and execution reports.
pub const MISSION_SCHEMA_VERSION: &str = "bioprism-devplat-mission/0.1";
pub const MISSION_TRACE_SCHEMA_VERSION: &str = "bioprism-devplat-mission-trace/0.1";
const MAX_STEPS: usize = 128;
const MAX_ALLOWED_TOOLS: usize = 512;
const MAX_STEP_OUTPUT_BYTES: usize = 20_000_000;
const MAX_TOTAL_OUTPUT_BYTES: usize = 20_000_000;
pub const MAX_PARALLEL_WAVE_WIDTH: usize = 16;
pub const MAX_CLAIM_REQUESTS: usize = 64;
pub const MAX_CLAIM_REFERENCES: usize = 32;
pub const MAX_CLAIM_EVALUATORS: usize = 16;
/// Maximum serialized size of the workflow instantiation contract carried by a mission.
///
/// This is deliberately below the larger workflow/reconciliation envelopes: a mission must be
/// able to carry its provenance through dispatch without becoming an unbounded transport cache.
pub const MAX_WORKFLOW_BINDING_BYTES: usize = 2_000_000;
/// Maximum serialized size of the ready capability-route review carried into mission preflight.
///
/// The review is provenance, not an execution cache. Keeping the bound explicit prevents a
/// caller from smuggling an unbounded route, schema attachment, or evidence envelope through the
/// mission digest.
pub const MAX_ROUTE_REVIEW_BYTES: usize = 2_000_000;

fn default_true() -> bool {
    true
}

fn default_max_steps() -> usize {
    64
}

fn default_max_step_output_bytes() -> usize {
    2_000_000
}

fn default_max_total_output_bytes() -> usize {
    10_000_000
}

fn default_execution_mode() -> String {
    "serial".into()
}

fn default_max_parallelism() -> usize {
    MAX_PARALLEL_WAVE_WIDTH
}

fn default_claim_level() -> String {
    "observation".into()
}

fn default_claim_evidence_mode() -> String {
    "completed_step".into()
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn require_text(field: &'static str, value: &str) -> Result<(), MissionError> {
    if value.trim().is_empty() {
        return Err(MissionError::EmptyField { field });
    }
    if value
        .chars()
        .any(|character| character == '\0' || character == '\n' || character == '\r')
    {
        return Err(MissionError::ControlCharacter { field });
    }
    Ok(())
}

fn validate_workflow_binding(binding: &Value) -> Result<(), MissionError> {
    let encoded =
        serde_json::to_vec(binding).map_err(|error| MissionError::InvalidWorkflowBinding {
            reason: format!("cannot measure binding: {error}"),
        })?;
    if encoded.len() > MAX_WORKFLOW_BINDING_BYTES {
        return Err(MissionError::InvalidWorkflowBinding {
            reason: format!(
                "serialized binding is {} bytes; maximum is {}",
                encoded.len(),
                MAX_WORKFLOW_BINDING_BYTES
            ),
        });
    }
    let object = binding
        .as_object()
        .ok_or_else(|| MissionError::InvalidWorkflowBinding {
            reason: "binding must be an object".into(),
        })?;
    let required = [
        "workflow_id",
        "workflow_digest",
        "catalog_digest",
        "domain_contract_digest",
        "domain_contract",
        "evidence_plan",
        "evidence_plan_digest",
    ];
    for key in required {
        if !object.contains_key(key) {
            return Err(MissionError::InvalidWorkflowBinding {
                reason: format!("missing required field `{key}`"),
            });
        }
    }
    for key in [
        "workflow_digest",
        "catalog_digest",
        "domain_contract_digest",
        "evidence_plan_digest",
    ] {
        let value = object.get(key).and_then(Value::as_str).ok_or_else(|| {
            MissionError::InvalidWorkflowBinding {
                reason: format!("`{key}` must be a string"),
            }
        })?;
        ContentHash::parse(value.to_owned()).map_err(|_| MissionError::InvalidWorkflowBinding {
            reason: format!("`{key}` must be a 64-character hexadecimal digest"),
        })?;
    }
    let workflow_id = object
        .get("workflow_id")
        .and_then(Value::as_str)
        .ok_or_else(|| MissionError::InvalidWorkflowBinding {
            reason: "`workflow_id` must be a string".into(),
        })?;
    require_text("workflow_id", workflow_id).map_err(|error| {
        MissionError::InvalidWorkflowBinding {
            reason: error.to_string(),
        }
    })?;
    if !object.get("domain_contract").is_some_and(Value::is_object) {
        return Err(MissionError::InvalidWorkflowBinding {
            reason: "`domain_contract` must be an object".into(),
        });
    }
    let evidence_plan = object
        .get("evidence_plan")
        .and_then(Value::as_object)
        .ok_or_else(|| MissionError::InvalidWorkflowBinding {
            reason: "`evidence_plan` must be an object".into(),
        })?;
    let steps = evidence_plan
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| MissionError::InvalidWorkflowBinding {
            reason: "`evidence_plan.steps` must be an array".into(),
        })?;
    if steps.is_empty() || steps.len() > MAX_STEPS {
        return Err(MissionError::InvalidWorkflowBinding {
            reason: format!("`evidence_plan.steps` must contain 1..{MAX_STEPS} entries"),
        });
    }
    let expected_digest = ContentHash::of_value(&Value::Object(evidence_plan.clone()))
        .map_err(|error| MissionError::InvalidWorkflowBinding {
            reason: format!("cannot hash evidence plan: {error}"),
        })?
        .to_string();
    if object.get("evidence_plan_digest").and_then(Value::as_str) != Some(expected_digest.as_str())
    {
        return Err(MissionError::InvalidWorkflowBinding {
            reason: "`evidence_plan_digest` does not match `evidence_plan`".into(),
        });
    }
    Ok(())
}

fn route_review_digest(value: &Value, field: &str) -> Result<String, MissionError> {
    let digest = value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| MissionError::InvalidRouteReview {
            reason: format!("`{field}` must be a non-empty string"),
        })?;
    ContentHash::parse(digest.to_owned()).map_err(|_| MissionError::InvalidRouteReview {
        reason: format!("`{field}` must be a 64-character hexadecimal digest"),
    })?;
    Ok(digest.to_owned())
}

fn optional_route_review_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, MissionError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| MissionError::InvalidRouteReview {
                reason: format!("`{field}` must be a non-empty string when supplied"),
            })
            .map(Some),
    }
}

/// Validate the exact non-executing handoff emitted by `capability_route_review`.
///
/// A route review is intentionally not a permission token. It is a caller-owned checkpoint that
/// proves which route, selections, dependency waves, and optional route evidence were reviewed.
/// The mission boundary therefore binds the review to the current goal and serialized steps and
/// carries only compact provenance into the plan. Any change after review is refused before a
/// nested tool can be dispatched.
fn validate_route_review(
    review: &Value,
    goal: &str,
    steps: &[MissionStep],
) -> Result<(), MissionError> {
    let encoded = serde_json::to_vec(review).map_err(|error| MissionError::InvalidRouteReview {
        reason: format!("cannot measure review: {error}"),
    })?;
    if encoded.len() > MAX_ROUTE_REVIEW_BYTES {
        return Err(MissionError::InvalidRouteReview {
            reason: format!(
                "serialized review is {} bytes; maximum is {}",
                encoded.len(),
                MAX_ROUTE_REVIEW_BYTES
            ),
        });
    }
    let object = review
        .as_object()
        .ok_or_else(|| MissionError::InvalidRouteReview {
            reason: "route_review must be an object".into(),
        })?;
    if object.get("ok").and_then(Value::as_bool) != Some(true)
        || object.get("workflow").and_then(Value::as_str) != Some("capability_route_review")
        || object.get("review_status").and_then(Value::as_str) != Some("ready")
        || object.get("handoff_status").and_then(Value::as_str)
            != Some("mission_preflight_required")
        || object.get("execution").and_then(Value::as_str) != Some("not_started")
    {
        return Err(MissionError::InvalidRouteReview {
            reason: "route_review must be an ok, ready, non-executing mission handoff".into(),
        });
    }
    let findings = object
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| MissionError::InvalidRouteReview {
            reason: "findings must be an array".into(),
        })?;
    if !findings.is_empty() {
        return Err(MissionError::InvalidRouteReview {
            reason: "blocked route review findings must be corrected before mission preflight"
                .into(),
        });
    }
    let review_id = route_review_digest(
        object
            .get("review_id")
            .ok_or_else(|| MissionError::InvalidRouteReview {
                reason: "missing required field `review_id`".into(),
            })?,
        "review_id",
    )?;
    let route_id = route_review_digest(
        object
            .get("route_id")
            .ok_or_else(|| MissionError::InvalidRouteReview {
                reason: "missing required field `route_id`".into(),
            })?,
        "route_id",
    )?;
    let catalog_digest = route_review_digest(
        object
            .get("catalog_digest")
            .ok_or_else(|| MissionError::InvalidRouteReview {
                reason: "missing required field `catalog_digest`".into(),
            })?,
        "catalog_digest",
    )?;
    let review_goal = object
        .get("goal")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| MissionError::InvalidRouteReview {
            reason: "route_review.goal must be a non-empty string".into(),
        })?;
    if review_goal != goal {
        return Err(MissionError::InvalidRouteReview {
            reason: "route_review.goal does not match mission goal".into(),
        });
    }
    let mission_draft = object
        .get("mission_draft")
        .and_then(Value::as_object)
        .ok_or_else(|| MissionError::InvalidRouteReview {
            reason: "ready route_review must include a mission_draft object".into(),
        })?;
    if mission_draft.get("goal").and_then(Value::as_str) != Some(goal) {
        return Err(MissionError::InvalidRouteReview {
            reason: "route_review.mission_draft.goal does not match mission goal".into(),
        });
    }
    let reviewed_steps = mission_draft
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| MissionError::InvalidRouteReview {
            reason: "route_review.mission_draft.steps must be an array".into(),
        })?;
    let expected_steps =
        serde_json::to_value(steps).map_err(|error| MissionError::InvalidRouteReview {
            reason: format!("cannot encode mission steps for comparison: {error}"),
        })?;
    if !expected_steps
        .as_array()
        .is_some_and(|expected| reviewed_steps == expected)
    {
        return Err(MissionError::InvalidRouteReview {
            reason: "route_review.mission_draft.steps do not exactly match mission steps".into(),
        });
    }
    if !mission_draft
        .get("dependency_waves")
        .is_some_and(Value::is_array)
    {
        return Err(MissionError::InvalidRouteReview {
            reason: "route_review.mission_draft.dependency_waves must be an array".into(),
        });
    }

    let evidence_digest = optional_route_review_text(object, "evidence_digest")?;
    let evidence_scope = optional_route_review_text(object, "evidence_scope")?;
    if evidence_digest.is_some() != evidence_scope.is_some() {
        return Err(MissionError::InvalidRouteReview {
            reason: "evidence_digest and evidence_scope must be supplied together".into(),
        });
    }
    if let Some(digest) = evidence_digest.as_deref() {
        ContentHash::parse(digest.to_owned()).map_err(|_| MissionError::InvalidRouteReview {
            reason: "evidence_digest must be a 64-character hexadecimal digest".into(),
        })?;
    }
    let binding_present = match object.get("evidence_binding") {
        None => false,
        Some(binding) => {
            let binding = binding
                .as_object()
                .ok_or_else(|| MissionError::InvalidRouteReview {
                    reason: "evidence_binding must be an object".into(),
                })?;
            binding
                .get("present")
                .and_then(Value::as_bool)
                .ok_or_else(|| MissionError::InvalidRouteReview {
                    reason: "evidence_binding.present must be a boolean".into(),
                })?
        }
    };
    if binding_present {
        let binding = object
            .get("evidence_binding")
            .and_then(Value::as_object)
            .expect("binding_present implies an object");
        let digest =
            evidence_digest
                .as_deref()
                .ok_or_else(|| MissionError::InvalidRouteReview {
                    reason: "present evidence_binding requires evidence_digest and evidence_scope"
                        .into(),
                })?;
        let scope = evidence_scope
            .as_deref()
            .expect("digest and scope are paired");
        if binding.get("evidence_digest").and_then(Value::as_str) != Some(digest)
            || binding.get("scope").and_then(Value::as_str) != Some(scope)
        {
            return Err(MissionError::InvalidRouteReview {
                reason: "evidence_binding digest and scope do not match route_review".into(),
            });
        }
        if binding.get("posture").and_then(Value::as_str) != Some("carried_forward_not_recomputed")
            || binding.get("readiness_claimed").and_then(Value::as_bool) != Some(false)
            || binding.get("execution").and_then(Value::as_str) != Some("not_started")
        {
            return Err(MissionError::InvalidRouteReview {
                reason:
                    "evidence_binding must remain carried-forward, non-ready, and non-executing"
                        .into(),
            });
        }
        let summary = binding
            .get("summary")
            .and_then(Value::as_object)
            .ok_or_else(|| MissionError::InvalidRouteReview {
                reason: "present evidence_binding requires a summary object".into(),
            })?;
        if summary.get("evidence_digest").and_then(Value::as_str) != Some(digest)
            || summary.get("scope").and_then(Value::as_str) != Some(scope)
        {
            return Err(MissionError::InvalidRouteReview {
                reason: "evidence_binding.summary does not match route_review evidence".into(),
            });
        }
        if mission_draft
            .get("route_evidence_digest")
            .and_then(Value::as_str)
            != Some(digest)
            || mission_draft
                .get("route_evidence_scope")
                .and_then(Value::as_str)
                != Some(scope)
        {
            return Err(MissionError::InvalidRouteReview {
                reason: "mission_draft route evidence does not match route_review evidence".into(),
            });
        }
    } else if evidence_digest.is_some()
        || evidence_scope.is_some()
        || object
            .get("evidence_binding")
            .and_then(Value::as_object)
            .and_then(|binding| binding.get("posture"))
            .is_some_and(|posture| posture != "not_supplied")
    {
        return Err(MissionError::InvalidRouteReview {
            reason: "legacy route review cannot claim route evidence without a present binding"
                .into(),
        });
    }
    let _ = (review_id, route_id, catalog_digest);
    Ok(())
}

fn route_review_provenance(review: Option<&Value>) -> Option<Value> {
    let review = review?.as_object()?;
    let mut provenance = Map::new();
    provenance.insert("present".into(), Value::Bool(true));
    for field in ["review_id", "route_id", "catalog_digest"] {
        if let Some(value) = review.get(field) {
            provenance.insert(field.into(), value.clone());
        }
    }
    let binding = review.get("evidence_binding").and_then(Value::as_object);
    let evidence_present = binding
        .and_then(|value| value.get("present"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    provenance.insert("evidence_present".into(), Value::Bool(evidence_present));
    provenance.insert(
        "posture".into(),
        binding
            .and_then(|value| value.get("posture"))
            .cloned()
            .unwrap_or_else(|| json!("not_supplied")),
    );
    provenance.insert("readiness_claimed".into(), json!(false));
    provenance.insert("execution".into(), json!("not_started"));
    if evidence_present {
        for field in ["evidence_digest", "evidence_scope"] {
            if let Some(value) = review.get(field) {
                provenance.insert(field.into(), value.clone());
            }
        }
    }
    Some(Value::Object(provenance))
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn valid_json_pointer(pointer: &str, allow_empty: bool) -> bool {
    if pointer.is_empty() {
        return allow_empty;
    }
    let bytes = pointer.as_bytes();
    if bytes.first() != Some(&b'/') {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                return false;
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    true
}

fn contains_confirmation(value: &Value) -> bool {
    let mut pending = vec![value];
    while let Some(current) = pending.pop() {
        match current {
            Value::Object(object) => {
                if object
                    .get("confirm")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    return true;
                }
                pending.extend(object.values());
            }
            Value::Array(values) => pending.extend(values),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    false
}

/// Execution and resource policy for a mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionPolicy {
    /// `false` produces a plan only; execution must be explicitly opted into.
    #[serde(default)]
    pub execute: bool,
    /// Stop launching dependent and later steps after a refusal.
    #[serde(default = "default_true")]
    pub stop_on_error: bool,
    /// Permit caller-supplied confirmation flags to reach side-effecting tools.
    #[serde(default)]
    pub allow_side_effects: bool,
    /// Maximum number of steps in the DAG.
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    /// Maximum serialized MCP response retained for one step.
    #[serde(default = "default_max_step_output_bytes")]
    pub max_step_output_bytes: usize,
    /// Maximum serialized MCP responses retained for the whole mission.
    #[serde(default = "default_max_total_output_bytes")]
    pub max_total_output_bytes: usize,
    /// `serial` preserves strict stop-before-next-call budgeting; `parallel_waves` runs each
    /// independent DAG wave concurrently after reserving its worst-case output budget.
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
    /// Maximum number of independent steps dispatched at once in a parallel wave.
    #[serde(default = "default_max_parallelism")]
    pub max_parallelism: usize,
    /// Tools that may execute. Required and non-empty when `execute` is true.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

impl Default for MissionPolicy {
    fn default() -> Self {
        MissionPolicy {
            execute: false,
            stop_on_error: true,
            allow_side_effects: false,
            max_steps: default_max_steps(),
            max_step_output_bytes: default_max_step_output_bytes(),
            max_total_output_bytes: default_max_total_output_bytes(),
            execution_mode: default_execution_mode(),
            max_parallelism: default_max_parallelism(),
            allowed_tools: Vec::new(),
        }
    }
}

impl MissionPolicy {
    fn validate(&self) -> Result<BTreeSet<String>, MissionError> {
        if !(1..=MAX_STEPS).contains(&self.max_steps) {
            return Err(MissionError::InvalidLimit {
                field: "policy.max_steps",
                value: self.max_steps,
            });
        }
        if !(1..=MAX_STEP_OUTPUT_BYTES).contains(&self.max_step_output_bytes) {
            return Err(MissionError::InvalidLimit {
                field: "policy.max_step_output_bytes",
                value: self.max_step_output_bytes,
            });
        }
        if !(1..=MAX_TOTAL_OUTPUT_BYTES).contains(&self.max_total_output_bytes) {
            return Err(MissionError::InvalidLimit {
                field: "policy.max_total_output_bytes",
                value: self.max_total_output_bytes,
            });
        }
        if self.max_step_output_bytes > self.max_total_output_bytes {
            return Err(MissionError::OutputBudgetOrder);
        }
        if !matches!(self.execution_mode.as_str(), "serial" | "parallel_waves") {
            return Err(MissionError::InvalidExecutionMode {
                mode: self.execution_mode.clone(),
            });
        }
        if !(1..=MAX_PARALLEL_WAVE_WIDTH).contains(&self.max_parallelism) {
            return Err(MissionError::InvalidLimit {
                field: "policy.max_parallelism",
                value: self.max_parallelism,
            });
        }
        if self.allowed_tools.len() > MAX_ALLOWED_TOOLS {
            return Err(MissionError::TooMany {
                kind: "allowed tools",
                count: self.allowed_tools.len(),
                maximum: MAX_ALLOWED_TOOLS,
            });
        }
        let mut allowed = BTreeSet::new();
        for tool in &self.allowed_tools {
            require_text("policy.allowed_tools", tool)?;
            if !valid_tool_name(tool) {
                return Err(MissionError::UnsafeTool { tool: tool.clone() });
            }
            if tool == "agent_mission" {
                return Err(MissionError::RecursiveTool);
            }
            if !allowed.insert(tool.clone()) {
                return Err(MissionError::Duplicate {
                    kind: "allowed tool",
                    id: tool.clone(),
                });
            }
        }
        if self.execute && allowed.is_empty() {
            return Err(MissionError::MissingAllowList);
        }
        Ok(allowed)
    }
}

/// One domain task in a mission DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionBinding {
    /// Direct prerequisite whose structured result supplies the value.
    pub from_step: String,
    /// RFC 6901 JSON pointer into the prerequisite's decoded MCP payload. Empty means the whole payload.
    pub source_pointer: String,
    /// RFC 6901 JSON pointer to an existing slot in this step's arguments.
    pub target_pointer: String,
}

impl MissionBinding {
    fn validate(&self) -> Result<(), MissionError> {
        require_text("binding.from_step", &self.from_step)?;
        if self.source_pointer.contains('\0')
            || self.source_pointer.contains('\n')
            || self.source_pointer.contains('\r')
            || !valid_json_pointer(&self.source_pointer, true)
        {
            return Err(MissionError::InvalidPointer {
                pointer: self.source_pointer.clone(),
            });
        }
        if self.target_pointer.is_empty()
            || self.target_pointer.contains('\0')
            || self.target_pointer.contains('\n')
            || self.target_pointer.contains('\r')
            || !valid_json_pointer(&self.target_pointer, false)
        {
            return Err(MissionError::InvalidPointer {
                pointer: self.target_pointer.clone(),
            });
        }
        Ok(())
    }
}

/// Apply one validated binding to an argument object.
pub fn apply_binding(
    arguments: &mut Value,
    binding: &MissionBinding,
    payload: &Value,
) -> Result<(), MissionError> {
    binding.validate()?;
    let source = if binding.source_pointer.is_empty() {
        payload
    } else {
        payload
            .pointer(&binding.source_pointer)
            .ok_or_else(|| MissionError::MissingPointer {
                pointer: binding.source_pointer.clone(),
            })?
    };
    let target = arguments
        .pointer_mut(&binding.target_pointer)
        .ok_or_else(|| MissionError::MissingPointer {
            pointer: binding.target_pointer.clone(),
        })?;
    *target = source.clone();
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionStep {
    pub id: String,
    pub domain: String,
    pub capability: String,
    pub objective: String,
    pub tool: String,
    #[serde(default = "empty_object")]
    pub arguments: Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub bindings: Vec<MissionBinding>,
    #[serde(default = "default_true")]
    pub required: bool,
}

impl MissionStep {
    fn validate(&self) -> Result<(), MissionError> {
        require_text("step.id", &self.id)?;
        require_text("step.domain", &self.domain)?;
        require_text("step.capability", &self.capability)?;
        require_text("step.objective", &self.objective)?;
        require_text("step.tool", &self.tool)?;
        if !valid_tool_name(&self.tool) {
            return Err(MissionError::UnsafeTool {
                tool: self.tool.clone(),
            });
        }
        if self.tool == "agent_mission" {
            return Err(MissionError::RecursiveTool);
        }
        if !self.arguments.is_object() {
            return Err(MissionError::ArgumentsMustBeObject {
                step: self.id.clone(),
            });
        }
        for dependency in &self.depends_on {
            require_text("step dependency", dependency)?;
        }
        for binding in &self.bindings {
            binding.validate()?;
        }
        Ok(())
    }
}

/// One explicit, caller-owned claim request that a mission must preserve as evidence lineage.
///
/// The mission runtime never interprets the statement as true. It only correlates the requested
/// claim to the exact step results named by `requires_steps` and reports transport/output posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionClaimRequest {
    pub id: String,
    pub claim: String,
    pub domains: Vec<String>,
    #[serde(default)]
    pub requires_steps: Vec<String>,
    #[serde(default = "default_claim_level")]
    pub level: String,
    #[serde(default = "default_claim_evidence_mode")]
    pub evidence_mode: String,
    #[serde(default)]
    pub evaluator_bindings: Vec<MissionClaimEvaluatorBinding>,
}

/// Explicit adapter/evaluator binding for one claim.
///
/// The adapter identifier and output pointer are caller-owned declarations. The mission runtime
/// only checks whether the named step returned a successful, retained value at that pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionClaimEvaluatorBinding {
    pub id: String,
    pub adapter_id: String,
    pub domain: String,
    pub step_id: String,
    pub output_pointer: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

impl MissionClaimEvaluatorBinding {
    fn validate(
        &self,
        claim_id: &str,
        known_steps: &BTreeSet<String>,
        required_steps: &BTreeSet<String>,
    ) -> Result<(), MissionError> {
        for (field, value, maximum) in [
            ("evaluator.id", &self.id, 128),
            ("evaluator.adapter_id", &self.adapter_id, 256),
            ("evaluator.domain", &self.domain, 256),
        ] {
            require_text(field, value)?;
            if value.len() > maximum {
                return Err(MissionError::InvalidClaim {
                    claim: claim_id.into(),
                    reason: format!("{field} must be at most {maximum} bytes"),
                });
            }
        }
        require_text("evaluator.step_id", &self.step_id)?;
        if !known_steps.contains(&self.step_id) {
            return Err(MissionError::UnknownClaimStep {
                claim: claim_id.into(),
                step: self.step_id.clone(),
            });
        }
        if !required_steps.contains(&self.step_id) {
            return Err(MissionError::InvalidClaim {
                claim: claim_id.into(),
                reason: format!(
                    "evaluator {} must reference one of the claim's requires_steps",
                    self.id
                ),
            });
        }
        if self.output_pointer.contains('\0')
            || self.output_pointer.contains('\n')
            || self.output_pointer.contains('\r')
            || !valid_json_pointer(&self.output_pointer, true)
        {
            return Err(MissionError::InvalidClaim {
                claim: claim_id.into(),
                reason: format!("evaluator {} has an invalid output_pointer", self.id),
            });
        }
        Ok(())
    }
}

impl MissionClaimRequest {
    fn validate(&self, known_steps: &BTreeSet<String>) -> Result<(), MissionError> {
        require_text("claim.id", &self.id)?;
        require_text("claim.claim", &self.claim)?;
        if self.id.len() > 128 {
            return Err(MissionError::InvalidClaim {
                claim: self.id.clone(),
                reason: "id must be at most 128 bytes".into(),
            });
        }
        if self.claim.len() > 4_096 {
            return Err(MissionError::InvalidClaim {
                claim: self.id.clone(),
                reason: "claim must be at most 4096 bytes".into(),
            });
        }
        if self.domains.is_empty() || self.domains.len() > 32 {
            return Err(MissionError::InvalidClaim {
                claim: self.id.clone(),
                reason: "domains must contain between 1 and 32 entries".into(),
            });
        }
        let mut domains = BTreeSet::new();
        for domain in &self.domains {
            require_text("claim.domain", domain)?;
            if domain.len() > 256 || !domains.insert(domain) {
                return Err(MissionError::InvalidClaim {
                    claim: self.id.clone(),
                    reason: "domains must contain unique entries of at most 256 bytes".into(),
                });
            }
        }
        if self.requires_steps.is_empty() || self.requires_steps.len() > MAX_CLAIM_REFERENCES {
            return Err(MissionError::InvalidClaim {
                claim: self.id.clone(),
                reason: format!(
                    "requires_steps must contain between 1 and {MAX_CLAIM_REFERENCES} entries"
                ),
            });
        }
        let mut steps = BTreeSet::<String>::new();
        for step in &self.requires_steps {
            require_text("claim.requires_steps", step)?;
            if !known_steps.contains(step) {
                return Err(MissionError::UnknownClaimStep {
                    claim: self.id.clone(),
                    step: step.clone(),
                });
            }
            if !steps.insert(step.clone()) {
                return Err(MissionError::InvalidClaim {
                    claim: self.id.clone(),
                    reason: "requires_steps must not contain duplicates".into(),
                });
            }
        }
        if !matches!(
            self.level.as_str(),
            "observation" | "evaluation" | "operational" | "release"
        ) {
            return Err(MissionError::InvalidClaim {
                claim: self.id.clone(),
                reason: "level must be observation, evaluation, operational, or release".into(),
            });
        }
        if !matches!(
            self.evidence_mode.as_str(),
            "completed_step" | "successful_tool_result"
        ) {
            return Err(MissionError::InvalidClaim {
                claim: self.id.clone(),
                reason: "evidence_mode must be completed_step or successful_tool_result".into(),
            });
        }
        if self.evaluator_bindings.len() > MAX_CLAIM_EVALUATORS {
            return Err(MissionError::TooMany {
                kind: "claim evaluator bindings",
                count: self.evaluator_bindings.len(),
                maximum: MAX_CLAIM_EVALUATORS,
            });
        }
        let mut evaluator_ids = BTreeSet::new();
        for evaluator in &self.evaluator_bindings {
            if !evaluator_ids.insert(evaluator.id.clone()) {
                return Err(MissionError::Duplicate {
                    kind: "claim evaluator",
                    id: evaluator.id.clone(),
                });
            }
            evaluator.validate(&self.id, known_steps, &steps)?;
        }
        Ok(())
    }
}

fn evaluator_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn normalized_evaluator_label(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Require a ready review to be the exact provenance envelope consumed by a mission.
///
/// A caller may still submit legacy evaluator bindings without a review for compatibility, but once
/// `evaluator_review` is present the handoff is fail-closed: every binding must appear exactly once
/// in the ready review, retain the same claim/adapter/domain/step/pointer fields, and still match
/// the current in-tree evaluator catalogue. This turns discovery/review into an auditable input to
/// dispatch rather than a suggestion that can silently be replaced before execution.
fn validate_evaluator_review(
    review: &Value,
    claims: &[MissionClaimRequest],
) -> Result<(), MissionError> {
    let object = review
        .as_object()
        .ok_or_else(|| MissionError::InvalidEvaluatorReview {
            reason: "evaluator_review must be an object".into(),
        })?;
    let field_text = |field: &'static str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: format!("evaluator_review.{field} must be a non-empty string"),
            })
    };
    if object.get("workflow").and_then(Value::as_str) != Some("mission_evaluator_review") {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: "workflow must be mission_evaluator_review".into(),
        });
    }
    if object.get("ok").and_then(Value::as_bool) != Some(true)
        || object.get("review_status").and_then(Value::as_str) != Some("ready")
        || object.get("binding_posture").and_then(Value::as_str)
            != Some("ready_for_mission_claim_bindings")
        || object.get("execution").and_then(Value::as_str) != Some("not_started")
    {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: "evaluator_review must be an ok, ready, non-executing review".into(),
        });
    }
    let review_id = field_text("review_id")?;
    let catalog_digest = field_text("catalog_digest")?;
    let discovery_digest = field_text("discovery_digest")?;
    if !evaluator_digest(review_id)
        || !evaluator_digest(catalog_digest)
        || !evaluator_digest(discovery_digest)
    {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: "review_id, catalog_digest, and discovery_digest must be 64-character digests"
                .into(),
        });
    }
    let catalogue = MissionEvaluatorCatalogue::standard();
    if catalog_digest != catalogue.digest().to_string() {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: format!("catalog_digest is stale; expected {}", catalogue.digest()),
        });
    }
    let findings = object
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| MissionError::InvalidEvaluatorReview {
            reason: "findings must be an array".into(),
        })?;
    if !findings.is_empty() {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: "blocked review findings must be corrected before dispatch".into(),
        });
    }
    let rows = object
        .get("bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| MissionError::InvalidEvaluatorReview {
            reason: "bindings must be an array".into(),
        })?;
    let expected = claims
        .iter()
        .flat_map(|claim| {
            claim
                .evaluator_bindings
                .iter()
                .map(move |binding| (claim, binding))
        })
        .collect::<Vec<_>>();
    if expected.is_empty() {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: "a ready evaluator review must accompany at least one evaluator binding".into(),
        });
    }
    if rows.len() != expected.len() {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: format!(
                "review contains {} binding rows but the mission carries {}",
                rows.len(),
                expected.len()
            ),
        });
    }
    if object.get("selection_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
        return Err(MissionError::InvalidEvaluatorReview {
            reason: "selection_count does not match bindings".into(),
        });
    }
    let mut rows_by_id = BTreeMap::<String, &Value>::new();
    for row in rows {
        let row_object = row
            .as_object()
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: "every evaluator review binding must be an object".into(),
            })?;
        let id = row_object
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: "every evaluator review binding requires a non-empty id".into(),
            })?;
        if rows_by_id.insert(id.to_string(), row).is_some() {
            return Err(MissionError::InvalidEvaluatorReview {
                reason: format!("review contains duplicate binding id `{id}`"),
            });
        }
        if row_object.get("binding_posture").and_then(Value::as_str) != Some("ready")
            || row_object.get("candidate_found").and_then(Value::as_bool) != Some(true)
            || row_object.get("domain_supported").and_then(Value::as_bool) != Some(true)
        {
            return Err(MissionError::InvalidEvaluatorReview {
                reason: format!("binding `{id}` is not ready for mission dispatch"),
            });
        }
        let adapter_id = row_object
            .get("adapter_id")
            .and_then(Value::as_str)
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: format!("binding `{id}` has no adapter_id"),
            })?;
        let domain = row_object
            .get("domain")
            .and_then(Value::as_str)
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: format!("binding `{id}` has no domain"),
            })?;
        let adapter = catalogue
            .adapters()
            .iter()
            .find(|adapter| adapter.id == adapter_id)
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: format!("binding `{id}` names unknown adapter `{adapter_id}`"),
            })?;
        if !adapter.domains.iter().any(|candidate| {
            normalized_evaluator_label(candidate).contains(&normalized_evaluator_label(domain))
        }) {
            return Err(MissionError::InvalidEvaluatorReview {
                reason: format!("binding `{id}` uses an unsupported domain `{domain}`"),
            });
        }
    }

    for (claim, binding) in expected {
        let row =
            rows_by_id
                .get(&binding.id)
                .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                    reason: format!("binding `{}` is absent from evaluator_review", binding.id),
                })?;
        let row_object = row.as_object().expect("review rows were checked above");
        for (field, expected_value) in [
            ("claim_id", claim.id.as_str()),
            ("adapter_id", binding.adapter_id.as_str()),
            ("domain", binding.domain.as_str()),
            ("step_id", binding.step_id.as_str()),
            ("output_pointer", binding.output_pointer.as_str()),
        ] {
            if row_object.get(field).and_then(Value::as_str) != Some(expected_value) {
                return Err(MissionError::InvalidEvaluatorReview {
                    reason: format!(
                        "binding `{}` does not match review field `{field}`",
                        binding.id
                    ),
                });
            }
        }
        if row_object.get("required").and_then(Value::as_bool) != Some(binding.required) {
            return Err(MissionError::InvalidEvaluatorReview {
                reason: format!(
                    "binding `{}` does not match review required posture",
                    binding.id
                ),
            });
        }
        let proposed = row_object
            .get("proposed_binding")
            .and_then(Value::as_object)
            .ok_or_else(|| MissionError::InvalidEvaluatorReview {
                reason: format!("binding `{}` has no proposed_binding", binding.id),
            })?;
        for (field, expected_value) in [
            ("id", binding.id.as_str()),
            ("claim_id", claim.id.as_str()),
            ("adapter_id", binding.adapter_id.as_str()),
            ("domain", binding.domain.as_str()),
            ("step_id", binding.step_id.as_str()),
            ("output_pointer", binding.output_pointer.as_str()),
        ] {
            if proposed.get(field).and_then(Value::as_str) != Some(expected_value) {
                return Err(MissionError::InvalidEvaluatorReview {
                    reason: format!(
                        "binding `{}` proposed_binding does not match `{field}`",
                        binding.id
                    ),
                });
            }
        }
        if proposed.get("required").and_then(Value::as_bool) != Some(binding.required) {
            return Err(MissionError::InvalidEvaluatorReview {
                reason: format!(
                    "binding `{}` proposed_binding does not match required posture",
                    binding.id
                ),
            });
        }
    }
    Ok(())
}

/// A complete mission request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionRequest {
    pub mission_id: String,
    pub goal: String,
    pub steps: Vec<MissionStep>,
    #[serde(default)]
    pub policy: MissionPolicy,
    #[serde(default)]
    pub claim_requests: Vec<MissionClaimRequest>,
    /// The ready, non-executing evaluator review consumed by this mission, when bindings were
    /// selected through `mission_evaluator_review`.
    #[serde(default)]
    pub evaluator_review: Option<Value>,
    /// The workflow instantiation contract that produced this mission, when the mission was
    /// created from the domain-workflow surface.
    #[serde(default)]
    pub workflow_binding: Option<Value>,
    /// The ready, non-executing capability route review consumed by this mission, when route
    /// selections were reviewed before mission preflight.
    #[serde(default)]
    pub route_review: Option<Value>,
}

impl MissionRequest {
    /// Validate all local invariants and return the normalized execution allow-list.
    pub fn validate(&self) -> Result<BTreeSet<String>, MissionError> {
        require_text("mission_id", &self.mission_id)?;
        require_text("goal", &self.goal)?;
        if self.steps.is_empty() {
            return Err(MissionError::NoSteps);
        }
        if self.steps.len() > self.policy.max_steps || self.steps.len() > MAX_STEPS {
            return Err(MissionError::TooMany {
                kind: "mission steps",
                count: self.steps.len(),
                maximum: self.policy.max_steps.min(MAX_STEPS),
            });
        }
        let allowed = self.policy.validate()?;
        let mut ids = BTreeSet::new();
        for step in &self.steps {
            step.validate()?;
            if !ids.insert(step.id.clone()) {
                return Err(MissionError::Duplicate {
                    kind: "mission step",
                    id: step.id.clone(),
                });
            }
            if self.policy.execute {
                if !allowed.contains(&step.tool) {
                    return Err(MissionError::ToolNotAllowed {
                        step: step.id.clone(),
                        tool: step.tool.clone(),
                    });
                }
                if !self.policy.allow_side_effects && contains_confirmation(&step.arguments) {
                    return Err(MissionError::SideEffectsDisallowed {
                        step: step.id.clone(),
                    });
                }
            }
        }
        if self.claim_requests.len() > MAX_CLAIM_REQUESTS {
            return Err(MissionError::TooMany {
                kind: "claim requests",
                count: self.claim_requests.len(),
                maximum: MAX_CLAIM_REQUESTS,
            });
        }
        let mut claim_ids = BTreeSet::new();
        for claim in &self.claim_requests {
            if !claim_ids.insert(claim.id.clone()) {
                return Err(MissionError::Duplicate {
                    kind: "mission claim",
                    id: claim.id.clone(),
                });
            }
            claim.validate(&ids)?;
        }
        if let Some(review) = &self.evaluator_review {
            validate_evaluator_review(review, &self.claim_requests)?;
        }
        if let Some(binding) = &self.workflow_binding {
            validate_workflow_binding(binding)?;
        }
        if let Some(review) = &self.route_review {
            validate_route_review(review, &self.goal, &self.steps)?;
        }
        for step in &self.steps {
            let mut dependencies = BTreeSet::new();
            for dependency in &step.depends_on {
                if dependency == &step.id {
                    return Err(MissionError::SelfDependency {
                        step: step.id.clone(),
                    });
                }
                if !ids.contains(dependency) {
                    return Err(MissionError::UnknownDependency {
                        step: step.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
                if !dependencies.insert(dependency.clone()) {
                    return Err(MissionError::Duplicate {
                        kind: "step dependency",
                        id: dependency.clone(),
                    });
                }
            }
            let mut binding_targets = BTreeSet::new();
            for binding in &step.bindings {
                if !ids.contains(&binding.from_step) {
                    return Err(MissionError::UnknownBindingSource {
                        step: step.id.clone(),
                        source_step: binding.from_step.clone(),
                    });
                }
                if !dependencies.contains(&binding.from_step) {
                    return Err(MissionError::BindingWithoutDependency {
                        step: step.id.clone(),
                        source_step: binding.from_step.clone(),
                    });
                }
                if !binding_targets.insert(binding.target_pointer.clone()) {
                    return Err(MissionError::Duplicate {
                        kind: "binding target",
                        id: binding.target_pointer.clone(),
                    });
                }
                if step.arguments.pointer(&binding.target_pointer).is_none() {
                    return Err(MissionError::MissingPointer {
                        pointer: binding.target_pointer.clone(),
                    });
                }
            }
        }
        Ok(allowed)
    }

    /// Return deterministic topological waves. Steps in one wave are independent of one another.
    pub fn waves(&self) -> Result<Vec<Vec<String>>, MissionError> {
        self.validate()?;
        let mut remaining = self
            .steps
            .iter()
            .map(|step| {
                (
                    step.id.clone(),
                    step.depends_on.iter().cloned().collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut waves = Vec::new();
        while !remaining.is_empty() {
            let ready = remaining
                .iter()
                .filter(|(_, dependencies)| dependencies.is_empty())
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            if ready.is_empty() {
                return Err(MissionError::DependencyCycle {
                    steps: remaining.keys().cloned().collect(),
                });
            }
            for id in &ready {
                remaining.remove(id);
            }
            for dependencies in remaining.values_mut() {
                for id in &ready {
                    dependencies.remove(id);
                }
            }
            waves.push(ready);
        }
        Ok(waves)
    }
}

/// A normalized step in a plan, annotated with its parallel wave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionStepPlan {
    pub id: String,
    pub domain: String,
    pub capability: String,
    pub objective: String,
    pub tool: String,
    pub depends_on: Vec<String>,
    pub bindings: Vec<MissionBinding>,
    pub required: bool,
    pub wave: usize,
}

/// Deterministic plan returned before any tool can execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionPlan {
    pub schema_version: String,
    pub mission_id: String,
    pub goal: String,
    pub digest: String,
    pub step_count: usize,
    pub ordered_steps: Vec<String>,
    pub waves: Vec<Vec<String>>,
    pub critical_path_length: usize,
    pub steps: Vec<MissionStepPlan>,
    pub execution: String,
    pub execution_mode: String,
    pub max_parallelism: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_binding: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_review_provenance: Option<Value>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

/// Result of one executed mission step. `wire` is the raw JSON-RPC envelope when retained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionStepResult {
    pub id: String,
    pub tool: String,
    pub status: String,
    pub required: bool,
    /// Content hash of the exact arguments sent to the nested tool after bindings were applied.
    pub arguments_digest: Option<String>,
    pub bytes: usize,
    pub wire: Option<Value>,
    pub error: Option<String>,
}

fn retained_result_payload(result: &MissionStepResult) -> Option<(Value, &'static str)> {
    let wire = result.wire.as_ref()?;
    if let Some(payload) = wire.pointer("/result/structuredContent") {
        return Some((payload.clone(), "structured_content"));
    }
    if let Some(text) = wire
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
    {
        if let Ok(payload) = serde_json::from_str::<Value>(text) {
            return Some((payload, "content_text_json"));
        }
    }
    Some((wire.clone(), "wire_envelope"))
}

fn json_value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn evaluator_outcome_state(status: &str) -> &'static str {
    match status {
        "refused" => "refused",
        "blocked" => "blocked",
        "cancelled" => "cancelled",
        _ => "step_not_successful",
    }
}

fn evaluator_review_provenance(review: Option<&Value>) -> Value {
    let Some(object) = review.and_then(Value::as_object) else {
        return json!({"present": false});
    };
    json!({
        "present": true,
        "review_id": object.get("review_id").cloned().unwrap_or(Value::Null),
        "catalog_digest": object.get("catalog_digest").cloned().unwrap_or(Value::Null),
        "discovery_digest": object.get("discovery_digest").cloned().unwrap_or(Value::Null),
        "catalogue_snapshot": object.get("catalogue_snapshot").cloned().unwrap_or(Value::Null),
        "review_status": object.get("review_status").cloned().unwrap_or(Value::Null),
        "binding_posture": object.get("binding_posture").cloned().unwrap_or(Value::Null),
        "execution": object.get("execution").cloned().unwrap_or(Value::Null),
    })
}

/// Build a bounded lineage projection without interpreting the claim statement.
pub fn mission_claim_lineage(
    claims: &[MissionClaimRequest],
    results: &[MissionStepResult],
) -> Value {
    mission_claim_lineage_with_review(claims, results, None)
}

/// Build lineage while retaining the provenance of the evaluator review consumed by dispatch.
pub fn mission_claim_lineage_with_review(
    claims: &[MissionClaimRequest],
    results: &[MissionStepResult],
    evaluator_review: Option<&Value>,
) -> Value {
    let result_by_id = results
        .iter()
        .map(|result| (result.id.as_str(), result))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::with_capacity(claims.len());
    for claim in claims.iter().take(MAX_CLAIM_REQUESTS) {
        let evidence = claim
            .requires_steps
            .iter()
            .take(MAX_CLAIM_REFERENCES)
            .map(|step_id| {
                let Some(result) = result_by_id.get(step_id.as_str()) else {
                    return json!({
                        "step_id": step_id,
                        "found": false,
                        "evidence_state": "missing_step_result"
                    });
                };
                let result_digest = result
                    .wire
                    .as_ref()
                    .and_then(|wire| ContentHash::of_value(wire).ok())
                    .map(|digest| digest.to_string());
                json!({
                    "step_id": result.id,
                    "tool": result.tool,
                    "status": result.status,
                    "required": result.required,
                    "arguments_digest": result.arguments_digest,
                    "bytes": result.bytes,
                    "found": true,
                    "result_retained": result.wire.is_some(),
                    "result_digest": result_digest,
                    "evidence_state": if result.status == "succeeded" {
                        if result.wire.is_some() { "completed_output_retained" } else { "completed_output_omitted" }
                    } else {
                        "step_not_successful"
                    }
                })
            })
            .collect::<Vec<_>>();
        let all_found = evidence
            .iter()
            .all(|row| row.get("found").and_then(Value::as_bool).unwrap_or(false));
        let all_succeeded = evidence
            .iter()
            .all(|row| row.get("status").and_then(Value::as_str) == Some("succeeded"));
        let all_outputs_retained = evidence.iter().all(|row| {
            row.get("result_retained")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });
        let evidence_state = if !all_found {
            "missing_step_result"
        } else if !all_succeeded {
            "step_not_successful"
        } else if !all_outputs_retained {
            "completed_output_omitted"
        } else {
            "completed_output_retained"
        };
        let evaluator_rows = claim
            .evaluator_bindings
            .iter()
            .take(MAX_CLAIM_EVALUATORS)
            .map(|binding| {
                let base = || {
                    json!({
                        "id": binding.id,
                        "adapter_id": binding.adapter_id,
                        "domain": binding.domain,
                        "step_id": binding.step_id,
                        "output_pointer": binding.output_pointer,
                        "required": binding.required
                    })
                };
                let Some(result) = result_by_id.get(binding.step_id.as_str()) else {
                    let mut row = base();
                    row["evaluator_state"] = json!("missing_step_result");
                    row["outcome_state"] = json!("missing_step_result");
                    row["step_status"] = Value::Null;
                    row["step_error"] = Value::Null;
                    return row;
                };
                if result.status != "succeeded" {
                    let mut row = base();
                    row["evaluator_state"] = json!("step_not_successful");
                    row["outcome_state"] = json!(evaluator_outcome_state(&result.status));
                    row["step_status"] = json!(result.status);
                    row["step_error"] = result
                        .error
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null);
                    return row;
                }
                let Some((payload, output_source)) = retained_result_payload(result) else {
                    let mut row = base();
                    row["evaluator_state"] = json!("evaluator_output_omitted");
                    row["outcome_state"] = json!("output_omitted");
                    row["step_status"] = json!(result.status);
                    row["step_error"] = result
                        .error
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null);
                    return row;
                };
                let output = if binding.output_pointer.is_empty() {
                    Some(&payload)
                } else {
                    payload.pointer(&binding.output_pointer)
                };
                let Some(output) = output else {
                    let mut row = base();
                    row["evaluator_state"] = json!("evaluator_pointer_missing");
                    row["outcome_state"] = json!("pointer_missing");
                    row["step_status"] = json!(result.status);
                    row["step_error"] = result
                        .error
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null);
                    row["output_source"] = json!(output_source);
                    return row;
                };
                let mut row = base();
                row["step_status"] = json!(result.status);
                row["step_error"] = result
                    .error
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null);
                row["output_source"] = json!(output_source);
                row["output_type"] = json!(json_value_type(output));
                row["output_bytes"] = serde_json::to_vec(output)
                    .map(|bytes| json!(bytes.len()))
                    .unwrap_or(Value::Null);
                row["output_digest"] = ContentHash::of_value(output)
                    .ok()
                    .map(|digest| json!(digest.to_string()))
                    .unwrap_or(Value::Null);
                row["evaluator_state"] = json!("evaluator_output_retained");
                row["outcome_state"] = json!("retained");
                row
            })
            .collect::<Vec<_>>();
        let outcome_counts = evaluator_rows
            .iter()
            .filter_map(|row| row.get("outcome_state").and_then(Value::as_str))
            .fold(BTreeMap::<String, usize>::new(), |mut counts, state| {
                *counts.entry(state.to_string()).or_default() += 1;
                counts
            });
        let mut output_digest_groups = BTreeMap::<String, Vec<String>>::new();
        for row in &evaluator_rows {
            if let (Some(digest), Some(id)) = (
                row.get("output_digest").and_then(Value::as_str),
                row.get("id").and_then(Value::as_str),
            ) {
                output_digest_groups
                    .entry(digest.to_string())
                    .or_default()
                    .push(id.to_string());
            }
        }
        let output_digest_groups = output_digest_groups
            .into_iter()
            .map(|(digest, mut binding_ids)| {
                binding_ids.sort();
                json!({"digest": digest, "binding_ids": binding_ids})
            })
            .collect::<Vec<_>>();
        let evaluator_required_total = evaluator_rows
            .iter()
            .filter(|row| {
                row.get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count();
        let evaluator_required_retained = evaluator_rows
            .iter()
            .filter(|row| {
                row.get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .filter(|row| {
                row.get("evaluator_state").and_then(Value::as_str)
                    == Some("evaluator_output_retained")
            })
            .count();
        let evaluator_required_complete = evaluator_required_total == evaluator_required_retained;
        let evaluator_retained = evaluator_rows
            .iter()
            .filter(|row| {
                row.get("evaluator_state").and_then(Value::as_str)
                    == Some("evaluator_output_retained")
            })
            .count();
        let evaluator_output_digests = evaluator_rows
            .iter()
            .filter_map(|row| row.get("output_digest").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let evaluator_disagreement_posture = if claim.evaluator_bindings.is_empty() {
            "not_requested"
        } else if evaluator_retained == 0 {
            "unavailable"
        } else if evaluator_retained < evaluator_rows.len() {
            "partial"
        } else if evaluator_retained == 1 {
            "single_observation"
        } else if evaluator_output_digests.len() == 1 {
            "unanimous_digest"
        } else {
            "disagreement"
        };
        let evaluator_coverage = json!({
            "requested": claim.evaluator_bindings.len(),
            "returned": evaluator_rows.len(),
            "omitted": claim.evaluator_bindings.len().saturating_sub(MAX_CLAIM_EVALUATORS),
            "required": evaluator_required_total,
            "required_retained": evaluator_required_retained,
            "required_complete": evaluator_required_complete,
            "retained": evaluator_retained,
            "distinct_output_digests": evaluator_output_digests.len(),
            "outcome_counts": outcome_counts,
            "output_digest_groups": output_digest_groups,
            "disagreement_posture": evaluator_disagreement_posture,
            "posture": if claim.evaluator_bindings.is_empty() {
                "not_requested"
            } else if evaluator_required_complete {
                "required_complete"
            } else {
                "required_incomplete"
            }
        });
        let claimable = all_found
            && all_succeeded
            && (claim.evidence_mode == "completed_step" || all_outputs_retained)
            && evaluator_required_complete;
        let mut row = json!({
            "id": claim.id,
            "claim": claim.claim,
            "domains": claim.domains,
            "level": claim.level,
            "evidence_mode": claim.evidence_mode,
            "requires_steps": claim.requires_steps,
            "evidence_state": evidence_state,
            "evidence": evidence,
            "evaluator_bindings": evaluator_rows,
            "evaluator_coverage": evaluator_coverage,
            "evaluator_review": evaluator_review_provenance(evaluator_review),
            "claim_status": "unreviewed",
            "claimable": claimable,
            "readiness_claimed": false,
            "non_claims": [
                "step completion does not establish the truth, calibration, causality, or clinical meaning of the statement",
                "the lineage records caller-declared dependencies and retained transport outputs only"
            ]
        });
        if let Ok(digest) = ContentHash::of_value(&row) {
            row["lineage_digest"] = Value::String(digest.to_string());
        }
        rows.push(row);
    }
    let mut output = json!({
        "schema": "bioprism-devplat-mission-claim-lineage/0.1",
        "claims": rows,
        "requested": claims.len(),
        "returned": claims.len().min(MAX_CLAIM_REQUESTS),
        "omitted": claims.len().saturating_sub(MAX_CLAIM_REQUESTS),
        "evaluator_review": evaluator_review_provenance(evaluator_review),
        "claim_status": "unreviewed",
        "readiness_claimed": false,
        "guarantees": [
            "each returned claim retains its caller-declared domain labels and required step IDs",
            "each evidence row retains the exact step status, argument digest, byte count, and retained result digest when available",
            "explicit evaluator bindings retain adapter identity, domain, source pointer, and required-retention coverage",
            "missing, refused, cancelled, and output-omitted steps remain distinct from completed retained outputs"
        ],
        "non_claims": [
            "no scientific, clinical, causal, operational, or release truth is inferred",
            "claim lineage is not evaluator calibration, authority approval, or evidence completeness beyond retained mission results"
        ]
    });
    if let Ok(digest) = ContentHash::of_value(&output) {
        output["lineage_digest"] = Value::String(digest.to_string());
    }
    output
}

/// Deterministic, clock-free execution evidence for one mission transition.
///
/// The sequence is assigned by the executor, not by a wall clock or thread completion order.
/// This makes serial and parallel runs comparable while retaining the exact step ordering,
/// refusal propagation, and output accounting decisions that produced the final report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionTraceEvent {
    pub sequence: usize,
    pub event: String,
    pub wave: Option<usize>,
    pub step_id: Option<String>,
    pub tool: Option<String>,
    pub status: Option<String>,
    pub arguments_digest: Option<String>,
    pub bytes: usize,
    pub detail: Option<String>,
}

/// Optional in-process observer used by the HTTP boundary to project live mission progress.
///
/// The observer receives serialized trace events as they are appended. It is deliberately skipped
/// from mission reports so progress observation cannot change the content-addressed report or its
/// wire contract.
#[derive(Clone)]
pub struct MissionTraceObserver(pub Arc<dyn Fn(Value) + Send + Sync>);

impl fmt::Debug for MissionTraceObserver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MissionTraceObserver")
            .finish_non_exhaustive()
    }
}

impl PartialEq for MissionTraceObserver {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for MissionTraceObserver {}

/// Full plan or execution report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionReport {
    pub schema_version: String,
    pub plan: MissionPlan,
    pub execution: String,
    pub mission_status: String,
    pub succeeded: usize,
    pub refused: usize,
    pub blocked: usize,
    pub cancelled: usize,
    pub required_failures: usize,
    pub returned_bytes: usize,
    pub results: Vec<MissionStepResult>,
    pub execution_trace_schema_version: String,
    pub execution_trace: Vec<MissionTraceEvent>,
    pub claim_requests: Vec<MissionClaimRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluator_review: Option<Value>,
    pub claim_lineage: Value,
    #[serde(skip)]
    pub trace_observer: Option<MissionTraceObserver>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

/// Mission validation and planning failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MissionError {
    #[error("{field} must be a non-empty string")]
    EmptyField { field: &'static str },
    #[error("{field} contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("mission has no steps")]
    NoSteps,
    #[error("{kind} contains too many entries: {count}; maximum is {maximum}")]
    TooMany {
        kind: &'static str,
        count: usize,
        maximum: usize,
    },
    #[error("{field} must be between 1 and its safety ceiling, got {value}")]
    InvalidLimit { field: &'static str, value: usize },
    #[error("one-step output budget cannot exceed total output budget")]
    OutputBudgetOrder,
    #[error("unknown mission execution mode `{mode}`; choose `serial` or `parallel_waves`")]
    InvalidExecutionMode { mode: String },
    #[error("parallel_waves supports at most {maximum} steps in one wave, got {width}")]
    ParallelWaveTooWide { width: usize, maximum: usize },
    #[error("parallel_waves requires total output budget {required} for a worst-case wave, got {available}")]
    ParallelWaveBudget { required: usize, available: usize },
    #[error("duplicate {kind} `{id}`")]
    Duplicate { kind: &'static str, id: String },
    #[error("tool name `{tool}` is not a safe MCP tool identifier")]
    UnsafeTool { tool: String },
    #[error("mission execution requires a non-empty explicit tool allow-list")]
    MissingAllowList,
    #[error("step `{step}` requests tool `{tool}`, which is not allow-listed")]
    ToolNotAllowed { step: String, tool: String },
    #[error("step `{step}` contains a confirmation flag while side effects are disabled")]
    SideEffectsDisallowed { step: String },
    #[error("agent_mission cannot invoke itself")]
    RecursiveTool,
    #[error("step `{step}` arguments must be a JSON object")]
    ArgumentsMustBeObject { step: String },
    #[error("step `{step}` depends on itself")]
    SelfDependency { step: String },
    #[error("step `{step}` depends on unknown step `{dependency}`")]
    UnknownDependency { step: String, dependency: String },
    #[error("step `{step}` binds from unknown step `{source_step}`")]
    UnknownBindingSource { step: String, source_step: String },
    #[error("step `{step}` binding source `{source_step}` is not a direct dependency")]
    BindingWithoutDependency { step: String, source_step: String },
    #[error("invalid JSON pointer `{pointer}`")]
    InvalidPointer { pointer: String },
    #[error("JSON pointer `{pointer}` did not resolve")]
    MissingPointer { pointer: String },
    #[error("mission dependency cycle contains {steps:?}")]
    DependencyCycle { steps: Vec<String> },
    #[error("cannot canonicalise mission: {0}")]
    Canonicalisation(String),
    #[error("invalid mission claim `{claim}`: {reason}")]
    InvalidClaim { claim: String, reason: String },
    #[error("mission claim `{claim}` requires unknown step `{step}`")]
    UnknownClaimStep { claim: String, step: String },
    #[error("invalid evaluator review: {reason}")]
    InvalidEvaluatorReview { reason: String },
    #[error("invalid workflow binding: {reason}")]
    InvalidWorkflowBinding { reason: String },
    #[error("invalid capability route review: {reason}")]
    InvalidRouteReview { reason: String },
}

/// Build a deterministic plan from a mission request.
pub fn plan_mission(request: &MissionRequest) -> Result<MissionPlan, MissionError> {
    request.validate()?;
    let waves = request.waves()?;
    let max_wave_width = waves.iter().map(Vec::len).max().unwrap_or(0);
    if request.policy.execution_mode == "parallel_waves" {
        if max_wave_width > MAX_PARALLEL_WAVE_WIDTH {
            return Err(MissionError::ParallelWaveTooWide {
                width: max_wave_width,
                maximum: MAX_PARALLEL_WAVE_WIDTH,
            });
        }
        let required = request
            .policy
            .max_step_output_bytes
            .saturating_mul(max_wave_width);
        if required > request.policy.max_total_output_bytes {
            return Err(MissionError::ParallelWaveBudget {
                required,
                available: request.policy.max_total_output_bytes,
            });
        }
    }
    let ordered_steps = waves.iter().flatten().cloned().collect::<Vec<_>>();
    let wave_by_id = waves
        .iter()
        .enumerate()
        .flat_map(|(wave, ids)| ids.iter().map(move |id| (id.as_str(), wave)))
        .collect::<BTreeMap<_, _>>();
    let by_id = request
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect::<BTreeMap<_, _>>();
    let steps = ordered_steps
        .iter()
        .map(|id| {
            let step = by_id[id.as_str()];
            MissionStepPlan {
                id: step.id.clone(),
                domain: step.domain.clone(),
                capability: step.capability.clone(),
                objective: step.objective.clone(),
                tool: step.tool.clone(),
                depends_on: step.depends_on.clone(),
                bindings: step.bindings.clone(),
                required: step.required,
                wave: wave_by_id[id.as_str()],
            }
        })
        .collect::<Vec<_>>();
    let encoded = serde_json::to_value(request)
        .map_err(|error| MissionError::Canonicalisation(error.to_string()))?;
    let digest = ContentHash::of_value(&encoded)
        .map_err(|error| MissionError::Canonicalisation(error.to_string()))?
        .to_string();
    Ok(MissionPlan {
        schema_version: MISSION_SCHEMA_VERSION.into(),
        mission_id: request.mission_id.clone(),
        goal: request.goal.clone(),
        digest,
        step_count: steps.len(),
        ordered_steps,
        critical_path_length: waves.len(),
        waves,
        steps,
        execution: if request.policy.execute {
            "authorized"
        } else {
            "planned"
        }
        .into(),
        execution_mode: request.policy.execution_mode.clone(),
        max_parallelism: request.policy.max_parallelism,
        workflow_binding: request.workflow_binding.clone(),
        route_review_provenance: route_review_provenance(request.route_review.as_ref()),
        guarantees: vec![
            "step dependencies are validated and ordered deterministically".into(),
            "execution requires an explicit tool allow-list and is opt-in".into(),
            "side-effect confirmation and output budgets are policy-controlled".into(),
            "the plan is content-addressed before any tool call".into(),
            "a supplied ready capability route review is bound to the exact mission goal and steps without implying readiness or execution".into(),
        ],
        limitations: vec![
            "the planner does not infer missing arguments or scientific meaning".into(),
            "route review provenance is caller-owned structure and does not authorize execution".into(),
            if request.policy.execution_mode == "parallel_waves" {
                format!(
                    "independent steps in each wave execute in bounded batches of at most {} in the server process",
                    request.policy.max_parallelism
                )
            } else {
                "parallel waves are reported for scheduling; the MCP adapter executes serially"
                    .into()
            },
            "tool results remain domain-owned and are not merged into a synthetic truth claim"
                .into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::{
        MissionEvaluatorQuery, MissionEvaluatorReviewRequest, MissionEvaluatorSelection,
    };

    fn step(id: &str, tool: &str, depends_on: &[&str]) -> MissionStep {
        MissionStep {
            id: id.into(),
            domain: "metrics".into(),
            capability: "analytics".into(),
            objective: format!("run {id}"),
            tool: tool.into(),
            arguments: empty_object(),
            depends_on: depends_on.iter().map(|value| (*value).into()).collect(),
            bindings: Vec::new(),
            required: true,
        }
    }

    fn request(steps: Vec<MissionStep>) -> MissionRequest {
        MissionRequest {
            mission_id: "mission-1".into(),
            goal: "compose evidence".into(),
            steps,
            policy: MissionPolicy::default(),
            claim_requests: Vec::new(),
            evaluator_review: None,
            workflow_binding: None,
            route_review: None,
        }
    }

    #[test]
    fn plan_orders_parallel_waves_and_is_digest_bound() {
        let plan = plan_mission(&request(vec![
            step("release", "release_audit", &["metrics", "evidence"]),
            step("metrics", "metrics_analytics_audit", &[]),
            step("evidence", "biocapability_evidence_audit", &[]),
        ]))
        .unwrap();
        assert_eq!(
            plan.waves,
            vec![
                vec![String::from("evidence"), String::from("metrics")],
                vec![String::from("release")]
            ]
        );
        assert_eq!(plan.ordered_steps, vec!["evidence", "metrics", "release"]);
        assert_eq!(plan.critical_path_length, 2);
        assert_eq!(plan.digest.len(), 64);
        assert_eq!(plan.execution, "planned");
    }

    #[test]
    fn workflow_binding_is_bounded_and_evidence_plan_digest_bound() {
        let evidence_plan = serde_json::json!({
            "schema": "workflow-contract/0.1",
            "steps": [{"step_id": "one", "tool": "metrics_analytics_audit"}],
            "completion": {"required_steps": "succeeded"}
        });
        let evidence_plan_digest = ContentHash::of_value(&evidence_plan).unwrap().to_string();
        let mut value = request(vec![step("one", "metrics_analytics_audit", &[])]);
        value.workflow_binding = Some(serde_json::json!({
            "workflow_id": "metrics_and_analytics",
            "workflow_digest": "a".repeat(64),
            "catalog_digest": "b".repeat(64),
            "domain_contract_digest": "c".repeat(64),
            "domain_contract": {"posture": "advisory_review_gated"},
            "evidence_plan": evidence_plan,
            "evidence_plan_digest": evidence_plan_digest,
        }));
        assert!(plan_mission(&value).is_ok());
        value.workflow_binding.as_mut().unwrap()["evidence_plan"]["steps"][0]["tool"] =
            serde_json::json!("tampered_tool");
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::InvalidWorkflowBinding { .. })
        ));
    }

    #[test]
    fn cycles_and_unknown_dependencies_refuse() {
        let mut value = request(vec![
            step("a", "metrics_analytics_audit", &["b"]),
            step("b", "metrics_analytics_audit", &["a"]),
        ]);
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::DependencyCycle { .. })
        ));
        value.steps[1].depends_on = vec!["missing".into()];
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::UnknownDependency { .. })
        ));
    }

    #[test]
    fn execution_requires_allow_list_and_blocks_confirmation_by_default() {
        let mut value = request(vec![step("one", "metrics_analytics_audit", &[])]);
        value.policy.execute = true;
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::MissingAllowList)
        ));
        value.policy.allowed_tools = vec!["metrics_analytics_audit".into()];
        value.steps[0].arguments = serde_json::json!({"confirm": true});
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::SideEffectsDisallowed { .. })
        ));
        value.policy.allow_side_effects = true;
        assert_eq!(plan_mission(&value).unwrap().execution, "authorized");
    }

    #[test]
    fn parallel_waves_are_explicit_and_budget_reserved_before_execution() {
        let mut value = request(vec![
            step("one", "metrics_analytics_audit", &[]),
            step("two", "metrics_analytics_audit", &[]),
        ]);
        value.policy.execution_mode = "parallel_waves".into();
        value.policy.max_step_output_bytes = 2_000_000;
        value.policy.max_total_output_bytes = 4_000_000;
        value.policy.max_parallelism = 2;
        let plan = plan_mission(&value).unwrap();
        assert_eq!(plan.execution_mode, "parallel_waves");
        assert_eq!(plan.max_parallelism, 2);
        assert_eq!(plan.waves[0].len(), 2);

        value.policy.max_total_output_bytes = 3_000_000;
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::ParallelWaveBudget { .. })
        ));
        value.policy.max_total_output_bytes = 4_000_000;
        value.policy.execution_mode = "unknown".into();
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::InvalidExecutionMode { .. })
        ));
        value.policy.execution_mode = "parallel_waves".into();
        value.policy.max_parallelism = MAX_PARALLEL_WAVE_WIDTH + 1;
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::InvalidLimit {
                field: "policy.max_parallelism",
                ..
            })
        ));
    }

    #[test]
    fn recursive_and_unsafe_tools_are_rejected() {
        let mut value = request(vec![step("one", "agent_mission", &[])]);
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::RecursiveTool)
        ));
        value.steps[0].tool = "../../shell".into();
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::UnsafeTool { .. })
        ));
    }

    #[test]
    fn serde_defaults_keep_preview_requests_small_and_explicit() {
        let value: MissionRequest = serde_json::from_value(serde_json::json!({
            "mission_id": "m",
            "goal": "g",
            "steps": [{"id": "s", "domain": "d", "capability": "c", "objective": "o", "tool": "metrics_analytics_audit"}]
        }))
        .unwrap();
        assert!(!value.policy.execute);
        assert!(value.steps[0].required);
        assert!(value.steps[0].arguments.is_object());
    }

    #[test]
    fn bindings_are_typed_dataflow_edges_and_require_direct_dependencies() {
        let binding = MissionBinding {
            from_step: "source".into(),
            source_pointer: "/value".into(),
            target_pointer: "/inputs/0".into(),
        };
        let mut arguments = serde_json::json!({"inputs": [null]});
        apply_binding(&mut arguments, &binding, &serde_json::json!({"value": 7})).unwrap();
        assert_eq!(arguments["inputs"][0], serde_json::json!(7));

        let mut value = request(vec![
            step("source", "metrics_analytics_audit", &[]),
            step("sink", "metrics_analytics_audit", &["source"]),
        ]);
        value.steps[1].bindings = vec![binding];
        value.steps[1].arguments = serde_json::json!({"inputs": [null]});
        assert!(plan_mission(&value).is_ok());
        value.steps[1].depends_on.clear();
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::BindingWithoutDependency { .. })
        ));
        value.steps[1].depends_on = vec!["source".into()];
        value.steps[1].arguments = empty_object();
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::MissingPointer { .. })
        ));
        value.steps[1].arguments = serde_json::json!({"inputs": [null]});
        value.steps[1].bindings[0].source_pointer = "/value~2".into();
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::InvalidPointer { .. })
        ));
    }

    #[test]
    fn claims_require_known_steps_and_lineage_stays_non_semantic() {
        let mut value = request(vec![step("observe", "metrics_analytics_audit", &[])]);
        value.claim_requests = vec![MissionClaimRequest {
            id: "metric-observed".into(),
            claim: "The requested metric was observed by the named evaluator.".into(),
            domains: vec!["metrics".into(), "evaluation".into()],
            requires_steps: vec!["observe".into()],
            level: "observation".into(),
            evidence_mode: "successful_tool_result".into(),
            evaluator_bindings: vec![
                MissionClaimEvaluatorBinding {
                    id: "metrics-evaluator".into(),
                    adapter_id: "metrics-audit-v1".into(),
                    domain: "metrics".into(),
                    step_id: "observe".into(),
                    output_pointer: "/value".into(),
                    required: true,
                },
                MissionClaimEvaluatorBinding {
                    id: "evaluation-evaluator".into(),
                    adapter_id: "evaluation-audit-v1".into(),
                    domain: "evaluation".into(),
                    step_id: "observe".into(),
                    output_pointer: "/ok".into(),
                    required: true,
                },
            ],
        }];
        assert!(plan_mission(&value).is_ok());

        value.claim_requests[0].requires_steps = vec!["missing".into()];
        assert!(matches!(
            plan_mission(&value),
            Err(MissionError::UnknownClaimStep { .. })
        ));

        let claim = MissionClaimRequest {
            id: "metric-observed".into(),
            claim: "The requested metric was observed by the named evaluator.".into(),
            domains: vec!["metrics".into()],
            requires_steps: vec!["observe".into()],
            level: "observation".into(),
            evidence_mode: "successful_tool_result".into(),
            evaluator_bindings: vec![
                MissionClaimEvaluatorBinding {
                    id: "metrics-evaluator".into(),
                    adapter_id: "metrics-audit-v1".into(),
                    domain: "metrics".into(),
                    step_id: "observe".into(),
                    output_pointer: "/value".into(),
                    required: true,
                },
                MissionClaimEvaluatorBinding {
                    id: "evaluation-evaluator".into(),
                    adapter_id: "evaluation-audit-v1".into(),
                    domain: "evaluation".into(),
                    step_id: "observe".into(),
                    output_pointer: "/ok".into(),
                    required: true,
                },
            ],
        };
        let retained = mission_claim_lineage(
            std::slice::from_ref(&claim),
            &[MissionStepResult {
                id: "observe".into(),
                tool: "metrics_analytics_audit".into(),
                status: "succeeded".into(),
                required: true,
                arguments_digest: Some("a".repeat(64)),
                bytes: 12,
                wire: Some(json!({"ok": true, "value": 3})),
                error: None,
            }],
        );
        assert_eq!(retained["claims"][0]["claim_status"], "unreviewed");
        assert_eq!(retained["claims"][0]["claimable"], true);
        assert_eq!(retained["claims"][0]["readiness_claimed"], false);
        assert_eq!(
            retained["claims"][0]["evidence"][0]["evidence_state"],
            "completed_output_retained"
        );
        assert_eq!(
            retained["claims"][0]["evaluator_coverage"]["posture"],
            "required_complete"
        );
        assert_eq!(
            retained["claims"][0]["evaluator_bindings"][0]["evaluator_state"],
            "evaluator_output_retained"
        );
        assert_eq!(
            retained["claims"][0]["evaluator_coverage"]["disagreement_posture"],
            "disagreement"
        );
        assert!(retained["claims"][0]["non_claims"][0]
            .as_str()
            .unwrap()
            .contains("does not establish"));
        assert_eq!(retained["lineage_digest"].as_str().unwrap().len(), 64);

        let omitted = mission_claim_lineage(
            &[claim],
            &[MissionStepResult {
                id: "observe".into(),
                tool: "metrics_analytics_audit".into(),
                status: "succeeded".into(),
                required: true,
                arguments_digest: None,
                bytes: 20_000_001,
                wire: None,
                error: None,
            }],
        );
        assert_eq!(omitted["claims"][0]["claimable"], false);
        assert_eq!(
            omitted["claims"][0]["evidence"][0]["evidence_state"],
            "completed_output_omitted"
        );
        assert_eq!(
            omitted["claims"][0]["evaluator_bindings"][0]["evaluator_state"],
            "evaluator_output_omitted"
        );
        assert_eq!(
            omitted["claims"][0]["evaluator_bindings"][0]["outcome_state"],
            "output_omitted"
        );
    }

    #[test]
    fn ready_evaluator_review_is_required_to_match_dispatch_bindings() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let search = catalogue
            .search(&MissionEvaluatorQuery {
                adapter_id: Some("oncoworlds.assay_fidelity".into()),
                max_items: 4,
                ..MissionEvaluatorQuery::default()
            })
            .unwrap();
        let mut discovery = serde_json::to_value(search).unwrap();
        discovery["workflow"] = json!("mission_evaluator_discover");
        discovery["selection_posture"] = json!("candidate_only");
        let review = catalogue
            .review(&MissionEvaluatorReviewRequest {
                discovery,
                selections: vec![MissionEvaluatorSelection {
                    id: "assay-evaluator".into(),
                    claim_id: "fidelity-claim".into(),
                    adapter_id: "oncoworlds.assay_fidelity".into(),
                    domain: "oncology".into(),
                    step_id: "assay".into(),
                    output_pointer: "/fidelity".into(),
                    required: true,
                }],
            })
            .unwrap();
        let claim = MissionClaimRequest {
            id: "fidelity-claim".into(),
            claim: "The assay output was retained for review.".into(),
            domains: vec!["oncology".into()],
            requires_steps: vec!["assay".into()],
            level: "evaluation".into(),
            evidence_mode: "successful_tool_result".into(),
            evaluator_bindings: vec![MissionClaimEvaluatorBinding {
                id: "assay-evaluator".into(),
                adapter_id: "oncoworlds.assay_fidelity".into(),
                domain: "oncology".into(),
                step_id: "assay".into(),
                output_pointer: "/fidelity".into(),
                required: true,
            }],
        };
        let mut mission = request(vec![step("assay", "capability_audit", &[])]);
        mission.claim_requests = vec![claim];
        mission.evaluator_review = Some(review.clone());
        assert!(plan_mission(&mission).is_ok());

        let mut mismatched = review;
        mismatched["bindings"][0]["domain"] = json!("unrelated");
        mission.evaluator_review = Some(mismatched);
        assert!(matches!(
            plan_mission(&mission),
            Err(MissionError::InvalidEvaluatorReview { .. })
        ));
    }

    #[test]
    fn lineage_retains_structured_evaluator_output_and_refusal_classes() {
        let claim = MissionClaimRequest {
            id: "fidelity-claim".into(),
            claim: "The assay output was retained for review.".into(),
            domains: vec!["oncology".into()],
            requires_steps: vec!["assay".into()],
            level: "evaluation".into(),
            evidence_mode: "successful_tool_result".into(),
            evaluator_bindings: vec![MissionClaimEvaluatorBinding {
                id: "assay-evaluator".into(),
                adapter_id: "oncoworlds.assay_fidelity".into(),
                domain: "oncology".into(),
                step_id: "assay".into(),
                output_pointer: "/fidelity".into(),
                required: true,
            }],
        };
        let retained = mission_claim_lineage(
            std::slice::from_ref(&claim),
            &[MissionStepResult {
                id: "assay".into(),
                tool: "oncoworlds_model_transport".into(),
                status: "succeeded".into(),
                required: true,
                arguments_digest: Some("a".repeat(64)),
                bytes: 120,
                wire: Some(json!({
                    "result": {"structuredContent": {"fidelity": {"score": 0.9}}}
                })),
                error: None,
            }],
        );
        assert_eq!(
            retained["claims"][0]["evaluator_bindings"][0]["outcome_state"],
            "retained"
        );
        assert_eq!(
            retained["claims"][0]["evaluator_bindings"][0]["output_source"],
            "structured_content"
        );
        assert_eq!(
            retained["claims"][0]["evaluator_bindings"][0]["output_type"],
            "object"
        );

        let refused = mission_claim_lineage(
            &[claim],
            &[MissionStepResult {
                id: "assay".into(),
                tool: "oncoworlds_model_transport".into(),
                status: "refused".into(),
                required: true,
                arguments_digest: Some("a".repeat(64)),
                bytes: 40,
                wire: None,
                error: Some("domain tool refused".into()),
            }],
        );
        assert_eq!(
            refused["claims"][0]["evaluator_bindings"][0]["outcome_state"],
            "refused"
        );
        assert_eq!(
            refused["claims"][0]["evaluator_coverage"]["outcome_counts"]["refused"],
            1
        );
    }
}
