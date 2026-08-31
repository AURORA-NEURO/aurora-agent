//! Claim-per-test coverage of the autopilot kernel: grant refusals, every classification row,
//! planner determinism and stop rules, repair construction, exhaustion unconstructability,
//! report digests, and the drive loop over a fake dispatcher.

use bioprism_autopilot::{
    build_autopilot_report, classify_step_result, drive_instantiation, drive_mission,
    drive_mission_with_checkpoint, drive_mission_with_schedule, plan_next_action,
    preview_first_action, resume_mission_with_checkpoint, seal_autopilot_checkpoint,
    validate_autopilot_checkpoint, verify_autopilot_report, AttemptKind, AttemptRecord,
    AutonomyGrant, AutopilotCheckpointPersistence, AutopilotCheckpointStore, AutopilotError,
    DriveHistory, FinalDisposition, FinalStatus, GrantError, JsonAutopilotCheckpointPersistence,
    NextAction, RetryClass, RetrySchedule, StepClass,
    TransactionalAutopilotCheckpointPersistenceCoordinator, TransactionalAutopilotCheckpointStore,
    TransactionalJsonAutopilotCheckpointPersistence, REQUIRED_LIMITATIONS,
};
use bioprism_devplat::{
    plan_mission, MissionReport, MissionRequest, MissionStepResult, MISSION_SCHEMA_VERSION,
    MISSION_TRACE_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};

fn grant_of(value: Value) -> AutonomyGrant {
    serde_json::from_value(value).expect("test grant must validate")
}

fn grant_error(value: Value) -> String {
    serde_json::from_value::<AutonomyGrant>(value)
        .expect_err("test grant must be refused")
        .to_string()
}

fn default_grant(tools: &[&str]) -> AutonomyGrant {
    grant_of(json!({ "allowed_tools": tools, "max_attempts": 4 }))
}

fn step(id: &str, tool: &str, deps: &[&str], arguments: Value, bindings: Value) -> Value {
    step_in_domain("metrics", id, tool, deps, arguments, bindings)
}

fn step_in_domain(
    domain: &str,
    id: &str,
    tool: &str,
    deps: &[&str],
    arguments: Value,
    bindings: Value,
) -> Value {
    json!({
        "id": id,
        "domain": domain,
        "capability": "analytics",
        "objective": format!("run {id}"),
        "tool": tool,
        "arguments": arguments,
        "depends_on": deps,
        "bindings": bindings,
        "required": true,
    })
}

fn binding_for(step_ids: &[&str]) -> Value {
    let plan = json!({
        "steps": step_ids
            .iter()
            .map(|id| json!({ "step_id": id }))
            .collect::<Vec<_>>()
    });
    let digest = ContentHash::of_value(&plan).unwrap().to_string();
    let domain_contract_digest = ContentHash::of_value(&json!({})).unwrap().to_string();
    let zeros = "0".repeat(64);
    json!({
        "workflow_id": "wf-1",
        "workflow_digest": zeros,
        "catalog_digest": zeros,
        "domain_contract_digest": domain_contract_digest,
        "domain_contract": {},
        "evidence_plan": plan,
        "evidence_plan_digest": digest,
    })
}

fn mission_of(steps: Vec<Value>, binding: Option<Value>) -> Value {
    let mut mission = json!({
        "mission_id": "m1",
        "goal": "drive the workflow",
        "steps": steps,
    });
    if let Some(mut binding) = binding {
        // Keep compact fixture authoring while satisfying the strict binding contract: evidence
        // rows carry the corresponding mission tool and a digest over the resulting plan.
        if let (Some(mission_steps), Some(plan_steps)) = (
            mission["steps"].as_array(),
            binding
                .get_mut("evidence_plan")
                .and_then(Value::as_object_mut)
                .and_then(|plan| plan.get_mut("steps"))
                .and_then(Value::as_array_mut),
        ) {
            for (plan_step, mission_step) in plan_steps.iter_mut().zip(mission_steps) {
                if let Some(tool) = mission_step.get("tool") {
                    plan_step["tool"] = tool.clone();
                }
            }
            let plan = binding.get("evidence_plan").unwrap();
            binding["evidence_plan_digest"] =
                json!(ContentHash::of_value(plan).unwrap().to_string());
        }
        mission["workflow_binding"] = binding;
    }
    mission
}

fn ok_envelope(payload: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "step",
        "result": { "content": [ { "type": "text", "text": payload.to_string() } ] }
    })
}

fn error_envelope(text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "step",
        "result": {
            "isError": true,
            "content": [ { "type": "text", "text": text } ]
        }
    })
}

fn ok_result(id: &str, tool: &str, payload: Option<&Value>) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "succeeded".into(),
        required: true,
        arguments_digest: Some("1".repeat(64)),
        bytes: 10,
        wire: payload.map(ok_envelope),
        error: None,
    }
}

fn refused_executor(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "refused".into(),
        required: true,
        arguments_digest: None,
        bytes: 0,
        wire: None,
        error: Some("mission binding refused: source step has no successful payload".into()),
    }
}

fn refused_tool(id: &str, tool: &str, error_text: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "refused".into(),
        required: true,
        arguments_digest: Some("2".repeat(64)),
        bytes: 20,
        wire: Some(error_envelope(error_text)),
        error: Some(error_text.into()),
    }
}

fn blocked_result(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "blocked".into(),
        required: true,
        arguments_digest: None,
        bytes: 0,
        wire: None,
        error: Some("a prerequisite mission step refused or was blocked".into()),
    }
}

fn cancelled_result(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "cancelled".into(),
        required: true,
        arguments_digest: None,
        bytes: 0,
        wire: None,
        error: Some("mission cancellation was requested before this step was dispatched".into()),
    }
}

/// Build a mission report the way the executor's own counters would, so the planner reads
/// internally consistent evidence.
fn report_for(
    mission: &Value,
    results: Vec<MissionStepResult>,
    status_override: Option<&str>,
) -> Value {
    let request: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
    let plan = plan_mission(&request).unwrap();
    let succeeded = results.iter().filter(|r| r.status == "succeeded").count();
    let refused = results.iter().filter(|r| r.status == "refused").count();
    let blocked = results.iter().filter(|r| r.status == "blocked").count();
    let cancelled = results.iter().filter(|r| r.status == "cancelled").count();
    let required_failures = results
        .iter()
        .filter(|r| r.required && r.status != "succeeded")
        .count();
    let mission_status = status_override.unwrap_or(if required_failures > 0 {
        "failed"
    } else if refused + blocked + cancelled > 0 {
        "partial"
    } else {
        "succeeded"
    });
    let report = MissionReport {
        schema_version: MISSION_SCHEMA_VERSION.into(),
        plan,
        execution: "executed".into(),
        mission_status: mission_status.into(),
        succeeded,
        refused,
        blocked,
        cancelled,
        required_failures,
        returned_bytes: 0,
        results,
        execution_trace_schema_version: MISSION_TRACE_SCHEMA_VERSION.into(),
        execution_trace: Vec::new(),
        claim_requests: Vec::new(),
        evaluator_review: None,
        claim_lineage: json!({}),
        trace_observer: None,
        guarantees: Vec::new(),
        limitations: Vec::new(),
    };
    serde_json::to_value(report).unwrap()
}

fn complete_reconciliation() -> Value {
    json!({
        "present": true,
        "reconciliation_digest": "a".repeat(64),
        "completion": { "status": "complete" },
        "integrity": { "valid": true },
    })
}

fn expect_full_dispatch(grant: &AutonomyGrant, history: &DriveHistory) -> Value {
    match plan_next_action(grant, history).unwrap() {
        NextAction::DispatchFull { mission, .. } => mission,
        other => panic!("expected a full dispatch, got {other:?}"),
    }
}

fn expect_repair_dispatch(grant: &AutonomyGrant, history: &DriveHistory) -> (Value, Vec<String>) {
    match plan_next_action(grant, history).unwrap() {
        NextAction::DispatchRepair {
            mission,
            included_step_ids,
            ..
        } => (mission, included_step_ids),
        other => panic!("expected a repair dispatch, got {other:?}"),
    }
}

fn expect_exhausted(grant: &AutonomyGrant, history: &DriveHistory) -> Value {
    match plan_next_action(grant, history).unwrap() {
        NextAction::StopExhausted { accounting } => accounting,
        other => panic!("expected an exhausted stop, got {other:?}"),
    }
}

fn push_full(
    grant: &AutonomyGrant,
    history: &mut DriveHistory,
    results: Vec<MissionStepResult>,
    reconciliation: Option<Value>,
    status_override: Option<&str>,
) {
    let mission = expect_full_dispatch(grant, history);
    let report = report_for(&mission, results, status_override);
    history.push(
        AttemptRecord::delivered(AttemptKind::Full, mission, report, reconciliation, None).unwrap(),
    );
}

fn push_repair(
    grant: &AutonomyGrant,
    history: &mut DriveHistory,
    results: Vec<MissionStepResult>,
    reconciliation: Option<Value>,
) -> Value {
    let (mission, _) = expect_repair_dispatch(grant, history);
    let report = report_for(&mission, results, None);
    history.push(
        AttemptRecord::delivered(
            AttemptKind::Repair,
            mission.clone(),
            report,
            reconciliation,
            None,
        )
        .unwrap(),
    );
    mission
}

fn three_step_mission() -> Value {
    mission_of(
        vec![
            step("a", "tool_a", &[], json!({}), json!([])),
            step(
                "b",
                "tool_b",
                &["a"],
                json!({ "seed": null }),
                json!([{ "from_step": "a", "source_pointer": "/value", "target_pointer": "/seed" }]),
            ),
            step("c", "tool_c", &["b"], json!({}), json!([])),
        ],
        Some(binding_for(&["a", "b", "c"])),
    )
}

fn declared_as_is_text() -> String {
    json!({ "retryability": "retryable_as_is" }).to_string()
}

mod grant {
    use super::*;

    #[test]
    fn a_grant_with_no_tools_is_refused_not_defaulted() {
        let error = grant_error(json!({ "allowed_tools": [], "max_attempts": 3 }));
        assert!(error.contains("no default allow-list"), "{error}");
    }

    #[test]
    fn a_grant_with_zero_attempts_is_refused() {
        let error = grant_error(json!({ "allowed_tools": ["tool_a"], "max_attempts": 0 }));
        assert!(error.contains("between 1 and 16"), "{error}");
    }

    #[test]
    fn a_grant_with_seventeen_attempts_is_refused() {
        let error = grant_error(json!({ "allowed_tools": ["tool_a"], "max_attempts": 17 }));
        assert!(error.contains("between 1 and 16"), "{error}");
    }

    #[test]
    fn a_grant_naming_agent_mission_is_refused_as_recursive() {
        let error = grant_error(json!({ "allowed_tools": ["agent_mission"], "max_attempts": 3 }));
        assert!(error.contains("recursive"), "{error}");
    }

    #[test]
    fn a_grant_with_a_duplicate_tool_is_refused() {
        let error =
            grant_error(json!({ "allowed_tools": ["tool_a", "tool_a"], "max_attempts": 3 }));
        assert!(error.contains("more than once"), "{error}");
    }

    #[test]
    fn a_grant_with_an_invalid_tool_name_is_refused() {
        let error = grant_error(json!({ "allowed_tools": ["not a tool"], "max_attempts": 3 }));
        assert!(error.contains("bare tool name"), "{error}");
    }

    #[test]
    fn a_grant_with_more_than_512_tools_is_refused() {
        let tools: Vec<String> = (0..513).map(|index| format!("tool_{index}")).collect();
        let error = grant_error(json!({ "allowed_tools": tools, "max_attempts": 3 }));
        assert!(error.contains("maximum is 512"), "{error}");
    }

    #[test]
    fn a_grant_with_an_unknown_field_is_refused_not_ignored() {
        let refused = serde_json::from_value::<AutonomyGrant>(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 3,
            "retry_terminal": true,
        }));
        assert!(refused.is_err(), "an unknown field must not be ignored");
    }

    #[test]
    fn a_grant_with_an_unknown_retry_field_is_refused() {
        let refused = serde_json::from_value::<AutonomyGrant>(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 3,
            "retry": { "retry_terminal": true },
        }));
        assert!(
            refused.is_err(),
            "an unknown retry field must not be ignored"
        );
    }

    #[test]
    fn the_default_retry_policy_retries_declared_as_is_only() {
        let grant = default_grant(&["tool_a"]);
        assert!(grant.retry().retryable_as_is());
        assert!(!grant.retry().retryable_after_change());
        assert!(!grant.retry().unknown());
        assert!(!grant.allow_side_effects());
        assert!(grant.require_reconciliation_complete());
        assert_eq!(grant.schedule().retry_base_delay(), 0);
        assert_eq!(grant.schedule().retry_max_delay(), 0);
    }

    #[test]
    fn a_retry_schedule_is_explicit_bounded_and_exponentially_capped() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 4,
            "schedule": { "retry_base_delay": 2, "retry_max_delay": 5 },
        }));
        assert_eq!(grant.schedule().delay_for_retry(0), 0);
        assert_eq!(grant.schedule().delay_for_retry(1), 2);
        assert_eq!(grant.schedule().delay_for_retry(2), 4);
        assert_eq!(grant.schedule().delay_for_retry(3), 5);
        assert_eq!(grant.schedule().delay_for_retry(usize::MAX), 5);
        let round_trip: RetrySchedule = serde_json::from_value::<AutonomyGrant>(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 4,
            "schedule": { "retry_base_delay": 2, "retry_max_delay": 5 },
        }))
        .unwrap()
        .schedule()
        .to_owned();
        assert_eq!(round_trip, *grant.schedule());
    }

    #[test]
    fn an_invalid_retry_schedule_is_refused_before_dispatch_authority_exists() {
        for schedule in [
            json!({ "retry_base_delay": 5, "retry_max_delay": 4 }),
            json!({ "retry_base_delay": 0, "retry_max_delay": 1 }),
            json!({ "retry_base_delay": 31_536_001, "retry_max_delay": 31_536_001 }),
        ] {
            let error = grant_error(json!({
                "allowed_tools": ["tool_a"],
                "max_attempts": 3,
                "schedule": schedule,
            }));
            assert!(error.contains("retry schedule is invalid"), "{error}");
        }
    }

    #[test]
    fn stop_on_first_success_false_is_refused_as_unsupported() {
        let refused = serde_json::from_value::<AutonomyGrant>(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 3,
            "stop_on_first_success": false,
        }));
        let message = refused.expect_err("false must be refused").to_string();
        assert!(message.contains("not supported"), "{message}");
        assert!(format!("{}", GrantError::UnsupportedStopOption).contains("only true is accepted"));
    }

    #[test]
    fn a_grant_round_trips_through_serde_with_a_stable_digest() {
        let grant = default_grant(&["tool_a", "tool_b"]);
        let encoded = serde_json::to_value(&grant).unwrap();
        let decoded: AutonomyGrant = serde_json::from_value(encoded).unwrap();
        assert_eq!(grant, decoded);
        assert_eq!(grant.digest().unwrap(), decoded.digest().unwrap());
    }
}

mod classification {
    use super::*;

    #[test]
    fn a_succeeded_step_classifies_as_succeeded() {
        let row = classify_step_result(&ok_result("a", "tool_a", Some(&json!({ "value": 1 }))));
        assert_eq!(row.class, StepClass::Succeeded);
        assert_eq!(row.signal, "succeeded");
    }

    #[test]
    fn a_blocked_step_classifies_as_blocked_not_failed() {
        let row = classify_step_result(&blocked_result("c", "tool_c"));
        assert_eq!(row.class, StepClass::Blocked);
    }

    #[test]
    fn an_executor_refusal_with_no_envelope_is_terminal() {
        let row = classify_step_result(&refused_executor("b", "tool_b"));
        assert_eq!(row.class, StepClass::Failed(RetryClass::Terminal));
        assert_eq!(row.signal, "executor_refusal");
    }

    #[test]
    fn a_tool_error_with_no_declared_decision_is_unknown_not_retryable() {
        let row = classify_step_result(&refused_tool("b", "tool_b", "cannot read the store"));
        assert_eq!(row.class, StepClass::Failed(RetryClass::Unknown));
        assert_eq!(row.signal, "undeclared_tool_error");
    }

    #[test]
    fn a_declared_retryable_as_is_in_structured_content_is_honoured() {
        let mut result = refused_tool("b", "tool_b", "failed");
        result.wire = Some(json!({
            "jsonrpc": "2.0",
            "result": {
                "isError": true,
                "structuredContent": { "retryability": "retryable_as_is" },
                "content": [ { "type": "text", "text": "failed" } ]
            }
        }));
        let row = classify_step_result(&result);
        assert_eq!(row.class, StepClass::Failed(RetryClass::RetryableAsIs));
        assert_eq!(row.signal, "declared_structured_content");
    }

    #[test]
    fn a_declared_decision_in_json_error_text_is_honoured() {
        let row = classify_step_result(&refused_tool("b", "tool_b", &declared_as_is_text()));
        assert_eq!(row.class, StepClass::Failed(RetryClass::RetryableAsIs));
        assert_eq!(row.signal, "declared_error_text");
    }

    #[test]
    fn a_declared_terminal_decision_is_honoured() {
        let text = json!({ "error": { "retryability": "terminal" } }).to_string();
        let row = classify_step_result(&refused_tool("b", "tool_b", &text));
        assert_eq!(row.class, StepClass::Failed(RetryClass::Terminal));
    }

    #[test]
    fn a_declared_after_change_decision_is_honoured() {
        let text = json!({ "retryability": "retryable_after_change" }).to_string();
        let row = classify_step_result(&refused_tool("b", "tool_b", &text));
        assert_eq!(
            row.class,
            StepClass::Failed(RetryClass::RetryableAfterChange)
        );
    }

    #[test]
    fn an_unrecognised_retryability_value_is_not_a_signal() {
        let text = json!({ "retryability": "maybe_later" }).to_string();
        let row = classify_step_result(&refused_tool("b", "tool_b", &text));
        assert_eq!(row.class, StepClass::Failed(RetryClass::Unknown));
        assert_eq!(row.signal, "undeclared_tool_error");
    }

    #[test]
    fn a_cancelled_step_is_unknown_class() {
        let row = classify_step_result(&cancelled_result("a", "tool_a"));
        assert_eq!(row.class, StepClass::Failed(RetryClass::Unknown));
        assert_eq!(row.signal, "cancelled");
    }

    #[test]
    fn an_unrecognised_status_is_unknown_class() {
        let mut result = ok_result("a", "tool_a", None);
        result.status = "observed".into();
        let row = classify_step_result(&result);
        assert_eq!(row.class, StepClass::Failed(RetryClass::Unknown));
        assert_eq!(row.signal, "unrecognised_status");
    }
}

mod planner {
    use super::*;

    #[test]
    fn an_empty_history_dispatches_the_full_mission_with_grant_policy_overwritten() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "allow_side_effects": false,
        }));
        let history = DriveHistory::new(three_step_mission()).unwrap();
        match plan_next_action(&grant, &history).unwrap() {
            NextAction::DispatchFull {
                mission,
                authorization,
            } => {
                assert_eq!(authorization.attempt_index(), 1);
                assert_eq!(mission["policy"]["execute"], json!(true));
                assert_eq!(
                    mission["policy"]["allowed_tools"],
                    json!(["tool_a", "tool_b", "tool_c"])
                );
                assert_eq!(mission["policy"]["allow_side_effects"], json!(false));
                assert_eq!(mission["mission_id"], json!("m1"));
            }
            other => panic!("expected a full dispatch, got {other:?}"),
        }
    }

    #[test]
    fn planning_is_deterministic_for_identical_inputs() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let first = plan_next_action(&grant, &history).unwrap();
        let second = plan_next_action(&grant, &history).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn a_mission_using_a_tool_outside_the_grant_is_a_policy_refusal() {
        let grant = default_grant(&["tool_a"]);
        let history = DriveHistory::new(three_step_mission()).unwrap();
        let refused = plan_next_action(&grant, &history).expect_err("must be refused");
        assert!(
            matches!(refused, AutopilotError::GrantDoesNotAuthorise { .. }),
            "{refused:?}"
        );
    }

    #[test]
    fn a_confirmation_flag_without_side_effect_authority_is_a_policy_refusal() {
        let grant = default_grant(&["tool_a"]);
        let mission = mission_of(
            vec![step(
                "a",
                "tool_a",
                &[],
                json!({ "confirm": true }),
                json!([]),
            )],
            None,
        );
        let history = DriveHistory::new(mission).unwrap();
        let refused = plan_next_action(&grant, &history).expect_err("must be refused");
        assert!(
            matches!(refused, AutopilotError::GrantDoesNotAuthorise { .. }),
            "{refused:?}"
        );
    }

    #[test]
    fn success_with_complete_reconciliation_stops_the_drive_with_evidence() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                ok_result("c", "tool_c", Some(&json!({ "done": true }))),
            ],
            Some(complete_reconciliation()),
            None,
        );
        match plan_next_action(&grant, &history).unwrap() {
            NextAction::StopSuccess { evidence } => {
                assert_eq!(evidence["mission_status"], json!("succeeded"));
                assert_eq!(evidence["steps"].as_array().unwrap().len(), 3);
                assert_eq!(evidence["reconciliation"]["required"], json!(true));
                assert_eq!(evidence["reconciliation"]["status"], json!("complete"));
                assert_eq!(evidence["reconciliation"]["scope"], json!("full_plan"));
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn success_without_required_reconciliation_is_not_success() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                ok_result("c", "tool_c", Some(&json!({ "done": true }))),
            ],
            None,
            None,
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(accounting["reason"], json!("reconciliation_incomplete"));
    }

    #[test]
    fn a_grant_waiving_reconciliation_accepts_a_bare_succeeded_report() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
        }));
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                ok_result("c", "tool_c", Some(&json!({ "done": true }))),
            ],
            None,
            None,
        );
        match plan_next_action(&grant, &history).unwrap() {
            NextAction::StopSuccess { evidence } => {
                assert_eq!(evidence["reconciliation"]["required"], json!(false));
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn a_terminal_executor_refusal_stops_the_drive_as_refused() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_executor("b", "tool_b"),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        match plan_next_action(&grant, &history).unwrap() {
            NextAction::StopRefused {
                first_terminal_refusal,
            } => {
                assert_eq!(first_terminal_refusal["step_id"], json!("b"));
                assert_eq!(first_terminal_refusal["signal"], json!("executor_refusal"));
            }
            other => panic!("expected a refusal stop, got {other:?}"),
        }
    }

    #[test]
    fn an_undeclared_tool_error_is_not_retried_by_default() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", "socket closed unexpectedly"),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(
            accounting["reason"],
            json!("unresolved_steps_not_retryable")
        );
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        let row_b = rows
            .iter()
            .find(|row| row["step_id"] == json!("b"))
            .unwrap();
        assert!(
            row_b["exclusion"]
                .as_str()
                .unwrap()
                .contains("`unknown` is not authorised"),
            "{row_b}"
        );
    }

    #[test]
    fn an_unknown_failure_is_retried_only_when_the_grant_says_so() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "retry": { "retry_unknown": true },
        }));
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", "socket closed unexpectedly"),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let (_, included) = expect_repair_dispatch(&grant, &history);
        assert_eq!(included, vec!["b".to_string(), "c".to_string()]);
    }

    #[test]
    fn a_repair_contains_only_unresolved_steps_and_their_blocked_dependents() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let (mission, included) = expect_repair_dispatch(&grant, &history);
        assert_eq!(included, vec!["b".to_string(), "c".to_string()]);
        let steps = mission["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["id"], json!("b"));
        assert_eq!(steps[1]["id"], json!("c"));
        assert_eq!(mission["mission_id"], json!("m1-repair-2"));
        assert_eq!(mission["claim_requests"], json!([]));
    }

    #[test]
    fn a_repair_rematerializes_bindings_from_retained_payloads() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let (mission, _) = expect_repair_dispatch(&grant, &history);
        let steps = mission["steps"].as_array().unwrap();
        assert_eq!(steps[0]["arguments"]["seed"], json!(7));
        assert_eq!(steps[0]["bindings"], json!([]));
        assert_eq!(steps[0]["depends_on"], json!([]));
        assert_eq!(steps[1]["depends_on"], json!(["b"]));
    }

    #[test]
    fn a_repair_mission_filters_the_binding_evidence_plan_and_restores_its_digest() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let (mission, _) = expect_repair_dispatch(&grant, &history);
        let binding = &mission["workflow_binding"];
        let plan_steps = binding["evidence_plan"]["steps"].as_array().unwrap();
        let ids: Vec<&str> = plan_steps
            .iter()
            .map(|entry| entry["step_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["b", "c"]);
        let recomputed = ContentHash::of_value(&binding["evidence_plan"])
            .unwrap()
            .to_string();
        assert_eq!(binding["evidence_plan_digest"], json!(recomputed));
        let request: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
        plan_mission(&request).expect("a planner-built repair must satisfy the mission contract");
    }

    #[test]
    fn a_binding_whose_payload_was_not_retained_stops_the_drive_with_the_reason_recorded() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", None),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(
            accounting["reason"],
            json!("unresolved_steps_not_retryable")
        );
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        let row_b = rows
            .iter()
            .find(|row| row["step_id"] == json!("b"))
            .unwrap();
        assert!(
            row_b["exclusion"]
                .as_str()
                .unwrap()
                .contains("cannot be re-materialized"),
            "{row_b}"
        );
        let row_c = rows
            .iter()
            .find(|row| row["step_id"] == json!("c"))
            .unwrap();
        assert!(
            row_c["exclusion"].as_str().unwrap().contains("excluded"),
            "{row_c}"
        );
    }

    #[test]
    fn a_missing_binding_pointer_in_the_retained_payload_excludes_the_step() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "other": 1 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let accounting = expect_exhausted(&grant, &history);
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        let row_b = rows
            .iter()
            .find(|row| row["step_id"] == json!("b"))
            .unwrap();
        assert!(
            row_b["exclusion"]
                .as_str()
                .unwrap()
                .contains("source pointer"),
            "{row_b}"
        );
    }

    #[test]
    fn after_max_attempts_no_dispatch_action_is_constructable() {
        for max_attempts in 1..=3usize {
            let grant = grant_of(json!({
                "allowed_tools": ["tool_a", "tool_b", "tool_c"],
                "max_attempts": max_attempts,
            }));
            let mut history = DriveHistory::new(three_step_mission()).unwrap();
            for attempt in 0..max_attempts {
                let results = vec![
                    ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                    refused_tool("b", "tool_b", &declared_as_is_text()),
                    blocked_result("c", "tool_c"),
                ];
                if attempt == 0 {
                    push_full(&grant, &mut history, results, None, None);
                } else {
                    let repair_results = vec![
                        refused_tool("b", "tool_b", &declared_as_is_text()),
                        blocked_result("c", "tool_c"),
                    ];
                    push_repair(&grant, &mut history, repair_results, None);
                }
            }
            let action = plan_next_action(&grant, &history).unwrap();
            assert!(
                matches!(action, NextAction::StopExhausted { .. }),
                "an exhausted history must never plan a dispatch (max_attempts={max_attempts}, \
                 got {action:?})"
            );
        }
    }

    #[test]
    fn a_cancelled_mission_is_never_re_dispatched_even_when_unknown_retries_are_authorized() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "retry": { "retry_unknown": true },
        }));
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                cancelled_result("b", "tool_b"),
                cancelled_result("c", "tool_c"),
            ],
            None,
            Some("cancelled"),
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(accounting["reason"], json!("mission_cancelled"));
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        assert_eq!(
            rows.len(),
            2,
            "the accounting must name the cancelled and blocked steps as unresolved: {rows:?}"
        );
    }

    #[test]
    fn a_cancelled_step_is_never_re_dispatched_even_inside_a_non_cancelled_report() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "retry": { "retry_unknown": true },
        }));
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                cancelled_result("b", "tool_b"),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(
            accounting["reason"],
            json!("unresolved_steps_not_retryable")
        );
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        let row_b = rows
            .iter()
            .find(|row| row["step_id"] == json!("b"))
            .unwrap();
        assert_eq!(row_b["signal"], json!("cancelled"));
        assert!(
            row_b["exclusion"].as_str().unwrap().contains("cancell"),
            "{row_b}"
        );
    }

    #[test]
    fn a_repair_dispatch_discloses_the_claim_ids_it_strips() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut mission = three_step_mission();
        mission["claim_requests"] = json!([{
            "id": "claim-1",
            "claim": "step a produced its value",
            "domains": ["metrics"],
            "requires_steps": ["a"],
        }]);
        let mut history = DriveHistory::new(mission).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        match plan_next_action(&grant, &history).unwrap() {
            NextAction::DispatchRepair {
                mission,
                dropped_claim_ids,
                ..
            } => {
                assert_eq!(dropped_claim_ids, vec!["claim-1".to_string()]);
                assert_eq!(mission["claim_requests"], json!([]));
            }
            other => panic!("expected a repair dispatch, got {other:?}"),
        }
    }

    #[test]
    fn a_transport_error_ends_the_drive_without_retry() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        let mission = expect_full_dispatch(&grant, &history);
        history.push(
            AttemptRecord::undelivered(AttemptKind::Full, mission, "connection reset".into())
                .unwrap(),
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(accounting["reason"], json!("dispatch_transport_error"));
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        assert_eq!(
            rows.len(),
            3,
            "an undelivered dispatch leaves every step unresolved and the accounting must say \
             so: {rows:?}"
        );
    }

    #[test]
    fn a_repair_needs_a_workflow_binding_when_reconciliation_is_required() {
        let grant = default_grant(&["tool_a", "tool_b"]);
        let mission = mission_of(
            vec![
                step("a", "tool_a", &[], json!({}), json!([])),
                step("b", "tool_b", &["a"], json!({}), json!([])),
            ],
            None,
        );
        let mut history = DriveHistory::new(mission).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
            ],
            None,
            None,
        );
        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(
            accounting["reason"],
            json!("repair_reconciliation_unavailable")
        );
    }

    #[test]
    fn a_repair_final_success_requires_the_subset_scoped_reconciliation() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                refused_tool("b", "tool_b", &declared_as_is_text()),
                blocked_result("c", "tool_c"),
            ],
            None,
            None,
        );
        push_repair(
            &grant,
            &mut history,
            vec![
                ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                ok_result("c", "tool_c", Some(&json!({ "done": true }))),
            ],
            Some(complete_reconciliation()),
        );
        match plan_next_action(&grant, &history).unwrap() {
            NextAction::StopSuccess { evidence } => {
                assert_eq!(evidence["reconciliation"]["scope"], json!("repair_subset"));
                let steps = evidence["steps"].as_array().unwrap();
                assert_eq!(steps[0]["attempt_index"], json!(1));
                assert_eq!(steps[1]["attempt_index"], json!(2));
                assert_eq!(steps[2]["attempt_index"], json!(2));
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn a_dry_run_preview_plans_the_first_action_without_any_history() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let action = preview_first_action(&grant, three_step_mission()).unwrap();
        assert!(matches!(action, NextAction::DispatchFull { .. }));
    }
}

mod report {
    use super::*;

    fn finished_history() -> (AutonomyGrant, DriveHistory) {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut history = DriveHistory::new(three_step_mission()).unwrap();
        push_full(
            &grant,
            &mut history,
            vec![
                ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                ok_result("c", "tool_c", Some(&json!({ "done": true }))),
            ],
            Some(complete_reconciliation()),
            None,
        );
        (grant, history)
    }

    #[test]
    fn an_autopilot_report_round_trips_through_verification() {
        let (grant, history) = finished_history();
        let NextAction::StopSuccess { evidence } = plan_next_action(&grant, &history).unwrap()
        else {
            panic!("expected success");
        };
        let report =
            build_autopilot_report(&grant, &history, &FinalDisposition::Succeeded { evidence })
                .unwrap();
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["valid"], json!(true));
        assert_eq!(verification["digest_match"], json!(true));
        assert_eq!(report["final_status"], json!("succeeded"));
        assert_eq!(report["totals"]["attempts_used"], json!(1));
        assert_eq!(report["attempts"][0]["kind"], json!("full"));
        assert_eq!(
            report["attempts"][0]["reconciliation_scope"],
            json!("full_plan")
        );
    }

    #[test]
    fn a_tampered_report_fails_digest_verification() {
        let (grant, history) = finished_history();
        let NextAction::StopSuccess { evidence } = plan_next_action(&grant, &history).unwrap()
        else {
            panic!("expected success");
        };
        let mut report =
            build_autopilot_report(&grant, &history, &FinalDisposition::Succeeded { evidence })
                .unwrap();
        report["final_status"] = json!("refused");
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["valid"], json!(false));
        assert_eq!(verification["digest_match"], json!(false));
        assert_eq!(verification["digest_malformed"], json!(false));
    }

    #[test]
    fn a_malformed_report_digest_fails_as_digest_malformed_not_as_tampering() {
        let (grant, history) = finished_history();
        let NextAction::StopSuccess { evidence } = plan_next_action(&grant, &history).unwrap()
        else {
            panic!("expected success");
        };
        let mut report =
            build_autopilot_report(&grant, &history, &FinalDisposition::Succeeded { evidence })
                .unwrap();
        report["report_sha256"] = json!("NOT-64-LOWERCASE-HEX-CHARACTERS");
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["valid"], json!(false));
        assert_eq!(verification["digest_malformed"], json!(true));
        assert_eq!(verification["digest_match"], json!(false));
    }

    #[test]
    fn every_report_carries_the_required_limitations() {
        let (grant, history) = finished_history();
        let report = build_autopilot_report(
            &grant,
            &history,
            &FinalDisposition::Exhausted {
                accounting: json!({ "reason": "attempt_budget_exhausted" }),
            },
        )
        .unwrap();
        let limitations: Vec<&str> = report["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        for required in REQUIRED_LIMITATIONS {
            assert!(
                limitations.contains(&required),
                "missing required limitation: {required}"
            );
        }
    }

    #[test]
    fn verification_refuses_a_foreign_schema() {
        let refused = verify_autopilot_report(&json!({ "schema": "something-else/1.0" }));
        assert!(
            matches!(refused, Err(AutopilotError::InvalidAutopilotReport { .. })),
            "{refused:?}"
        );
    }
}

mod drive {
    use super::*;

    #[test]
    fn the_drive_repairs_a_declared_retryable_failure_and_chains_receipts() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let mut calls: Vec<Value> = Vec::new();
        let outcome = {
            let mut dispatcher = |mission: &Value| -> Result<Value, String> {
                calls.push(mission.clone());
                let step_count = mission["steps"].as_array().unwrap().len();
                if step_count == 3 {
                    Ok(report_for(
                        mission,
                        vec![
                            ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                            refused_tool("b", "tool_b", &declared_as_is_text()),
                            blocked_result("c", "tool_c"),
                        ],
                        None,
                    ))
                } else {
                    let mut report = report_for(
                        mission,
                        vec![
                            ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                            ok_result("c", "tool_c", Some(&json!({ "done": true }))),
                        ],
                        None,
                    );
                    report["workflow_reconciliation"] = complete_reconciliation();
                    Ok(report)
                }
            };
            drive_mission(&grant, three_step_mission(), &mut dispatcher).unwrap()
        };
        assert_eq!(outcome.final_status, FinalStatus::Succeeded);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1]["steps"].as_array().unwrap().len(), 2);
        assert_eq!(calls[1]["steps"][0]["arguments"]["seed"], json!(7));
        let verification = verify_autopilot_report(&outcome.report).unwrap();
        assert_eq!(verification["valid"], json!(true));
        assert_eq!(outcome.report["attempts"][1]["kind"], json!("repair"));
        assert_eq!(
            outcome.report["attempts"][1]["reconciliation_scope"],
            json!("repair_subset")
        );
    }

    #[test]
    fn the_drive_never_exceeds_the_attempt_budget() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 3,
        }));
        let mission = mission_of(
            vec![step("a", "tool_a", &[], json!({}), json!([]))],
            Some(binding_for(&["a"])),
        );
        let mut dispatched = 0usize;
        let outcome = {
            let mut dispatcher = |mission: &Value| -> Result<Value, String> {
                dispatched += 1;
                Ok(report_for(
                    mission,
                    vec![refused_tool("a", "tool_a", &declared_as_is_text())],
                    None,
                ))
            };
            drive_mission(&grant, mission, &mut dispatcher).unwrap()
        };
        assert_eq!(dispatched, 3);
        assert_eq!(outcome.final_status, FinalStatus::Exhausted);
        assert_eq!(outcome.report["totals"]["attempts_used"], json!(3));
        assert_eq!(
            outcome.report["accounting"]["reason"],
            json!("attempt_budget_exhausted")
        );
    }

    #[test]
    fn a_reconciliation_requiring_grant_refuses_a_bindingless_mission_before_any_dispatch() {
        let grant = default_grant(&["tool_a"]);
        let mission = mission_of(vec![step("a", "tool_a", &[], json!({}), json!([]))], None);
        let outcome = {
            let mut dispatcher = |_mission: &Value| -> Result<Value, String> {
                panic!("a drive that provably cannot reach success must not dispatch")
            };
            drive_mission(&grant, mission, &mut dispatcher).unwrap()
        };
        assert_eq!(outcome.final_status, FinalStatus::Exhausted);
        assert_eq!(outcome.report["totals"]["attempts_used"], json!(0));
        assert_eq!(
            outcome.report["accounting"]["reason"],
            json!("reconciliation_unavailable")
        );
        let rows = outcome.report["accounting"]["unresolved_steps"]
            .as_array()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["step_id"], json!("a"));
        let verification = verify_autopilot_report(&outcome.report).unwrap();
        assert_eq!(verification["valid"], json!(true));
    }

    #[test]
    fn an_undelivered_dispatch_ends_the_drive_and_is_counted() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let outcome = {
            let mut dispatcher =
                |_mission: &Value| -> Result<Value, String> { Err("connection reset".into()) };
            drive_mission(&grant, three_step_mission(), &mut dispatcher).unwrap()
        };
        assert_eq!(outcome.final_status, FinalStatus::Exhausted);
        assert_eq!(outcome.report["totals"]["attempts_used"], json!(1));
        assert_eq!(
            outcome.report["attempts"][0]["dispatch_error"],
            json!("connection reset")
        );
        assert_eq!(
            outcome.report["accounting"]["reason"],
            json!("dispatch_transport_error")
        );
    }

    #[test]
    fn an_instantiation_that_was_not_accepted_is_refused() {
        let grant = default_grant(&["tool_a"]);
        let instantiation = json!({
            "workflow": "domain_workflow_instantiate",
            "ok": false,
            "mission": mission_of(vec![step("a", "tool_a", &[], json!({}), json!([]))], None),
        });
        let mut dispatcher = |_mission: &Value| -> Result<Value, String> {
            panic!("a refused instantiation must never dispatch")
        };
        let refused = drive_instantiation(&grant, &instantiation, &mut dispatcher)
            .expect_err("must be refused");
        assert!(
            matches!(refused, AutopilotError::InvalidInstantiation { .. }),
            "{refused:?}"
        );
    }

    #[test]
    fn a_terminal_step_failure_surfaces_as_a_refused_drive() {
        let grant = default_grant(&["tool_a", "tool_b", "tool_c"]);
        let outcome = {
            let mut dispatcher = |mission: &Value| -> Result<Value, String> {
                Ok(report_for(
                    mission,
                    vec![
                        ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                        refused_executor("b", "tool_b"),
                        blocked_result("c", "tool_c"),
                    ],
                    None,
                ))
            };
            drive_mission(&grant, three_step_mission(), &mut dispatcher).unwrap()
        };
        assert_eq!(outcome.final_status, FinalStatus::Refused);
        assert_eq!(
            outcome.report["first_terminal_refusal"]["step_id"],
            json!("b")
        );
    }

    #[test]
    fn the_drive_waits_only_before_authorized_repairs_and_caps_backoff() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
            "schedule": { "retry_base_delay": 3, "retry_max_delay": 5 },
        }));
        let mut waits = Vec::new();
        let mut calls = 0;
        let outcome = {
            let mut dispatcher = |mission: &Value| -> Result<Value, String> {
                calls += 1;
                if calls == 1 {
                    Ok(report_for(
                        mission,
                        vec![
                            ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                            refused_tool("b", "tool_b", &declared_as_is_text()),
                            blocked_result("c", "tool_c"),
                        ],
                        None,
                    ))
                } else {
                    Ok(report_for(
                        mission,
                        vec![
                            ok_result("b", "tool_b", Some(&json!({ "done": true }))),
                            ok_result("c", "tool_c", Some(&json!({ "done": true }))),
                        ],
                        None,
                    ))
                }
            };
            drive_mission_with_schedule(
                &grant,
                three_step_mission(),
                &mut dispatcher,
                |delay| {
                    waits.push(delay);
                    Ok(())
                },
                |_history| Ok(()),
            )
            .unwrap()
        };
        assert_eq!(outcome.final_status, FinalStatus::Succeeded);
        assert_eq!(calls, 2);
        assert_eq!(waits, vec![3]);
    }

    #[test]
    fn a_wait_failure_stops_before_the_repair_dispatch() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
            "schedule": { "retry_base_delay": 7, "retry_max_delay": 7 },
        }));
        let mut calls = 0;
        let error = {
            let mut dispatcher = |mission: &Value| -> Result<Value, String> {
                calls += 1;
                Ok(report_for(
                    mission,
                    vec![
                        ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                        refused_tool("b", "tool_b", &declared_as_is_text()),
                        blocked_result("c", "tool_c"),
                    ],
                    None,
                ))
            };
            drive_mission_with_schedule(
                &grant,
                three_step_mission(),
                &mut dispatcher,
                |_delay| Err("worker shutdown".into()),
                |_history| Ok(()),
            )
            .expect_err("wait failure must not be swallowed")
        };
        assert_eq!(calls, 1);
        assert_eq!(
            error,
            AutopilotError::Scheduling {
                reason: "worker shutdown".into()
            }
        );
    }
}

mod persistence {
    use super::*;

    /// The snapshot's retained *values*, without the structural vocabulary that names them.
    ///
    /// A leak scan over the serialised document greps its own field names and policy sentences.
    /// Two of these assertions failed against a projection that was leaking nothing: `retention`
    /// is the literal `metadata_only_autopilot;missions_arguments_provider_output_credentials_and_evidence_not_retained`,
    /// which contains `arguments` inside the clause promising arguments are *not* retained; and
    /// the field name `result_metadata_digest` contains `data`, which is one of the twelve
    /// built-in domain names. The guard was tripping on the sentence that declares the guarantee
    /// and on the key that carries the digest.
    ///
    /// Keys and the policy constants are compile-time strings that no mission, attempt or report
    /// can influence, so the scan belongs on the values alone — which is also the stronger claim,
    /// since leaked material would have to arrive as a value. `leak_scan_still_fires_on_planted_payload`
    /// holds this honest: a scan that cannot fail is worth nothing.
    fn retained_values(snapshot: &Value) -> String {
        fn collect(value: &Value, into: &mut Vec<String>) {
            match value {
                Value::String(text) => into.push(text.clone()),
                Value::Number(number) => into.push(number.to_string()),
                Value::Bool(flag) => into.push(flag.to_string()),
                Value::Array(items) => items.iter().for_each(|item| collect(item, into)),
                Value::Object(entries) => entries.values().for_each(|entry| collect(entry, into)),
                Value::Null => {}
            }
        }
        let mut scanned = snapshot.clone();
        if let Some(map) = scanned.as_object_mut() {
            for declaration in ["schema", "retention", "secret_material"] {
                map.remove(declaration);
            }
        }
        let mut values = Vec::new();
        collect(&scanned, &mut values);
        values.join("\u{1f}")
    }

    #[test]
    fn leak_scan_still_fires_on_planted_payload() {
        let snapshot = json!({
            "retention": bioprism_autopilot::AUTOPILOT_CHECKPOINT_RETENTION,
            "secret_material": "never_returned",
            "attempts": [{ "mission_digest": "aa" }],
        });
        assert!(
            !retained_values(&snapshot).contains("private_task"),
            "a clean snapshot must scan clean"
        );

        let mut planted = snapshot.clone();
        planted["attempts"][0]["mission_digest"] = json!("private_task text");
        assert!(
            retained_values(&planted).contains("private_task"),
            "the scan must catch payload planted where an attempt projection would carry it"
        );
    }
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct SharedStore(Rc<RefCell<Option<String>>>);

    impl AutopilotCheckpointStore for SharedStore {
        fn read(&mut self) -> Result<Option<String>, String> {
            Ok(self.0.borrow().clone())
        }

        fn write(&mut self, value: String) -> Result<(), String> {
            *self.0.borrow_mut() = Some(value);
            Ok(())
        }
    }

    impl TransactionalAutopilotCheckpointStore for SharedStore {
        fn write_if_unchanged(
            &mut self,
            expected_snapshot_digest: Option<&str>,
            value: String,
        ) -> Result<bool, String> {
            let actual_snapshot_digest = self
                .0
                .borrow()
                .as_ref()
                .and_then(|encoded| serde_json::from_str::<Value>(encoded).ok())
                .and_then(|snapshot| {
                    snapshot
                        .get("snapshot_digest")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                });
            if actual_snapshot_digest.as_deref() != expected_snapshot_digest {
                return Ok(false);
            }
            *self.0.borrow_mut() = Some(value);
            Ok(true)
        }
    }

    #[test]
    fn checkpoint_is_restart_safe_metadata_only_and_resume_does_not_replay_success() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
        }));
        let mission = mission_of(
            vec![step(
                "a",
                "tool_a",
                &[],
                json!({ "private_task": "do not persist this" }),
                json!([]),
            )],
            None,
        );
        let mut rehydrated_attempts = Vec::new();
        let mut snapshots = Vec::new();
        let mut generation = 1;
        let mut predecessor = None;
        let outcome = {
            let mut dispatcher = |dispatched: &Value| -> Result<Value, String> {
                let report = report_for(
                    dispatched,
                    vec![ok_result(
                        "a",
                        "tool_a",
                        Some(&json!({ "provider_secret": "do not persist" })),
                    )],
                    None,
                );
                rehydrated_attempts.push(
                    AttemptRecord::delivered(
                        AttemptKind::Full,
                        dispatched.clone(),
                        report.clone(),
                        None,
                        None,
                    )
                    .unwrap(),
                );
                Ok(report)
            };
            drive_mission_with_checkpoint(&grant, mission.clone(), &mut dispatcher, |history| {
                let snapshot =
                    seal_autopilot_checkpoint(&grant, history, generation, predecessor.as_deref())?;
                predecessor = snapshot["snapshot_digest"].as_str().map(str::to_owned);
                generation += 1;
                snapshots.push(snapshot);
                Ok(())
            })
            .unwrap()
        };
        assert_eq!(outcome.final_status, FinalStatus::Succeeded);
        assert_eq!(snapshots.len(), 1);
        let snapshot = snapshots.last().unwrap();
        validate_autopilot_checkpoint(snapshot).unwrap();
        let encoded = retained_values(snapshot);
        assert!(!encoded.contains("private_task"));
        assert!(!encoded.contains("provider_secret"));
        assert!(!encoded.contains("arguments"));
        assert_eq!(snapshot["attempts_used"], json!(1));

        let mut redispatches = 0;
        let resumed = resume_mission_with_checkpoint(
            &grant,
            snapshot,
            mission,
            rehydrated_attempts,
            &mut |_dispatched: &Value| {
                redispatches += 1;
                Err("resume must not replay a completed attempt".into())
            },
            |_history| Ok(()),
        )
        .unwrap();
        assert_eq!(resumed.final_status, FinalStatus::Succeeded);
        assert_eq!(redispatches, 0);
    }

    #[test]
    fn checkpoint_store_is_canonical_and_stale_writers_are_rejected() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 2,
            "require_reconciliation_complete": false,
        }));
        let mission = mission_of(
            vec![step(
                "a",
                "tool_a",
                &[],
                json!({ "private": true }),
                json!([]),
            )],
            None,
        );
        let attempt_mission = mission.clone();
        let report = report_for(
            &attempt_mission,
            vec![ok_result("a", "tool_a", Some(&json!({ "result": true })))],
            None,
        );
        let attempt =
            AttemptRecord::delivered(AttemptKind::Full, attempt_mission, report, None, None)
                .unwrap();
        let history = DriveHistory::from_attempts(mission, vec![attempt]).unwrap();
        let snapshot = seal_autopilot_checkpoint(&grant, &history, 1, None).unwrap();
        let shared = SharedStore::default();
        let mut stale = TransactionalAutopilotCheckpointPersistenceCoordinator::new(
            TransactionalJsonAutopilotCheckpointPersistence::new(shared.clone()),
        );
        assert_eq!(stale.restore().unwrap(), None);
        let mut writer = TransactionalAutopilotCheckpointPersistenceCoordinator::new(
            TransactionalJsonAutopilotCheckpointPersistence::new(shared.clone()),
        );
        assert_eq!(writer.restore().unwrap(), None);
        let stored = writer.flush(&snapshot).unwrap();
        assert_eq!(stored, snapshot);

        let mut reader = JsonAutopilotCheckpointPersistence::new(shared.clone());
        assert_eq!(reader.read_snapshot().unwrap(), Some(snapshot.clone()));

        let conflict = stale.flush(&snapshot).unwrap_err();
        assert_eq!(conflict, AutopilotError::CompareAndSwapConflict);
    }

    #[test]
    fn checkpoint_tampering_is_refused_before_rehydration() {
        let grant = default_grant(&["tool_a"]);
        let mission = mission_of(vec![step("a", "tool_a", &[], json!({}), json!([]))], None);
        let history = DriveHistory::new(mission).unwrap();
        let mut checkpoint = seal_autopilot_checkpoint(&grant, &history, 1, None).unwrap();
        checkpoint["attempts_used"] = json!(1);
        let error = validate_autopilot_checkpoint(&checkpoint).unwrap_err();
        assert!(matches!(error, AutopilotError::InvalidCheckpoint { .. }));
    }

    #[test]
    fn checkpoint_projection_is_domain_neutral_across_all_builtin_domains() {
        let domains = [
            "coding",
            "browser",
            "data",
            "science",
            "biomedical",
            "neuroscience",
            "operations",
            "enterprise",
            "multi_agent",
            "multimodal",
            "cross_domain",
            "evaluation",
        ];
        let grant = grant_of(json!({
            "allowed_tools": ["domain_tool"],
            "max_attempts": 2,
            "require_reconciliation_complete": false,
        }));
        let mission = mission_of(
            domains
                .iter()
                .enumerate()
                .map(|(index, domain)| {
                    step_in_domain(
                        domain,
                        &format!("step-{index}"),
                        "domain_tool",
                        &[],
                        json!({ "domain_private_input": domain }),
                        json!([]),
                    )
                })
                .collect(),
            None,
        );
        let request: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
        let results = request
            .steps
            .iter()
            .map(|step| ok_result(&step.id, &step.tool, Some(&json!({ "ok": true }))))
            .collect();
        let attempt = AttemptRecord::delivered(
            AttemptKind::Full,
            mission.clone(),
            report_for(&mission, results, None),
            None,
            None,
        )
        .unwrap();
        let history = DriveHistory::from_attempts(mission, vec![attempt]).unwrap();
        let checkpoint = seal_autopilot_checkpoint(&grant, &history, 1, None).unwrap();
        validate_autopilot_checkpoint(&checkpoint).unwrap();
        assert_eq!(checkpoint["base_step_count"], json!(domains.len()));
        assert_eq!(
            checkpoint["attempts"][0]["step_count"],
            json!(domains.len())
        );
        let encoded = retained_values(&checkpoint);
        for domain in domains {
            assert!(
                !encoded.contains(domain),
                "raw domain material leaked: {domain}"
            );
        }
    }
}

/// Claims about a *whole drive's* receipt chain, and about the difference one grant knob makes
/// on one fixed instantiation.
///
/// The claims elsewhere in this file read a single planner decision or a two-attempt drive. These
/// read every attempt of a drive that runs to its budget, and read the two drives a single
/// `retry_unknown` flip produces from byte-identical inputs — the axes a one-shot test is blind
/// to. Each digest is recomputed here from the artifact the report names it for, so a digest
/// taken over the wrong value fails rather than merely disagreeing with itself.
mod receipt_chain_depth {
    use super::*;

    /// Four steps whose recorded outcomes reproduce what the in-process executor actually
    /// produces for a mission with one failing step: the failure classifies `unknown` (a tool
    /// error envelope declaring no 40.36 decision), its dependent is blocked, and so is the
    /// independent sibling — the executor aborts the remainder of the mission once a required
    /// step fails, so "blocked" covers more than "my prerequisite failed".
    fn four_step_mission() -> Value {
        mission_of(
            vec![
                step("a", "tool_a", &[], json!({}), json!([])),
                step("b", "tool_b", &["a"], json!({}), json!([])),
                step("c", "tool_c", &["b"], json!({}), json!([])),
                step("d", "tool_a", &["a"], json!({}), json!([])),
            ],
            Some(binding_for(&["a", "b", "c", "d"])),
        )
    }

    fn instantiation_of(mission: Value) -> Value {
        json!({
            "ok": true,
            "workflow": "domain_workflow_instantiate",
            "mission": mission,
        })
    }

    fn unknown_failure_results(mission: &Value) -> Vec<MissionStepResult> {
        let dispatched: Vec<String> = mission["steps"]
            .as_array()
            .expect("steps array")
            .iter()
            .map(|step| step["id"].as_str().expect("step id").to_string())
            .collect();
        dispatched
            .iter()
            .map(|id| match id.as_str() {
                "a" => ok_result("a", "tool_a", Some(&json!({ "value": 7 }))),
                "b" => refused_tool("b", "tool_b", "the named document is not present"),
                "c" => blocked_result("c", "tool_c"),
                other => blocked_result(other, "tool_a"),
            })
            .collect()
    }

    /// The reconciliation the mission boundary attaches to an attempt whose evidence plan was not
    /// satisfied: present and digested, but neither complete nor integrity-valid. Attaching it is
    /// what makes the attempt's `reconciliation_scope` meaningful — the scope of a record that
    /// does not exist is `null`, which says nothing about what a repair may claim.
    fn failed_reconciliation() -> Value {
        json!({
            "present": true,
            "reconciliation_digest": "b".repeat(64),
            "completion": { "status": "failed" },
            "integrity": { "valid": false },
        })
    }

    /// Drive the four-step mission under `grant`, recording every mission document dispatched.
    fn drive_with_one_unknown_failure(grant: &AutonomyGrant) -> (Value, FinalStatus, Vec<Value>) {
        let mut dispatched: Vec<Value> = Vec::new();
        let outcome = {
            let mut dispatcher = |mission: &Value| -> Result<Value, String> {
                dispatched.push(mission.clone());
                let results = unknown_failure_results(mission);
                let mut report = report_for(mission, results, None);
                report["workflow_reconciliation"] = failed_reconciliation();
                Ok(report)
            };
            drive_instantiation(
                grant,
                &instantiation_of(four_step_mission()),
                &mut dispatcher,
            )
            .expect("the drive completes")
        };
        (outcome.report, outcome.final_status, dispatched)
    }

    fn grant_with_unknown_retries(retry_unknown: bool, max_attempts: usize) -> AutonomyGrant {
        grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": max_attempts,
            "retry": {
                "retry_retryable_as_is": true,
                "retry_retryable_after_change": false,
                "retry_unknown": retry_unknown,
            },
        }))
    }

    #[test]
    fn the_report_grant_digest_recomputes_from_the_grant_document_the_report_embeds() {
        let grant = grant_with_unknown_retries(true, 3);
        let (report, ..) = drive_with_one_unknown_failure(&grant);
        let embedded = report
            .get("grant")
            .expect("the report carries the grant it was driven under");
        assert_eq!(
            ContentHash::of_value(embedded).unwrap().to_string(),
            report["grant_digest"].as_str().expect("grant digest"),
            "the grant digest must recompute from the grant document the report carries, or a \
             reader cannot check which authority the drive ran under"
        );
        assert_eq!(
            report["grant_digest"],
            json!(grant.digest().unwrap()),
            "the report's grant digest must be the digest of the grant the caller supplied"
        );
    }

    #[test]
    fn every_attempt_mission_digest_recomputes_from_the_mission_that_attempt_dispatched() {
        let grant = grant_with_unknown_retries(true, 4);
        let (report, _, dispatched) = drive_with_one_unknown_failure(&grant);
        let attempts = report["attempts"].as_array().expect("attempts array");
        assert_eq!(
            attempts.len(),
            dispatched.len(),
            "the report must carry one row per dispatch actually performed"
        );
        assert!(
            attempts.len() >= 2,
            "this claim is only meaningful once a repair has been dispatched"
        );
        for (index, attempt) in attempts.iter().enumerate() {
            assert_eq!(
                attempt["mission_digest"],
                json!(ContentHash::of_value(&dispatched[index])
                    .unwrap()
                    .to_string()),
                "attempt {}'s mission digest must recompute from the mission it dispatched",
                index + 1
            );
            let step_ids: Vec<Value> = dispatched[index]["steps"]
                .as_array()
                .expect("steps array")
                .iter()
                .map(|step| step["id"].clone())
                .collect();
            assert_eq!(
                attempt["dispatched_step_ids"],
                Value::Array(step_ids),
                "attempt {}'s recorded step ids must be the ids of the mission it dispatched",
                index + 1
            );
            assert!(
                attempt["report_digest"].as_str().is_some_and(|digest| {
                    digest.len() == 64 && ContentHash::parse(digest).is_ok()
                }),
                "a delivered attempt must carry a well-formed report digest"
            );
        }
        assert_eq!(
            report["base_mission_digest"],
            json!(ContentHash::of_value(&four_step_mission())
                .unwrap()
                .to_string()),
            "the base mission digest must recompute from the instantiation's own mission, \
             unmodified by the grant's policy overwrite"
        );
    }

    #[test]
    fn the_previewed_first_mission_is_byte_identical_to_the_mission_attempt_one_dispatches() {
        let grant = grant_with_unknown_retries(false, 3);
        let preview = match preview_first_action(&grant, four_step_mission()).unwrap() {
            NextAction::DispatchFull { mission, .. } => mission,
            other => panic!("a no-dispatch preview must plan a full dispatch, got {other:?}"),
        };
        let (report, _, dispatched) = drive_with_one_unknown_failure(&grant);
        assert_eq!(
            dispatched[0], preview,
            "a preview that showed a different mission than the drive sends would be a preview \
             of nothing"
        );
        assert_eq!(
            report["attempts"][0]["mission_digest"],
            json!(ContentHash::of_value(&preview).unwrap().to_string()),
            "the digest attempt 1 records must be the digest of the previewable mission"
        );
    }

    #[test]
    fn retry_unknown_changes_only_the_documented_difference_on_one_failing_instantiation() {
        let (refusing, refusing_status, refusing_dispatches) =
            drive_with_one_unknown_failure(&grant_with_unknown_retries(false, 3));
        let (retrying, retrying_status, retrying_dispatches) =
            drive_with_one_unknown_failure(&grant_with_unknown_retries(true, 3));

        assert_eq!(refusing_status, FinalStatus::Exhausted);
        assert_eq!(retrying_status, FinalStatus::Exhausted);

        assert_eq!(
            refusing_dispatches.len(),
            1,
            "an undeclared failure is not re-dispatched without explicit authority"
        );
        assert_eq!(
            refusing["accounting"]["reason"],
            json!("unresolved_steps_not_retryable"),
            "the refusing drive must stop because the grant authorises no repair, not because \
             it ran out of budget"
        );
        let excluded = refusing["accounting"]["unresolved_steps"]
            .as_array()
            .expect("unresolved rows")
            .iter()
            .find(|row| row["step_id"] == json!("b"))
            .expect("the failing step is accounted for");
        assert_eq!(
            excluded["exclusion"],
            json!("retry of class `unknown` is not authorised by the grant"),
            "the exclusion must name the class the grant declined, not merely report a stop"
        );

        assert_eq!(
            retrying_dispatches.len(),
            3,
            "authorising unknown retries spends the whole budget on a failure that persists"
        );
        assert_eq!(
            retrying["accounting"]["reason"],
            json!("attempt_budget_exhausted"),
            "the retrying drive must stop on the budget, which is the documented difference"
        );

        assert_eq!(
            refusing["grant"]["retry"]["retry_unknown"],
            json!(false),
            "the two reports must differ in the flag under test"
        );
        assert_eq!(retrying["grant"]["retry"]["retry_unknown"], json!(true));
        for field in ["base_mission_id", "base_mission_digest"] {
            assert_eq!(
                refusing[field], retrying[field],
                "{field} must be identical: only the grant differed between these drives"
            );
        }
        assert_eq!(
            refusing["attempts"][0]["mission_digest"], retrying["attempts"][0]["mission_digest"],
            "attempt 1 dispatches the same mission under both grants: retry authority is read \
             only after a failure is recorded"
        );
    }

    #[test]
    fn no_repair_in_a_budget_exhausting_drive_ever_re_dispatches_a_succeeded_step() {
        let grant = grant_with_unknown_retries(true, 5);
        let (report, status, dispatched) = drive_with_one_unknown_failure(&grant);
        assert_eq!(status, FinalStatus::Exhausted);
        assert_eq!(
            report["totals"]["attempts_used"],
            json!(5),
            "a persistent failure must consume exactly the authorised budget"
        );

        let mut succeeded: Vec<String> = Vec::new();
        for (index, attempt) in report["attempts"]
            .as_array()
            .expect("attempts array")
            .iter()
            .enumerate()
        {
            let redispatched: Vec<String> = attempt["dispatched_step_ids"]
                .as_array()
                .expect("dispatched ids")
                .iter()
                .map(|id| id.as_str().expect("step id").to_string())
                .collect();
            if attempt["kind"] == json!("repair") {
                for step_id in &succeeded {
                    assert!(
                        !redispatched.contains(step_id),
                        "repair attempt {} re-dispatched `{step_id}`, which had already \
                         succeeded; re-running a succeeded step re-runs its side effects",
                        index + 1
                    );
                }
                assert_eq!(
                    redispatched,
                    vec!["b".to_string(), "c".to_string(), "d".to_string()],
                    "a repair must carry exactly the unresolved steps: the failure, its \
                     dependent, and the sibling the executor never dispatched"
                );
                assert_eq!(
                    attempt["reconciliation_scope"],
                    json!("repair_subset"),
                    "a repair's reconciliation may only claim the subset it re-dispatched"
                );
            } else {
                assert_eq!(
                    attempt["reconciliation_scope"],
                    json!("full_plan"),
                    "a full dispatch's reconciliation covers the whole plan"
                );
            }
            assert_eq!(
                attempt["reconciliation_status"],
                json!({ "completion": "failed", "integrity_valid": false }),
                "attempt {}'s reconciliation status must be the one the boundary recorded, not \
                 a repaired reading of it",
                index + 1
            );
            for step_id in &redispatched {
                let classified = attempt["classification_table"]
                    .as_array()
                    .expect("classification table")
                    .iter()
                    .any(|row| row["step_id"] == json!(step_id));
                assert!(
                    classified,
                    "attempt {} dispatched `{step_id}` without classifying it",
                    index + 1
                );
            }
            assert_eq!(
                attempt["classification_table"]
                    .as_array()
                    .expect("classification table")
                    .len(),
                redispatched.len(),
                "attempt {}'s classification table must cover its dispatch exactly",
                index + 1
            );
            for row in attempt["classification_table"]
                .as_array()
                .expect("classification table")
            {
                if row["class"] == json!("succeeded") {
                    succeeded.push(row["step_id"].as_str().expect("step id").to_string());
                }
            }
        }
        assert!(
            succeeded.contains(&"a".to_string()),
            "the fixture must actually produce a succeeded step, or this claim is vacuous"
        );
        assert!(
            !succeeded.contains(&"b".to_string()),
            "the fixture must keep its failure failing across every attempt, or the budget \
             would never be reached"
        );

        let repairs = dispatched
            .iter()
            .filter(|mission| mission["steps"].as_array().expect("steps").len() == 3)
            .count();
        assert_eq!(
            repairs, 4,
            "every attempt after the first must be a three-step repair"
        );
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["valid"], json!(true), "{verification}");
    }

    #[test]
    fn a_repair_mission_narrows_its_evidence_plan_to_the_steps_it_re_dispatches() {
        let grant = grant_with_unknown_retries(true, 2);
        let (report, _, dispatched) = drive_with_one_unknown_failure(&grant);
        let repair = &dispatched[1];
        let plan_steps: Vec<&str> = repair["workflow_binding"]["evidence_plan"]["steps"]
            .as_array()
            .expect("the repair keeps an evidence plan")
            .iter()
            .map(|entry| entry["step_id"].as_str().expect("step id"))
            .collect();
        assert_eq!(
            plan_steps,
            vec!["b", "c", "d"],
            "a repair's evidence plan must cover exactly the steps it re-dispatches, so its \
             reconciliation is honestly scoped"
        );
        assert_eq!(
            repair["workflow_binding"]["evidence_plan_digest"],
            json!(
                ContentHash::of_value(&repair["workflow_binding"]["evidence_plan"])
                    .unwrap()
                    .to_string()
            ),
            "the narrowed plan's digest must recompute from the narrowed plan"
        );
        assert_eq!(
            report["attempts"][1]["mission_digest"],
            json!(ContentHash::of_value(repair).unwrap().to_string())
        );
    }
}
