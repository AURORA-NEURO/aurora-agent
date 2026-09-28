//! Deterministic route selection and laboratory routing handlers.

use super::*;

impl Server {
    pub(super) fn routing_decide(&self, arguments: &Value) -> Result<Value, String> {
        let raw_fingerprint = arguments
            .get("fingerprint")
            .cloned()
            .ok_or("fingerprint is required and must be a serialized Fingerprint")?;
        let fingerprint: Fingerprint = serde_json::from_value(raw_fingerprint)
            .map_err(|error| format!("invalid routing fingerprint: {error}"))?;

        let raw_evidence = arguments
            .get("evidence")
            .cloned()
            .ok_or("evidence is required and must be an array of observations")?;
        let evidence_items = raw_evidence
            .as_array()
            .ok_or("evidence must be an array of serialized observations")?;
        if evidence_items.len() > 20_000 {
            return Err("evidence may contain at most 20000 observations".into());
        }
        let evidence: EvidenceLedger = serde_json::from_value(raw_evidence)
            .map_err(|error| format!("invalid routing evidence ledger: {error}"))?;

        let raw_policy = arguments
            .get("policy")
            .cloned()
            .ok_or("policy is required and must be a serialized RoutingPolicy")?;
        let policy: RoutingPolicy = serde_json::from_value(raw_policy)
            .map_err(|error| format!("invalid routing policy: {error}"))?;
        policy
            .validate()
            .map_err(|error| format!("routing policy is invalid: {error}"))?;

        let task_id = arguments.get("task_id").and_then(Value::as_str);
        if arguments.get("task_id").is_some() && task_id.is_none() {
            return Err("task_id must be a string when supplied".into());
        }
        if task_id.is_some_and(str::is_empty) {
            return Err("task_id must not be empty".into());
        }
        let decision = match task_id {
            Some(task_id) => policy
                .route_unseen(task_id, &fingerprint, &evidence)
                .map_err(|error| format!("routing refused: {error}"))?,
            None => policy
                .route(&fingerprint, &evidence)
                .map_err(|error| format!("routing refused: {error}"))?,
        };
        let neighbourhood = evidence
            .neighbourhood(&fingerprint, policy.neighbourhood_radius)
            .len();

        Ok(json!({
            "ok": true,
            "decision": decision,
            "task_id": task_id,
            "holdout_check": if task_id.is_some() { "enforced" } else { "caller_must_supply_unseen_identity" },
            "evidence": {
                "observations": evidence.len(),
                "distinct_tasks": evidence.task_ids().len(),
                "neighbourhood_observations": neighbourhood,
                "neighbourhood_radius": policy.neighbourhood_radius,
            },
            "guarantees": [
                "the policy can select only from its approved architecture set",
                "insufficient coverage or margin abstains to the declared safe default",
                "route_unseen refuses evidence containing the routed task identity",
                "confidence is a routing score, not a posterior probability",
            ],
        }))
    }

    pub(super) fn routing_lab_run(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure routing-lab input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("routing-lab input exceeds the 20000000-byte safety bound".into());
        }
        let raw_tasks = arguments
            .get("tasks")
            .and_then(Value::as_array)
            .ok_or("tasks is required and must be an array of {task_id, world, query} objects")?;
        if raw_tasks.is_empty() || raw_tasks.len() > 256 {
            return Err("tasks must contain between 1 and 256 task objects".into());
        }
        let mut tasks = Vec::with_capacity(raw_tasks.len());
        for (index, raw_task) in raw_tasks.iter().enumerate() {
            let object = raw_task
                .as_object()
                .ok_or_else(|| format!("tasks[{index}] must be an object"))?;
            let task_id = object
                .get("task_id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("tasks[{index}].task_id must be a string"))?;
            if task_id.is_empty() || task_id.len() > 512 {
                return Err(format!(
                    "tasks[{index}].task_id must contain between 1 and 512 bytes"
                ));
            }
            let raw_world = object
                .get("world")
                .cloned()
                .ok_or_else(|| format!("tasks[{index}].world is required"))?;
            let world = World::from_json(raw_world)
                .map_err(|error| format!("tasks[{index}].world is invalid: {error}"))?;
            let raw_query = object
                .get("query")
                .cloned()
                .ok_or_else(|| format!("tasks[{index}].query is required"))?;
            let query = Query::from_json(raw_query)
                .map_err(|error| format!("tasks[{index}].query is invalid: {error}"))?;
            let task = Task::new(task_id, world, query)
                .map_err(|error| format!("tasks[{index}] is invalid: {error}"))?;
            tasks.push(task);
        }

        let raw_settings = arguments
            .get("settings")
            .cloned()
            .ok_or("settings is required and must be a serialized LabSettings")?;
        let settings: LabSettings = serde_json::from_value(raw_settings)
            .map_err(|error| format!("invalid routing-lab settings: {error}"))?;
        if settings.calibration_bins == 0 || settings.calibration_bins > 100 {
            return Err("settings.calibration_bins must be between 1 and 100".into());
        }
        let include_rows = arguments
            .get("include_rows")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }

        let report = match run_routing_lab(&tasks, &settings) {
            Ok(report) => report,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/routing-lab-run/0.1",
                    "stage": "lab_execution",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "a routing report is not emitted when an architecture outcome is unjudged or holdout evidence cannot be constructed",
                        "partial comparator rows are not promoted to a complete regret account",
                    ],
                }));
            }
        };
        let task_rows = if include_rows {
            serde_json::to_value(
                report
                    .tasks
                    .iter()
                    .take(max_rows as usize)
                    .collect::<Vec<_>>(),
            )
            .map_err(|error| format!("cannot serialize routing-lab task rows: {error}"))?
        } else {
            json!([])
        };
        let account = serde_json::to_value(&report.account)
            .map_err(|error| format!("cannot serialize routing-lab regret account: {error}"))?;
        let calibration = serde_json::to_value(&report.calibration)
            .map_err(|error| format!("cannot serialize routing-lab calibration: {error}"))?;
        let verdict = serde_json::to_value(report.verdict)
            .map_err(|error| format!("cannot serialize routing-lab verdict: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/routing-lab-run/0.1",
            "tasks": tasks.len(),
            "holdout": settings.holdout,
            "holdout_label": settings.holdout.as_str(),
            "approved_architectures": settings.policy.approved.iter().map(|architecture| architecture.label()).collect::<Vec<_>>(),
            "fixed_default": settings.fixed_default,
            "include_rows": include_rows,
            "report": {
                "account": account,
                "calibration": calibration,
                "verdict": verdict,
                "abstention_rate": report.abstention_rate,
                "oracle_agreement_rate": report.oracle_agreement_rate,
                "tasks_won": report.tasks_won,
                "tasks_lost": report.tasks_lost,
                "tasks_tied": report.tasks_tied,
                "caveats": report.caveats,
                "task_rows": task_rows,
                "task_rows_omitted": if include_rows { report.tasks.len().saturating_sub(max_rows as usize) } else { report.tasks.len() },
            },
            "guarantees": [
                "the full approved architecture panel is observed before routing, so the oracle comparator remains retrospective and labelled",
                "each routed task is evaluated through route_unseen against task- or regime-restricted evidence",
                "fixed-default, most-expensive-default, router, and oracle outcomes remain separate",
                "captured gain, regret, abstention, and calibration remain report fields rather than a single flattering win rate",
            ],
            "limitations": [
                "this is an offline context-architecture lab; it does not select models, prompts, providers, or production agent topologies",
                "worlds and queries are caller-supplied and the lab performs no network, filesystem, model, or patient-facing action",
                "the oracle retrospective selector is a ceiling, not a deployable policy, and confidence is not a posterior probability",
            ],
        }))
    }
}
