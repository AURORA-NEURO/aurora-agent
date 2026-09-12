use bioprism_brain::AutonomousPlanRequest;
use bioprism_ids::{to_canonical_bytes, ContentHash};
use bioprism_mcp::{Lifecycle, Request, Server};
use bioprism_research::ResearchRequest;
use bioprism_research_campaign::validate_campaign_checkpoint;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(1);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let ordinal = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bioprism-mcp-{label}-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("temporary root is created once");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ready(server: &mut Server) {
    if server.lifecycle() == Lifecycle::New {
        let initialize =
            Request::parse(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#)
                .expect("initialize parses");
        server.handle(&initialize).expect("initialize is answered");
    }
    if server.lifecycle() == Lifecycle::Initialized {
        let notification =
            Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .expect("initialized notification parses");
        assert!(server.handle(&notification).is_none());
    }
    assert_eq!(server.lifecycle(), Lifecycle::Ready);
}

fn call(server: &mut Server, name: &str, arguments: Value) -> (bool, Value) {
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
    .expect("tool call parses");
    let response = server
        .handle(&request)
        .expect("tool call is answered")
        .to_json();
    let is_error = response["result"]["isError"]
        .as_bool()
        .expect("tool result labels errors");
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool result has JSON text");
    let value = serde_json::from_str(text).expect("tool result text is JSON");
    (is_error, value)
}

fn write_value(root: &Path, relative: &str, value: &Value) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent exists");
    }
    fs::write(
        path,
        to_canonical_bytes(value).expect("fixture canonicalizes"),
    )
    .expect("fixture writes");
}

fn read_value(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("fixture file reads"))
        .expect("fixture file contains JSON")
}

fn brain_input(effect: &str, steps: bool) -> (Value, String) {
    let value = json!({
        "objective": "PRIVATE_BRAIN_OBJECTIVE",
        "steps": if steps { json!([{
            "id": "private-step",
            "objective": "PRIVATE_STEP_OBJECTIVE",
            "tool": "fixture.read",
            "arguments": { "secret": "PRIVATE_PLAN_ARGUMENT" },
            "depends_on": [],
            "effect": effect,
            "estimated_cost": 1
        }]) } else { json!([]) },
        "allowed_tools": ["fixture.read"],
        "max_cost": 10,
        "require_approval_for_effects": true,
        "max_parallelism": 1
    });
    let request: AutonomousPlanRequest =
        serde_json::from_value(value.clone()).expect("brain request parses");
    let normalized = serde_json::to_value(request).expect("brain request encodes");
    let digest = ContentHash::of_value(&normalized)
        .expect("brain request canonicalizes")
        .to_string();
    (value, digest)
}

fn research_input() -> (Value, String) {
    let value = json!({
        "research_id": "campaign-surface-test",
        "question": "PRIVATE_RESEARCH_QUESTION",
        "family": "reference_like",
        "distractor_points": [0],
        "seed": 7,
        "run_sweep": false,
        "run_mutation": false,
        "run_minimize": false
    });
    let request: ResearchRequest =
        serde_json::from_value(value.clone()).expect("research request parses");
    let digest = request.digest().expect("research request digests");
    (value, digest)
}

fn campaign_spec(stage_id: &str, kind: &str, input_digest: &str) -> Value {
    json!({
        "campaign_id": "surface-campaign",
        "objective": "PRIVATE_CAMPAIGN_OBJECTIVE",
        "reconciliation_authority": {
            "authority_id": "caller-owned-journal",
            "protocol_version": "1",
            "config_digest": "a".repeat(64)
        },
        "stages": [{
            "stage_id": stage_id,
            "kind": kind,
            "input_digest": input_digest,
            "depends_on": []
        }],
        "max_actions": 1
    })
}

fn write_brain_case(root: &Path, output: &str, effect: &str, steps: bool) -> Value {
    let (input, digest) = brain_input(effect, steps);
    write_value(
        root,
        "campaign/spec.json",
        &campaign_spec("plan", "brain_plan", &digest),
    );
    write_value(root, "campaign/plan.json", &input);
    json!({
        "spec_path": "campaign/spec.json",
        "stage_input_paths": { "plan": "campaign/plan.json" },
        "output_dir": output
    })
}

#[test]
fn offline_campaign_is_advertised_and_preview_is_a_metadata_only_no_write_state() {
    let root = TempRoot::new("campaign-preview");
    let mut server = Server::new(root.path().to_path_buf());
    let arguments = write_brain_case(root.path(), "preview-output", "read_only", true);

    ready(&mut server);
    let list = Request::parse(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
        .expect("tools/list parses");
    let listed = server
        .handle(&list)
        .expect("tools/list is answered")
        .to_json();
    let tool = listed["result"]["tools"]
        .as_array()
        .expect("tool list is an array")
        .iter()
        .find(|tool| tool["name"] == "research_campaign_run_offline")
        .expect("offline campaign tool is advertised");
    assert_eq!(tool["inputSchema"]["additionalProperties"], json!(false));
    let argument_names = tool["inputSchema"]["properties"]
        .as_object()
        .expect("tool properties are an object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        argument_names,
        BTreeSet::from(["confirm", "output_dir", "spec_path", "stage_input_paths"])
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["stage_input_paths"]["maxProperties"],
        json!(8)
    );
    let (capabilities_error, capabilities) = call(&mut server, "workspace_capabilities", json!({}));
    assert!(!capabilities_error);
    assert!(capabilities
        .as_array()
        .expect("capability groups are an array")
        .iter()
        .any(|group| group["id"] == "autonomous_research_campaigns"));

    let (is_error, preview) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!is_error);
    assert_eq!(preview["execution"], json!({ "state": "not_started" }));
    assert_eq!(preview["campaign_status"], json!("planned"));
    assert_eq!(preview["actions_used"], json!(0));
    assert_eq!(preview["stages"][0]["state"], json!("not_started"));
    assert!(preview["checkpoint"].is_null());
    assert!(preview["trusted_head"].is_null());
    assert!(preview["manifest"].is_null());
    assert_eq!(preview["written"], json!([]));
    assert!(!root.path().join("preview-output").exists());

    let keys = preview
        .as_object()
        .expect("preview is an object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "actions_used",
            "campaign_id",
            "campaign_status",
            "checkpoint",
            "execution",
            "limitations",
            "manifest",
            "schema",
            "spec_digest",
            "stages",
            "trusted_head",
            "workflow",
            "written",
        ])
    );
    let wire = preview.to_string();
    assert!(!wire.contains("PRIVATE_CAMPAIGN_OBJECTIVE"));
    assert!(!wire.contains("PRIVATE_BRAIN_OBJECTIVE"));
    assert!(!wire.contains("PRIVATE_PLAN_ARGUMENT"));
}

#[test]
fn malformed_spec_errors_do_not_expose_resolved_paths_or_private_bytes() {
    let root = TempRoot::new("campaign-private-spec-error");
    fs::create_dir_all(root.path().join("campaign")).expect("campaign fixture directory exists");
    let private_marker = "PRIVATE_MALFORMED_SPEC_CONTENT";
    fs::write(
        root.path().join("campaign/spec.json"),
        format!(
            "{{\"private\":\"{private_marker}\",\"absolute_path\":\"{}\"",
            root.path().display()
        ),
    )
    .expect("malformed campaign spec is written");
    let mut server = Server::new(root.path().to_path_buf());

    let (is_error, error) = call(
        &mut server,
        "research_campaign_run_offline",
        json!({
            "spec_path": "campaign/spec.json",
            "stage_input_paths": { "measure": "campaign/missing.json" },
            "output_dir": "unused-output"
        }),
    );
    assert!(is_error);
    let message = error["error"].as_str().expect("tool error is a string");
    assert!(message.contains("invalid JSON in campaign spec"));
    assert!(!message.contains(private_marker));
    assert!(!message.contains(&root.path().display().to_string()));
}

#[test]
fn confirmed_brain_campaign_persists_a_valid_append_only_chain_and_metadata_response() {
    let root = TempRoot::new("campaign-confirmed");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments = write_brain_case(root.path(), "confirmed-output", "read_only", true);
    arguments["confirm"] = json!(true);

    let (is_error, result) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!is_error, "{result}");
    assert_eq!(result["execution"], json!({ "state": "completed" }));
    assert_eq!(result["campaign_status"], json!("completed"));
    assert_eq!(result["actions_used"], json!(1));
    assert_eq!(result["stages"][0]["state"], json!("settled"));
    assert_eq!(result["stages"][0]["disposition"], json!("succeeded"));
    assert_eq!(result["written"].as_array().unwrap().len(), 6);

    let output = root.path().join("confirmed-output");
    let checkpoint: Value = serde_json::from_slice(
        &fs::read(output.join("campaign.checkpoint.json")).expect("checkpoint exists"),
    )
    .expect("checkpoint parses");
    let validated = validate_campaign_checkpoint(&checkpoint).expect("checkpoint validates");
    assert_eq!(validated.generation(), 2);
    assert_eq!(
        validated.snapshot_digest(),
        result["trusted_head"]["snapshot_digest"]
            .as_str()
            .expect("result has head digest")
    );
    assert!(output.join("authority/0001-authorization.json").is_file());
    assert!(output.join("authority/0002-terminal.json").is_file());
    assert!(output.join("campaign.head.json").is_file());
    assert!(output
        .join("artifacts/0001-brain-plan-report.json")
        .is_file());

    let response_wire = result.to_string();
    assert!(!response_wire.contains("PRIVATE_CAMPAIGN_OBJECTIVE"));
    assert!(!response_wire.contains("PRIVATE_BRAIN_OBJECTIVE"));
    assert!(!response_wire.contains("PRIVATE_PLAN_ARGUMENT"));
    let head_before = fs::read(output.join("campaign.head.json")).expect("head exists");
    let (duplicate_is_error, duplicate) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!duplicate_is_error, "{duplicate}");
    assert_eq!(duplicate["execution"], json!({ "state": "completed" }));
    assert_eq!(duplicate["spec_digest"], result["spec_digest"]);
    assert_eq!(duplicate["manifest"], result["manifest"]);
    assert_eq!(duplicate["written"], result["written"]);
    assert_eq!(
        fs::read(output.join("campaign.head.json")).expect("head remains"),
        head_before
    );
}

#[test]
fn a_tampered_authorization_claim_never_replays_as_completed() {
    let root = TempRoot::new("campaign-tampered-claim");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments = write_brain_case(root.path(), "tampered-claim-output", "read_only", true);
    arguments["confirm"] = json!(true);
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");
    assert_eq!(run["execution"], json!({ "state": "completed" }));

    let authorization_path = root
        .path()
        .join("tampered-claim-output/authority/0001-authorization.json");
    let mut authorization = read_value(&authorization_path);
    authorization["claim"]["stage_id"] = json!("forged-stage");
    write_value(
        root.path(),
        "tampered-claim-output/authority/0001-authorization.json",
        &authorization,
    );

    let (inspect_error, inspected) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!inspect_error, "{inspected}");
    assert_eq!(
        inspected["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(inspected["actions_used"], json!(0));
    assert!(inspected["checkpoint"].is_null());
    assert_eq!(inspected["stages"][0]["state"], json!("not_started"));
}

#[test]
fn a_replaced_terminal_envelope_never_replays_as_completed() {
    let root = TempRoot::new("campaign-replaced-terminal");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments =
        write_brain_case(root.path(), "replaced-terminal-output", "read_only", true);
    arguments["confirm"] = json!(true);
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");

    let output = root.path().join("replaced-terminal-output");
    let authorization = read_value(&output.join("authority/0001-authorization.json"));
    write_value(
        root.path(),
        "replaced-terminal-output/authority/0002-terminal.json",
        &authorization,
    );

    let (inspect_error, inspected) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!inspect_error, "{inspected}");
    assert_eq!(
        inspected["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(inspected["actions_used"], json!(1));
    assert_eq!(
        inspected["stages"][0]["state"],
        json!("reconciliation_required")
    );
    assert!(inspected["manifest"].is_null());
}

#[test]
fn a_renamed_partial_authorization_cannot_invent_generation_9999() {
    let root = TempRoot::new("campaign-renamed-authorization");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments = write_brain_case(
        root.path(),
        "renamed-authorization-output",
        "read_only",
        true,
    );
    arguments["confirm"] = json!(true);
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");

    let output = root.path().join("renamed-authorization-output");
    for relative in [
        "authority/0002-terminal.json",
        "campaign.checkpoint.json",
        "campaign.manifest.json",
        "campaign.head.json",
    ] {
        fs::remove_file(output.join(relative)).expect("simulated partial removes commit material");
    }
    fs::rename(
        output.join("authority/0001-authorization.json"),
        output.join("authority/9999-authorization.json"),
    )
    .expect("authorization filename is tampered");

    let (inspect_error, inspected) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!inspect_error, "{inspected}");
    assert_eq!(
        inspected["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(inspected["actions_used"], json!(0));
    assert!(inspected["checkpoint"].is_null());
    assert_eq!(inspected["written"], json!([]));
}

#[test]
fn authority_inspection_is_bounded_and_does_not_echo_unexpected_entry_names() {
    let root = TempRoot::new("campaign-authority-bound");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments = write_brain_case(root.path(), "authority-bound-output", "read_only", true);
    arguments["confirm"] = json!(true);
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");
    assert_eq!(run["execution"], json!({ "state": "completed" }));

    let authority = root.path().join("authority-bound-output/authority");
    let private_name = "PRIVATE_UNEXPECTED_OBJECTIVE.txt";
    fs::write(authority.join(private_name), b"private").expect("unexpected entry is created");

    let (unexpected_error, unexpected) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!unexpected_error, "{unexpected}");
    assert_eq!(
        unexpected["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(unexpected["actions_used"], json!(1));
    let unexpected_reason = unexpected["execution"]["reason"]
        .as_str()
        .expect("reconciliation has a reason");
    assert!(unexpected_reason.contains("unexpected entries"));
    assert!(!unexpected_reason.contains(private_name));
    assert!(!unexpected_reason.contains(&root.path().display().to_string()));

    for index in 0..7 {
        fs::write(
            authority.join(format!("PRIVATE_OVERFLOW_{index}.txt")),
            b"private",
        )
        .expect("overflow entry is created");
    }
    let (overflow_error, overflow) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!overflow_error, "{overflow}");
    assert_eq!(
        overflow["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(overflow["actions_used"], json!(0));
    assert!(overflow["checkpoint"].is_null());
    assert_eq!(overflow["stages"][0]["state"], json!("not_started"));
    let overflow_reason = overflow["execution"]["reason"]
        .as_str()
        .expect("reconciliation has a reason");
    assert!(overflow_reason.contains("exceeds the 9-entry bound"));
    assert!(!overflow_reason.contains("PRIVATE_OVERFLOW"));
    assert!(!overflow_reason.contains(&root.path().display().to_string()));
}

#[test]
fn invalid_authority_filenames_are_not_echoed_during_reconciliation() {
    let root = TempRoot::new("campaign-private-authority-name");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments = write_brain_case(
        root.path(),
        "private-authority-name-output",
        "read_only",
        true,
    );
    arguments["confirm"] = json!(true);
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");

    let authority = root.path().join("private-authority-name-output/authority");
    let private_name = "PRIVATE_INVALID_AUTHORIZATION-authorization.json";
    fs::rename(
        authority.join("0001-authorization.json"),
        authority.join(private_name),
    )
    .expect("authorization filename is tampered");

    let (inspect_error, inspected) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!inspect_error, "{inspected}");
    assert_eq!(
        inspected["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(inspected["actions_used"], json!(0));
    let reason = inspected["execution"]["reason"]
        .as_str()
        .expect("reconciliation has a reason");
    assert!(reason.contains("authorization envelope at ordinal 1"));
    assert!(!reason.contains(private_name));
    assert!(!reason.contains(&root.path().display().to_string()));
}

#[test]
fn malformed_canonical_authority_content_does_not_leak_paths_or_private_bytes() {
    let root = TempRoot::new("campaign-private-authority-content");
    let mut server = Server::new(root.path().to_path_buf());
    let mut arguments = write_brain_case(
        root.path(),
        "private-authority-content-output",
        "read_only",
        true,
    );
    arguments["confirm"] = json!(true);
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");

    let private_marker = "PRIVATE_CANONICAL_AUTHORITY_CONTENT";
    let authorization_path = root
        .path()
        .join("private-authority-content-output/authority/0001-authorization.json");
    fs::write(
        authorization_path,
        format!(
            "{{\"private\":\"{private_marker}\",\"absolute_path\":\"{}\"",
            root.path().display()
        ),
    )
    .expect("canonical authorization is malformed");

    let (inspect_error, inspected) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!inspect_error, "{inspected}");
    assert_eq!(
        inspected["campaign_status"],
        json!("reconciliation_required")
    );
    assert_eq!(inspected["actions_used"], json!(0));
    let reason = inspected["execution"]["reason"]
        .as_str()
        .expect("reconciliation has a reason");
    assert!(reason.contains("cannot be read as bounded JSON"));
    assert!(!reason.contains(private_marker));
    assert!(!reason.contains(&root.path().display().to_string()));
}

#[test]
fn a_two_stage_partial_keeps_the_prior_stage_settled_and_fences_only_the_active_stage() {
    let root = TempRoot::new("campaign-partial");
    let (first_input, first_digest) = brain_input("read_only", true);
    let mut second_input = first_input.clone();
    second_input["objective"] = json!("PRIVATE_SECOND_BRAIN_OBJECTIVE");
    second_input["steps"][0]["id"] = json!("private-second-step");
    second_input["steps"][0]["objective"] = json!("PRIVATE_SECOND_STEP_OBJECTIVE");
    let second_request: AutonomousPlanRequest =
        serde_json::from_value(second_input.clone()).expect("second request parses");
    let second_digest = ContentHash::of_value(
        &serde_json::to_value(second_request).expect("second request encodes"),
    )
    .expect("second request canonicalizes")
    .to_string();
    let spec = json!({
        "campaign_id": "surface-campaign",
        "objective": "PRIVATE_CAMPAIGN_OBJECTIVE",
        "reconciliation_authority": {
            "authority_id": "caller-owned-journal",
            "protocol_version": "1",
            "config_digest": "a".repeat(64)
        },
        "stages": [
            {
                "stage_id": "first",
                "kind": "brain_plan",
                "input_digest": first_digest,
                "depends_on": []
            },
            {
                "stage_id": "second",
                "kind": "brain_plan",
                "input_digest": second_digest,
                "depends_on": ["first"]
            }
        ],
        "max_actions": 2
    });
    write_value(root.path(), "campaign/spec.json", &spec);
    write_value(root.path(), "campaign/first.json", &first_input);
    write_value(root.path(), "campaign/second.json", &second_input);
    let arguments = json!({
        "spec_path": "campaign/spec.json",
        "stage_input_paths": {
            "first": "campaign/first.json",
            "second": "campaign/second.json"
        },
        "output_dir": "partial-output",
        "confirm": true
    });
    let mut server = Server::new(root.path().to_path_buf());
    let (run_error, run) = call(
        &mut server,
        "research_campaign_run_offline",
        arguments.clone(),
    );
    assert!(!run_error, "{run}");
    assert_eq!(run["execution"], json!({ "state": "completed" }));

    let output = root.path().join("partial-output");
    for relative in [
        "authority/0003-terminal.json",
        "artifacts/0002-brain-plan-report.json",
        "campaign.checkpoint.json",
        "campaign.manifest.json",
        "campaign.head.json",
    ] {
        fs::remove_file(output.join(relative)).expect("simulated crash removes final material");
    }

    let (inspect_error, partial) = call(&mut server, "research_campaign_run_offline", arguments);
    assert!(!inspect_error, "{partial}");
    assert_eq!(partial["campaign_status"], json!("reconciliation_required"));
    assert_eq!(partial["actions_used"], json!(2));
    assert_eq!(partial["stages"][0]["state"], json!("settled"));
    assert_eq!(
        partial["stages"][1]["state"],
        json!("reconciliation_required")
    );
    assert_eq!(
        partial["stages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|stage| stage["state"] == "reconciliation_required")
            .count(),
        1
    );
    assert_eq!(
        partial["checkpoint"]["locator"],
        json!("authority/0002-authorization.json#/checkpoint")
    );
    assert!(partial["manifest"].is_null());
    assert_eq!(partial["written"].as_array().unwrap().len(), 3);
}

#[test]
fn a_native_error_after_authorization_is_committed_as_unknown_not_returned_as_failure() {
    let root = TempRoot::new("campaign-unknown");
    let (mut input, _) = brain_input("read_only", true);
    input["objective"] = json!("");
    let request: AutonomousPlanRequest =
        serde_json::from_value(input.clone()).expect("structurally valid brain request parses");
    let digest =
        ContentHash::of_value(&serde_json::to_value(request).expect("brain request encodes"))
            .expect("brain request canonicalizes")
            .to_string();
    write_value(
        root.path(),
        "campaign/spec.json",
        &campaign_spec("plan", "brain_plan", &digest),
    );
    write_value(root.path(), "campaign/plan.json", &input);
    let mut server = Server::new(root.path().to_path_buf());
    let (is_error, result) = call(
        &mut server,
        "research_campaign_run_offline",
        json!({
            "spec_path": "campaign/spec.json",
            "stage_input_paths": { "plan": "campaign/plan.json" },
            "output_dir": "unknown-output",
            "confirm": true
        }),
    );
    assert!(!is_error, "{result}");
    assert_eq!(result["campaign_status"], json!("reconciliation_required"));
    assert_eq!(
        result["execution"]["state"],
        json!("reconciliation_required")
    );
    assert_eq!(
        result["stages"][0]["state"],
        json!("reconciliation_required")
    );
    assert!(result["checkpoint"].is_object());
    assert!(result["trusted_head"].is_object());
    assert!(result["manifest"].is_object());
    assert!(root
        .path()
        .join("unknown-output/campaign.head.json")
        .is_file());
}

#[test]
fn effectful_and_refused_brain_plans_remain_distinct_terminal_states() {
    let review_root = TempRoot::new("campaign-review");
    let mut review_server = Server::new(review_root.path().to_path_buf());
    let mut review_args =
        write_brain_case(review_root.path(), "review-output", "external_write", true);
    review_args["confirm"] = json!(true);
    let (review_error, review) = call(
        &mut review_server,
        "research_campaign_run_offline",
        review_args,
    );
    assert!(!review_error, "{review}");
    assert_eq!(
        review["execution"],
        json!({ "state": "awaiting_human_review", "stage_id": "plan" })
    );
    assert_eq!(review["campaign_status"], json!("awaiting_human_review"));
    assert_eq!(
        review["stages"][0]["disposition"],
        json!("awaiting_human_review")
    );

    let refused_root = TempRoot::new("campaign-refused");
    let mut refused_server = Server::new(refused_root.path().to_path_buf());
    let mut refused_args =
        write_brain_case(refused_root.path(), "refused-output", "read_only", false);
    refused_args["confirm"] = json!(true);
    let (refused_error, refused) = call(
        &mut refused_server,
        "research_campaign_run_offline",
        refused_args,
    );
    assert!(!refused_error, "{refused}");
    assert_eq!(
        refused["execution"],
        json!({ "state": "refused", "stage_id": "plan" })
    );
    assert_eq!(refused["campaign_status"], json!("refused"));
    assert_eq!(refused["stages"][0]["disposition"], json!("refused"));
}

#[test]
fn unsupported_kinds_digest_mismatches_and_traversal_fail_before_writes() {
    let unsupported_root = TempRoot::new("campaign-unsupported");
    write_value(
        unsupported_root.path(),
        "campaign/spec.json",
        &campaign_spec("drive", "autopilot_drive", &"b".repeat(64)),
    );
    let mut unsupported_server = Server::new(unsupported_root.path().to_path_buf());
    let (unsupported_error, unsupported) = call(
        &mut unsupported_server,
        "research_campaign_run_offline",
        json!({
            "spec_path": "campaign/spec.json",
            "stage_input_paths": { "drive": "campaign/absent.json" },
            "output_dir": "unsupported-output",
            "confirm": true
        }),
    );
    assert!(unsupported_error);
    assert!(unsupported["error"]
        .as_str()
        .unwrap()
        .contains("unsupported"));
    assert!(!unsupported_root.path().join("unsupported-output").exists());

    let mismatch_root = TempRoot::new("campaign-mismatch");
    let (input, _) = brain_input("read_only", true);
    write_value(
        mismatch_root.path(),
        "campaign/spec.json",
        &campaign_spec("plan", "brain_plan", &"c".repeat(64)),
    );
    write_value(mismatch_root.path(), "campaign/plan.json", &input);
    let mut mismatch_server = Server::new(mismatch_root.path().to_path_buf());
    let (mismatch_error, mismatch) = call(
        &mut mismatch_server,
        "research_campaign_run_offline",
        json!({
            "spec_path": "campaign/spec.json",
            "stage_input_paths": { "plan": "campaign/plan.json" },
            "output_dir": "mismatch-output",
            "confirm": true
        }),
    );
    assert!(mismatch_error);
    assert!(mismatch["error"]
        .as_str()
        .unwrap()
        .contains("input digest does not match"));
    assert!(!mismatch_root.path().join("mismatch-output").exists());

    let mut traversal_args =
        write_brain_case(mismatch_root.path(), "unused-output", "read_only", true);
    traversal_args["output_dir"] = json!("../outside");
    traversal_args["confirm"] = json!(true);
    let (traversal_error, traversal) = call(
        &mut mismatch_server,
        "research_campaign_run_offline",
        traversal_args,
    );
    assert!(traversal_error);
    assert!(traversal["error"].as_str().unwrap().contains("escapes"));
}

#[test]
fn stage_ids_never_shape_artifact_paths() {
    let root = TempRoot::new("campaign-stage-id");
    let (input, digest) = brain_input("read_only", true);
    let malicious = "../../private-stage";
    write_value(
        root.path(),
        "campaign/spec.json",
        &campaign_spec(malicious, "brain_plan", &digest),
    );
    write_value(root.path(), "campaign/plan.json", &input);
    let mut server = Server::new(root.path().to_path_buf());
    let (is_error, preview) = call(
        &mut server,
        "research_campaign_run_offline",
        json!({
            "spec_path": "campaign/spec.json",
            "stage_input_paths": { malicious: "campaign/plan.json" },
            "output_dir": "safe-output"
        }),
    );
    assert!(!is_error, "{preview}");
    assert_eq!(
        preview["stages"][0]["artifact_locator"],
        json!("artifacts/0001-brain-plan-report.json")
    );
}

#[test]
fn confirmed_synthetic_research_preserves_negative_findings_without_echoing_the_question() {
    let root = TempRoot::new("campaign-research");
    let (input, digest) = research_input();
    write_value(
        root.path(),
        "campaign/spec.json",
        &campaign_spec("measure", "synthetic_research", &digest),
    );
    write_value(root.path(), "campaign/research.json", &input);
    let mut server = Server::new(root.path().to_path_buf());
    let (is_error, result) = call(
        &mut server,
        "research_campaign_run_offline",
        json!({
            "spec_path": "campaign/spec.json",
            "stage_input_paths": { "measure": "campaign/research.json" },
            "output_dir": "research-output",
            "confirm": true
        }),
    );
    assert!(!is_error, "{result}");
    assert_eq!(result["execution"], json!({ "state": "completed" }));
    assert_eq!(
        result["stages"][0]["disposition"],
        json!("completed_with_negative_findings")
    );
    assert!(!result.to_string().contains("PRIVATE_RESEARCH_QUESTION"));
    assert!(root
        .path()
        .join("research-output/artifacts/0001-research-dossier.json")
        .is_file());
}
