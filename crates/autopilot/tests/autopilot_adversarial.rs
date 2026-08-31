//! Adversarial coverage of the drive: attacks on the grant's authority, the repair subset's
//! boundary, the attempt budget, and the fidelity of the receipts a drive publishes.
//!
//! The existing claim suite demonstrates the kernel on well-formed inputs. This suite assumes the
//! opposite: a mission authored to widen its own authority, a nested tool whose payload is chosen
//! to poison the repair it triggers, and a dispatcher that returns reports which do not describe
//! what it was asked to run. Every test states the property the attack must fail to break, and
//! every dispatch-refusal test counts dispatches so that "refused" cannot quietly mean "refused
//! after the side effects ran".

use bioprism_autopilot::{
    build_autopilot_report, classify_step_result, drive_mission, plan_next_action,
    verify_autopilot_report, AttemptKind, AttemptRecord, AutonomyGrant, AutopilotError,
    DriveHistory, DriveOutcome, FinalDisposition, FinalStatus, NextAction, RetryClass, StepClass,
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

fn grant(tools: &[&str]) -> AutonomyGrant {
    grant_of(json!({ "allowed_tools": tools, "max_attempts": 4 }))
}

fn step(id: &str, tool: &str, deps: &[&str]) -> Value {
    json!({
        "id": id,
        "domain": "metrics",
        "capability": "analytics",
        "objective": format!("run {id}"),
        "tool": tool,
        "arguments": {},
        "depends_on": deps,
        "bindings": [],
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
    let zeros = "0".repeat(64);
    json!({
        "workflow_id": "wf-adversarial",
        "workflow_digest": zeros,
        "catalog_digest": zeros,
        "domain_contract_digest": zeros,
        "domain_contract": {},
        "evidence_plan": plan,
        "evidence_plan_digest": digest,
    })
}

fn mission_of(steps: Vec<Value>, binding: Option<Value>) -> Value {
    let mut mission = json!({
        "mission_id": "m-adversarial",
        "goal": "drive the workflow",
        "steps": steps,
    });
    if let Some(binding) = binding {
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

fn declared(class: &str) -> String {
    json!({ "retryability": class }).to_string()
}

fn succeeded(id: &str, tool: &str, payload: Value) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "succeeded".into(),
        required: true,
        arguments_digest: Some("1".repeat(64)),
        bytes: 10,
        wire: Some(ok_envelope(&payload)),
        error: None,
    }
}

fn failed_with(id: &str, tool: &str, class: &str) -> MissionStepResult {
    let text = declared(class);
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "refused".into(),
        required: true,
        arguments_digest: Some("2".repeat(64)),
        bytes: 20,
        wire: Some(json!({
            "jsonrpc": "2.0",
            "id": "step",
            "result": { "isError": true, "content": [ { "type": "text", "text": text } ] }
        })),
        error: Some(text),
    }
}

fn failed_undeclared(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "refused".into(),
        required: true,
        arguments_digest: Some("3".repeat(64)),
        bytes: 20,
        wire: Some(json!({
            "jsonrpc": "2.0",
            "id": "step",
            "result": { "isError": true, "content": [ { "type": "text", "text": "the domain tool failed" } ] }
        })),
        error: Some("the domain tool failed".into()),
    }
}

fn blocked(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "blocked".into(),
        required: true,
        arguments_digest: None,
        bytes: 0,
        wire: None,
        error: Some("a prerequisite refused".into()),
    }
}

fn cancelled(id: &str, tool: &str) -> MissionStepResult {
    MissionStepResult {
        id: id.into(),
        tool: tool.into(),
        status: "cancelled".into(),
        required: true,
        arguments_digest: None,
        bytes: 0,
        wire: None,
        error: Some("the operator cancelled the mission".into()),
    }
}

fn report_for(mission: &Value, results: Vec<MissionStepResult>, status: Option<&str>) -> Value {
    let request: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
    let plan = plan_mission(&request).unwrap();
    let succeeded_count = results.iter().filter(|r| r.status == "succeeded").count();
    let refused = results.iter().filter(|r| r.status == "refused").count();
    let blocked_count = results.iter().filter(|r| r.status == "blocked").count();
    let cancelled_count = results.iter().filter(|r| r.status == "cancelled").count();
    let required_failures = results
        .iter()
        .filter(|r| r.required && r.status != "succeeded")
        .count();
    let mission_status = status.unwrap_or(if required_failures > 0 {
        "failed"
    } else if refused + blocked_count + cancelled_count > 0 {
        "partial"
    } else {
        "succeeded"
    });
    let report = MissionReport {
        schema_version: MISSION_SCHEMA_VERSION.into(),
        plan,
        execution: "executed".into(),
        mission_status: mission_status.into(),
        succeeded: succeeded_count,
        refused,
        blocked: blocked_count,
        cancelled: cancelled_count,
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

/// A dispatcher that records every mission handed to it, so a test can prove a refusal happened
/// *before* any side effect rather than after one.
struct RecordingDispatcher {
    dispatched: Vec<Value>,
    reports: Vec<Value>,
}

impl RecordingDispatcher {
    fn new(reports: Vec<Value>) -> Self {
        RecordingDispatcher {
            dispatched: Vec::new(),
            reports,
        }
    }

    fn refusing() -> Self {
        RecordingDispatcher::new(Vec::new())
    }

    fn calls(&self) -> usize {
        self.dispatched.len()
    }
}

impl bioprism_autopilot::MissionDispatch for RecordingDispatcher {
    fn dispatch(&mut self, mission: &Value) -> Result<Value, String> {
        self.dispatched.push(mission.clone());
        if self.reports.is_empty() {
            return Err("this dispatcher was never supposed to be called".into());
        }
        Ok(self.reports.remove(0))
    }
}

fn push_attempt(
    history: &mut DriveHistory,
    kind: AttemptKind,
    mission: Value,
    results: Vec<MissionStepResult>,
    status: Option<&str>,
    reconciliation: Option<Value>,
) {
    let report = report_for(&mission, results, status);
    history.push(AttemptRecord::delivered(kind, mission, report, reconciliation, None).unwrap());
}

fn first_full_mission(grant: &AutonomyGrant, history: &DriveHistory) -> Value {
    match plan_next_action(grant, history).unwrap() {
        NextAction::DispatchFull { mission, .. } => mission,
        other => panic!("expected a full dispatch, got {other:?}"),
    }
}

fn expect_repair(grant: &AutonomyGrant, history: &DriveHistory) -> (Value, Vec<String>) {
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

fn report_of(outcome: &DriveOutcome) -> &Value {
    &outcome.report
}

mod authority_through_the_repair_path {
    use super::*;

    /// A mission whose second step reads a slot out of the first step's payload. Attempt 1 gives
    /// the first step a payload the attacker controls, which is the only value a repair inlines.
    fn poisoned_binding_mission(payload_slot: Value) -> (Value, MissionStepResult) {
        let mut source = step("source", "tool_a", &[]);
        let mut sink = step("sink", "tool_b", &["source"]);
        sink["arguments"] = json!({ "options": null });
        sink["bindings"] = json!([{
            "from_step": "source",
            "source_pointer": "/options",
            "target_pointer": "/options",
        }]);
        source["arguments"] = json!({});
        let mission = mission_of(vec![source, sink], Some(binding_for(&["source", "sink"])));
        let payload = succeeded("source", "tool_a", json!({ "options": payload_slot }));
        (mission, payload)
    }

    #[test]
    fn a_rematerialized_binding_carrying_a_confirmation_flag_is_refused_not_dispatched() {
        let grant = grant(&["tool_a", "tool_b"]);
        let (mission, poisoned) = poisoned_binding_mission(json!({ "confirm": true }));
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![poisoned, failed_with("sink", "tool_b", "retryable_as_is")],
            None,
            Some(complete_reconciliation()),
        );

        let error = plan_next_action(&grant, &history)
            .expect_err("a confirmation inlined from a tool payload must not be dispatched");
        assert!(
            matches!(error, AutopilotError::GrantDoesNotAuthorise { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_confirmation_hidden_deep_in_a_rematerialized_payload_is_still_refused() {
        let grant = grant(&["tool_a", "tool_b"]);
        let (mission, poisoned) =
            poisoned_binding_mission(json!({ "batch": [{ "steps": [{ "confirm": true }] }] }));
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![poisoned, failed_with("sink", "tool_b", "retryable_as_is")],
            None,
            Some(complete_reconciliation()),
        );

        let error = plan_next_action(&grant, &history)
            .expect_err("nesting the flag inside arrays must not evade the side-effect posture");
        assert!(
            matches!(error, AutopilotError::GrantDoesNotAuthorise { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_grant_that_allows_side_effects_accepts_the_same_rematerialized_confirmation() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b"],
            "max_attempts": 4,
            "allow_side_effects": true,
        }));
        let (mission, poisoned) = poisoned_binding_mission(json!({ "confirm": true }));
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![poisoned, failed_with("sink", "tool_b", "retryable_as_is")],
            None,
            Some(complete_reconciliation()),
        );

        let (repair, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["sink".to_string()]);
        assert_eq!(
            repair["steps"][0]["arguments"]["options"]["confirm"],
            json!(true),
            "the refusal must come from the grant's posture, not from the flag being dropped"
        );
    }

    #[test]
    fn a_mission_authored_wider_than_its_grant_is_refused_before_any_dispatch() {
        let mut mission = mission_of(
            vec![step("one", "tool_a", &[]), step("two", "tool_evil", &[])],
            Some(binding_for(&["one", "two"])),
        );
        mission["policy"] = json!({
            "execute": true,
            "allowed_tools": ["tool_a", "tool_evil"],
        });
        let grant = grant(&["tool_a"]);
        let mut dispatcher = RecordingDispatcher::refusing();
        let error = drive_mission(&grant, mission, &mut dispatcher)
            .expect_err("the grant narrows the mission, never the reverse");
        assert!(
            matches!(error, AutopilotError::GrantDoesNotAuthorise { .. }),
            "{error:?}"
        );
        assert_eq!(
            dispatcher.calls(),
            0,
            "an under-scoped grant must refuse before a single tool runs"
        );
    }

    #[test]
    fn a_mission_authored_with_side_effect_permission_cannot_keep_it_past_the_grant() {
        let mut confirming = step("one", "tool_a", &[]);
        confirming["arguments"] = json!({ "confirm": true });
        let mut mission = mission_of(vec![confirming], Some(binding_for(&["one"])));
        mission["policy"] = json!({
            "execute": true,
            "allow_side_effects": true,
            "allowed_tools": ["tool_a"],
        });
        let grant = grant(&["tool_a"]);
        let mut dispatcher = RecordingDispatcher::refusing();
        let error = drive_mission(&grant, mission, &mut dispatcher)
            .expect_err("the grant's side-effect posture overwrites the mission's");
        assert!(
            matches!(error, AutopilotError::GrantDoesNotAuthorise { .. }),
            "{error:?}"
        );
        assert_eq!(dispatcher.calls(), 0);
    }

    #[test]
    fn every_repair_step_keeps_the_tool_its_base_step_declared() {
        let grant = grant(&["tool_a", "tool_b", "tool_c"]);
        let mission = mission_of(
            vec![
                step("a", "tool_a", &[]),
                step("b", "tool_b", &["a"]),
                step("c", "tool_c", &["b"]),
            ],
            Some(binding_for(&["a", "b", "c"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                succeeded("a", "tool_a", json!({})),
                failed_with("b", "tool_b", "retryable_as_is"),
                blocked("c", "tool_c"),
            ],
            None,
            Some(complete_reconciliation()),
        );

        let (repair, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["b".to_string(), "c".to_string()]);
        let tools = repair["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["tool"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(tools, vec!["tool_b".to_string(), "tool_c".to_string()]);
        assert_eq!(
            repair["policy"]["allowed_tools"],
            json!(["tool_a", "tool_b", "tool_c"]),
            "the repair carries the grant's allow-list verbatim, never a wider one"
        );
    }
}

mod recursion_through_the_drive {
    use super::*;

    #[test]
    fn a_base_mission_naming_agent_mission_is_refused_before_the_first_dispatch() {
        let mission = mission_of(
            vec![step("one", "agent_mission", &[])],
            Some(binding_for(&["one"])),
        );
        let grant = grant(&["tool_a"]);
        let mut dispatcher = RecordingDispatcher::refusing();
        let error = drive_mission(&grant, mission, &mut dispatcher)
            .expect_err("a mission may not invoke the mission tool");
        assert!(
            matches!(error, AutopilotError::InvalidMission { ref reason } if reason.contains("itself")),
            "the refusal must name recursion, not a generic policy failure"
        );
        assert_eq!(dispatcher.calls(), 0);
    }

    #[test]
    fn a_grant_naming_a_case_variant_of_agent_mission_is_refused_at_construction() {
        for disguise in ["Agent_Mission", "AGENT_MISSION", "agent_Mission"] {
            let refused = serde_json::from_value::<AutonomyGrant>(json!({
                "allowed_tools": [disguise],
                "max_attempts": 2,
            }))
            .expect_err("a case variant of the mission tool must not be grantable");
            assert!(
                refused.to_string().contains("cannot allow agent_mission"),
                "`{disguise}` must be refused as recursive: {refused}"
            );
        }
    }

    #[test]
    fn a_case_variant_of_agent_mission_in_a_mission_step_is_refused_before_any_dispatch() {
        let mission = mission_of(
            vec![step("one", "Agent_Mission", &[])],
            Some(binding_for(&["one"])),
        );
        let grant = grant(&["tool_a"]);
        let mut dispatcher = RecordingDispatcher::refusing();
        let error = drive_mission(&grant, mission, &mut dispatcher)
            .expect_err("ASCII case must not launder the mission tool into a step");
        assert!(
            matches!(error, AutopilotError::InvalidMission { ref reason } if reason.contains("itself")),
            "{error:?}"
        );
        assert_eq!(dispatcher.calls(), 0);
    }
}

mod repair_subset_boundary {
    use super::*;

    /// A chain whose middle step the attempt-1 report claims succeeded even though its own
    /// dependency did not. No faithful executor produces this report; the planner must still
    /// refuse to re-dispatch the step the report called succeeded.
    fn chain_with_a_lying_middle() -> (AutonomyGrant, DriveHistory) {
        let grant = grant(&["tool_u", "tool_s", "tool_v"]);
        let mission = mission_of(
            vec![
                step("u", "tool_u", &[]),
                step("s", "tool_s", &["u"]),
                step("v", "tool_v", &["s"]),
            ],
            Some(binding_for(&["u", "s", "v"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                failed_with("u", "tool_u", "retryable_as_is"),
                succeeded("s", "tool_s", json!({})),
                failed_with("v", "tool_v", "retryable_as_is"),
            ],
            None,
            Some(complete_reconciliation()),
        );
        (grant, history)
    }

    #[test]
    fn a_step_that_succeeded_while_its_dependency_failed_is_never_re_dispatched() {
        let (grant, history) = chain_with_a_lying_middle();
        let (repair, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["u".to_string(), "v".to_string()]);
        let ids = repair["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec!["u".to_string(), "v".to_string()],
            "a step the report called succeeded is evidence, not a repair candidate"
        );
    }

    #[test]
    fn a_repair_drops_only_the_dependencies_that_are_outside_the_subset() {
        let (grant, history) = chain_with_a_lying_middle();
        let (repair, _) = expect_repair(&grant, &history);
        let steps = repair["steps"].as_array().unwrap();
        assert_eq!(steps[0]["depends_on"], json!([]));
        assert_eq!(
            steps[1]["depends_on"],
            json!([]),
            "`v` depended only on the excluded `s`, so the repair records no dependency it cannot honour"
        );
    }

    #[test]
    fn a_three_deep_chain_repairs_the_failed_middle_and_its_blocked_dependent_only() {
        let grant = grant(&["tool_a", "tool_b", "tool_c"]);
        let mission = mission_of(
            vec![
                step("a", "tool_a", &[]),
                step("b", "tool_b", &["a"]),
                step("c", "tool_c", &["b"]),
            ],
            Some(binding_for(&["a", "b", "c"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                succeeded("a", "tool_a", json!({ "value": 1 })),
                failed_with("b", "tool_b", "retryable_as_is"),
                blocked("c", "tool_c"),
            ],
            None,
            Some(complete_reconciliation()),
        );

        let (repair, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["b".to_string(), "c".to_string()]);
        let steps = repair["steps"].as_array().unwrap();
        assert_eq!(steps[0]["id"], "b");
        assert_eq!(steps[0]["depends_on"], json!([]));
        assert_eq!(steps[1]["id"], "c");
        assert_eq!(
            steps[1]["depends_on"],
            json!(["b"]),
            "the surviving link between two included steps must be kept, or `c` could run first"
        );
        assert_eq!(
            repair["workflow_binding"]["evidence_plan"]["steps"],
            json!([{ "step_id": "b" }, { "step_id": "c" }]),
            "the subset-scoped reconciliation contract must name exactly the re-dispatched steps"
        );
    }

    #[test]
    fn an_undeclared_error_class_is_excluded_from_the_repair_with_its_reason_recorded() {
        let grant = grant(&["tool_a", "tool_b"]);
        let mission = mission_of(
            vec![step("a", "tool_a", &[]), step("b", "tool_b", &["a"])],
            Some(binding_for(&["a", "b"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![failed_undeclared("a", "tool_a"), blocked("b", "tool_b")],
            None,
            Some(complete_reconciliation()),
        );

        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(accounting["reason"], "unresolved_steps_not_retryable");
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        let row = rows
            .iter()
            .find(|row| row["step_id"] == "a")
            .expect("the undeclared failure must be accounted for");
        assert_eq!(row["state"], "unknown");
        assert_eq!(row["signal"], "undeclared_tool_error");
        assert!(row["exclusion"]
            .as_str()
            .unwrap()
            .contains("not authorised by the grant"));
    }

    #[test]
    fn an_undeclared_error_class_is_repaired_only_under_an_explicit_unknown_authority() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b"],
            "max_attempts": 4,
            "retry": { "retry_unknown": true },
        }));
        let mission = mission_of(
            vec![step("a", "tool_a", &[]), step("b", "tool_b", &["a"])],
            Some(binding_for(&["a", "b"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![failed_undeclared("a", "tool_a"), blocked("b", "tool_b")],
            None,
            Some(complete_reconciliation()),
        );

        let (_, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn a_cancelled_middle_link_excludes_itself_and_its_dependent_with_distinct_reasons() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a", "tool_b", "tool_c"],
            "max_attempts": 4,
            "retry": { "retry_unknown": true },
        }));
        let mission = mission_of(
            vec![
                step("a", "tool_a", &[]),
                step("b", "tool_b", &["a"]),
                step("c", "tool_c", &["b"]),
            ],
            Some(binding_for(&["a", "b", "c"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                succeeded("a", "tool_a", json!({})),
                cancelled("b", "tool_b"),
                blocked("c", "tool_c"),
            ],
            Some("partial"),
            Some(complete_reconciliation()),
        );

        let accounting = expect_exhausted(&grant, &history);
        let rows = accounting["unresolved_steps"].as_array().unwrap();
        let cancelled_row = rows.iter().find(|row| row["step_id"] == "b").unwrap();
        let dependent_row = rows.iter().find(|row| row["step_id"] == "c").unwrap();
        assert!(cancelled_row["exclusion"]
            .as_str()
            .unwrap()
            .contains("cancellation is an authority"));
        assert!(
            dependent_row["exclusion"]
                .as_str()
                .unwrap()
                .contains("depends on `b`"),
            "the dependent must be excluded for depending on an excluded step, not re-classified"
        );
    }

    #[test]
    fn a_binding_source_pointer_resolving_to_null_is_rematerialized_not_called_missing() {
        let grant = grant(&["tool_a", "tool_b"]);
        let mut sink = step("sink", "tool_b", &["source"]);
        sink["arguments"] = json!({ "slot": "placeholder" });
        sink["bindings"] = json!([{
            "from_step": "source",
            "source_pointer": "/value",
            "target_pointer": "/slot",
        }]);
        let mission = mission_of(
            vec![step("source", "tool_a", &[]), sink],
            Some(binding_for(&["source", "sink"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                succeeded("source", "tool_a", json!({ "value": null })),
                failed_with("sink", "tool_b", "retryable_as_is"),
            ],
            None,
            Some(complete_reconciliation()),
        );

        let (repair, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["sink".to_string()]);
        assert_eq!(
            repair["steps"][0]["arguments"]["slot"],
            Value::Null,
            "a retained null is a value the upstream step produced, not a missing pointer"
        );
        assert_eq!(
            repair["steps"][0]["bindings"],
            json!([]),
            "an inlined binding must not also remain as an edge to a step outside the subset"
        );
    }

    #[test]
    fn a_repair_never_includes_a_step_whose_binding_source_lost_its_payload() {
        let grant = grant(&["tool_a", "tool_b"]);
        let mut sink = step("sink", "tool_b", &["source"]);
        sink["arguments"] = json!({ "slot": null });
        sink["bindings"] = json!([{
            "from_step": "source",
            "source_pointer": "/value",
            "target_pointer": "/slot",
        }]);
        let mission = mission_of(
            vec![step("source", "tool_a", &[]), sink],
            Some(binding_for(&["source", "sink"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        let mut dropped = succeeded("source", "tool_a", json!({ "value": 1 }));
        dropped.wire = None;
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![dropped, failed_with("sink", "tool_b", "retryable_as_is")],
            None,
            Some(complete_reconciliation()),
        );

        let accounting = expect_exhausted(&grant, &history);
        let row = accounting["unresolved_steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["step_id"] == "sink")
            .unwrap();
        assert!(row["exclusion"]
            .as_str()
            .unwrap()
            .contains("output was not retained"));
    }
}

mod budgets_across_a_repair {
    use super::*;

    fn budgeted_mission(mode: &str, per_step: usize, total: usize) -> Value {
        let mut mission = mission_of(
            vec![
                step("u", "tool_u", &[]),
                step("s", "tool_s", &["u"]),
                step("v", "tool_v", &["s"]),
            ],
            Some(binding_for(&["u", "s", "v"])),
        );
        mission["policy"] = json!({
            "execution_mode": mode,
            "max_step_output_bytes": per_step,
            "max_total_output_bytes": total,
        });
        mission
    }

    fn history_with_a_widening_repair(grant: &AutonomyGrant, mission: Value) -> DriveHistory {
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                failed_with("u", "tool_u", "retryable_as_is"),
                succeeded("s", "tool_s", json!({})),
                failed_with("v", "tool_v", "retryable_as_is"),
            ],
            None,
            Some(complete_reconciliation()),
        );
        history
    }

    #[test]
    fn a_repair_inherits_the_base_missions_output_budgets_verbatim() {
        let grant = grant(&["tool_u", "tool_s", "tool_v"]);
        let mission = budgeted_mission("serial", 1_000, 4_000);
        let history = history_with_a_widening_repair(&grant, mission);
        let (repair, _) = expect_repair(&grant, &history);
        assert_eq!(repair["policy"]["max_step_output_bytes"], json!(1_000));
        assert_eq!(repair["policy"]["max_total_output_bytes"], json!(4_000));
        assert_eq!(
            repair["policy"]["execution_mode"],
            json!("serial"),
            "a repair may not quietly switch execution mode and change what gets reserved"
        );
    }

    #[test]
    fn a_repair_that_would_widen_a_parallel_wave_is_refused_rather_than_dispatched_unreserved() {
        let grant = grant(&["tool_u", "tool_s", "tool_v"]);
        let mission = budgeted_mission("parallel_waves", 6_000_000, 10_000_000);
        let base: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
        let base_plan =
            plan_mission(&base).expect("a serialized chain reserves one step at a time");
        assert!(
            base_plan.waves.iter().all(|wave| wave.len() == 1),
            "the base mission is a chain, so its worst-case reservation is one step"
        );

        let history = history_with_a_widening_repair(&grant, mission);
        let error = plan_next_action(&grant, &history)
            .expect_err("dropping the dependency on a succeeded step widens the repair's wave");
        assert!(
            matches!(error, AutopilotError::InvalidMission { ref reason } if reason.contains("worst-case wave")),
            "the repair must be re-checked against the same reservation rule as the base: {error:?}"
        );
    }

    /// Characterises an unfixed consequence rather than a guarantee. A repair the planner refuses
    /// mid-drive leaves `plan_next_action` returning an error, and the drive loop propagates it
    /// instead of stopping with an accounting, so `build_autopilot_report` never runs and the
    /// receipts of every attempt already made are dropped. The authority decision is right — the
    /// unreserved repair is not dispatched — but a drive that spent a dispatch should still be
    /// able to say what it spent it on. Closing that needs a stop variant the planner does not
    /// have, so this test pins the current behaviour and will fail the moment it is fixed.
    #[test]
    fn a_planner_refusal_mid_drive_discards_the_receipts_of_the_attempts_already_made() {
        let grant = grant(&["tool_u", "tool_s", "tool_v"]);
        let mission = budgeted_mission("parallel_waves", 6_000_000, 10_000_000);
        let planned = {
            let history = DriveHistory::new(mission.clone()).unwrap();
            first_full_mission(&grant, &history)
        };
        let report = report_for(
            &planned,
            vec![
                failed_with("u", "tool_u", "retryable_as_is"),
                succeeded("s", "tool_s", json!({})),
                failed_with("v", "tool_v", "retryable_as_is"),
            ],
            None,
        );
        let mut dispatcher = RecordingDispatcher::new(vec![report]);
        let error = drive_mission(&grant, mission, &mut dispatcher)
            .expect_err("the widened repair is refused rather than dispatched");
        assert!(
            matches!(error, AutopilotError::InvalidMission { ref reason } if reason.contains("worst-case wave")),
            "{error:?}"
        );
        assert_eq!(
            dispatcher.calls(),
            1,
            "one attempt ran, so one attempt's receipts existed and are now unreachable"
        );
    }

    #[test]
    fn a_repair_whose_widened_wave_still_fits_the_reservation_is_dispatched() {
        let grant = grant(&["tool_u", "tool_s", "tool_v"]);
        let mission = budgeted_mission("parallel_waves", 4_000_000, 10_000_000);
        let history = history_with_a_widening_repair(&grant, mission);
        let (repair, included) = expect_repair(&grant, &history);
        assert_eq!(included, vec!["u".to_string(), "v".to_string()]);
        let request: MissionRequest = serde_json::from_value(repair).unwrap();
        let plan = plan_mission(&request).unwrap();
        assert_eq!(plan.waves, vec![vec!["u".to_string(), "v".to_string()]]);
    }

    #[test]
    fn an_output_budget_refusal_is_terminal_and_ends_the_drive_as_refused() {
        let over_budget = MissionStepResult {
            id: "one".into(),
            tool: "tool_a".into(),
            status: "refused".into(),
            required: true,
            arguments_digest: Some("4".repeat(64)),
            bytes: 3_000_000,
            wire: None,
            error: Some(
                "nested result is 3000000 bytes, above the per-step output budget of 2000000"
                    .into(),
            ),
        };
        let classification = classify_step_result(&over_budget);
        assert_eq!(
            classification.class,
            StepClass::Failed(RetryClass::Terminal),
            "a budget refusal is the executor's own policy, and identical bytes stay over budget"
        );
        assert_eq!(classification.signal, "executor_refusal");

        let grant = grant(&["tool_a"]);
        let mission = mission_of(
            vec![step("one", "tool_a", &[])],
            Some(binding_for(&["one"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![over_budget],
            None,
            Some(complete_reconciliation()),
        );

        match plan_next_action(&grant, &history).unwrap() {
            NextAction::StopRefused {
                first_terminal_refusal,
            } => {
                assert_eq!(first_terminal_refusal["step_id"], "one");
                assert!(first_terminal_refusal["error"]
                    .as_str()
                    .unwrap()
                    .contains("output budget"));
            }
            other => panic!("an over-budget refusal must stop the drive, got {other:?}"),
        }
    }

    #[test]
    fn a_step_whose_retained_bytes_exceed_the_missions_own_budget_cannot_be_reported_as_success() {
        let grant = grant(&["tool_a"]);
        let mut mission = mission_of(
            vec![step("one", "tool_a", &[])],
            Some(binding_for(&["one"])),
        );
        mission["policy"] = json!({
            "max_step_output_bytes": 1_000,
            "max_total_output_bytes": 4_000,
        });
        let mut oversized = succeeded("one", "tool_a", json!({ "value": 1 }));
        oversized.bytes = 2_500;

        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![oversized],
            Some("succeeded"),
            Some(complete_reconciliation()),
        );

        let action = plan_next_action(&grant, &history).unwrap();
        assert!(
            !matches!(action, NextAction::StopSuccess { .. }),
            "a result claiming success while its own byte count breaches the mission's per-step \
             budget is a self-contradictory receipt, and success is never inferred from one"
        );
    }
}

mod exhaustion_and_refusal {
    use super::*;

    #[test]
    fn the_attempt_budget_is_spent_exactly_and_never_by_one_more() {
        for budget in 1..=3usize {
            let grant = grant_of(json!({
                "allowed_tools": ["tool_a"],
                "max_attempts": budget,
                "require_reconciliation_complete": false,
            }));
            let mission = mission_of(vec![step("one", "tool_a", &[])], None);
            let mut history = DriveHistory::new(mission).unwrap();
            for expected in 1..=budget {
                let authorization = match plan_next_action(&grant, &history).unwrap() {
                    NextAction::DispatchFull { authorization, .. } => authorization,
                    NextAction::DispatchRepair { authorization, .. } => authorization,
                    other => panic!("attempt {expected} of {budget} must dispatch, got {other:?}"),
                };
                assert_eq!(authorization.attempt_index(), expected);
                let mission = match plan_next_action(&grant, &history).unwrap() {
                    NextAction::DispatchFull { mission, .. } => mission,
                    NextAction::DispatchRepair { mission, .. } => mission,
                    other => panic!("expected a dispatch, got {other:?}"),
                };
                let kind = if expected == 1 {
                    AttemptKind::Full
                } else {
                    AttemptKind::Repair
                };
                push_attempt(
                    &mut history,
                    kind,
                    mission,
                    vec![failed_with("one", "tool_a", "retryable_as_is")],
                    None,
                    None,
                );
            }
            let accounting = expect_exhausted(&grant, &history);
            assert_eq!(accounting["reason"], "attempt_budget_exhausted");
            assert_eq!(accounting["attempts_used"], json!(budget));
            assert_eq!(accounting["max_attempts"], json!(budget));
        }
    }

    #[test]
    fn a_grant_that_cannot_reach_success_refuses_before_spending_a_dispatch() {
        let mission = mission_of(vec![step("one", "tool_a", &[])], None);
        let grant = grant(&["tool_a"]);
        let mut dispatcher = RecordingDispatcher::refusing();
        let outcome = drive_mission(&grant, mission, &mut dispatcher)
            .expect("an unreachable success rule is an accounted stop, not a crash");
        assert_eq!(outcome.final_status, FinalStatus::Exhausted);
        assert_eq!(
            dispatcher.calls(),
            0,
            "a bindingless mission under a reconciliation-requiring grant must not run"
        );
        assert_eq!(
            report_of(&outcome)["accounting"]["reason"],
            "reconciliation_unavailable"
        );
        assert_eq!(report_of(&outcome)["accounting"]["attempts_used"], json!(0));
    }

    #[test]
    fn an_unrecognised_status_is_unknown_and_is_not_retried_without_authority() {
        let mut odd = succeeded("one", "tool_a", json!({}));
        odd.status = "partially_succeeded".into();
        assert_eq!(
            classify_step_result(&odd).class,
            StepClass::Failed(RetryClass::Unknown)
        );

        let grant = grant(&["tool_a"]);
        let mission = mission_of(
            vec![step("one", "tool_a", &[])],
            Some(binding_for(&["one"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![odd],
            Some("partial"),
            Some(complete_reconciliation()),
        );

        let accounting = expect_exhausted(&grant, &history);
        assert_eq!(accounting["reason"], "unresolved_steps_not_retryable");
    }

    #[test]
    fn authorising_unknown_retries_does_not_make_a_terminal_refusal_retryable() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 4,
            "retry": {
                "retry_unknown": true,
                "retry_retryable_as_is": true,
                "retry_retryable_after_change": true,
            },
        }));
        let mission = mission_of(
            vec![step("one", "tool_a", &[])],
            Some(binding_for(&["one"])),
        );
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![failed_with("one", "tool_a", "terminal")],
            None,
            Some(complete_reconciliation()),
        );

        match plan_next_action(&grant, &history).unwrap() {
            NextAction::StopRefused {
                first_terminal_refusal,
            } => assert_eq!(first_terminal_refusal["step_id"], "one"),
            other => panic!("no retry option may purchase a terminal decision, got {other:?}"),
        }
    }

    #[test]
    fn a_terminal_refusal_stops_the_drive_before_the_budget_is_spent() {
        let grant = grant_of(json!({
            "allowed_tools": ["tool_a"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
        }));
        let mission = mission_of(vec![step("one", "tool_a", &[])], None);
        let planned = {
            let history = DriveHistory::new(mission.clone()).unwrap();
            first_full_mission(&grant, &history)
        };
        let report = report_for(
            &planned,
            vec![failed_with("one", "tool_a", "terminal")],
            None,
        );
        let mut dispatcher = RecordingDispatcher::new(vec![report]);
        let outcome = drive_mission(&grant, mission, &mut dispatcher).unwrap();
        assert_eq!(outcome.final_status, FinalStatus::Refused);
        assert_eq!(
            dispatcher.calls(),
            1,
            "a terminal refusal must consume one attempt, not the whole budget"
        );
    }
}

mod receipt_fidelity {
    use super::*;

    fn attempt_with_edited_report(mission: Value, edit: impl FnOnce(&mut Value)) -> DriveHistory {
        let grant = grant(&["tool_a", "tool_b"]);
        let mut history = DriveHistory::new(mission).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        let mut report = report_for(
            &dispatched,
            vec![
                succeeded("a", "tool_a", json!({})),
                failed_with("b", "tool_b", "retryable_as_is"),
            ],
            None,
        );
        edit(&mut report);
        history.push(
            AttemptRecord::delivered(
                AttemptKind::Full,
                dispatched,
                report,
                Some(complete_reconciliation()),
                None,
            )
            .unwrap(),
        );
        history
    }

    fn two_step_mission() -> Value {
        mission_of(
            vec![step("a", "tool_a", &[]), step("b", "tool_b", &["a"])],
            Some(binding_for(&["a", "b"])),
        )
    }

    fn attempt_table(report: &Value) -> Vec<String> {
        report["attempts"][0]["classification_table"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["step_id"].as_str().unwrap().to_string())
            .collect()
    }

    fn build(history: &DriveHistory) -> Value {
        let grant = grant(&["tool_a", "tool_b"]);
        build_autopilot_report(
            &grant,
            history,
            &FinalDisposition::Exhausted {
                accounting: json!({ "reason": "test" }),
            },
        )
        .unwrap()
    }

    #[test]
    fn every_step_an_attempt_dispatched_appears_in_that_attempts_classification_table() {
        let history = attempt_with_edited_report(two_step_mission(), |report| {
            let results = report["results"].as_array_mut().unwrap();
            results.retain(|result| result["id"] != "b");
        });
        let report = build(&history);
        assert_eq!(
            report["attempts"][0]["dispatched_step_ids"],
            json!(["a", "b"])
        );
        assert_eq!(
            attempt_table(&report),
            vec!["a".to_string(), "b".to_string()],
            "a dispatched step whose result row the executor omitted must still be classified, \
             or the table understates what the drive ran"
        );
    }

    #[test]
    fn no_step_appears_in_a_classification_table_that_its_attempt_did_not_dispatch() {
        let history = attempt_with_edited_report(two_step_mission(), |report| {
            let phantom = json!({
                "id": "never_dispatched",
                "tool": "tool_evil",
                "status": "succeeded",
                "required": true,
                "arguments_digest": null,
                "bytes": 0,
                "wire": null,
                "error": null,
            });
            report["results"].as_array_mut().unwrap().push(phantom);
        });
        let report = build(&history);
        assert_eq!(
            report["attempts"][0]["dispatched_step_ids"],
            json!(["a", "b"])
        );
        assert_eq!(
            attempt_table(&report),
            vec!["a".to_string(), "b".to_string()],
            "a result row for a step the mission never contained must not become a classified \
             step in the drive's own receipt"
        );
    }

    #[test]
    fn the_classification_table_reports_the_status_the_mission_report_actually_recorded() {
        let history = attempt_with_edited_report(two_step_mission(), |_| {});
        let report = build(&history);
        let rows = report["attempts"][0]["classification_table"]
            .as_array()
            .unwrap();
        assert_eq!(rows[0]["step_id"], "a");
        assert_eq!(rows[0]["status"], "succeeded");
        assert_eq!(rows[0]["class"], "succeeded");
        assert_eq!(rows[1]["step_id"], "b");
        assert_eq!(rows[1]["status"], "refused");
        assert_eq!(rows[1]["class"], "retryable_as_is");
        assert_eq!(rows[1]["signal"], "declared_error_text");
    }

    #[test]
    fn every_attempt_report_digest_recomputes_from_the_report_that_attempt_received() {
        let history = attempt_with_edited_report(two_step_mission(), |_| {});
        let report = build(&history);
        let claimed = report["attempts"][0]["report_digest"].as_str().unwrap();
        let recomputed = ContentHash::of_value(history.attempts()[0].report().unwrap())
            .unwrap()
            .to_string();
        assert_eq!(claimed, recomputed);
    }

    #[test]
    fn every_attempt_mission_digest_recomputes_from_the_mission_that_attempt_dispatched() {
        let history = attempt_with_edited_report(two_step_mission(), |_| {});
        let report = build(&history);
        let claimed = report["attempts"][0]["mission_digest"].as_str().unwrap();
        let recomputed = ContentHash::of_value(history.attempts()[0].mission())
            .unwrap()
            .to_string();
        assert_eq!(claimed, recomputed);
    }

    /// Characterises an unfixed gap rather than a guarantee: the digest an attempt row publishes
    /// for its reconciliation is copied out of the record and never checked against it, so a
    /// record naming the wrong digest is republished unchallenged and the report still verifies.
    /// Closing it needs a field the attempt row does not have — the recomputed value or a match
    /// flag — which is a wire-shape change, so this test pins the current behaviour and will fail
    /// the moment it is fixed.
    #[test]
    fn an_attempts_reconciliation_digest_is_republished_verbatim_and_never_recomputed() {
        let grant = grant(&["tool_a", "tool_b"]);
        let mut history = DriveHistory::new(two_step_mission()).unwrap();
        let dispatched = first_full_mission(&grant, &history);
        let mut record = json!({
            "present": true,
            "completion": { "status": "complete" },
            "integrity": { "valid": true },
        });
        let honest = ContentHash::of_value(&record).unwrap().to_string();
        record["reconciliation_digest"] = json!("f".repeat(64));
        push_attempt(
            &mut history,
            AttemptKind::Full,
            dispatched,
            vec![
                succeeded("a", "tool_a", json!({})),
                succeeded("b", "tool_b", json!({})),
            ],
            Some("succeeded"),
            Some(record.clone()),
        );

        let report = build(&history);
        let claimed = report["attempts"][0]["reconciliation_digest"]
            .as_str()
            .unwrap();
        let mut without_digest = record.clone();
        without_digest
            .as_object_mut()
            .unwrap()
            .remove("reconciliation_digest");
        let recomputed = ContentHash::of_value(&without_digest).unwrap().to_string();
        assert_eq!(recomputed, honest);
        assert_eq!(
            claimed,
            "f".repeat(64),
            "the attempt row copies whatever digest the record claims"
        );
        assert_ne!(
            claimed, recomputed,
            "the fixture must actually plant a wrong digest, or this characterisation is vacuous"
        );
        assert_eq!(
            verify_autopilot_report(&report).unwrap()["valid"],
            true,
            "report verification covers the report's own digest and says nothing about whether a \
             reconciliation digest matches the record it names"
        );
    }

    #[test]
    fn an_edited_classification_table_fails_the_reports_own_digest_verification() {
        let history = attempt_with_edited_report(two_step_mission(), |_| {});
        let report = build(&history);
        assert_eq!(verify_autopilot_report(&report).unwrap()["valid"], true);

        let mut tampered = report;
        tampered["attempts"][0]["classification_table"][0]["class"] = json!("terminal");
        let verification = verify_autopilot_report(&tampered).unwrap();
        assert_eq!(verification["valid"], false);
        assert_eq!(verification["digest_match"], false);
        assert_eq!(verification["digest_malformed"], false);
    }
}
