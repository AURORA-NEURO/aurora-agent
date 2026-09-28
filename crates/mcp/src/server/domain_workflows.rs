//! Domain acquisition catalogues and retained workflow lifecycle handlers.

use super::*;

impl Server {
    pub(super) fn domain_acquisition_catalogue(&self, arguments: &Value) -> Result<Value, String> {
        let query: DomainAcquisitionQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid domain acquisition query: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let report =
            build_domain_acquisition_catalogue(&catalogue, &AdapterRegistry::default(), &query)
                .map_err(|error| format!("domain acquisition catalogue refused: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_ACQUISITION_SCHEMA_VERSION,
            "workflow": DOMAIN_ACQUISITION_WORKFLOW,
            "catalogue": report,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "transport and adapter interpretation are exposed as separate planes for every selected declared domain",
                "the report is bound to both the authoritative workspace catalogue digest and adapter registry digest",
                "the operation performs no I/O, source retrieval, Python import, adapter execution, or credential resolution"
            ],
            "does_not_claim": [
                "scope-label overlap is ontology resolution or scientific validity",
                "declared transport membership proves an external source was called or is authentic",
                "a native or Python-delegated route proves source-specific conformance or dependency availability"
            ]
        }))
    }

    /// Build the complete deterministic workflow catalogue, including a per-group domain contract
    /// for scope, evidence retention, review gates, and completion posture.
    ///
    /// This is intentionally a planning operation, not a semantic router or permission grant.
    /// Domain contracts are structural handoff evidence; they do not infer domain truth or
    /// readiness and do not dispatch any tool.
    pub(super) fn domain_workflow_catalogue(&self, arguments: &Value) -> Result<Value, String> {
        if !arguments
            .as_object()
            .is_some_and(|object| object.is_empty())
        {
            return Err("domain_workflow_catalogue accepts an empty arguments object".into());
        }
        build_domain_workflow_catalogue(
            &workspace_capabilities(),
            &Value::Array(tool_definitions()),
        )
        .map_err(|error| format!("domain workflow catalogue refused: {error}"))
    }

    /// Build an execution-disabled, schema-aware starting mission for one capability group.
    ///
    /// Kernel scaffolding chooses tools; this transport layer adds the authoritative live
    /// `tools/list` preflight. Missing required arguments therefore remain a structured blocked
    /// handoff rather than being turned into a transport-level success or an opaque failure.
    pub(super) fn domain_workflow_scaffold(&self, arguments: &Value) -> Result<Value, String> {
        let mut output = scaffold_domain_workflow(
            &workspace_capabilities(),
            &Value::Array(tool_definitions()),
            arguments,
        )
        .map_err(|error| format!("domain workflow scaffold refused: {error}"))?;
        let mission = output
            .get("mission")
            .cloned()
            .ok_or_else(|| "domain workflow scaffold omitted mission".to_string())?;
        let preflight_report = match self.preflight_agent_mission(&mission) {
            Ok(report) => report,
            Err(error) => json!({
                "ok": false,
                "workflow": "agent_mission",
                "preflight": true,
                "dispatch": "not_started",
                "schema_valid": false,
                "error": error,
                "guarantees": [
                    "the scaffold remains execution-disabled",
                    "authoritative schema refusal is retained for caller correction"
                ],
                "readiness_claimed": false
            }),
        };
        let preflight_ok = preflight_report.get("ok") == Some(&Value::Bool(true));
        output["preflight_status"] = json!(if preflight_ok { "ready" } else { "blocked" });
        output["preflight_report"] = preflight_report.clone();
        output["instantiation"]["preflight_report"] = preflight_report;
        output["next_actions"] = if preflight_ok {
            json!([
                "review the selected and omitted tools for domain-specific sufficiency",
                "obtain any required operations gate acceptance",
                "execute only through an explicit allow-list and reconcile the retained report"
            ])
        } else {
            json!([
                "fill every reported required argument field from the authoritative tool schema",
                "rerun the scaffold or mission preflight after correcting arguments",
                "obtain any required operations gate acceptance before execution"
            ])
        };
        Ok(output)
    }

    /// Instantiate one explicitly selected, group-scoped workflow into a validated mission and
    /// a step-by-step evidence plan.
    ///
    /// The kernel validates mission invariants here; the server then adds the authoritative MCP
    /// schema preflight report. Neither operation dispatches a selected tool, and unavailable
    /// tools or out-of-scope policy entries fail before preflight.
    pub(super) fn domain_workflow_instantiate(&self, arguments: &Value) -> Result<Value, String> {
        let mut output = instantiate_domain_workflow(
            &workspace_capabilities(),
            &Value::Array(tool_definitions()),
            arguments,
        )
        .map_err(|error| format!("domain workflow instantiation refused: {error}"))?;
        let mission = output
            .get("mission")
            .cloned()
            .ok_or_else(|| "domain workflow instantiation omitted mission".to_string())?;
        let preflight_report = self
            .preflight_agent_mission(&mission)
            .map_err(|error| format!("domain workflow mission preflight refused: {error}"))?;
        output["preflight_report"] = preflight_report;
        Ok(output)
    }

    /// Plan multiple group-scoped workflows while retaining independent authoritative preflight
    /// outcomes for each item. This is a composition boundary only; it never dispatches a group
    /// tool or turns a complete portfolio into execution authorization.
    pub(super) fn domain_workflow_portfolio(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode domain workflow portfolio input: {error}"))?;
        if encoded.len() > bioprism_devplat::MAX_DOMAIN_WORKFLOW_BYTES {
            return Err(format!(
                "domain workflow portfolio input exceeds the {}-byte safety bound",
                bioprism_devplat::MAX_DOMAIN_WORKFLOW_BYTES
            ));
        }
        let mut output = build_domain_workflow_portfolio(
            &workspace_capabilities(),
            &Value::Array(tool_definitions()),
            arguments,
        )
        .map_err(|error| format!("domain workflow portfolio refused: {error}"))?;
        let mut preflight_blocked_count = 0usize;
        if let Some(items) = output.get_mut("items").and_then(Value::as_array_mut) {
            for item in items.iter_mut() {
                if item.get("status").and_then(Value::as_str) != Some("instantiated") {
                    continue;
                }
                let mission = item
                    .pointer("/instantiation/mission")
                    .cloned()
                    .ok_or_else(|| "portfolio item omitted instantiated mission".to_string())?;
                let preflight = match self.preflight_agent_mission(&mission) {
                    Ok(value) => value,
                    Err(error) => json!({
                        "ok": false,
                        "workflow": "agent_mission",
                        "preflight": true,
                        "dispatch": "not_started",
                        "schema_valid": false,
                        "error": error,
                        "fail_closed": true,
                        "readiness_claimed": false
                    }),
                };
                let preflight_ok = preflight.get("ok") == Some(&Value::Bool(true));
                let observed_plan_digest = preflight
                    .pointer("/plan/digest")
                    .cloned()
                    .unwrap_or(Value::Null);
                let preflight_status = if preflight_ok { "matched" } else { "blocked" };
                item["mission_preflight"] = json!({
                    "status": preflight_status,
                    "matched": preflight_ok,
                    "ok": preflight_ok,
                    "observed_plan_digest": observed_plan_digest,
                    "dispatch": "not_started"
                });
                item["instantiation"]["preflight_report"] = preflight.clone();
                if !preflight_ok {
                    preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                    item["status"] = json!("blocked_by_mission_preflight");
                    item["issues"] = json!([{
                        "code": "mission_preflight_blocked",
                        "message": "authoritative mission schema preflight blocked this portfolio item",
                        "preflight": preflight
                    }]);
                }
            }
        }
        let kernel_valid = output
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let valid = kernel_valid && preflight_blocked_count == 0;
        output["valid"] = json!(valid);
        output["portfolio_ready"] = json!(valid);
        output["summary"]["preflight_blocked_count"] = json!(preflight_blocked_count);
        output["summary"]["preflight_status"] = json!(if preflight_blocked_count == 0 {
            "matched"
        } else {
            "blocked"
        });
        output["preflight"] = json!({
            "required": true,
            "status": if preflight_blocked_count == 0 { "matched" } else { "blocked" },
            "matched": preflight_blocked_count == 0,
            "blocked_count": preflight_blocked_count,
            "dispatch": "not_started"
        });
        if preflight_blocked_count > 0 {
            let allow_partial = output
                .pointer("/policy/allow_partial")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            output["portfolio_status"] = json!(if allow_partial { "partial" } else { "blocked" });
        }
        output["dispatch"] = json!("not_started");
        output["execution"] = json!("not_started");
        if let Some(object) = output.as_object_mut() {
            object.remove("portfolio_digest");
        }
        let digest = bioprism_ids::ContentHash::of_value(&output)
            .map_err(|error| format!("cannot digest domain workflow portfolio: {error}"))?;
        output["portfolio_digest"] = json!(digest.to_string());
        Ok(output)
    }

    /// Verify a retained multi-domain portfolio, replay aligned requests when supplied, and
    /// preflight every structurally successful item against the authoritative live tool schemas.
    /// This is an audit projection only: no selected tool is dispatched, retried, resumed, or
    /// promoted to execution readiness.
    pub(super) fn domain_workflow_portfolio_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode domain workflow portfolio verification input: {error}")
        })?;
        if encoded.len() > bioprism_devplat::MAX_DOMAIN_WORKFLOW_BYTES {
            return Err(format!(
                "domain workflow portfolio verification input exceeds the {}-byte safety bound",
                bioprism_devplat::MAX_DOMAIN_WORKFLOW_BYTES
            ));
        }
        let mut output = verify_domain_workflow_portfolio(
            &workspace_capabilities(),
            &Value::Array(tool_definitions()),
            arguments,
        )
        .map_err(|error| format!("domain workflow portfolio verification refused: {error}"))?;
        let mut preflight_attempted_count = 0usize;
        let mut preflight_blocked_count = 0usize;
        if let Some(items) = output.get_mut("items").and_then(Value::as_array_mut) {
            for item in items.iter_mut() {
                let structurally_verified = matches!(
                    item.get("status").and_then(Value::as_str),
                    Some("verified") | Some("verified_without_replay")
                );
                if !structurally_verified {
                    continue;
                }
                let mission = item
                    .pointer("/instantiation/mission")
                    .cloned()
                    .ok_or_else(|| {
                        "portfolio verification item omitted verified mission".to_string()
                    })?;
                preflight_attempted_count = preflight_attempted_count.saturating_add(1);
                let preflight = match self.preflight_agent_mission(&mission) {
                    Ok(value) => value,
                    Err(error) => json!({
                        "ok": false,
                        "workflow": "agent_mission",
                        "preflight": true,
                        "dispatch": "not_started",
                        "schema_valid": false,
                        "error": error,
                        "fail_closed": true,
                        "readiness_claimed": false
                    }),
                };
                let preflight_ok = preflight.get("ok") == Some(&Value::Bool(true));
                let expected_plan_digest = item
                    .pointer("/instantiation/preflight_report/plan/digest")
                    .cloned()
                    .unwrap_or(Value::Null);
                let observed_plan_digest = preflight
                    .pointer("/plan/digest")
                    .cloned()
                    .unwrap_or(Value::Null);
                let mut item_mismatches = item
                    .get("mismatches")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let preflight_status = if !preflight_ok {
                    item_mismatches.push(json!({
                        "code": "mission_preflight_blocked",
                        "expected": expected_plan_digest,
                        "observed": observed_plan_digest,
                        "message": "authoritative mission schema preflight blocked this portfolio verification item"
                    }));
                    preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                    item["status"] = json!("blocked_by_mission_preflight");
                    "blocked"
                } else if expected_plan_digest.is_null() {
                    item_mismatches.push(json!({
                        "code": "retained_preflight_missing",
                        "message": "the retained instantiation has no authoritative preflight plan digest",
                        "observed": observed_plan_digest
                    }));
                    preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                    item["status"] = json!("blocked_by_mission_preflight");
                    "retained_projection_missing"
                } else if expected_plan_digest != observed_plan_digest {
                    item_mismatches.push(json!({
                        "code": "mission_plan_digest_mismatch",
                        "expected": expected_plan_digest,
                        "observed": observed_plan_digest
                    }));
                    preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                    item["status"] = json!("blocked_by_mission_preflight");
                    "mismatched"
                } else {
                    "matched"
                };
                item["mission_preflight"] = json!({
                    "requested": true,
                    "status": preflight_status,
                    "matched": preflight_status == "matched",
                    "ok": preflight_ok,
                    "expected_plan_digest": expected_plan_digest,
                    "observed_plan_digest": observed_plan_digest,
                    "dispatch": "not_started"
                });
                item["verification"]["mission_preflight"] = item["mission_preflight"].clone();
                item["verification"]["preflight_report"] = preflight.clone();
                item["instantiation"]["preflight_report"] = preflight;
                item["mismatches"] = Value::Array(item_mismatches);
            }
        }
        let kernel_valid = output
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let valid = kernel_valid && preflight_blocked_count == 0;
        output["valid"] = json!(valid);
        output["portfolio_ready"] = json!(valid);
        let kernel_blocked_count = output
            .pointer("/summary/blocked_count")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        output["summary"]["blocked_count"] =
            json!(kernel_blocked_count.saturating_add(preflight_blocked_count));
        output["summary"]["preflight_attempted_count"] = json!(preflight_attempted_count);
        output["summary"]["preflight_blocked_count"] = json!(preflight_blocked_count);
        output["summary"]["preflight_status"] = json!(if preflight_blocked_count > 0 {
            "blocked"
        } else if preflight_attempted_count > 0 {
            "matched"
        } else {
            "deferred"
        });
        output["preflight"] = json!({
            "required": true,
            "status": if preflight_blocked_count > 0 {
                "blocked"
            } else if preflight_attempted_count > 0 {
                "matched"
            } else {
                "deferred"
            },
            "matched": preflight_blocked_count == 0 && preflight_attempted_count > 0,
            "attempted_count": preflight_attempted_count,
            "blocked_count": preflight_blocked_count,
            "dispatch": "not_started"
        });
        let mut mismatches = output
            .get("mismatches")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if preflight_blocked_count > 0 {
            mismatches.push(json!({
                "code": "portfolio_mission_preflight_blocked",
                "blocked_count": preflight_blocked_count
            }));
        }
        output["mismatches"] = Value::Array(mismatches);
        let status = if valid {
            output
                .get("verification_status")
                .cloned()
                .unwrap_or_else(|| json!("verified"))
        } else if preflight_blocked_count > 0 {
            json!(if output
                .pointer("/policy/allow_partial")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "partial"
            } else {
                "blocked_by_mission_preflight"
            })
        } else {
            output
                .get("verification_status")
                .cloned()
                .unwrap_or_else(|| json!("mismatch"))
        };
        output["verification_status"] = status;
        output["dispatch"] = json!("not_started");
        output["execution"] = json!("not_started");
        if let Some(object) = output.as_object_mut() {
            object.remove("portfolio_verify_digest");
        }
        let digest = bioprism_ids::ContentHash::of_value(&output).map_err(|error| {
            format!("cannot digest domain workflow portfolio verification: {error}")
        })?;
        output["portfolio_verify_digest"] = json!(digest.to_string());
        Ok(output)
    }

    /// Verify a retained domain workflow against the live catalogue and authoritative mission
    /// preflight without dispatching any selected tool.
    pub(super) fn domain_workflow_verify(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode domain workflow verification input: {error}")
        })?;
        if encoded.len() > bioprism_devplat::MAX_DOMAIN_WORKFLOW_BYTES {
            return Err(format!(
                "domain workflow verification input exceeds the {}-byte safety bound",
                bioprism_devplat::MAX_DOMAIN_WORKFLOW_BYTES
            ));
        }
        let mut output = bioprism_devplat::verify_domain_workflow(
            &workspace_capabilities(),
            &Value::Array(tool_definitions()),
            arguments,
        )
        .map_err(|error| format!("domain workflow verification refused: {error}"))?;
        let instantiation = arguments
            .get("instantiation")
            .ok_or("domain workflow verification requires instantiation")?;
        let mission = instantiation
            .get("mission")
            .ok_or("domain workflow verification instantiation omitted mission")?;
        let preflight = match self.preflight_agent_mission(mission) {
            Ok(value) => value,
            Err(error) => json!({
                "ok": false,
                "workflow": "agent_mission",
                "preflight": true,
                "dispatch": "not_started",
                "schema_valid": false,
                "error": error,
                "fail_closed": true,
            }),
        };
        let preflight_ok = preflight.get("ok") == Some(&Value::Bool(true));
        let expected_plan_digest = instantiation
            .pointer("/preflight_report/plan/digest")
            .cloned()
            .unwrap_or(Value::Null);
        let observed_plan_digest = preflight
            .pointer("/plan/digest")
            .cloned()
            .unwrap_or(Value::Null);
        let mut mismatches = output
            .get("mismatches")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let preflight_status = if !preflight_ok {
            mismatches.push(json!({
                "code": "mission_preflight_blocked",
                "expected": expected_plan_digest,
                "observed": observed_plan_digest,
            }));
            "blocked"
        } else if expected_plan_digest.is_null() {
            mismatches.push(json!({
                "code": "retained_preflight_missing",
                "message": "the retained instantiation has no authoritative preflight plan digest",
                "observed": observed_plan_digest,
            }));
            "retained_projection_missing"
        } else if expected_plan_digest != observed_plan_digest {
            mismatches.push(json!({
                "code": "mission_plan_digest_mismatch",
                "expected": expected_plan_digest,
                "observed": observed_plan_digest,
            }));
            "mismatched"
        } else {
            "matched"
        };
        output["mission_preflight"] = json!({
            "requested": true,
            "status": preflight_status,
            "matched": preflight_status == "matched",
            "ok": preflight_ok,
            "expected_plan_digest": expected_plan_digest,
            "observed_plan_digest": observed_plan_digest,
            "dispatch": "not_started",
        });
        output["preflight_report"] = preflight;
        output["mismatches"] = Value::Array(mismatches.clone());
        let valid = mismatches.is_empty();
        let replay_status = output
            .pointer("/replay/status")
            .and_then(Value::as_str)
            .unwrap_or("not_requested")
            .to_owned();
        let replay_requested = output
            .pointer("/replay/requested")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        output["valid"] = json!(valid);
        output["verification_status"] = json!(if valid {
            if replay_requested {
                "verified"
            } else {
                "verified_without_replay"
            }
        } else if replay_status == "blocked" {
            "blocked_by_replay"
        } else if preflight_status == "blocked" || preflight_status == "retained_projection_missing"
        {
            "blocked_by_mission_preflight"
        } else {
            "mismatch"
        });
        output["execution"] = json!("not_started");
        output["dispatch"] = json!("not_started");
        Ok(output)
    }

    /// Reconcile a retained mission report or evidence bundle against an instantiated workflow.
    ///
    /// This is an audit projection only. It checks plan and tool identity, result counters,
    /// trace lifecycle, output retention, refusal/omission posture, and completion readiness;
    /// it never retries or dispatches a domain tool.
    pub(super) fn domain_workflow_reconcile(&self, arguments: &Value) -> Result<Value, String> {
        reconcile_domain_workflow(arguments)
            .map_err(|error| format!("domain workflow reconciliation refused: {error}"))
    }

    /// Import a previously produced reconciliation report into the bounded process-local audit
    /// registry. The report digest is recomputed before it becomes queryable.
    pub(super) fn domain_workflow_reconciliation_import(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode domain workflow reconciliation import input: {error}")
        })?;
        if encoded.len() > bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES {
            return Err(format!(
                "domain workflow reconciliation import input exceeds the {}-byte bound",
                bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES
            ));
        }
        let record = arguments.get("record").ok_or("record is required")?;
        self.workflow_reconciliation_registry
            .lock()
            .map_err(|_| "workflow reconciliation registry lock is poisoned".to_string())?
            .import(record)
            .map_err(|error| format!("domain workflow reconciliation import refused: {error}"))
    }

    /// Query digest-ordered reconciliation rows without executing or re-evaluating anything.
    pub(super) fn domain_workflow_reconciliation_query(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode domain workflow reconciliation query input: {error}")
        })?;
        if encoded.len() > 1_000_000 {
            return Err(
                "domain workflow reconciliation query input exceeds the 1000000-byte bound".into(),
            );
        }
        let optional_string = |name: &str| -> Result<Option<&str>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| format!("{name} must be a string"))
                })
                .transpose()
        };
        let mission_id = optional_string("mission_id")?;
        let workflow_id = optional_string("workflow_id")?;
        let mission_plan_digest = optional_string("mission_plan_digest")?;
        let completion_status = optional_string("completion_status")?;
        let decision_readiness_state = optional_string("decision_readiness_state")?;
        let after = optional_string("after")?;
        let max_items = arguments
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_items must be an integer".to_string())
                    .and_then(|number| {
                        usize::try_from(number).map_err(|_| "max_items is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(100);
        if !(1..=bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS)
            .contains(&max_items)
        {
            return Err(format!(
                "max_items must be between 1 and {}",
                bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS
            ));
        }
        let include_records = arguments
            .get("include_records")
            .map(|value| value.as_bool().ok_or("include_records must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        let decision_readiness_gate_satisfied = arguments
            .get("decision_readiness_gate_satisfied")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or("decision_readiness_gate_satisfied must be a boolean")
            })
            .transpose()?;
        self.workflow_reconciliation_registry
            .lock()
            .map_err(|_| "workflow reconciliation registry lock is poisoned".to_string())?
            .query(
                mission_id,
                workflow_id,
                mission_plan_digest,
                completion_status,
                decision_readiness_state,
                decision_readiness_gate_satisfied,
                after,
                max_items,
                include_records,
            )
            .map_err(|error| format!("domain workflow reconciliation query refused: {error}"))
    }

    /// Fetch one imported reconciliation report by its content hash.
    pub(super) fn domain_workflow_reconciliation_get(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let digest = arguments
            .get("reconciliation_digest")
            .and_then(Value::as_str)
            .ok_or("reconciliation_digest is required and must be a content hash")?;
        bioprism_ids::ContentHash::parse(digest.to_string())
            .map_err(|error| format!("reconciliation_digest is invalid: {error}"))?;
        let record = self
            .workflow_reconciliation_registry
            .lock()
            .map_err(|_| "workflow reconciliation registry lock is poisoned".to_string())?
            .get(digest)
            .ok_or_else(|| format!("reconciliation {digest:?} is not present in the registry"))?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/domain-workflow-reconciliation-record/0.1",
            "workflow": "domain_workflow_reconciliation_get",
            "reconciliation_digest": digest,
            "record": record,
            "execution": "not_started",
            "guarantees": [
                "the returned report passed the shared reconciliation digest check before import",
                "lookup does not execute, retry, resume, or mutate a mission"
            ],
            "limitations": [
                "the process-local registry is bounded and is not a distributed audit store",
                "report presence does not establish scientific validity, clinical safety, or release approval"
            ]
        }))
    }
}
