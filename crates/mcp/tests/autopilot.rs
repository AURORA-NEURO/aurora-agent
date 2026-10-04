use bioprism_mcp::{Lifecycle, Request, Server};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}

fn server() -> Server {
    Server::new(repo_root())
}

fn ready(server: &mut Server) {
    if server.lifecycle() == Lifecycle::New {
        let request =
            Request::parse(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#)
                .expect("initialize parses");
        server.handle(&request).expect("initialize is answered");
    }
    if server.lifecycle() == Lifecycle::Initialized {
        let request = Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .expect("initialized notification parses");
        assert!(server.handle(&request).is_none());
    }
    assert_eq!(server.lifecycle(), Lifecycle::Ready);
}

fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    ready(server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })
        .to_string(),
    )
    .expect("tool request parses");
    let response = server.handle(&request).expect("tool call is answered");
    let wire = response.to_json();
    let text = wire["result"]["content"][0]["text"]
        .as_str()
        .expect("tool response has text content");
    let mut result: Value = serde_json::from_str(text).expect("tool response is JSON");
    result["__isError"] = wire["result"]["isError"].clone();
    result
}

fn instantiate(server: &mut Server, mission_id: &str) -> Value {
    let mut report = call(
        server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": mission_id,
            "goal": "discover the repository capability surface",
            "steps": [{
                "id": "catalog",
                "tool": "workspace_capabilities",
                "arguments": {}
            }]
        }),
    );
    assert_eq!(report["__isError"], false);
    report
        .as_object_mut()
        .expect("instantiation report is an object")
        .remove("__isError");
    report
}

fn grant(tool: &str) -> Value {
    json!({
        "allowed_tools": [tool],
        "max_attempts": 2
    })
}

fn goal_grant() -> Value {
    json!({
        "allowed_tools": ["workspace_capabilities"],
        "max_attempts": 2,
        "require_reconciliation_complete": false
    })
}

#[test]
fn mcp_autopilot_previews_executes_and_verifies_a_real_instantiation() {
    let mut server = server();
    let instantiation = instantiate(&mut server, "mcp-autopilot-round-trip");

    let preview = call(
        &mut server,
        "autopilot_drive",
        json!({
            "instantiation": instantiation.clone(),
            "grant": grant("workspace_capabilities")
        }),
    );
    assert_eq!(preview["__isError"], false);
    assert_eq!(preview["workflow"], "autopilot_drive");
    assert_eq!(preview["mode"], "preview");
    assert_eq!(preview["no_dispatch"], true);
    assert_eq!(preview["dispatch"], "not_started");
    assert_eq!(preview["writes"], "none");
    assert_eq!(
        preview["planned_first_action"]["mission"]["policy"]["execute"],
        true
    );

    let execution = call(
        &mut server,
        "autopilot_drive",
        json!({
            "instantiation": instantiation,
            "grant": grant("workspace_capabilities"),
            "mode": "execute"
        }),
    );
    assert_eq!(execution["__isError"], false);
    assert_eq!(execution["workflow"], "autopilot_drive");
    assert_eq!(execution["mode"], "execute");
    assert_eq!(execution["ok"], true);
    assert_eq!(execution["dispatch_started"], true);
    assert_eq!(execution["attempts_this_call"], 1);
    assert_eq!(execution["attempts_used"], 1);
    assert_eq!(execution["report_attempts_used"], 1);
    assert_eq!(execution["attempt_accounting_matches"], true);
    assert_eq!(execution["report"]["final_status"], "succeeded");
    assert_eq!(execution["report_verification"]["valid"], true);
    assert_eq!(
        execution["report_verification"]["reconciliation_digest_verification_supported"],
        true
    );
    assert_eq!(
        execution["report"]["attempts"][0]["reconciliation_digest_verified"],
        true
    );
    assert!(execution["report"]["attempts"][0]["reconciliation_record"].is_object());
    assert_eq!(
        execution["report"]["evidence"]["reconciliation"]["digest_verified"],
        true
    );
    assert_eq!(execution["scheduling"]["logical_wait_ticks"], json!([]));
    assert_eq!(execution["scheduling"]["wall_clock_waited"], false);

    let verified = call(
        &mut server,
        "autopilot_verify",
        json!({"report": execution["report"].clone()}),
    );
    assert_eq!(verified["__isError"], false);
    assert_eq!(verified["ok"], true);
    assert_eq!(verified["verification"]["digest_match"], true);
    assert_eq!(
        verified["verification"]["reconciliation_digest_verification_supported"],
        true
    );

    let mut tampered_report = execution["report"].clone();
    tampered_report["base_mission_id"] = json!("edited-after-execution");
    let rejected = call(
        &mut server,
        "autopilot_verify",
        json!({"report": tampered_report}),
    );
    assert_eq!(rejected["__isError"], false);
    assert_eq!(rejected["ok"], false);
    assert_eq!(rejected["verification"]["digest_match"], false);
}

#[test]
fn executed_missions_embed_the_canonical_reconciliation_record_used_by_autopilot() {
    let mut server = server();
    let instantiation = instantiate(&mut server, "mcp-canonical-reconciliation");
    let mut mission = instantiation["mission"].clone();
    mission["policy"]["execute"] = json!(true);
    mission["policy"]["allowed_tools"] = json!(["workspace_capabilities"]);

    let report = server
        .execute_agent_mission_with_cancellation(&mission, &AtomicBool::new(false))
        .expect("mission execution returns its reconciliation");
    let projection = &report["workflow_reconciliation"];
    assert_eq!(projection["present"], true);
    assert!(projection["canonical_record"].is_object());
    assert!(projection
        .get("reconciliation_digest_verification")
        .is_none());

    let mut canonical_record = projection["canonical_record"].clone();
    let digest = canonical_record["reconciliation_digest"]
        .as_str()
        .expect("canonical reconciliation has a digest")
        .to_string();
    canonical_record
        .as_object_mut()
        .expect("canonical record is an object")
        .remove("reconciliation_digest");
    assert_eq!(
        bioprism_ids::ContentHash::of_value(&canonical_record)
            .expect("canonical record hashes")
            .to_string(),
        digest
    );
    assert_eq!(projection["reconciliation_digest"], digest);
    assert_eq!(
        projection["registry_import"]["reconciliation_digest"],
        digest
    );
    assert_eq!(projection["registry_import"]["ok"], true);
}

#[test]
fn mcp_autopilot_refuses_an_invalid_grant_before_any_dispatch() {
    let mut server = server();
    let instantiation = instantiate(&mut server, "mcp-autopilot-invalid-grant");
    let refused = call(
        &mut server,
        "autopilot_drive",
        json!({
            "instantiation": instantiation,
            "grant": {
                "allowed_tools": ["workspace_capabilities"],
                "max_attempts": 0
            },
            "mode": "execute"
        }),
    );
    assert_eq!(refused["__isError"], true);
    assert!(
        refused["error"]
            .as_str()
            .unwrap_or_default()
            .contains("attempt"),
        "invalid authority should explain the rejected budget: {refused}"
    );
}

#[test]
fn mcp_autopilot_returns_and_accepts_caller_owned_continuation_state() {
    let mut server = server();
    let mut instantiation = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "developer_and_release_contracts",
            "mission_id": "mcp-autopilot-resume-chunks",
            "goal": "exercise bounded caller-owned autopilot continuation",
            "steps": [{
                "id": "verify",
                "tool": "autopilot_verify",
                "arguments": { "report": {} }
            }]
        }),
    );
    assert_eq!(instantiation["__isError"], false);
    instantiation
        .as_object_mut()
        .expect("instantiation is an object")
        .remove("__isError");

    let grant = json!({
        "allowed_tools": ["autopilot_verify"],
        "max_attempts": 3,
        "require_reconciliation_complete": false,
        "retry": { "retry_unknown": true }
    });
    let first = call(
        &mut server,
        "autopilot_drive",
        json!({
            "instantiation": instantiation,
            "grant": grant,
            "mode": "execute",
            "max_dispatches_this_call": 1
        }),
    );
    assert_eq!(first["__isError"], false, "first chunk failed: {first}");
    assert_eq!(first["paused"], true);
    assert_eq!(first["report"]["final_status"], "paused");
    assert_eq!(first["report_verification"]["valid"], true);
    assert_eq!(first["attempts_this_call"], 1);
    assert_eq!(first["attempts_used"], 1);
    assert_eq!(first["report_attempts_used"], 1);
    assert_eq!(first["attempt_accounting_matches"], true);
    assert_eq!(first["recovery"]["available"], true);
    let continuation = first["recovery"]["next_request"].clone();
    assert_eq!(continuation["mode"], "resume");
    assert_eq!(
        continuation["rehydrated_attempts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let mut tampered_continuation = continuation.clone();
    tampered_continuation["rehydrated_attempts"][0]["report"]["mission_status"] =
        json!("succeeded");
    let rejected = call(&mut server, "autopilot_drive", tampered_continuation);
    assert_eq!(rejected["__isError"], true);

    let second = call(&mut server, "autopilot_drive", continuation);
    assert_eq!(second["__isError"], false, "resume failed: {second}");
    assert_eq!(second["mode"], "resume");
    assert_eq!(second["paused"], true);
    assert_eq!(second["attempts_this_call"], 1);
    assert_eq!(second["attempts_used"], 2);
    assert_eq!(second["report_attempts_used"], 2);
    assert_eq!(second["attempt_accounting_matches"], true);
    assert_eq!(second["recovery"]["available"], true);
    assert_eq!(
        second["recovery"]["next_request"]["rehydrated_attempts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let final_chunk = call(
        &mut server,
        "autopilot_drive",
        second["recovery"]["next_request"].clone(),
    );
    assert_eq!(final_chunk["__isError"], false);
    assert_eq!(final_chunk["paused"], false);
    assert_eq!(final_chunk["attempts_this_call"], 1);
    assert_eq!(final_chunk["attempts_used"], 3);
    assert_eq!(final_chunk["report_attempts_used"], 3);
    assert_eq!(final_chunk["attempt_accounting_matches"], true);
    assert_eq!(final_chunk["report"]["final_status"], "exhausted");
    assert_eq!(final_chunk["report_verification"]["valid"], true);
}

#[test]
fn mcp_goal_step_executes_resumes_completes_and_verifies_without_replay() {
    let mut server = server();
    let first_mission = instantiate(&mut server, "mcp-goal-cycle-one")["mission"].clone();
    let second_mission = instantiate(&mut server, "mcp-goal-cycle-two")["mission"].clone();
    let grant = goal_grant();

    let first = call(
        &mut server,
        "autopilot_goal_step",
        json!({
            "goal_id": "mcp-goal-resume-round-trip",
            "grant": grant,
            "budget": { "max_cycles": 3, "max_total_dispatches": 6 },
            "decision": { "kind": "run_mission", "mission": first_mission }
        }),
    );
    assert_eq!(first["__isError"], false, "first goal step failed: {first}");
    assert_eq!(first["ok"], true);
    assert_eq!(first["goal_status"], "stopped");
    assert_eq!(first["dispatch_started"], true);
    assert_eq!(first["dispatches_this_call"], 1);
    assert_eq!(first["report_verification"]["valid"], true);
    assert_eq!(first["total_dispatches"], 1);
    assert_eq!(first["report"]["cycles"].as_array().unwrap().len(), 1);
    assert_eq!(
        first["cycle_autopilot_reports"].as_array().unwrap().len(),
        1
    );
    assert_eq!(first["checkpoint_generation"], 1);
    assert_eq!(first["checkpoint"]["generation"], 1);

    let verified = call(
        &mut server,
        "autopilot_goal_verify",
        json!({
            "report": first["report"].clone(),
            "checkpoint": first["checkpoint"].clone()
        }),
    );
    assert_eq!(verified["__isError"], false);
    assert_eq!(verified["ok"], true);
    assert_eq!(verified["report_verification"]["valid"], true);
    assert_eq!(verified["checkpoint_verification"]["valid"], true);

    let second = call(
        &mut server,
        "autopilot_goal_step",
        json!({
            "goal_id": "mcp-goal-resume-round-trip",
            "grant": goal_grant(),
            "decision": { "kind": "run_mission", "mission": second_mission },
            "checkpoint": first["checkpoint"].clone(),
            "previous_autopilot_report": first["cycle_autopilot_reports"][0].clone()
        }),
    );
    assert_eq!(
        second["__isError"], false,
        "second goal step failed: {second}"
    );
    assert_eq!(second["goal_status"], "stopped");
    assert_eq!(second["dispatch_started"], true);
    assert_eq!(second["dispatches_this_call"], 1);
    assert_eq!(second["report_verification"]["valid"], true);
    assert_eq!(second["total_dispatches"], 2);
    assert_eq!(second["report"]["cycles"].as_array().unwrap().len(), 2);
    assert_eq!(
        second["cycle_autopilot_reports"].as_array().unwrap().len(),
        1
    );
    assert_eq!(second["checkpoint_generation"], 2);
    assert_eq!(
        second["checkpoint"]["previous_snapshot_digest"],
        first["checkpoint"]["snapshot_digest"]
    );

    let completed = call(
        &mut server,
        "autopilot_goal_step",
        json!({
            "goal_id": "mcp-goal-resume-round-trip",
            "grant": goal_grant(),
            "decision": {
                "kind": "complete",
                "evaluator_id": "acceptance-evaluator-v1",
                "evidence_sha256": "a".repeat(64)
            },
            "checkpoint": second["checkpoint"].clone(),
            "previous_autopilot_report": second["cycle_autopilot_reports"][0].clone()
        }),
    );
    assert_eq!(
        completed["__isError"], false,
        "goal completion failed: {completed}"
    );
    assert_eq!(completed["goal_status"], "completed");
    assert_eq!(completed["goal_complete"], true);
    assert_eq!(completed["dispatch_started"], false);
    assert_eq!(completed["dispatches_this_call"], 0);
    assert_eq!(completed["total_dispatches"], 2);
    assert_eq!(completed["report"]["cycles"].as_array().unwrap().len(), 2);
    assert_eq!(completed["checkpoint"], Value::Null);
    assert_eq!(completed["report_verification"]["valid"], true);

    let final_verification = call(
        &mut server,
        "autopilot_goal_verify",
        json!({ "report": completed["report"].clone() }),
    );
    assert_eq!(final_verification["__isError"], false);
    assert_eq!(final_verification["ok"], true);
    assert_eq!(final_verification["dispatch"], "not_started");
}

#[test]
fn mcp_goal_resume_refuses_missing_or_tampered_continuation_before_dispatch() {
    let mut server = server();
    let workflow = instantiate(&mut server, "mcp-goal-invalid-resume");
    let first = call(
        &mut server,
        "autopilot_goal_step",
        json!({
            "goal_id": "mcp-goal-invalid-resume",
            "grant": goal_grant(),
            "budget": { "max_cycles": 2, "max_total_dispatches": 4 },
            "decision": {
                "kind": "run_mission",
                "mission": workflow["mission"].clone()
            }
        }),
    );
    assert_eq!(first["__isError"], false);

    let missing_report = call(
        &mut server,
        "autopilot_goal_step",
        json!({
            "goal_id": "mcp-goal-invalid-resume",
            "grant": goal_grant(),
            "decision": { "kind": "run_mission", "mission": workflow["mission"].clone() },
            "checkpoint": first["checkpoint"].clone()
        }),
    );
    assert_eq!(missing_report["__isError"], true);
    assert!(missing_report["error"].as_str().unwrap().contains("resume"));

    let mut tampered_checkpoint = first["checkpoint"].clone();
    tampered_checkpoint["generation"] = json!(2);
    let tampered = call(
        &mut server,
        "autopilot_goal_step",
        json!({
            "goal_id": "mcp-goal-invalid-resume",
            "grant": goal_grant(),
            "decision": { "kind": "run_mission", "mission": workflow["mission"].clone() },
            "checkpoint": tampered_checkpoint,
            "previous_autopilot_report": first["cycle_autopilot_reports"][0].clone()
        }),
    );
    assert_eq!(tampered["__isError"], true);
    assert!(tampered["error"].as_str().unwrap().contains("checkpoint"));
}
