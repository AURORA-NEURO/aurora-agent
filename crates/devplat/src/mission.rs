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
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Schema version for mission requests, plans, and execution reports.
pub const MISSION_SCHEMA_VERSION: &str = "bioprism-devplat-mission/0.1";
const MAX_STEPS: usize = 128;
const MAX_ALLOWED_TOOLS: usize = 512;
const MAX_STEP_OUTPUT_BYTES: usize = 20_000_000;
const MAX_TOTAL_OUTPUT_BYTES: usize = 20_000_000;
pub const MAX_PARALLEL_WAVE_WIDTH: usize = 16;

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

/// A complete mission request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionRequest {
    pub mission_id: String,
    pub goal: String,
    pub steps: Vec<MissionStep>,
    #[serde(default)]
    pub policy: MissionPolicy,
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
    pub required_failures: usize,
    pub returned_bytes: usize,
    pub results: Vec<MissionStepResult>,
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
        guarantees: vec![
            "step dependencies are validated and ordered deterministically".into(),
            "execution requires an explicit tool allow-list and is opt-in".into(),
            "side-effect confirmation and output budgets are policy-controlled".into(),
            "the plan is content-addressed before any tool call".into(),
        ],
        limitations: vec![
            "the planner does not infer missing arguments or scientific meaning".into(),
            if request.policy.execution_mode == "parallel_waves" {
                "independent steps in each wave execute concurrently in the bounded server process"
                    .into()
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
        let plan = plan_mission(&value).unwrap();
        assert_eq!(plan.execution_mode, "parallel_waves");
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
}
