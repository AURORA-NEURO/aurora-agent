//! Deterministic, bounded workflow templates for the complete capability catalogue.
//!
//! A capability catalogue is useful for discovery, and [`crate::mission`] is useful for
//! execution planning, but an agent still needs a stable handoff between those two surfaces.
//! This module supplies that handoff without inventing a second tool registry: it consumes the
//! explicit capability groups and the authoritative MCP tool definitions, then emits one
//! content-addressed workflow template per group. Instantiation is deliberately narrower than
//! discovery. Every selected step must belong to the chosen group, every executable mission is
//! allow-listed from the selected steps, and all dispatch remains outside this module.
//!
//! Stage labels are lexical routing hints, not claims about tool semantics. Domain-specific
//! arguments remain caller-owned and authoritative schema validation is deferred to the MCP
//! server's preflight boundary.

use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use crate::mission::MissionRequest;

pub const DOMAIN_WORKFLOW_SCHEMA_VERSION: &str = "bioprism-devplat-domain-workflow/0.1";
pub const DOMAIN_WORKFLOW_CATALOGUE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-catalogue/0.1";
pub const DOMAIN_WORKFLOW_INSTANTIATE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-instantiate/0.1";
pub const DOMAIN_WORKFLOW_CONTRACT_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-contract/0.1";
pub const MAX_DOMAIN_WORKFLOW_GROUPS: usize = 128;
pub const MAX_DOMAIN_WORKFLOW_TOOLS: usize = 256;
pub const MAX_DOMAIN_WORKFLOW_STEPS: usize = 128;
pub const MAX_DOMAIN_WORKFLOW_BYTES: usize = 20_000_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainWorkflowError {
    #[error("capability catalogue must be an array")]
    CatalogueNotArray,
    #[error("capability catalogue is empty")]
    EmptyCatalogue,
    #[error("capability catalogue contains {count} groups; maximum is {maximum}")]
    TooManyGroups { count: usize, maximum: usize },
    #[error("invalid capability group {group:?}: {reason}")]
    InvalidGroup { group: usize, reason: String },
    #[error("tool definitions must be an array")]
    ToolDefinitionsNotArray,
    #[error("invalid tool definition: {0}")]
    InvalidToolDefinition(String),
    #[error("workflow request must be an object")]
    RequestNotObject,
    #[error("invalid workflow request: {0}")]
    InvalidRequest(String),
    #[error("unknown workflow {workflow_id:?}")]
    UnknownWorkflow { workflow_id: String },
    #[error("workflow has too many steps; maximum is {maximum}")]
    InvalidStepCount { maximum: usize },
    #[error("invalid workflow step {step}: {reason}")]
    InvalidStep { step: usize, reason: String },
    #[error("step {step} selects tool {tool:?}, outside workflow {workflow_id:?}")]
    ToolOutsideWorkflow {
        step: usize,
        tool: String,
        workflow_id: String,
    },
    #[error("step {step} selects unavailable tool {tool:?} from workflow {workflow_id:?}")]
    ToolUnavailable {
        step: usize,
        tool: String,
        workflow_id: String,
    },
    #[error("policy allow-list selects tool {tool:?} outside workflow {workflow_id:?}")]
    PolicyToolOutsideWorkflow { tool: String, workflow_id: String },
    #[error("cannot canonicalise workflow document: {0}")]
    Canonicalisation(String),
    #[error("workflow document is {actual} bytes; maximum is {maximum}")]
    TooLarge { actual: usize, maximum: usize },
}

fn visible_text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, String> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a non-empty string"))?;
    if value.trim().is_empty()
        || value
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
    {
        return Err(format!("{field} must be a non-empty control-free string"));
    }
    Ok(value)
}

fn string_array(
    object: &Map<String, Value>,
    field: &str,
    required: bool,
    maximum: usize,
) -> Result<Vec<String>, String> {
    let Some(value) = object.get(field) else {
        return if required {
            Err(format!("{field} must be an array"))
        } else {
            Ok(Vec::new())
        };
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?;
    if values.len() > maximum {
        return Err(format!(
            "{field} contains too many values; maximum is {maximum}"
        ));
    }
    let mut seen = BTreeSet::new();
    let mut output = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let item = value
            .as_str()
            .filter(|item| !item.trim().is_empty())
            .ok_or_else(|| format!("{field}[{index}] must be a non-empty string"))?;
        if !seen.insert(item.to_string()) {
            return Err(format!("{field} contains duplicate {item:?}"));
        }
        output.push(item.to_string());
    }
    Ok(output)
}

fn checked_bytes(value: &Value) -> Result<(), DomainWorkflowError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|error| DomainWorkflowError::Canonicalisation(error.to_string()))?;
    if encoded.len() > MAX_DOMAIN_WORKFLOW_BYTES {
        return Err(DomainWorkflowError::TooLarge {
            actual: encoded.len(),
            maximum: MAX_DOMAIN_WORKFLOW_BYTES,
        });
    }
    Ok(())
}

fn digest(value: &Value) -> Result<String, DomainWorkflowError> {
    ContentHash::of_value(value)
        .map(|hash| hash.to_string())
        .map_err(|error| DomainWorkflowError::Canonicalisation(error.to_string()))
}

fn tool_role(tool: &str) -> &'static str {
    let name = tool.to_ascii_lowercase();
    if [
        "catalog", "discover", "search", "plan", "route", "profile", "status",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        "discover"
    } else if [
        "audit", "check", "review", "verify", "gate", "compare", "validate", "health", "screen",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        "validate_or_review"
    } else if [
        "ingest",
        "compile",
        "generate",
        "simulate",
        "run",
        "apply",
        "analyze",
        "assess",
        "classify",
        "join",
        "transport",
        "synthesize",
        "project",
        "index",
        "minimize",
        "mutate",
        "combine",
        "missingness",
        "reference",
        "acquisition",
        "evaluator",
        "replay",
        "trace",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        "transform_or_analyze"
    } else if [
        "release", "publish", "delivery", "receipt", "submit", "accept",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        "handoff_or_release"
    } else {
        "inspect"
    }
}

fn title_for(id: &str) -> String {
    id.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn tool_definition_records(value: &Value) -> Result<BTreeMap<String, Value>, DomainWorkflowError> {
    let definitions = value
        .as_array()
        .ok_or(DomainWorkflowError::ToolDefinitionsNotArray)?;
    if definitions.len() > MAX_DOMAIN_WORKFLOW_TOOLS * 4 {
        return Err(DomainWorkflowError::InvalidToolDefinition(format!(
            "{} definitions exceed the bounded catalogue input",
            definitions.len()
        )));
    }
    let mut records = BTreeMap::new();
    for definition in definitions {
        let object = definition.as_object().ok_or_else(|| {
            DomainWorkflowError::InvalidToolDefinition("each definition must be an object".into())
        })?;
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| {
                DomainWorkflowError::InvalidToolDefinition(
                    "each definition must have a non-empty name".into(),
                )
            })?;
        if records
            .insert(name.to_string(), definition.clone())
            .is_some()
        {
            return Err(DomainWorkflowError::InvalidToolDefinition(format!(
                "duplicate tool name {name:?}"
            )));
        }
    }
    Ok(records)
}

fn tool_contracts(
    advertised_tools: &[String],
    available: &BTreeSet<String>,
    definitions: &BTreeMap<String, Value>,
) -> Result<Vec<Value>, DomainWorkflowError> {
    advertised_tools
        .iter()
        .map(|tool| {
            let definition = definitions.get(tool);
            let is_available = available.contains(tool);
            let schema = definition.and_then(|definition| definition.get("inputSchema"));
            let schema_state = if !is_available {
                "unavailable"
            } else if schema.is_some_and(Value::is_object) {
                "present"
            } else {
                "missing"
            };
            let schema_digest = schema
                .filter(|schema| schema.is_object())
                .map(digest)
                .transpose()?;
            Ok(json!({
                "name": tool,
                "role": tool_role(tool),
                "declared": true,
                "available": is_available,
                "schema_state": schema_state,
                "schema_digest": schema_digest,
                "argument_validation": "authoritative_mcp_preflight_required",
                "evidence": {
                    "capture": ["arguments_digest", "result_status", "result_digest"],
                    "retain_refusal_or_omission": true,
                    "claim_binding": "caller_owned_claims_must_bind_to_explicit_step_evidence",
                },
            }))
        })
        .collect()
}

fn group_workflow(
    group_index: usize,
    group: &Map<String, Value>,
    advertised_tools: &[String],
    available: &BTreeSet<String>,
    definitions: &BTreeMap<String, Value>,
    catalog_digest: &str,
) -> Result<Value, DomainWorkflowError> {
    let id = visible_text(group, "id").map_err(|reason| DomainWorkflowError::InvalidGroup {
        group: group_index,
        reason,
    })?;
    let domains =
        string_array(group, "domains", true, MAX_DOMAIN_WORKFLOW_GROUPS).map_err(|reason| {
            DomainWorkflowError::InvalidGroup {
                group: group_index,
                reason,
            }
        })?;
    let crates =
        string_array(group, "crates", true, MAX_DOMAIN_WORKFLOW_GROUPS).map_err(|reason| {
            DomainWorkflowError::InvalidGroup {
                group: group_index,
                reason,
            }
        })?;
    let cli_entrypoints = string_array(group, "cli_entrypoints", true, MAX_DOMAIN_WORKFLOW_GROUPS)
        .map_err(|reason| DomainWorkflowError::InvalidGroup {
            group: group_index,
            reason,
        })?;
    if advertised_tools.is_empty() {
        return Err(DomainWorkflowError::InvalidGroup {
            group: group_index,
            reason: "mcp_tools must contain at least one tool".into(),
        });
    }
    if advertised_tools.len() > MAX_DOMAIN_WORKFLOW_TOOLS {
        return Err(DomainWorkflowError::InvalidGroup {
            group: group_index,
            reason: format!("mcp_tools contains more than {MAX_DOMAIN_WORKFLOW_TOOLS} tools"),
        });
    }
    let available_tools = advertised_tools
        .iter()
        .filter(|tool| available.contains(*tool))
        .cloned()
        .collect::<Vec<_>>();
    let missing_tools = advertised_tools
        .iter()
        .filter(|tool| !available.contains(*tool))
        .cloned()
        .collect::<Vec<_>>();

    let mut role_tools: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for tool in advertised_tools {
        role_tools
            .entry(tool_role(tool))
            .or_default()
            .push(tool.clone());
    }
    let recommended_stages = role_tools
        .into_iter()
        .map(|(stage, tools)| {
            json!({
                "stage": stage,
                "tools": tools,
                "selection": "caller_selected",
                "arguments": "domain_specific_object_required",
                "posture": "advisory_lexical_routing_hint",
            })
        })
        .collect::<Vec<_>>();
    let tool_contracts = tool_contracts(advertised_tools, available, definitions)?;
    let all_tools_available = tool_contracts
        .iter()
        .all(|contract| contract["available"].as_bool().unwrap_or(false));
    let all_schemas_present = tool_contracts
        .iter()
        .all(|contract| contract["schema_state"] == "present");
    let domain_contract = json!({
        "schema": DOMAIN_WORKFLOW_CONTRACT_SCHEMA_VERSION,
        "posture": "advisory_review_gated",
        "scope": {
            "capability_group": id,
            "domains": domains,
            "crates": crates,
            "cli_entrypoints": cli_entrypoints,
            "declared_tools": advertised_tools,
        },
        "readiness": {
            "template": "available",
            "tool_inventory": if all_tools_available { "complete" } else { "incomplete" },
            "argument_schemas": if all_schemas_present { "present" } else { "requires_preflight" },
            "dispatch": "blocked_until_explicit_policy_and_authoritative_preflight",
        },
        "pre_dispatch_gates": [
            {"id": "scope_review", "required": true, "rule": "every selected step remains inside this capability group"},
            {"id": "tool_availability", "required": true, "rule": "every selected tool is present in authoritative tools/list"},
            {"id": "argument_schema_preflight", "required": true, "rule": "each caller-owned argument object passes the authoritative input schema"},
            {"id": "execution_policy", "required": true, "rule": "execution is explicit and every executable tool is allow-listed"},
        ],
        "evidence_contract": {
            "per_step": ["step_id", "tool", "arguments_digest", "result_status", "result_digest"],
            "retain_refusal_or_omission": true,
            "claim_binding": "claims must point to explicit completed-step evidence; refusal and omission remain non-support",
            "unresolved_work": "must remain visible until caller review resolves or accepts it",
        },
        "completion_contract": {
            "required_steps": "succeeded",
            "optional_steps": "succeeded_or_explicit_refusal_or_omission",
            "review": "required_before_domain_or_release_claims",
            "truth_posture": "no scientific, clinical, operational, or release conclusion is inferred",
        },
    });
    let domain_contract_digest = digest(&domain_contract)?;
    let mut workflow = json!({
        "schema": DOMAIN_WORKFLOW_SCHEMA_VERSION,
        "workflow_id": id,
        "title": format!("{} workflow", title_for(id)),
        "group_id": id,
        "domains": domains,
        "crates": crates,
        "cli_entrypoints": cli_entrypoints,
        "status": group.get("status").and_then(Value::as_str).unwrap_or("available"),
        "catalog_digest": catalog_digest,
        "tools": {
            "declared": advertised_tools,
            "available": available_tools,
            "missing": missing_tools,
        },
        "tool_contracts": tool_contracts,
        "domain_contract": domain_contract,
        "domain_contract_digest": domain_contract_digest,
        "recommended_stages": recommended_stages,
        "instantiation": {
            "requires": ["mission_id", "goal", "steps"],
            "arguments": "explicit caller-owned JSON objects",
            "default_execution": "planned",
            "preflight": "required",
            "scope": "selected steps must use tools declared by this workflow",
        },
        "execution": "not_started",
        "non_claims": [
            "a workflow template does not execute tools",
            "stage labels do not prove scientific validity or readiness",
            "available tool names do not prove that domain-specific arguments are valid",
        ],
    });
    let workflow_digest = digest(&workflow)?;
    workflow["workflow_digest"] = json!(workflow_digest);
    Ok(workflow)
}

pub fn build_domain_workflow_catalogue(
    catalogue: &Value,
    tool_definitions: &Value,
) -> Result<Value, DomainWorkflowError> {
    checked_bytes(catalogue)?;
    checked_bytes(tool_definitions)?;
    let groups = catalogue
        .as_array()
        .ok_or(DomainWorkflowError::CatalogueNotArray)?;
    if groups.is_empty() {
        return Err(DomainWorkflowError::EmptyCatalogue);
    }
    if groups.len() > MAX_DOMAIN_WORKFLOW_GROUPS {
        return Err(DomainWorkflowError::TooManyGroups {
            count: groups.len(),
            maximum: MAX_DOMAIN_WORKFLOW_GROUPS,
        });
    }
    let definitions = tool_definition_records(tool_definitions)?;
    let available = definitions.keys().cloned().collect::<BTreeSet<_>>();
    let catalog_digest = digest(catalogue)?;
    let mut workflows = Vec::with_capacity(groups.len());
    let mut declared_tools = BTreeSet::new();
    let mut domains = BTreeSet::new();
    for (index, raw_group) in groups.iter().enumerate() {
        let group = raw_group
            .as_object()
            .ok_or_else(|| DomainWorkflowError::InvalidGroup {
                group: index,
                reason: "group must be an object".into(),
            })?;
        let advertised = string_array(group, "mcp_tools", true, MAX_DOMAIN_WORKFLOW_TOOLS)
            .map_err(|reason| DomainWorkflowError::InvalidGroup {
                group: index,
                reason,
            })?;
        let group_domains = string_array(group, "domains", true, MAX_DOMAIN_WORKFLOW_GROUPS)
            .map_err(|reason| DomainWorkflowError::InvalidGroup {
                group: index,
                reason,
            })?;
        declared_tools.extend(advertised.iter().cloned());
        domains.extend(group_domains);
        let mut workflow = group_workflow(
            index,
            group,
            &advertised,
            &available,
            &definitions,
            &catalog_digest,
        )?;
        workflow["group_index"] = json!(index);
        workflows.push(workflow);
    }
    workflows.sort_by(|left, right| {
        left["workflow_id"]
            .as_str()
            .cmp(&right["workflow_id"].as_str())
    });
    let workflow_digests = workflows
        .iter()
        .filter_map(|workflow| workflow.get("workflow_digest").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let workflow_catalog_digest = digest(&json!(workflow_digests))?;
    let groups_with_missing_tools = workflows
        .iter()
        .filter(|workflow| {
            workflow["tools"]["missing"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        })
        .count();
    let output = json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_CATALOGUE_SCHEMA_VERSION,
        "workflow": "domain_workflow_catalogue",
        "catalog_digest": catalog_digest,
        "workflow_catalog_digest": workflow_catalog_digest,
        "workflow_count": workflows.len(),
        "workflows": workflows,
        "coverage": {
            "group_count": groups.len(),
            "workflow_count": groups.len(),
            "domain_label_count": domains.len(),
            "groups_with_missing_tools": groups_with_missing_tools,
            "all_groups_have_workflow": true,
            "all_declared_tools_advertised": declared_tools.iter().all(|tool| available.contains(tool)),
            "all_workflows_have_domain_contract": workflows.iter().all(|workflow| workflow["domain_contract"].is_object()),
        },
        "execution": "not_started",
        "guarantees": [
            "one deterministic workflow template exists for every explicit capability group",
            "workflow digests bind templates to the input catalogue and tool names",
            "missing tool definitions remain visible rather than silently becoming executable",
        ],
        "limitations": [
            "stage ordering is advisory and does not infer domain-specific dependencies",
            "tool schemas and external authority remain separate preflight obligations",
            "this catalogue does not dispatch, schedule, or claim domain readiness",
        ],
    });
    checked_bytes(&output)?;
    Ok(output)
}

fn array_field(object: &Map<String, Value>, field: &str) -> Result<Vec<Value>, String> {
    match object.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => Ok(values.clone()),
        Some(_) => Err(format!("{field} must be an array")),
    }
}

fn normalized_step(
    value: &Value,
    index: usize,
    workflow_id: &str,
    group_domains: &[String],
    declared_tools: &BTreeSet<String>,
    available_tools: &BTreeSet<String>,
) -> Result<Value, DomainWorkflowError> {
    let object = value
        .as_object()
        .ok_or_else(|| DomainWorkflowError::InvalidStep {
            step: index,
            reason: "step must be an object".into(),
        })?;
    let id = visible_text(object, "id").map_err(|reason| DomainWorkflowError::InvalidStep {
        step: index,
        reason,
    })?;
    let tool = visible_text(object, "tool").map_err(|reason| DomainWorkflowError::InvalidStep {
        step: index,
        reason,
    })?;
    if !declared_tools.contains(tool) {
        return Err(DomainWorkflowError::ToolOutsideWorkflow {
            step: index,
            tool: tool.to_string(),
            workflow_id: workflow_id.to_string(),
        });
    }
    if !available_tools.contains(tool) {
        return Err(DomainWorkflowError::ToolUnavailable {
            step: index,
            tool: tool.to_string(),
            workflow_id: workflow_id.to_string(),
        });
    }
    let arguments = object
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Err(DomainWorkflowError::InvalidStep {
            step: index,
            reason: "arguments must be an object".into(),
        });
    }
    let depends_on =
        array_field(object, "depends_on").map_err(|reason| DomainWorkflowError::InvalidStep {
            step: index,
            reason,
        })?;
    let bindings =
        array_field(object, "bindings").map_err(|reason| DomainWorkflowError::InvalidStep {
            step: index,
            reason,
        })?;
    let domain = object
        .get("domain")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            group_domains
                .first()
                .map(String::as_str)
                .unwrap_or(workflow_id)
        });
    let capability = object
        .get("capability")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(workflow_id);
    let objective = object
        .get("objective")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("apply the selected {tool} capability for {workflow_id}"));
    let required = object
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Ok(json!({
        "id": id,
        "domain": domain,
        "capability": capability,
        "objective": objective,
        "tool": tool,
        "arguments": arguments,
        "depends_on": depends_on,
        "bindings": bindings,
        "required": required,
    }))
}

pub fn instantiate_domain_workflow(
    catalogue: &Value,
    tool_definitions: &Value,
    request: &Value,
) -> Result<Value, DomainWorkflowError> {
    checked_bytes(request)?;
    let object = request
        .as_object()
        .ok_or(DomainWorkflowError::RequestNotObject)?;
    let workflow_id = visible_text(object, "workflow_id")
        .map_err(DomainWorkflowError::InvalidRequest)?
        .to_string();
    let mission_id = visible_text(object, "mission_id")
        .map_err(DomainWorkflowError::InvalidRequest)?
        .to_string();
    let goal = visible_text(object, "goal")
        .map_err(DomainWorkflowError::InvalidRequest)?
        .to_string();
    let catalogue_report = build_domain_workflow_catalogue(catalogue, tool_definitions)?;
    let workflow = catalogue_report["workflows"]
        .as_array()
        .and_then(|workflows| {
            workflows
                .iter()
                .find(|item| item["workflow_id"] == workflow_id)
        })
        .ok_or_else(|| DomainWorkflowError::UnknownWorkflow {
            workflow_id: workflow_id.clone(),
        })?;
    let group_domains = workflow["domains"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let declared_tools = workflow["tools"]["declared"]
        .as_array()
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("workflow has no declared tools".into())
        })?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let available_tools = workflow["tools"]["available"]
        .as_array()
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("workflow has no available tools".into())
        })?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let raw_steps = object
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainWorkflowError::InvalidRequest("steps must be an array".into()))?;
    if raw_steps.is_empty() || raw_steps.len() > MAX_DOMAIN_WORKFLOW_STEPS {
        return Err(DomainWorkflowError::InvalidStepCount {
            maximum: MAX_DOMAIN_WORKFLOW_STEPS,
        });
    }
    let mut steps = Vec::with_capacity(raw_steps.len());
    let mut step_ids = BTreeSet::new();
    let mut selected_tools = BTreeSet::new();
    for (index, raw_step) in raw_steps.iter().enumerate() {
        let step = normalized_step(
            raw_step,
            index,
            &workflow_id,
            &group_domains,
            &declared_tools,
            &available_tools,
        )?;
        let id = step["id"].as_str().unwrap_or_default().to_string();
        if !step_ids.insert(id.clone()) {
            return Err(DomainWorkflowError::InvalidStep {
                step: index,
                reason: format!("duplicate step id {id:?}"),
            });
        }
        selected_tools.insert(step["tool"].as_str().unwrap_or_default().to_string());
        steps.push(step);
    }
    let mut policy = object.get("policy").cloned().unwrap_or_else(|| {
        json!({
            "execute": false,
            "stop_on_error": true,
            "allow_side_effects": false,
            "max_steps": MAX_DOMAIN_WORKFLOW_STEPS,
            "allowed_tools": [],
        })
    });
    if !policy.is_object() {
        return Err(DomainWorkflowError::InvalidRequest(
            "policy must be an object".into(),
        ));
    }
    let execute = policy["execute"].as_bool().unwrap_or(false);
    if let Some(allowed_tools) = policy["allowed_tools"].as_array() {
        for allowed_tool in allowed_tools.iter().filter_map(Value::as_str) {
            if !declared_tools.contains(allowed_tool) {
                return Err(DomainWorkflowError::PolicyToolOutsideWorkflow {
                    tool: allowed_tool.to_string(),
                    workflow_id: workflow_id.clone(),
                });
            }
            if !available_tools.contains(allowed_tool) {
                return Err(DomainWorkflowError::PolicyToolOutsideWorkflow {
                    tool: allowed_tool.to_string(),
                    workflow_id: workflow_id.clone(),
                });
            }
        }
    }
    let mut allow_list_derived = false;
    if execute && policy["allowed_tools"].as_array().is_none_or(Vec::is_empty) {
        policy["allowed_tools"] = json!(selected_tools.iter().cloned().collect::<Vec<_>>());
        allow_list_derived = true;
    }
    let mission = json!({
        "mission_id": mission_id,
        "goal": goal,
        "steps": steps,
        "policy": policy,
        "claim_requests": object.get("claim_requests").cloned().unwrap_or_else(|| json!([])),
        "evaluator_review": object.get("evaluator_review").cloned().unwrap_or(Value::Null),
    });
    let parsed: MissionRequest = serde_json::from_value(mission.clone()).map_err(|error| {
        DomainWorkflowError::InvalidRequest(format!("mission shape is invalid: {error}"))
    })?;
    parsed.validate().map_err(|error| {
        DomainWorkflowError::InvalidRequest(format!("mission validation failed: {error}"))
    })?;
    let selected_tools = selected_tools.into_iter().collect::<Vec<_>>();
    let evidence_plan = steps
        .iter()
        .map(|step| {
            let tool = step["tool"].as_str().unwrap_or_default();
            let tool_contract = workflow["tool_contracts"]
                .as_array()
                .and_then(|contracts| contracts.iter().find(|contract| contract["name"] == tool))
                .cloned()
                .unwrap_or_else(|| json!({"name": tool, "schema_state": "unknown"}));
            json!({
                "step_id": step["id"],
                "tool": tool,
                "required": step["required"],
                "tool_contract": tool_contract,
                "preconditions": ["scope_review", "tool_availability", "argument_schema_preflight", "execution_policy"],
                "capture": ["arguments_digest", "result_status", "result_digest"],
                "on_refusal": "retain structured refusal and mark unresolved",
                "on_omission": "retain explicit omission reason and mark unresolved",
                "claim_binding": "caller-owned claims must reference explicit step evidence",
            })
        })
        .collect::<Vec<_>>();
    let output = json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_INSTANTIATE_SCHEMA_VERSION,
        "workflow": "domain_workflow_instantiate",
        "workflow_id": workflow_id,
        "workflow_digest": workflow["workflow_digest"],
        "catalog_digest": catalogue_report["catalog_digest"],
        "mission": mission,
        "selection": {
            "step_count": raw_steps.len(),
            "selected_tools": selected_tools,
            "all_selected_tools_declared": true,
            "all_selected_tools_available": true,
            "allow_list_derived_from_selected_steps": allow_list_derived,
        },
        "domain_contract": workflow["domain_contract"],
        "domain_contract_digest": workflow["domain_contract_digest"],
        "evidence_plan": {
            "schema": DOMAIN_WORKFLOW_CONTRACT_SCHEMA_VERSION,
            "steps": evidence_plan,
            "completion": workflow["domain_contract"]["completion_contract"],
        },
        "preflight": {
            "required": true,
            "dispatch": "not_started",
            "tool_schema_validation": "deferred_to_authoritative_tools_list",
        },
        "execution": "not_started",
        "guarantees": [
            "selected tools are scoped to the chosen capability group",
            "selected tools are present in the authoritative tool inventory before instantiation",
            "each selected step receives an explicit evidence and review contract",
            "mission invariants are validated before a caller can dispatch",
            "workflow instantiation never executes a tool",
        ],
        "limitations": [
            "domain-specific arguments still require authoritative MCP schema preflight",
            "a valid plan is not evidence that a tool call will succeed",
            "execution, scheduling, and external side effects remain outside this kernel",
        ],
        "links": {
            "catalogue": "/v1/domain-workflows",
            "preflight": "/v1/missions/preflight",
            "capability_route": "/v1/tools/capability_route",
        },
    });
    checked_bytes(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> (Value, Value) {
        (
            json!([
                {"id":"oncology_workflows","domains":["oncology"],"crates":["bioprism-onco"],"mcp_tools":["onco_boundary_check","onco_outcome_analyze"],"cli_entrypoints":[],"status":"available"},
                {"id":"genomics_workflows","domains":["genomics"],"crates":["bioprism-genomics"],"mcp_tools":["bioql_compile","missing_tool"],"cli_entrypoints":[],"status":"available"}
            ]),
            json!([
                {"name":"onco_boundary_check"},
                {"name":"onco_outcome_analyze"},
                {"name":"bioql_compile"}
            ]),
        )
    }

    #[test]
    fn catalogue_emits_one_workflow_per_group_and_preserves_missing_tools() {
        let (catalogue, tools) = inputs();
        let report = build_domain_workflow_catalogue(&catalogue, &tools).unwrap();
        assert_eq!(report["workflow_count"], 2);
        assert_eq!(report["coverage"]["groups_with_missing_tools"], 1);
        assert_eq!(report["workflows"][0]["workflow_id"], "genomics_workflows");
        assert_eq!(
            report["workflows"][0]["tools"]["missing"][0],
            "missing_tool"
        );
        assert!(report["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|workflow| workflow["workflow_digest"].is_string()));
        assert!(report["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|workflow| workflow["domain_contract"].is_object()));
        assert_eq!(
            report["workflows"][0]["tool_contracts"][1]["schema_state"],
            "unavailable"
        );
        assert_eq!(
            report["coverage"]["all_workflows_have_domain_contract"],
            true
        );
    }

    #[test]
    fn instantiation_is_scoped_and_defaults_to_no_dispatch() {
        let (catalogue, tools) = inputs();
        let request = json!({
            "workflow_id":"oncology_workflows",
            "mission_id":"m-1",
            "goal":"review the oncology boundary",
            "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}],
            "policy":{"execute":true,"max_steps":4}
        });
        let report = instantiate_domain_workflow(&catalogue, &tools, &request).unwrap();
        assert_eq!(report["execution"], "not_started");
        assert_eq!(
            report["selection"]["selected_tools"][0],
            "onco_boundary_check"
        );
        assert_eq!(
            report["mission"]["policy"]["allowed_tools"][0],
            "onco_boundary_check"
        );
        assert_eq!(report["selection"]["all_selected_tools_declared"], true);
        assert_eq!(report["selection"]["all_selected_tools_available"], true);
        assert_eq!(report["evidence_plan"]["steps"][0]["step_id"], "boundary");
        assert_eq!(
            report["evidence_plan"]["steps"][0]["tool_contract"]["schema_state"],
            "missing"
        );
    }

    #[test]
    fn instantiation_refuses_a_tool_from_another_group() {
        let (catalogue, tools) = inputs();
        let request = json!({
            "workflow_id":"oncology_workflows",
            "mission_id":"m-1",
            "goal":"review",
            "steps":[{"id":"compile","tool":"bioql_compile"}]
        });
        assert!(matches!(
            instantiate_domain_workflow(&catalogue, &tools, &request),
            Err(DomainWorkflowError::ToolOutsideWorkflow { .. })
        ));
    }

    #[test]
    fn instantiation_refuses_a_declared_but_unavailable_tool() {
        let (catalogue, tools) = inputs();
        let request = json!({
            "workflow_id":"genomics_workflows",
            "mission_id":"m-2",
            "goal":"compile the genomic query",
            "steps":[{"id":"compile","tool":"missing_tool"}]
        });
        assert!(matches!(
            instantiate_domain_workflow(&catalogue, &tools, &request),
            Err(DomainWorkflowError::ToolUnavailable { .. })
        ));
    }

    #[test]
    fn instantiation_refuses_policy_tools_outside_the_selected_scope() {
        let (catalogue, tools) = inputs();
        let request = json!({
            "workflow_id":"oncology_workflows",
            "mission_id":"m-3",
            "goal":"review",
            "steps":[{"id":"boundary","tool":"onco_boundary_check"}],
            "policy":{"execute":true,"allowed_tools":["bioql_compile"]}
        });
        assert!(matches!(
            instantiate_domain_workflow(&catalogue, &tools, &request),
            Err(DomainWorkflowError::PolicyToolOutsideWorkflow { .. })
        ));
    }
}
