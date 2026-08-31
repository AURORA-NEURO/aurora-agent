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
use crate::summarize_domain_decision_readiness;

pub const DOMAIN_WORKFLOW_SCHEMA_VERSION: &str = "bioprism-devplat-domain-workflow/0.1";
pub const DOMAIN_WORKFLOW_CATALOGUE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-catalogue/0.1";
pub const DOMAIN_WORKFLOW_INSTANTIATE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-instantiate/0.1";
pub const DOMAIN_WORKFLOW_VERIFY_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-verify/0.1";
pub const DOMAIN_WORKFLOW_PORTFOLIO_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-portfolio/0.1";
pub const DOMAIN_WORKFLOW_PORTFOLIO_VERIFY_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-portfolio-verify/0.1";
pub const DOMAIN_WORKFLOW_SCAFFOLD_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-scaffold/0.1";
pub const DOMAIN_WORKFLOW_CONTRACT_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-contract/0.1";
pub const MAX_DOMAIN_WORKFLOW_GROUPS: usize = 128;
pub const MAX_DOMAIN_WORKFLOW_TOOLS: usize = 256;
pub const MAX_DOMAIN_WORKFLOW_STEPS: usize = 128;
pub const MAX_DOMAIN_WORKFLOW_BYTES: usize = 20_000_000;
pub const MAX_DOMAIN_WORKFLOW_PORTFOLIO_ITEMS: usize = 64;
const MAX_DOMAIN_WORKFLOW_TEXT_BYTES: usize = 4_096;

fn readiness_projection(
    object: &Map<String, Value>,
    policy: &Map<String, Value>,
) -> Result<Value, DomainWorkflowError> {
    let required = policy
        .get("require_readiness")
        .map(|value| {
            value.as_bool().ok_or_else(|| {
                DomainWorkflowError::InvalidRequest(
                    "policy.require_readiness must be a boolean".into(),
                )
            })
        })
        .transpose()?
        .unwrap_or(false);
    match object.get("readiness_audit") {
        Some(audit) => summarize_domain_decision_readiness(audit, required).map_err(|error| {
            DomainWorkflowError::InvalidRequest(format!(
                "readiness_audit is not a valid domain decision-readiness audit: {error}"
            ))
        }),
        None => Ok(json!({
            "required": required,
            "provided": false,
            "subject_id": Value::Null,
            "audit_digest": Value::Null,
            "decision_state": Value::Null,
            "policy_satisfied": false,
            "gate_satisfied": !required,
            "readiness_claimed": false,
            "execution": "not_started",
            "reason": "readiness_audit_not_supplied"
        })),
    }
}

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
        || value != value.trim()
        || value.len() > MAX_DOMAIN_WORKFLOW_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!("{field} must be a non-empty control-free string"));
    }
    Ok(value)
}

fn valid_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && value.len() <= MAX_DOMAIN_WORKFLOW_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
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
            .filter(|item| {
                !item.trim().is_empty()
                    && *item == item.trim()
                    && item.len() <= MAX_DOMAIN_WORKFLOW_TEXT_BYTES
            })
            .ok_or_else(|| format!("{field}[{index}] must be a non-empty string"))?;
        if item.chars().any(char::is_control) {
            return Err(format!(
                "{field}[{index}] must not contain control characters"
            ));
        }
        if !seen.insert(item.to_ascii_lowercase()) {
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

/// A conservative execution classification used for planning and authorization review.
///
/// This is deliberately lexical metadata, just like `tool_role`: it does not inspect a provider
/// implementation or prove that a particular domain operation is safe. Unknown names remain
/// `review_required`; the catalogue must never turn missing semantic knowledge into a benign
/// read-only claim.
fn execution_resource_class(tool: &str) -> &'static str {
    let name = tool.to_ascii_lowercase();
    if name.contains("compile") {
        "compile"
    } else if name.contains("ingest") || name.contains("import") || name.contains("acquisition") {
        "ingest"
    } else if name.contains("sandbox") || name.contains("security") {
        "sandbox"
    } else if name.contains("index") || name.contains("catalog") || name.contains("register") {
        "index"
    } else if name.contains("mutate") || name.contains("apply") || name.contains("publish") {
        "mutate"
    } else {
        "evaluate"
    }
}

fn execution_side_effect_posture(tool: &str) -> (&'static str, &'static str) {
    let name = tool.to_ascii_lowercase();
    if [
        "mutate", "apply", "publish", "delivery", "submit", "accept", "release", "delete", "write",
        "upload", "send", "rebind",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        (
            "potential_external_effect",
            "non_idempotent_requires_explicit_authorization",
        )
    } else if matches!(tool_role(tool), "discover" | "validate_or_review") {
        ("no_declared_external_effect", "idempotent_after_preflight")
    } else {
        ("unknown_requires_review", "unknown_requires_review")
    }
}

fn execution_contract(tool: &str, available: bool) -> Value {
    let (side_effects, idempotency) = execution_side_effect_posture(tool);
    let dispatch = if !available {
        "blocked_missing_tool_definition"
    } else if side_effects == "potential_external_effect" {
        "authorization_and_provider_review_required"
    } else if side_effects == "unknown_requires_review" {
        "semantic_side_effect_review_required"
    } else {
        "authoritative_preflight_required"
    };
    json!({
        "resource_class": execution_resource_class(tool),
        "idempotency": idempotency,
        "side_effects": side_effects,
        "dispatch": dispatch,
        "providers": {
            "mcp_in_process": {
                "state": if available { "available" } else { "unavailable" },
                "scope": "bounded_bioprism_mcp_dispatcher",
            },
            "subprocess": {
                "state": "unavailable",
                "reason": "subprocess provider is declared but not implemented",
            },
            "container": {
                "state": "unavailable",
                "reason": "container provider is declared but not implemented",
            },
        },
        "claims": {
            "provider_ready": false,
            "side_effect_safe": side_effects == "no_declared_external_effect",
            "readiness_claimed": false,
        },
    })
}

fn workflow_execution_boundary(tool_contracts: &[Value], all_tools_available: bool) -> Value {
    let mut potential_effect_count = 0usize;
    let mut unknown_count = 0usize;
    let mut unavailable_count = 0usize;
    for contract in tool_contracts {
        match contract
            .pointer("/execution_contract/side_effects")
            .and_then(Value::as_str)
        {
            Some("potential_external_effect") => potential_effect_count += 1,
            Some("unknown_requires_review") => unknown_count += 1,
            _ => {}
        }
        if contract
            .pointer("/execution_contract/providers/mcp_in_process/state")
            .and_then(Value::as_str)
            == Some("unavailable")
        {
            unavailable_count += 1;
        }
    }
    let side_effect_posture = if potential_effect_count > 0 {
        "authorization_required"
    } else if unknown_count > 0 {
        "review_required"
    } else {
        "no_declared_external_effect"
    };
    json!({
        "provider_boundary": {
            "in_process_mcp": if all_tools_available { "available_for_bounded_dispatch" } else { "blocked_by_missing_tools" },
            "subprocess": "unavailable",
            "container": "unavailable",
        },
        "queue_resource_class": "evaluate",
        "side_effect_posture": side_effect_posture,
        "potential_external_effect_tools": potential_effect_count,
        "unknown_semantics_tools": unknown_count,
        "unavailable_provider_tools": unavailable_count,
        "dispatch": "blocked_until_authoritative_preflight_and_explicit_policy",
        "readiness_claimed": false,
    })
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
    let mut normalized_names = BTreeSet::new();
    for definition in definitions {
        let object = definition.as_object().ok_or_else(|| {
            DomainWorkflowError::InvalidToolDefinition("each definition must be an object".into())
        })?;
        let name =
            visible_text(object, "name").map_err(DomainWorkflowError::InvalidToolDefinition)?;
        if !normalized_names.insert(name.to_ascii_lowercase()) {
            return Err(DomainWorkflowError::InvalidToolDefinition(format!(
                "duplicate or case-colliding tool name {name:?}"
            )));
        }
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
                "argument_contract": argument_contract(schema),
                "execution_contract": execution_contract(tool, is_available),
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

/// Project only the argument facts an agent needs to author a call. The complete authoritative
/// schema remains owned by `tools/list`; this bounded summary avoids copying arbitrary schema
/// branches into every workflow row while keeping required/optional fields, common constraints,
/// and composition keywords visible before mission preflight.
fn argument_contract(schema: Option<&Value>) -> Value {
    let Some(schema) = schema.filter(|value| value.is_object()) else {
        return json!({
            "state": "missing",
            "required": [],
            "optional": [],
            "properties": {},
            "composition_keywords": [],
            "additional_properties": "unspecified",
        });
    };
    let Some(object) = schema.as_object() else {
        return json!({
            "state": "invalid",
            "required": [],
            "optional": [],
            "properties": {},
            "composition_keywords": [],
            "additional_properties": "unspecified",
        });
    };
    let required = object
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let required_set = required.iter().cloned().collect::<BTreeSet<_>>();
    let mut properties = BTreeMap::new();
    if let Some(raw_properties) = object.get("properties").and_then(Value::as_object) {
        for (name, property) in raw_properties.iter().take(MAX_DOMAIN_WORKFLOW_TOOLS) {
            let Some(property) = property.as_object() else {
                properties.insert(
                    name.clone(),
                    json!({"state": "invalid", "required": required_set.contains(name)}),
                );
                continue;
            };
            let mut summary = serde_json::Map::new();
            for field in [
                "type",
                "format",
                "minimum",
                "maximum",
                "minItems",
                "maxItems",
                "minLength",
                "maxLength",
                "pattern",
            ] {
                if let Some(value) = property.get(field) {
                    summary.insert(field.to_string(), value.clone());
                }
            }
            if let Some(values) = property.get("enum").and_then(Value::as_array) {
                summary.insert("enum_count".into(), json!(values.len()));
            }
            summary.insert("required".into(), json!(required_set.contains(name)));
            summary.insert(
                "description_present".into(),
                json!(property.contains_key("description")),
            );
            properties.insert(name.clone(), Value::Object(summary));
        }
    }
    let property_names = properties.keys().cloned().collect::<BTreeSet<_>>();
    let optional = property_names
        .difference(&required_set)
        .cloned()
        .collect::<Vec<_>>();
    let composition_keywords = ["oneOf", "anyOf", "allOf", "not", "$ref"]
        .iter()
        .filter(|key| object.contains_key(**key))
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    let additional_properties = match object.get("additionalProperties") {
        None => json!("unspecified"),
        Some(Value::Bool(value)) => json!(value),
        Some(Value::Object(_)) => json!("schema"),
        Some(_) => json!("invalid"),
    };
    json!({
        "state": "present",
        "type": object.get("type").cloned().unwrap_or(Value::Null),
        "required": required,
        "optional": optional,
        "properties": properties,
        "composition_keywords": composition_keywords,
        "additional_properties": additional_properties,
    })
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
    let status = match group.get("status") {
        None => "available",
        Some(_) => {
            visible_text(group, "status").map_err(|reason| DomainWorkflowError::InvalidGroup {
                group: group_index,
                reason,
            })?
        }
    };
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
        "execution_boundary": workflow_execution_boundary(&tool_contracts, all_tools_available),
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
        "status": status,
        "catalog_digest": catalog_digest,
        "tools": {
            "declared": advertised_tools,
            "available": available_tools,
            "missing": missing_tools,
        },
        "tool_contracts": tool_contracts,
        "domain_contract": domain_contract,
        "execution_contract": domain_contract["execution_boundary"],
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
    let mut workflow_ids = BTreeSet::new();
    let mut declared_tools = BTreeSet::new();
    let mut domains = BTreeSet::new();
    for (index, raw_group) in groups.iter().enumerate() {
        let group = raw_group
            .as_object()
            .ok_or_else(|| DomainWorkflowError::InvalidGroup {
                group: index,
                reason: "group must be an object".into(),
            })?;
        let workflow_id =
            visible_text(group, "id").map_err(|reason| DomainWorkflowError::InvalidGroup {
                group: index,
                reason,
            })?;
        if !workflow_ids.insert(workflow_id.to_ascii_lowercase()) {
            return Err(DomainWorkflowError::InvalidGroup {
                group: index,
                reason: format!("duplicate workflow id {workflow_id:?}"),
            });
        }
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

fn optional_visible_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    match object.get(field) {
        None => Ok(None),
        Some(_) => visible_text(object, field).map(str::to_owned).map(Some),
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
    let domain = optional_visible_text(object, "domain")
        .map_err(|reason| DomainWorkflowError::InvalidStep {
            step: index,
            reason,
        })?
        .unwrap_or_else(|| {
            group_domains
                .first()
                .cloned()
                .unwrap_or_else(|| workflow_id.to_string())
        });
    let capability = optional_visible_text(object, "capability")
        .map_err(|reason| DomainWorkflowError::InvalidStep {
            step: index,
            reason,
        })?
        .unwrap_or_else(|| workflow_id.into());
    let objective = optional_visible_text(object, "objective")
        .map_err(|reason| DomainWorkflowError::InvalidStep {
            step: index,
            reason,
        })?
        .unwrap_or_else(|| format!("apply the selected {tool} capability for {workflow_id}"));
    let required = match object.get("required") {
        None => true,
        Some(value) => value
            .as_bool()
            .ok_or_else(|| DomainWorkflowError::InvalidStep {
                step: index,
                reason: "required must be a boolean when supplied".into(),
            })?,
    };
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
    let mut normalized_step_ids = BTreeSet::new();
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
        if !step_ids.insert(id.clone()) || !normalized_step_ids.insert(id.to_ascii_lowercase()) {
            return Err(DomainWorkflowError::InvalidStep {
                step: index,
                reason: format!("duplicate or case-colliding step id {id:?}"),
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
    let default_policy = json!({
        "execute": false,
        "stop_on_error": true,
        "allow_side_effects": false,
        "require_readiness": false,
        "max_steps": MAX_DOMAIN_WORKFLOW_STEPS,
        "allowed_tools": [],
    });
    let policy_object = policy
        .as_object_mut()
        .ok_or_else(|| DomainWorkflowError::InvalidRequest("policy must be an object".into()))?;
    let Some(default_policy) = default_policy.as_object() else {
        return Err(DomainWorkflowError::InvalidRequest(
            "internal default policy is not an object".into(),
        ));
    };
    for (field, default_value) in default_policy {
        policy_object
            .entry(field.clone())
            .or_insert_with(|| default_value.clone());
    }
    let policy_bool = |field: &str| -> Result<bool, DomainWorkflowError> {
        policy
            .get(field)
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    DomainWorkflowError::InvalidRequest(format!("policy.{field} must be a boolean"))
                })
            })
            .transpose()
            .map(|value| value.unwrap_or(false))
    };
    let execute = policy_bool("execute")?;
    let _ = policy_bool("stop_on_error")?;
    let _ = policy_bool("allow_side_effects")?;
    let _ = policy_bool("require_readiness")?;
    let allowed_tools = match policy.get("allowed_tools") {
        Some(value) => Some(value.as_array().ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("policy.allowed_tools must be an array".into())
        })?),
        None => None,
    };
    if let Some(allowed_tools) = allowed_tools {
        for (index, allowed_tool) in allowed_tools.iter().enumerate() {
            let allowed_tool = allowed_tool.as_str().ok_or_else(|| {
                DomainWorkflowError::InvalidRequest(format!(
                    "policy.allowed_tools[{index}] must be a string"
                ))
            })?;
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
    if execute && allowed_tools.is_none_or(Vec::is_empty) {
        policy["allowed_tools"] = json!(selected_tools.iter().cloned().collect::<Vec<_>>());
        allow_list_derived = true;
    }
    let mut mission = json!({
        "mission_id": mission_id,
        "goal": goal,
        "steps": steps,
        "policy": policy,
        "claim_requests": object.get("claim_requests").cloned().unwrap_or_else(|| json!([])),
        "evaluator_review": object.get("evaluator_review").cloned().unwrap_or(Value::Null),
        "route_review": object.get("route_review").cloned().unwrap_or(Value::Null),
    });
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
    let workflow_evidence_plan = json!({
        "schema": DOMAIN_WORKFLOW_CONTRACT_SCHEMA_VERSION,
        "steps": evidence_plan,
        "completion": workflow["domain_contract"]["completion_contract"],
    });
    let evidence_plan_digest = ContentHash::of_value(&workflow_evidence_plan)
        .map_err(|error| {
            DomainWorkflowError::InvalidRequest(format!("evidence plan cannot be hashed: {error}"))
        })?
        .to_string();
    mission["workflow_binding"] = json!({
        "workflow_id": workflow_id,
        "workflow_digest": workflow["workflow_digest"],
        "catalog_digest": catalogue_report["catalog_digest"],
        "domain_contract_digest": workflow["domain_contract_digest"],
        "domain_contract": workflow["domain_contract"],
        "evidence_plan": workflow_evidence_plan,
        "evidence_plan_digest": evidence_plan_digest,
    });
    let parsed: MissionRequest = serde_json::from_value(mission.clone()).map_err(|error| {
        DomainWorkflowError::InvalidRequest(format!("mission shape is invalid: {error}"))
    })?;
    parsed.validate().map_err(|error| {
        DomainWorkflowError::InvalidRequest(format!("mission validation failed: {error}"))
    })?;
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
        "execution_contract": workflow["execution_contract"],
        "evidence_plan": workflow_evidence_plan,
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

/// Plan a bounded portfolio of explicitly authored domain workflows.
///
/// A portfolio is a structural composition boundary for callers that need to prepare more than
/// one capability group at once. Each request is instantiated independently, so one malformed or
/// out-of-scope group remains a retained blocked item instead of erasing the other domain plans.
/// The transport adds authoritative mission preflight to each successful instantiation; this
/// kernel never dispatches, retries, resumes, or infers a domain goal or tool argument.
pub fn build_domain_workflow_portfolio(
    catalogue: &Value,
    tool_definitions: &Value,
    request: &Value,
) -> Result<Value, DomainWorkflowError> {
    checked_bytes(request)?;
    let object = request
        .as_object()
        .ok_or(DomainWorkflowError::RequestNotObject)?;
    let requests = object
        .get("requests")
        .and_then(Value::as_array)
        .ok_or_else(|| DomainWorkflowError::InvalidRequest("requests must be an array".into()))?;
    if requests.is_empty() || requests.len() > MAX_DOMAIN_WORKFLOW_PORTFOLIO_ITEMS {
        return Err(DomainWorkflowError::InvalidRequest(format!(
            "requests must contain between 1 and {} items",
            MAX_DOMAIN_WORKFLOW_PORTFOLIO_ITEMS
        )));
    }
    let policy = object.get("policy").cloned().unwrap_or_else(|| json!({}));
    let policy = policy
        .as_object()
        .ok_or_else(|| DomainWorkflowError::InvalidRequest("policy must be an object".into()))?;
    let allow_partial = policy
        .get("allow_partial")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let require_complete_catalogue = policy
        .get("require_complete_catalogue")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let decision_readiness = readiness_projection(object, policy)?;
    let decision_readiness_gate_satisfied = decision_readiness
        .get("gate_satisfied")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let catalogue_report = build_domain_workflow_catalogue(catalogue, tool_definitions)?;
    let catalogue_workflows = catalogue_report
        .get("workflows")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "workflow catalogue omitted its workflow rows".into(),
            )
        })?;
    let catalogue_ids = catalogue_workflows
        .iter()
        .filter_map(|workflow| workflow.get("workflow_id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    let mut seen_workflow_ids = BTreeSet::new();
    let mut seen_mission_ids = BTreeSet::new();
    let mut items = Vec::with_capacity(requests.len());
    let mut blocked_count = 0usize;
    let mut selected_tool_count = 0usize;

    for (index, raw_request) in requests.iter().enumerate() {
        let request_digest = digest(raw_request)?;
        let workflow_id = raw_request.get("workflow_id").and_then(Value::as_str);
        let mission_id = raw_request.get("mission_id").and_then(Value::as_str);
        let mut item_issues = Vec::new();
        if !raw_request.is_object() {
            item_issues.push(json!({
                "code": "request_not_object",
                "message": "portfolio request must be an object"
            }));
        }
        if let Some(workflow_id) = workflow_id {
            if !seen_workflow_ids.insert(workflow_id.to_owned()) {
                item_issues.push(json!({
                    "code": "duplicate_workflow_id",
                    "workflow_id": workflow_id,
                    "message": "each workflow_id may occur at most once in a portfolio"
                }));
            }
        }
        if let Some(mission_id) = mission_id {
            if !seen_mission_ids.insert(mission_id.to_owned()) {
                item_issues.push(json!({
                    "code": "duplicate_mission_id",
                    "mission_id": mission_id,
                    "message": "each mission_id may occur at most once in a portfolio"
                }));
            }
        }
        if item_issues.is_empty() {
            match instantiate_domain_workflow(catalogue, tool_definitions, raw_request) {
                Ok(instantiation) => {
                    selected_tool_count = selected_tool_count.saturating_add(
                        instantiation
                            .pointer("/selection/selected_tools")
                            .and_then(Value::as_array)
                            .map_or(0, Vec::len),
                    );
                    items.push(json!({
                        "index": index,
                        "workflow_id": workflow_id,
                        "mission_id": mission_id,
                        "request_digest": request_digest,
                        "status": "instantiated",
                        "instantiation": instantiation,
                        "mission_preflight": {
                            "status": "deferred",
                            "matched": false,
                            "dispatch": "not_started"
                        }
                    }));
                    continue;
                }
                Err(error) => item_issues.push(json!({
                    "code": "workflow_instantiation_blocked",
                    "message": error.to_string()
                })),
            }
        }
        blocked_count = blocked_count.saturating_add(1);
        items.push(json!({
            "index": index,
            "workflow_id": workflow_id,
            "mission_id": mission_id,
            "request_digest": request_digest,
            "status": "blocked",
            "issues": item_issues,
            "mission_preflight": {
                "status": "not_requested",
                "matched": false,
                "dispatch": "not_started"
            }
        }));
    }

    let requested_ids = seen_workflow_ids
        .intersection(&catalogue_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_workflow_ids = catalogue_ids
        .difference(&requested_ids)
        .cloned()
        .collect::<Vec<_>>();
    let extra_workflow_ids = seen_workflow_ids
        .difference(&catalogue_ids)
        .cloned()
        .collect::<Vec<_>>();
    let complete_catalogue = missing_workflow_ids.is_empty() && extra_workflow_ids.is_empty();
    let valid = blocked_count == 0
        && (!require_complete_catalogue || complete_catalogue)
        && decision_readiness_gate_satisfied;
    let portfolio_status = if blocked_count > 0 {
        if allow_partial {
            "partial"
        } else {
            "blocked"
        }
    } else if !decision_readiness_gate_satisfied {
        "blocked_by_decision_readiness"
    } else if require_complete_catalogue && !complete_catalogue {
        "incomplete_scope"
    } else {
        "ready_for_authoritative_preflight"
    };
    let mut output = json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_PORTFOLIO_SCHEMA_VERSION,
        "workflow": "domain_workflow_portfolio",
        "valid": valid,
        "portfolio_ready": valid,
        "portfolio_status": portfolio_status,
        "policy": {
            "allow_partial": allow_partial,
            "require_complete_catalogue": require_complete_catalogue,
            "require_readiness": decision_readiness["required"]
        },
        "decision_readiness": decision_readiness,
        "coverage": {
            "catalogue_group_count": catalogue_ids.len(),
            "requested_item_count": requests.len(),
            "unique_workflow_count": seen_workflow_ids.len(),
            "complete_catalogue": complete_catalogue,
            "missing_workflow_ids": missing_workflow_ids,
            "extra_workflow_ids": extra_workflow_ids
        },
        "summary": {
            "instantiated_count": requests.len().saturating_sub(blocked_count),
            "blocked_count": blocked_count,
            "selected_tool_count": selected_tool_count,
            "preflight_status": "deferred"
        },
        "items": items,
        "preflight": {
            "required": true,
            "status": "deferred",
            "dispatch": "not_started"
        },
        "dispatch": "not_started",
        "execution": "not_started",
        "guarantees": [
            "each requested workflow is scoped and instantiated independently",
            "blocked domain requests remain visible with structured issue codes",
            "complete-catalogue coverage is explicit rather than inferred from a partial portfolio",
            "decision-readiness is an optional explicit gate and remains separate from mission preflight and execution",
            "portfolio planning never dispatches, retries, resumes, or grants readiness"
        ],
        "limitations": [
            "authoritative mission schema preflight is added by the transport boundary",
            "partial portfolios are not complete coverage and require caller review",
            "workflow plans do not establish semantic sufficiency, provider availability, authorization, or domain validity",
            "a decision-readiness audit is a structural policy result and does not establish scientific, clinical, operational, regulatory, release, or execution validity"
        ]
    });
    let portfolio_digest = digest(&output)?;
    output["portfolio_digest"] = Value::String(portfolio_digest);
    checked_bytes(&output)?;
    Ok(output)
}

/// Verify a retained domain-workflow instantiation against its internal identities and, when
/// supplied, the original caller request.
///
/// This kernel check is deliberately catalogue-bound but transport-independent. It does not
/// perform authoritative MCP schema preflight (the MCP server adds that step), and it never
/// dispatches, retries, resumes, or treats a digest match as semantic or operational validity.
pub fn verify_domain_workflow(
    catalogue: &Value,
    tool_definitions: &Value,
    request: &Value,
) -> Result<Value, DomainWorkflowError> {
    checked_bytes(request)?;
    let object = request
        .as_object()
        .ok_or(DomainWorkflowError::RequestNotObject)?;
    let instantiation = object
        .get("instantiation")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "domain workflow verification requires an instantiation object".into(),
            )
        })?;
    if instantiation.get("workflow").and_then(Value::as_str) != Some("domain_workflow_instantiate")
    {
        return Err(DomainWorkflowError::InvalidRequest(
            "instantiation.workflow must be domain_workflow_instantiate".into(),
        ));
    }
    if instantiation.get("execution").and_then(Value::as_str) != Some("not_started") {
        return Err(DomainWorkflowError::InvalidRequest(
            "instantiation.execution must be not_started".into(),
        ));
    }
    let required_text = |field: &str| -> Result<String, DomainWorkflowError> {
        let value = instantiation
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| valid_text(value))
            .ok_or_else(|| {
                DomainWorkflowError::InvalidRequest(format!(
                    "instantiation.{field} must be a non-empty string"
                ))
            })?;
        if !valid_digest(value) {
            return Err(DomainWorkflowError::InvalidRequest(format!(
                "instantiation.{field} must be a lowercase 64-character hexadecimal digest"
            )));
        }
        Ok(value.to_owned())
    };
    let workflow_id = instantiation
        .get("workflow_id")
        .and_then(Value::as_str)
        .filter(|value| valid_text(value))
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "instantiation.workflow_id must be a non-empty string".into(),
            )
        })?
        .to_owned();
    let workflow_digest = required_text("workflow_digest")?;
    let catalog_digest = required_text("catalog_digest")?;
    let domain_contract_digest = required_text("domain_contract_digest")?;
    let mission = instantiation
        .get("mission")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("instantiation.mission must be an object".into())
        })?;
    let mission_id = mission
        .get("mission_id")
        .and_then(Value::as_str)
        .filter(|value| valid_text(value))
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "instantiation.mission.mission_id must be a non-empty string".into(),
            )
        })?
        .to_owned();
    let binding = mission
        .get("workflow_binding")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "instantiation.mission.workflow_binding must be an object".into(),
            )
        })?;
    for field in [
        "workflow_id",
        "workflow_digest",
        "catalog_digest",
        "domain_contract_digest",
    ] {
        if instantiation.get(field) != binding.get(field) {
            return Err(DomainWorkflowError::InvalidRequest(format!(
                "instantiation identity field {field:?} does not match mission.workflow_binding"
            )));
        }
    }
    let domain_contract = instantiation.get("domain_contract").ok_or_else(|| {
        DomainWorkflowError::InvalidRequest("instantiation.domain_contract is required".into())
    })?;
    if binding.get("domain_contract") != Some(domain_contract) {
        return Err(DomainWorkflowError::InvalidRequest(
            "instantiation.domain_contract does not match mission.workflow_binding".into(),
        ));
    }
    if digest(domain_contract)? != domain_contract_digest {
        return Err(DomainWorkflowError::InvalidRequest(
            "instantiation.domain_contract_digest does not match domain_contract".into(),
        ));
    }
    let evidence_plan = instantiation.get("evidence_plan").ok_or_else(|| {
        DomainWorkflowError::InvalidRequest("instantiation.evidence_plan is required".into())
    })?;
    if binding.get("evidence_plan") != Some(evidence_plan) {
        return Err(DomainWorkflowError::InvalidRequest(
            "instantiation.evidence_plan does not match mission.workflow_binding".into(),
        ));
    }
    let evidence_plan_digest = binding
        .get("evidence_plan_digest")
        .and_then(Value::as_str)
        .filter(|value| valid_digest(value))
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "mission.workflow_binding.evidence_plan_digest must be canonical".into(),
            )
        })?;
    if digest(evidence_plan)? != evidence_plan_digest {
        return Err(DomainWorkflowError::InvalidRequest(
            "mission.workflow_binding.evidence_plan_digest does not match evidence_plan".into(),
        ));
    }
    let mission_request: MissionRequest = serde_json::from_value(Value::Object(mission.clone()))
        .map_err(|error| {
            DomainWorkflowError::InvalidRequest(format!(
                "retained workflow mission is invalid: {error}"
            ))
        })?;
    mission_request.validate().map_err(|error| {
        DomainWorkflowError::InvalidRequest(format!(
            "retained workflow mission validation failed: {error}"
        ))
    })?;

    let replay_requested = object.contains_key("replay_request");
    let mut replay = json!({
        "requested": replay_requested,
        "status": if replay_requested { "pending" } else { "not_requested" },
        "matched": Value::Null,
        "mismatches": [],
    });
    let mut mismatches = Vec::new();
    if let Some(replay_request) = object.get("replay_request") {
        if !replay_request.is_object() {
            return Err(DomainWorkflowError::InvalidRequest(
                "replay_request must be an object".into(),
            ));
        }
        let replayed =
            match instantiate_domain_workflow(catalogue, tool_definitions, replay_request) {
                Ok(value) => value,
                Err(error) => {
                    let message = error.to_string();
                    mismatches.push(json!({
                        "code": "workflow_replay_blocked",
                        "message": message,
                    }));
                    replay = json!({
                        "requested": true,
                        "status": "blocked",
                        "matched": false,
                        "mismatches": mismatches,
                    });
                    Value::Null
                }
            };
        if replayed.is_object() {
            let mut replay_mismatches = Vec::new();
            for field in [
                "workflow_id",
                "workflow_digest",
                "catalog_digest",
                "domain_contract",
                "domain_contract_digest",
                "execution_contract",
                "evidence_plan",
                "mission",
                "selection",
                "execution",
            ] {
                if instantiation.get(field) != replayed.get(field) {
                    let expected = instantiation.get(field).cloned().unwrap_or(Value::Null);
                    let observed = replayed.get(field).cloned().unwrap_or(Value::Null);
                    let (expected, observed) = if expected.is_object() || expected.is_array() {
                        (
                            json!({ "digest": digest(&expected)? }),
                            json!({ "digest": digest(&observed)? }),
                        )
                    } else {
                        (expected, observed)
                    };
                    replay_mismatches.push(json!({
                        "code": "workflow_replay_field_mismatch",
                        "field": field,
                        "expected": expected,
                        "observed": observed,
                    }));
                }
            }
            let matched = replay_mismatches.is_empty();
            replay = json!({
                "requested": true,
                "status": if matched { "matched" } else { "mismatched" },
                "matched": matched,
                "mismatches": replay_mismatches,
            });
            if !matched {
                mismatches.extend(replay["mismatches"].as_array().cloned().unwrap_or_default());
            }
        }
    }
    let retained_mission_digest = digest(&Value::Object(mission.clone()))?;
    Ok(json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_VERIFY_SCHEMA_VERSION,
        "workflow": "domain_workflow_verify",
        "workflow_id": workflow_id,
        "workflow_digest": workflow_digest,
        "catalog_digest": catalog_digest,
        "domain_contract_digest": domain_contract_digest,
        "mission_id": mission_id,
        "mission_digest": retained_mission_digest,
        "structural_valid": mismatches.is_empty(),
        "replay": replay,
        "mismatches": mismatches,
        "dispatch": "not_started",
        "execution": "not_started",
        "guarantees": [
            "retained workflow identities and mission binding are checked before authoritative preflight",
            "optional replay recomputes the workflow from the caller request against the live catalogue",
            "verification never dispatches, retries, resumes, or grants readiness"
        ],
        "limitations": [
            "authoritative MCP schema preflight is added by the transport boundary",
            "without replay_request, current catalogue membership is not proof that the original request is unchanged",
            "structural identity does not establish semantic sufficiency, provider availability, authorization, or scientific validity"
        ]
    }))
}

/// Verify a retained portfolio as one bounded, independently diagnosable artifact.
///
/// The portfolio digest protects the retained scope and item ordering, while each item is passed
/// through [`verify_domain_workflow`] so identity, mission binding, and optional caller replay are
/// checked independently. A malformed or blocked row is retained in the result rather than
/// aborting the other rows. This kernel intentionally stops before authoritative MCP preflight:
/// the transport may add that check, but neither layer dispatches, retries, resumes, or grants
/// execution readiness.
pub fn verify_domain_workflow_portfolio(
    catalogue: &Value,
    tool_definitions: &Value,
    request: &Value,
) -> Result<Value, DomainWorkflowError> {
    checked_bytes(request)?;
    let object = request
        .as_object()
        .ok_or(DomainWorkflowError::RequestNotObject)?;
    let portfolio = object
        .get("portfolio")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "portfolio verification requires a portfolio object".into(),
            )
        })?;
    if portfolio.get("workflow").and_then(Value::as_str) != Some("domain_workflow_portfolio") {
        return Err(DomainWorkflowError::InvalidRequest(
            "portfolio.workflow must be domain_workflow_portfolio".into(),
        ));
    }
    let expected_portfolio_digest = portfolio
        .get("portfolio_digest")
        .and_then(Value::as_str)
        .filter(|value| valid_digest(value))
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest(
                "portfolio.portfolio_digest must be a lowercase 64-character hexadecimal digest"
                    .into(),
            )
        })?
        .to_owned();
    let mut portfolio_without_digest = Value::Object(portfolio.clone());
    {
        let Some(portfolio_object) = portfolio_without_digest.as_object_mut() else {
            return Err(DomainWorkflowError::InvalidRequest(
                "portfolio digest projection is not an object".into(),
            ));
        };
        portfolio_object.remove("portfolio_digest");
        // REST and JSON-RPC adapters may append envelope metadata after the portfolio digest was
        // computed. These fields are transport provenance, not retained portfolio content.
        portfolio_object.remove("request_id");
        portfolio_object.remove("__isError");
    }
    let observed_portfolio_digest = digest(&portfolio_without_digest)?;
    let portfolio_digest_matched = observed_portfolio_digest == expected_portfolio_digest;

    let items = portfolio
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("portfolio.items must be an array".into())
        })?;
    if items.is_empty() || items.len() > MAX_DOMAIN_WORKFLOW_PORTFOLIO_ITEMS {
        return Err(DomainWorkflowError::InvalidRequest(format!(
            "portfolio.items must contain between 1 and {} items",
            MAX_DOMAIN_WORKFLOW_PORTFOLIO_ITEMS
        )));
    }
    let mut retained_contract_mismatches = Vec::new();
    let mut retained_instantiated_count = 0usize;
    let mut retained_blocked_count = 0usize;
    let mut retained_selected_tool_count = 0usize;
    let mut seen_instantiated_workflow_ids = BTreeSet::new();
    let mut seen_instantiated_mission_ids = BTreeSet::new();
    let coverage = portfolio
        .get("coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("portfolio.coverage must be an object".into())
        })?;
    let portfolio_policy = portfolio
        .get("policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let policy = object.get("policy").cloned().unwrap_or(portfolio_policy);
    let policy = policy
        .as_object()
        .ok_or_else(|| DomainWorkflowError::InvalidRequest("policy must be an object".into()))?;
    let policy_bool = |field: &str| -> Result<bool, DomainWorkflowError> {
        policy
            .get(field)
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    DomainWorkflowError::InvalidRequest(format!("policy.{field} must be a boolean"))
                })
            })
            .transpose()
            .map(|value| value.unwrap_or(false))
    };
    let allow_partial = policy_bool("allow_partial")?;
    let require_complete_catalogue = policy_bool("require_complete_catalogue")?;
    let require_replay = policy_bool("require_replay")?;
    let require_readiness = policy_bool("require_readiness")?;
    let retained_readiness = portfolio
        .get("decision_readiness")
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "required": false,
                "provided": false,
                "subject_id": Value::Null,
                "audit_digest": Value::Null,
                "decision_state": Value::Null,
                "policy_satisfied": false,
                "gate_satisfied": true,
                "readiness_claimed": false,
                "execution": "not_started",
                "reason": "legacy_portfolio_without_readiness_binding"
            })
        });
    let retained_readiness_gate_satisfied = retained_readiness
        .get("gate_satisfied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let readiness_binding_mismatch = if let Some(audit) = object.get("readiness_audit") {
        let expected =
            summarize_domain_decision_readiness(audit, require_readiness).map_err(|error| {
                DomainWorkflowError::InvalidRequest(format!(
                    "readiness_audit is not a valid domain decision-readiness audit: {error}"
                ))
            })?;
        expected.get("audit_digest") != retained_readiness.get("audit_digest")
            || expected.get("decision_state") != retained_readiness.get("decision_state")
            || expected.get("policy_satisfied") != retained_readiness.get("policy_satisfied")
    } else {
        false
    };

    let replay_requests = match object.get("replay_requests") {
        None => None,
        Some(value) => {
            let requests = value.as_array().ok_or_else(|| {
                DomainWorkflowError::InvalidRequest("replay_requests must be an array".into())
            })?;
            if requests.len() != items.len() {
                return Err(DomainWorkflowError::InvalidRequest(format!(
                    "replay_requests must contain exactly {} items",
                    items.len()
                )));
            }
            for (index, replay_request) in requests.iter().enumerate() {
                if !replay_request.is_null() && !replay_request.is_object() {
                    return Err(DomainWorkflowError::InvalidRequest(format!(
                        "replay_requests[{index}] must be an object or null"
                    )));
                }
            }
            Some(requests.clone())
        }
    };

    let mut verified_count = 0usize;
    let mut verified_without_replay_count = 0usize;
    let mut mismatch_count = 0usize;
    let mut blocked_count = 0usize;
    let mut replay_requested_count = 0usize;
    let mut replay_matched_count = 0usize;
    let mut output_items = Vec::with_capacity(items.len());

    for (index, item) in items.iter().enumerate() {
        let item_object = item.as_object();
        let workflow_id = item_object
            .and_then(|item| item.get("workflow_id"))
            .cloned()
            .unwrap_or(Value::Null);
        let mission_id = item_object
            .and_then(|item| item.get("mission_id"))
            .cloned()
            .unwrap_or(Value::Null);
        let request_digest = item_object
            .and_then(|item| item.get("request_digest"))
            .cloned()
            .unwrap_or(Value::Null);
        let instantiation = item_object.and_then(|item| item.get("instantiation"));
        let mut mismatches = item_object
            .and_then(|item| item.get("issues"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(item_object) = item_object {
            if item_object.get("index").and_then(Value::as_u64) != Some(index as u64) {
                mismatches.push(json!({
                    "code": "portfolio_item_index_mismatch",
                    "expected": index,
                    "observed": item_object.get("index").cloned().unwrap_or(Value::Null)
                }));
            }
            if !item_object
                .get("request_digest")
                .and_then(Value::as_str)
                .is_some_and(valid_digest)
            {
                mismatches.push(json!({
                    "code": "portfolio_item_request_digest_invalid",
                    "message": "each retained portfolio item must carry a canonical request digest"
                }));
            }
            for (field, value) in [("workflow_id", &workflow_id), ("mission_id", &mission_id)] {
                if !value.is_null() && value.as_str().is_none() {
                    mismatches.push(json!({
                        "code": "portfolio_item_identity_invalid",
                        "field": field,
                        "message": "retained item identity fields must be strings or null"
                    }));
                }
            }
            match item_object.get("status").and_then(Value::as_str) {
                Some("instantiated") => {
                    retained_instantiated_count = retained_instantiated_count.saturating_add(1);
                    if let Some(workflow_id) = workflow_id.as_str() {
                        if !seen_instantiated_workflow_ids.insert(workflow_id.to_owned()) {
                            mismatches.push(json!({
                                "code": "portfolio_instantiated_workflow_id_duplicate",
                                "workflow_id": workflow_id
                            }));
                        }
                    } else {
                        mismatches.push(json!({
                            "code": "portfolio_instantiated_workflow_id_missing",
                            "message": "instantiated rows must identify a workflow"
                        }));
                    }
                    if let Some(mission_id) = mission_id.as_str() {
                        if !seen_instantiated_mission_ids.insert(mission_id.to_owned()) {
                            mismatches.push(json!({
                                "code": "portfolio_instantiated_mission_id_duplicate",
                                "mission_id": mission_id
                            }));
                        }
                    } else {
                        mismatches.push(json!({
                            "code": "portfolio_instantiated_mission_id_missing",
                            "message": "instantiated rows must identify a mission"
                        }));
                    }
                    if !item_object
                        .get("instantiation")
                        .is_some_and(Value::is_object)
                    {
                        mismatches.push(json!({
                            "code": "portfolio_instantiated_row_missing_instantiation",
                            "message": "instantiated rows must retain an instantiation object"
                        }));
                    }
                    if let Some(instantiation) = item_object
                        .get("instantiation")
                        .filter(|value| value.is_object())
                    {
                        let selected_tools = instantiation
                            .pointer("/selection/selected_tools")
                            .and_then(Value::as_array);
                        if let Some(selected_tools) = selected_tools {
                            retained_selected_tool_count =
                                retained_selected_tool_count.saturating_add(selected_tools.len());
                        } else {
                            mismatches.push(json!({
                                "code": "portfolio_instantiated_selection_invalid",
                                "message": "instantiated rows must retain selection.selected_tools as an array"
                            }));
                        }
                    }
                }
                Some("blocked") => {
                    retained_blocked_count = retained_blocked_count.saturating_add(1);
                    if item_object
                        .get("instantiation")
                        .is_some_and(|value| !value.is_null())
                    {
                        mismatches.push(json!({
                            "code": "portfolio_blocked_row_has_instantiation",
                            "message": "blocked rows must not retain an instantiation object"
                        }));
                    }
                    if item_object
                        .get("issues")
                        .and_then(Value::as_array)
                        .is_none_or(Vec::is_empty)
                    {
                        mismatches.push(json!({
                            "code": "portfolio_blocked_row_missing_issues",
                            "message": "blocked rows must retain at least one issue"
                        }));
                    }
                }
                Some(status) => mismatches.push(json!({
                    "code": "portfolio_item_status_invalid",
                    "observed": status,
                    "message": "retained portfolio item status must be instantiated or blocked"
                })),
                None => mismatches.push(json!({
                    "code": "portfolio_item_status_missing",
                    "message": "retained portfolio items must carry a status"
                })),
            }
        } else {
            mismatches.push(json!({
                "code": "portfolio_item_not_object",
                "message": "retained portfolio items must be objects"
            }));
        }
        let verification;
        let mut replay_requested = false;
        let mut status = "blocked";

        if let Some(instantiation) = instantiation.filter(|value| value.is_object()) {
            let replay_request = replay_requests
                .as_ref()
                .and_then(|requests| requests.get(index))
                .filter(|request| !request.is_null());
            if let Some(replay_request) = replay_request {
                replay_requested = true;
                replay_requested_count = replay_requested_count.saturating_add(1);
                if let Some(expected_digest) = request_digest.as_str() {
                    let observed_digest = digest(replay_request)?;
                    if observed_digest != expected_digest {
                        mismatches.push(json!({
                            "code": "replay_request_digest_mismatch",
                            "expected": expected_digest,
                            "observed": observed_digest,
                            "message": "aligned replay request does not match the retained portfolio request digest"
                        }));
                    }
                }
            }
            let mut verification_request = json!({"instantiation": instantiation});
            if let Some(replay_request) = replay_request {
                verification_request["replay_request"] = replay_request.clone();
            }
            match verify_domain_workflow(catalogue, tool_definitions, &verification_request) {
                Ok(report) => {
                    let replay_matched = report
                        .pointer("/replay/matched")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if replay_matched {
                        replay_matched_count = replay_matched_count.saturating_add(1);
                    }
                    mismatches.extend(
                        report
                            .get("mismatches")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default(),
                    );
                    let observed_workflow_id =
                        report.get("workflow_id").cloned().unwrap_or(Value::Null);
                    let observed_mission_id =
                        report.get("mission_id").cloned().unwrap_or(Value::Null);
                    if workflow_id != observed_workflow_id {
                        mismatches.push(json!({
                            "code": "portfolio_workflow_id_mismatch",
                            "expected": workflow_id,
                            "observed": observed_workflow_id
                        }));
                    }
                    if mission_id != observed_mission_id {
                        mismatches.push(json!({
                            "code": "portfolio_mission_id_mismatch",
                            "expected": mission_id,
                            "observed": observed_mission_id
                        }));
                    }
                    let replay_status = report
                        .pointer("/replay/status")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    verification = report;
                    if mismatches.is_empty() {
                        if replay_requested {
                            if replay_matched {
                                status = "verified";
                                verified_count = verified_count.saturating_add(1);
                            } else {
                                status = "mismatch";
                                mismatch_count = mismatch_count.saturating_add(1);
                            }
                        } else {
                            status = "verified_without_replay";
                            verified_without_replay_count =
                                verified_without_replay_count.saturating_add(1);
                        }
                    } else if replay_requested && replay_status.as_deref() == Some("blocked") {
                        status = "blocked_by_replay";
                        blocked_count = blocked_count.saturating_add(1);
                    } else {
                        status = "mismatch";
                        mismatch_count = mismatch_count.saturating_add(1);
                    }
                }
                Err(error) => {
                    mismatches.push(json!({
                        "code": "retained_instantiation_blocked",
                        "message": error.to_string()
                    }));
                    verification = json!({
                        "ok": false,
                        "workflow": "domain_workflow_verify",
                        "structural_valid": false,
                        "replay": {
                            "requested": replay_requested,
                            "status": if replay_requested { "blocked" } else { "not_requested" },
                            "matched": false
                        },
                        "mismatches": mismatches.clone(),
                        "dispatch": "not_started",
                        "execution": "not_started"
                    });
                    status = if replay_requested {
                        "blocked_by_replay"
                    } else {
                        "blocked"
                    };
                    blocked_count = blocked_count.saturating_add(1);
                }
            }
        } else {
            mismatches.push(json!({
                "code": "portfolio_item_has_no_instantiation",
                "message": "blocked portfolio rows cannot be structurally verified"
            }));
            verification = json!({
                "ok": false,
                "workflow": "domain_workflow_verify",
                "structural_valid": false,
                "replay": {"requested": false, "status": "not_requested", "matched": false},
                "mismatches": mismatches.clone(),
                "dispatch": "not_started",
                "execution": "not_started"
            });
            blocked_count = blocked_count.saturating_add(1);
        }

        output_items.push(json!({
            "index": index,
            "workflow_id": workflow_id,
            "mission_id": mission_id,
            "request_digest": request_digest,
            "status": status,
            "instantiation": instantiation.cloned().unwrap_or(Value::Null),
            "verification": verification,
            "mismatches": mismatches,
            "mission_preflight": {
                "status": "deferred",
                "matched": false,
                "dispatch": "not_started"
            }
        }));
    }

    let mut check_retained_count = |field: &str, expected: usize| {
        let observed = portfolio
            .get("summary")
            .and_then(Value::as_object)
            .and_then(|summary| summary.get(field))
            .and_then(Value::as_u64);
        if observed != Some(expected as u64) {
            retained_contract_mismatches.push(json!({
                "code": "portfolio_summary_count_mismatch",
                "field": field,
                "expected": expected,
                "observed": observed.map(Value::from).unwrap_or(Value::Null)
            }));
        }
    };
    if portfolio
        .get("summary")
        .and_then(Value::as_object)
        .is_none()
    {
        retained_contract_mismatches.push(json!({
            "code": "portfolio_summary_missing",
            "message": "retained portfolios must carry a summary object"
        }));
    } else {
        check_retained_count("instantiated_count", retained_instantiated_count);
        check_retained_count("blocked_count", retained_blocked_count);
        check_retained_count("selected_tool_count", retained_selected_tool_count);
        let summary_preflight_status = portfolio
            .get("summary")
            .and_then(Value::as_object)
            .and_then(|summary| summary.get("preflight_status"))
            .and_then(Value::as_str);
        let retained_preflight_status = portfolio
            .get("preflight")
            .and_then(Value::as_object)
            .and_then(|preflight| preflight.get("status"))
            .and_then(Value::as_str);
        if !matches!(
            summary_preflight_status,
            Some("deferred" | "matched" | "blocked")
        ) || summary_preflight_status != retained_preflight_status
        {
            retained_contract_mismatches.push(json!({
                "code": "portfolio_summary_preflight_status_invalid",
                "message": "retained portfolio summary and preflight statuses must agree and be deferred, matched, or blocked"
            }));
        }
    }
    let expected_unique_workflow_count = items
        .iter()
        .filter_map(|item| item.get("workflow_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>()
        .len();
    let observed_requested_item_count =
        coverage.get("requested_item_count").and_then(Value::as_u64);
    if observed_requested_item_count != Some(items.len() as u64) {
        retained_contract_mismatches.push(json!({
            "code": "portfolio_coverage_item_count_mismatch",
            "expected": items.len(),
            "observed": observed_requested_item_count
                .map(Value::from)
                .unwrap_or(Value::Null)
        }));
    }
    let observed_unique_workflow_count = coverage
        .get("unique_workflow_count")
        .and_then(Value::as_u64);
    if observed_unique_workflow_count != Some(expected_unique_workflow_count as u64) {
        retained_contract_mismatches.push(json!({
            "code": "portfolio_coverage_unique_workflow_count_mismatch",
            "expected": expected_unique_workflow_count,
            "observed": observed_unique_workflow_count
                .map(Value::from)
                .unwrap_or(Value::Null)
        }));
    }
    let check_sorted_id_list = |field: &str, mismatches: &mut Vec<Value>| {
        let Some(values) = coverage.get(field).and_then(Value::as_array) else {
            mismatches.push(json!({
                "code": "portfolio_coverage_id_list_invalid",
                "field": field,
                "message": "coverage workflow ID lists must be arrays"
            }));
            return;
        };
        let mut previous = None;
        for value in values {
            let Some(value) = value.as_str() else {
                mismatches.push(json!({
                    "code": "portfolio_coverage_id_list_invalid",
                    "field": field,
                    "message": "coverage workflow ID lists must contain strings"
                }));
                continue;
            };
            if previous.is_some_and(|previous: &str| value <= previous) {
                mismatches.push(json!({
                    "code": "portfolio_coverage_id_list_noncanonical",
                    "field": field,
                    "message": "coverage workflow ID lists must be strictly increasing"
                }));
                break;
            }
            previous = Some(value);
        }
    };
    check_sorted_id_list("missing_workflow_ids", &mut retained_contract_mismatches);
    check_sorted_id_list("extra_workflow_ids", &mut retained_contract_mismatches);
    let missing_workflow_ids_empty = coverage
        .get("missing_workflow_ids")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty);
    let extra_workflow_ids_empty = coverage
        .get("extra_workflow_ids")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty);
    if coverage.get("complete_catalogue").and_then(Value::as_bool)
        != Some(missing_workflow_ids_empty && extra_workflow_ids_empty)
    {
        retained_contract_mismatches.push(json!({
            "code": "portfolio_coverage_completeness_mismatch",
            "message": "coverage.complete_catalogue must match its missing and extra workflow ID lists"
        }));
    }
    let retained_contract_valid = retained_contract_mismatches.is_empty();

    let complete_catalogue = coverage
        .get("complete_catalogue")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let replay_complete = replay_requests.is_some()
        && replay_requested_count == items.len()
        && replay_matched_count == items.len();
    let item_failures = blocked_count.saturating_add(mismatch_count);
    let valid = portfolio_digest_matched
        && retained_contract_valid
        && item_failures == 0
        && (!require_complete_catalogue || complete_catalogue)
        && (!require_replay || replay_complete)
        && (!require_readiness || retained_readiness_gate_satisfied)
        && !readiness_binding_mismatch;
    let verification_status = if valid {
        if replay_requested_count > 0 {
            "verified"
        } else {
            "verified_without_replay"
        }
    } else if !portfolio_digest_matched || !retained_contract_valid {
        "mismatch"
    } else if require_replay && !replay_complete {
        "replay_incomplete"
    } else if require_complete_catalogue && !complete_catalogue {
        "incomplete_scope"
    } else if require_readiness
        && (!retained_readiness_gate_satisfied || readiness_binding_mismatch)
    {
        "blocked_by_decision_readiness"
    } else if blocked_count > 0 {
        if allow_partial {
            "partial"
        } else {
            "blocked"
        }
    } else {
        "mismatch"
    };
    let mut mismatches = Vec::new();
    if !portfolio_digest_matched {
        mismatches.push(json!({
            "code": "portfolio_digest_mismatch",
            "expected": expected_portfolio_digest,
            "observed": observed_portfolio_digest
        }));
    }
    mismatches.extend(retained_contract_mismatches);
    if require_replay && replay_requests.is_none() {
        mismatches.push(json!({
            "code": "required_replay_requests_missing",
            "message": "policy.require_replay requires an aligned replay_requests array"
        }));
    } else if require_replay && !replay_complete {
        mismatches.push(json!({
            "code": "required_replay_incomplete",
            "requested_count": replay_requested_count,
            "matched_count": replay_matched_count,
            "item_count": items.len()
        }));
    }
    if require_complete_catalogue && !complete_catalogue {
        mismatches.push(json!({
            "code": "portfolio_catalogue_incomplete",
            "message": "policy.require_complete_catalogue requires complete catalogue coverage"
        }));
    }
    if item_failures > 0 {
        mismatches.push(json!({
            "code": "portfolio_items_not_verified",
            "blocked_count": blocked_count,
            "mismatch_count": mismatch_count
        }));
    }
    if require_readiness && !retained_readiness_gate_satisfied {
        mismatches.push(json!({
            "code": "decision_readiness_gate_not_satisfied",
            "message": "policy.require_readiness requires a ready_for_human_review audit bound to the retained portfolio"
        }));
    }
    if readiness_binding_mismatch {
        mismatches.push(json!({
            "code": "decision_readiness_binding_mismatch",
            "message": "supplied readiness_audit does not match the retained portfolio readiness projection"
        }));
    }
    let mut output = json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_PORTFOLIO_VERIFY_SCHEMA_VERSION,
        "workflow": "domain_workflow_portfolio_verify",
        "valid": valid,
        "portfolio_ready": valid,
        "verification_status": verification_status,
        "policy": {
            "allow_partial": allow_partial,
            "require_complete_catalogue": require_complete_catalogue,
            "require_replay": require_replay,
            "require_readiness": require_readiness
        },
        "decision_readiness": retained_readiness,
        "portfolio_digest": expected_portfolio_digest,
        "observed_portfolio_digest": observed_portfolio_digest,
        "portfolio_digest_matched": portfolio_digest_matched,
        "coverage": {
            "catalogue_group_count": coverage.get("catalogue_group_count").cloned().unwrap_or(Value::Null),
            "requested_item_count": coverage.get("requested_item_count").cloned().unwrap_or(Value::Null),
            "verified_item_count": items.len().saturating_sub(item_failures),
            "complete_catalogue": complete_catalogue,
            "missing_workflow_ids": coverage.get("missing_workflow_ids").cloned().unwrap_or_else(|| json!([])),
            "extra_workflow_ids": coverage.get("extra_workflow_ids").cloned().unwrap_or_else(|| json!([])),
            "replay_complete": replay_complete
        },
        "summary": {
            "verified_count": verified_count,
            "verified_without_replay_count": verified_without_replay_count,
            "mismatch_count": mismatch_count,
            "blocked_count": blocked_count,
            "replay_requested_count": replay_requested_count,
            "replay_matched_count": replay_matched_count,
            "preflight_status": "deferred"
        },
        "items": output_items,
        "mismatches": mismatches,
        "preflight": {
            "required": true,
            "status": "deferred",
            "dispatch": "not_started"
        },
        "dispatch": "not_started",
        "execution": "not_started",
        "guarantees": [
            "the retained portfolio digest and item ordering are checked before readiness is reported",
            "each retained instantiation is verified independently and blocked rows remain visible",
            "aligned replay requests are content-addressed against retained request digests",
            "portfolio verification never dispatches, retries, resumes, or grants execution"
        ],
        "limitations": [
            "authoritative MCP mission schema preflight is added by the transport boundary",
            "without replay_requests, current catalogue membership is not proof that original requests are unchanged",
            "structural and replay validity do not establish semantic sufficiency, provider availability, authorization, or scientific validity"
        ]
    });
    output["portfolio_verify_digest"] = Value::String(digest(&output)?);
    checked_bytes(&output)?;
    Ok(output)
}

/// Build a deterministic, execution-disabled starting mission for one capability-group workflow.
///
/// The scaffold selects the first available tool from each lexical stage unless the caller gives
/// an explicit `tools` list. It never claims those tools are semantically sufficient: each step's
/// argument object remains caller-owned, and the authoritative MCP preflight must still be run by
/// the transport adapter. The returned `instantiation` is intentionally preserved verbatim so it
/// can be passed to `domain_workflow_reconcile` after a caller executes the planned mission.
pub fn scaffold_domain_workflow(
    catalogue: &Value,
    tool_definitions: &Value,
    request: &Value,
) -> Result<Value, DomainWorkflowError> {
    checked_bytes(request)?;
    let object = request
        .as_object()
        .ok_or(DomainWorkflowError::RequestNotObject)?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "workflow_id" | "mission_id" | "goal" | "tools" | "arguments"
        ) {
            return Err(DomainWorkflowError::InvalidRequest(format!(
                "scaffold does not accept the {key:?} field"
            )));
        }
    }
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
    let available = workflow["tools"]["available"]
        .as_array()
        .ok_or_else(|| {
            DomainWorkflowError::InvalidRequest("workflow has no available tool list".into())
        })?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if available.is_empty() {
        return Err(DomainWorkflowError::InvalidRequest(format!(
            "workflow {workflow_id:?} has no available tools to scaffold"
        )));
    }
    let scaffoldable = available
        .iter()
        .filter(|tool| !matches!(tool.as_str(), "agent_mission" | "domain_workflow_scaffold"))
        .cloned()
        .collect::<BTreeSet<_>>();
    if scaffoldable.is_empty() {
        return Err(DomainWorkflowError::InvalidRequest(format!(
            "workflow {workflow_id:?} has no non-recursive tools to scaffold"
        )));
    }

    let explicit_tools = match object.get("tools") {
        None => None,
        Some(_) => Some(
            string_array(object, "tools", true, MAX_DOMAIN_WORKFLOW_STEPS)
                .map_err(DomainWorkflowError::InvalidRequest)?,
        ),
    };
    let (selection_strategy, selected_tools) = if let Some(tools) = explicit_tools {
        if tools.is_empty() {
            return Err(DomainWorkflowError::InvalidRequest(
                "tools must contain at least one selected tool".into(),
            ));
        }
        if let Some(tool) = tools
            .iter()
            .find(|tool| matches!(tool.as_str(), "agent_mission" | "domain_workflow_scaffold"))
        {
            return Err(DomainWorkflowError::InvalidRequest(format!(
                "scaffold cannot select recursive tool `{tool}`"
            )));
        }
        ("explicit_tools", tools)
    } else {
        let mut selected = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(stages) = workflow["recommended_stages"].as_array() {
            for stage in stages {
                let Some(tool) = stage["tools"].as_array().and_then(|tools| {
                    tools
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|tool| scaffoldable.contains(*tool))
                }) else {
                    continue;
                };
                if seen.insert(tool.to_string()) {
                    selected.push(tool.to_string());
                }
            }
        }
        if selected.is_empty() {
            selected.push(scaffoldable.iter().next().cloned().ok_or_else(|| {
                DomainWorkflowError::InvalidRequest("workflow has no scaffoldable tool".into())
            })?);
        }
        ("one_available_tool_per_stage", selected)
    };
    for (index, tool) in selected_tools.iter().enumerate() {
        if !available.contains(tool) {
            return Err(DomainWorkflowError::ToolUnavailable {
                step: index,
                tool: tool.clone(),
                workflow_id: workflow_id.clone(),
            });
        }
    }

    let arguments = match object.get("arguments") {
        None => BTreeMap::new(),
        Some(Value::Object(values)) => values
            .iter()
            .map(|(tool, value)| {
                if !selected_tools.iter().any(|selected| selected == tool) {
                    return Err(DomainWorkflowError::InvalidRequest(format!(
                        "arguments contains unselected tool {tool:?}"
                    )));
                }
                if !value.is_object() {
                    return Err(DomainWorkflowError::InvalidRequest(format!(
                        "arguments[{tool:?}] must be an object"
                    )));
                }
                Ok((tool.clone(), value.clone()))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?,
        Some(_) => {
            return Err(DomainWorkflowError::InvalidRequest(
                "arguments must be an object keyed by selected tool name".into(),
            ))
        }
    };
    let domain = workflow["domains"]
        .as_array()
        .and_then(|domains| domains.first())
        .and_then(Value::as_str)
        .unwrap_or(&workflow_id)
        .to_string();
    let steps = selected_tools
        .iter()
        .enumerate()
        .map(|(index, tool)| {
            let role = tool_role(tool);
            json!({
                "id": format!("scaffold-{}-{}", index + 1, role),
                "domain": domain,
                "capability": workflow_id,
                "objective": format!("review the {role} stage using {tool}"),
                "tool": tool,
                "arguments": arguments.get(tool).cloned().unwrap_or_else(|| json!({})),
                "required": true,
            })
        })
        .collect::<Vec<_>>();
    let instantiation_request = json!({
        "workflow_id": workflow_id,
        "mission_id": mission_id,
        "goal": goal,
        "steps": steps,
        "policy": {
            "execute": false,
            "stop_on_error": true,
            "allow_side_effects": false,
            "allowed_tools": []
        }
    });
    let instantiation =
        instantiate_domain_workflow(catalogue, tool_definitions, &instantiation_request)?;
    let omitted_tools = available
        .difference(&selected_tools.iter().cloned().collect::<BTreeSet<_>>())
        .cloned()
        .collect::<Vec<_>>();
    let mut output = json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_SCAFFOLD_SCHEMA_VERSION,
        "workflow": "domain_workflow_scaffold",
        "workflow_id": instantiation["workflow_id"],
        "workflow_digest": instantiation["workflow_digest"],
        "catalog_digest": instantiation["catalog_digest"],
        "selection": {
            "strategy": selection_strategy,
            "selected_tools": selected_tools,
            "available_tool_count": available.len(),
            "omitted_available_tools": omitted_tools,
            "caller_arguments_supplied": arguments.len(),
        },
        "instantiation": instantiation,
        "mission": instantiation["mission"],
        "domain_contract": instantiation["domain_contract"],
        "domain_contract_digest": instantiation["domain_contract_digest"],
        "execution_contract": instantiation["execution_contract"],
        "evidence_plan": instantiation["evidence_plan"],
        "execution": "not_started",
        "readiness_claimed": false,
        "preflight": {
            "required": true,
            "dispatch": "not_started",
            "argument_schema_validation": "deferred_to_authoritative_tools_list",
        },
        "guarantees": [
            "the scaffold selects only available tools from one capability-group workflow",
            "the generated mission is execution-disabled and carries an explicit evidence plan",
            "caller-supplied arguments remain visible and unmodified",
            "omitted available tools remain visible instead of being silently treated as unnecessary",
        ],
        "limitations": [
            "lexical stage selection is a starting point, not semantic tool sufficiency",
            "authoritative schema preflight remains required before dispatch",
            "a scaffold is not permission, scientific evidence, readiness, or a domain conclusion",
        ],
        "next_actions": [
            "fill every required argument field from the authoritative tool schema",
            "review the selected and omitted tools for domain-specific sufficiency",
            "run mission preflight and obtain any required operations gate acceptance",
            "execute only through an explicit allow-list and reconcile the retained report",
        ],
    });
    output["scaffold_digest"] = Value::String(digest(&output)?);
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
        assert!(report["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|workflow| workflow["execution_contract"].is_object()));
        assert!(report["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|workflow| workflow["tool_contracts"].as_array().unwrap())
            .all(|contract| {
                contract["execution_contract"]["providers"]["subprocess"]["state"] == "unavailable"
                    && contract["execution_contract"]["claims"]["readiness_claimed"] == false
            }));
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
    fn catalogue_rejects_duplicate_workflow_ids_before_instantiation_can_be_ambiguous() {
        let (mut catalogue, tools) = inputs();
        catalogue[1]["id"] = json!("oncology_workflows");

        let error = build_domain_workflow_catalogue(&catalogue, &tools)
            .expect_err("duplicate workflow IDs must not create first-match ambiguity");
        assert_eq!(
            error,
            DomainWorkflowError::InvalidGroup {
                group: 1,
                reason: "duplicate workflow id \"oncology_workflows\"".into(),
            }
        );
    }

    #[test]
    fn catalogue_rejects_case_colliding_tool_definition_names() {
        let (catalogue, mut tools) = inputs();
        tools
            .as_array_mut()
            .expect("tool definitions are an array")
            .push(json!({"name": "ONCO_BOUNDARY_CHECK"}));
        let error = build_domain_workflow_catalogue(&catalogue, &tools)
            .expect_err("case-colliding tool names must not create selection ambiguity");
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidToolDefinition(message)
                if message.contains("case-colliding")
        ));
    }

    #[test]
    fn catalogue_rejects_case_collisions_controls_and_wrong_status_types() {
        let (mut catalogue, tools) = inputs();
        catalogue[1]["id"] = json!("Oncology_Workflows");
        let error = build_domain_workflow_catalogue(&catalogue, &tools).unwrap_err();
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidGroup { group: 1, .. }
        ));

        let (mut catalogue, tools) = inputs();
        catalogue[0]["domains"] = json!(["oncology\nunsafe"]);
        let error = build_domain_workflow_catalogue(&catalogue, &tools).unwrap_err();
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidGroup { group: 0, .. }
        ));

        let (mut catalogue, tools) = inputs();
        catalogue[0]["status"] = json!(false);
        let error = build_domain_workflow_catalogue(&catalogue, &tools).unwrap_err();
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidGroup { group: 0, .. }
        ));
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
        assert_eq!(report["execution_contract"]["readiness_claimed"], false);
        assert_eq!(
            report["execution_contract"]["provider_boundary"]["container"],
            "unavailable"
        );
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
        assert_eq!(report["mission"]["policy"]["stop_on_error"], true);
        assert_eq!(report["mission"]["policy"]["allow_side_effects"], false);
        assert_eq!(report["mission"]["policy"]["require_readiness"], false);
        assert_eq!(report["mission"]["policy"]["max_steps"], 4);
        assert_eq!(report["evidence_plan"]["steps"][0]["step_id"], "boundary");
        assert_eq!(
            report["mission"]["workflow_binding"]["evidence_plan"],
            report["evidence_plan"]
        );
        assert_eq!(
            report["mission"]["workflow_binding"]["evidence_plan_digest"],
            ContentHash::of_value(&report["evidence_plan"])
                .unwrap()
                .to_string()
        );
        assert_eq!(
            report["evidence_plan"]["steps"][0]["tool_contract"]["schema_state"],
            "missing"
        );
    }

    #[test]
    fn instantiation_rejects_wrongly_typed_optional_step_metadata() {
        let (catalogue, tools) = inputs();
        let error = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":"review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","required":"false","arguments":{}}]
            }),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidStep { step: 0, .. }
        ));
    }

    #[test]
    fn instantiation_rejects_malformed_policy_types_and_padded_identity() {
        let (catalogue, tools) = inputs();
        let malformed_execute = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":"review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}],
                "policy":{"execute":"true"}
            }),
        )
        .expect_err("non-boolean execute must not silently disable execution");
        assert!(matches!(
            malformed_execute,
            DomainWorkflowError::InvalidRequest(message) if message.contains("policy.execute")
        ));

        let malformed_allow_list = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":"review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}],
                "policy":{"allowed_tools":"onco_boundary_check"}
            }),
        )
        .expect_err("non-array allow-lists must not be replaced by a derived list");
        assert!(matches!(
            malformed_allow_list,
            DomainWorkflowError::InvalidRequest(message) if message.contains("policy.allowed_tools")
        ));

        let padded_identity = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":" review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}]
            }),
        )
        .expect_err("padded workflow identity must be refused");
        assert!(matches!(
            padded_identity,
            DomainWorkflowError::InvalidRequest(message) if message.contains("goal")
        ));
    }

    #[test]
    fn instantiation_rejects_case_colliding_step_ids() {
        let (catalogue, tools) = inputs();
        let error = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":"review the oncology boundary",
                "steps":[
                    {"id":"boundary","tool":"onco_boundary_check","arguments":{}},
                    {"id":"BOUNDARY","tool":"onco_boundary_check","arguments":{}}
                ]
            }),
        )
        .expect_err("case-colliding step ids must be refused");
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidStep { reason, .. }
                if reason.contains("case-colliding")
        ));
    }

    #[test]
    fn retained_verification_rejects_noncanonical_identity_digests() {
        let (catalogue, tools) = inputs();
        let base = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":"review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}]
            }),
        )
        .unwrap();

        let mut uppercase = base.clone();
        uppercase["workflow_digest"] =
            json!(base["workflow_digest"].as_str().unwrap().to_uppercase());
        let error =
            verify_domain_workflow(&catalogue, &tools, &json!({"instantiation": uppercase}))
                .expect_err("uppercase retained digest must be refused");
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidRequest(message)
                if message.contains("lowercase 64-character hexadecimal digest")
        ));

        let mut padded = base;
        padded["workflow_id"] = json!(" oncology_workflows");
        let error = verify_domain_workflow(&catalogue, &tools, &json!({"instantiation": padded}))
            .expect_err("padded retained workflow id must be refused");
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidRequest(message)
                if message.contains("instantiation.workflow_id")
        ));

        let mut tampered = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"m-1",
                "goal":"review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}]
            }),
        )
        .unwrap();
        tampered["evidence_plan"]["steps"][0]["capture"] = json!(["tampered"]);
        let error = verify_domain_workflow(&catalogue, &tools, &json!({"instantiation": tampered}))
            .expect_err("top-level evidence plan drift must be rejected before replay");
        assert!(matches!(
            error,
            DomainWorkflowError::InvalidRequest(message)
                if message.contains("evidence_plan does not match")
        ));
    }

    #[test]
    fn portfolio_plans_complete_catalogue_with_independent_items() {
        let (catalogue, tools) = inputs();
        let report = build_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "requests": [
                    {
                        "workflow_id": "genomics_workflows",
                        "mission_id": "portfolio-genomics",
                        "goal": "compile the genomic query",
                        "steps": [{"id": "compile", "tool": "bioql_compile", "arguments": {}}]
                    },
                    {
                        "workflow_id": "oncology_workflows",
                        "mission_id": "portfolio-oncology",
                        "goal": "review the oncology boundary",
                        "steps": [{"id": "boundary", "tool": "onco_boundary_check", "arguments": {}}]
                    }
                ],
                "policy": {"require_complete_catalogue": true}
            }),
        )
        .unwrap();
        assert_eq!(report["valid"], true);
        assert_eq!(report["portfolio_ready"], true);
        assert_eq!(
            report["portfolio_status"],
            "ready_for_authoritative_preflight"
        );
        assert_eq!(report["coverage"]["complete_catalogue"], true);
        assert_eq!(report["summary"]["instantiated_count"], 2);
        assert_eq!(report["summary"]["blocked_count"], 0);
        assert_eq!(report["items"][0]["status"], "instantiated");
        assert_eq!(
            report["items"][1]["mission_preflight"]["status"],
            "deferred"
        );
        assert_eq!(report["dispatch"], "not_started");
        assert_eq!(report["execution"], "not_started");
        assert_eq!(report["portfolio_digest"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn portfolio_retains_blocked_rows_and_explicit_partial_scope() {
        let (catalogue, tools) = inputs();
        let report = build_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "requests": [
                    {
                        "workflow_id": "oncology_workflows",
                        "mission_id": "portfolio-duplicate",
                        "goal": "review",
                        "steps": [{"id": "boundary", "tool": "onco_boundary_check"}]
                    },
                    {
                        "workflow_id": "oncology_workflows",
                        "mission_id": "portfolio-duplicate",
                        "goal": "review again",
                        "steps": [{"id": "boundary", "tool": "onco_boundary_check"}]
                    }
                ],
                "policy": {"allow_partial": true}
            }),
        )
        .unwrap();
        assert_eq!(report["valid"], false);
        assert_eq!(report["portfolio_ready"], false);
        assert_eq!(report["portfolio_status"], "partial");
        assert_eq!(report["summary"]["instantiated_count"], 1);
        assert_eq!(report["summary"]["blocked_count"], 1);
        assert_eq!(report["items"][1]["status"], "blocked");
        assert_eq!(
            report["items"][1]["issues"][0]["code"],
            "duplicate_workflow_id"
        );
    }

    #[test]
    fn portfolio_require_readiness_fails_closed_without_an_attached_audit() {
        let (catalogue, tools) = inputs();
        let report = build_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "requests": [{
                    "workflow_id": "oncology_workflows",
                    "mission_id": "portfolio-readiness-gate",
                    "goal": "review the oncology boundary",
                    "steps": [{"id": "boundary", "tool": "onco_boundary_check", "arguments": {}}]
                }],
                "policy": {"require_readiness": true}
            }),
        )
        .unwrap();
        assert_eq!(report["valid"], false);
        assert_eq!(report["portfolio_status"], "blocked_by_decision_readiness");
        assert_eq!(report["decision_readiness"]["provided"], false);
        assert_eq!(report["decision_readiness"]["gate_satisfied"], false);
        assert_eq!(report["decision_readiness"]["readiness_claimed"], false);
    }

    #[test]
    fn portfolio_verification_replays_aligned_requests_without_dispatch() {
        let (catalogue, tools) = inputs();
        let requests = json!([
            {
                "workflow_id": "genomics_workflows",
                "mission_id": "verify-genomics",
                "goal": "compile the genomic query",
                "steps": [{"id": "compile", "tool": "bioql_compile", "arguments": {}}]
            },
            {
                "workflow_id": "oncology_workflows",
                "mission_id": "verify-oncology",
                "goal": "review the oncology boundary",
                "steps": [{"id": "boundary", "tool": "onco_boundary_check", "arguments": {}}]
            }
        ]);
        let portfolio = build_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "requests": requests,
                "policy": {"require_complete_catalogue": true}
            }),
        )
        .unwrap();
        let report = verify_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "portfolio": portfolio,
                "replay_requests": requests,
                "policy": {"require_replay": true, "require_complete_catalogue": true}
            }),
        )
        .unwrap();
        assert_eq!(report["valid"], true);
        assert_eq!(report["verification_status"], "verified");
        assert_eq!(report["summary"]["verified_count"], 2);
        assert_eq!(report["summary"]["replay_matched_count"], 2);
        assert_eq!(report["items"][0]["status"], "verified");
        assert_eq!(report["dispatch"], "not_started");
        assert_eq!(report["execution"], "not_started");
        assert_eq!(
            report["portfolio_verify_digest"].as_str().unwrap().len(),
            64
        );
    }

    #[test]
    fn portfolio_verification_retains_digest_and_replay_mismatches() {
        let (catalogue, tools) = inputs();
        let request = json!({
            "workflow_id": "oncology_workflows",
            "mission_id": "verify-tampered",
            "goal": "review the oncology boundary",
            "steps": [{"id": "boundary", "tool": "onco_boundary_check", "arguments": {}}]
        });
        let portfolio = build_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({"requests": [request], "policy": {}}),
        )
        .unwrap();
        let mut tampered = portfolio;
        tampered["items"][0]["instantiation"]["mission"]["goal"] =
            json!("a retained goal was tampered with");
        let report = verify_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "portfolio": tampered,
                "replay_requests": [{
                    "workflow_id": "oncology_workflows",
                    "mission_id": "verify-tampered",
                    "goal": "a different replay goal",
                    "steps": [{"id": "boundary", "tool": "onco_boundary_check", "arguments": {}}]
                }]
            }),
        )
        .unwrap();
        assert_eq!(report["valid"], false);
        assert_eq!(report["portfolio_digest_matched"], false);
        assert_eq!(report["verification_status"], "mismatch");
        assert_eq!(report["items"][0]["status"], "mismatch");
        assert!(report["items"][0]["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "replay_request_digest_mismatch"));
        assert_eq!(report["dispatch"], "not_started");
    }

    #[test]
    fn portfolio_verification_rejects_resealed_internal_row_contract_drift() {
        let (catalogue, tools) = inputs();
        let mut tampered = build_domain_workflow_portfolio(
            &catalogue,
            &tools,
            &json!({
                "requests": [{
                    "workflow_id": "oncology_workflows",
                    "mission_id": "portfolio-contract-drift",
                    "goal": "review the oncology boundary",
                    "steps": [{"id": "boundary", "tool": "onco_boundary_check", "arguments": {}}]
                }],
                "policy": {}
            }),
        )
        .unwrap();
        tampered["items"][0]["index"] = json!(1);
        tampered["summary"]["instantiated_count"] = json!(0);
        let mut without_digest = tampered.clone();
        without_digest
            .as_object_mut()
            .expect("portfolio is an object")
            .remove("portfolio_digest");
        tampered["portfolio_digest"] = json!(digest(&without_digest).unwrap());

        let report =
            verify_domain_workflow_portfolio(&catalogue, &tools, &json!({"portfolio": tampered}))
                .unwrap();
        assert_eq!(report["portfolio_digest_matched"], true);
        assert_eq!(report["valid"], false);
        assert_eq!(report["verification_status"], "mismatch");
        assert!(report["items"][0]["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "portfolio_item_index_mismatch"));
        assert!(report["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "portfolio_summary_count_mismatch"));
    }

    #[test]
    fn instantiation_carries_a_reviewed_route_into_the_mission_contract() {
        let (catalogue, tools) = inputs();
        let steps = json!([{
            "id": "boundary",
            "domain": "oncology",
            "capability": "oncology_workflows",
            "objective": "review the oncology boundary",
            "tool": "onco_boundary_check",
            "arguments": {},
            "depends_on": [],
            "bindings": [],
            "required": true
        }]);
        let route_review = json!({
            "ok": true,
            "workflow": "capability_route_review",
            "review_id": "a".repeat(64),
            "route_id": "b".repeat(64),
            "catalog_digest": "c".repeat(64),
            "goal": "review the oncology boundary",
            "findings": [],
            "review_status": "ready",
            "handoff_status": "mission_preflight_required",
            "mission_draft": {
                "goal": "review the oncology boundary",
                "steps": steps.clone(),
                "dependency_waves": [["boundary"]]
            },
            "execution": "not_started"
        });
        let report = instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id": "oncology_workflows",
                "mission_id": "reviewed-workflow",
                "goal": "review the oncology boundary",
                "steps": steps,
                "route_review": route_review
            }),
        )
        .unwrap();
        assert_eq!(
            report["mission"]["route_review"]["route_id"],
            "b".repeat(64)
        );
        assert_eq!(
            report["mission"]["route_review"]["execution"],
            "not_started"
        );
        let plan = crate::mission::plan_mission(
            &serde_json::from_value(report["mission"].clone()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            plan.route_review_provenance.as_ref().unwrap()["review_id"],
            "a".repeat(64)
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

    #[test]
    fn scaffold_selects_available_tools_by_stage_and_forces_plan_only_execution() {
        let (catalogue, tools) = inputs();
        let output = scaffold_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id": "oncology_workflows",
                "mission_id": "scaffold-1",
                "goal": "build an oncology review starting point"
            }),
        )
        .unwrap();
        assert_eq!(output["workflow"], "domain_workflow_scaffold");
        assert_eq!(
            output["selection"]["strategy"],
            "one_available_tool_per_stage"
        );
        assert_eq!(
            output["selection"]["selected_tools"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(output["mission"]["policy"]["execute"], false);
        assert_eq!(output["execution"], "not_started");
        assert_eq!(output["readiness_claimed"], false);
        assert_eq!(output["execution_contract"]["readiness_claimed"], false);
        assert_eq!(
            output["execution_contract"]["provider_boundary"]["subprocess"],
            "unavailable"
        );
        assert_eq!(
            output["instantiation"]["workflow"],
            "domain_workflow_instantiate"
        );
        assert_eq!(
            output["mission"]["workflow_binding"]["workflow_id"],
            "oncology_workflows"
        );
        assert_eq!(output["scaffold_digest"].as_str().unwrap().len(), 64);
        assert!(output["next_actions"].as_array().unwrap().len() >= 3);
    }

    #[test]
    fn scaffold_honours_explicit_tools_and_rejects_unselected_arguments() {
        let (catalogue, tools) = inputs();
        let output = scaffold_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id": "oncology_workflows",
                "mission_id": "scaffold-2",
                "goal": "focus on outcome analysis",
                "tools": ["onco_outcome_analyze"],
                "arguments": {"onco_outcome_analyze": {"subject": "S1"}}
            }),
        )
        .unwrap();
        assert_eq!(output["selection"]["strategy"], "explicit_tools");
        assert_eq!(
            output["selection"]["selected_tools"],
            json!(["onco_outcome_analyze"])
        );
        assert_eq!(
            output["mission"]["steps"][0]["arguments"],
            json!({"subject": "S1"})
        );

        let refused = scaffold_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id": "oncology_workflows",
                "mission_id": "scaffold-3",
                "goal": "reject an argument for an unselected tool",
                "tools": ["onco_outcome_analyze"],
                "arguments": {"onco_boundary_check": {}}
            }),
        );
        assert!(matches!(
            refused,
            Err(DomainWorkflowError::InvalidRequest(_))
        ));

        let recursive_catalogue = json!([{
            "id": "recursive_workflow",
            "domains": ["orchestration"],
            "crates": ["orchestration"],
            "mcp_tools": ["domain_workflow_scaffold"],
            "cli_entrypoints": [],
            "status": "available"
        }]);
        let recursive_tools = json!([{"name": "domain_workflow_scaffold"}]);
        let recursive = scaffold_domain_workflow(
            &recursive_catalogue,
            &recursive_tools,
            &json!({
                "workflow_id": "recursive_workflow",
                "mission_id": "scaffold-4",
                "goal": "reject recursive planning",
                "tools": ["domain_workflow_scaffold"]
            }),
        );
        assert!(matches!(
            recursive,
            Err(DomainWorkflowError::InvalidRequest(_))
        ));
    }

    #[test]
    fn catalogue_exposes_bounded_required_argument_contracts() {
        let catalogue = json!([{
            "id": "schema_workflow",
            "domains": ["schema"],
            "crates": ["schema"],
            "mcp_tools": ["schema_tool"],
            "cli_entrypoints": [],
            "status": "available"
        }]);
        let tools = json!([{
            "name": "schema_tool",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "required_value": {"type": "string", "minLength": 2},
                    "optional_count": {"type": "integer", "minimum": 1}
                },
                "required": ["required_value"],
                "additionalProperties": false,
                "oneOf": [{"required": ["required_value"]}]
            }
        }]);
        let report = build_domain_workflow_catalogue(&catalogue, &tools).unwrap();
        let contract = &report["workflows"][0]["tool_contracts"][0]["argument_contract"];
        assert_eq!(contract["state"], "present");
        assert_eq!(contract["required"], json!(["required_value"]));
        assert_eq!(contract["optional"], json!(["optional_count"]));
        assert_eq!(contract["properties"]["required_value"]["required"], true);
        assert_eq!(contract["properties"]["optional_count"]["minimum"], 1);
        assert_eq!(contract["additional_properties"], false);
        assert_eq!(contract["composition_keywords"], json!(["oneOf"]));
    }
}
