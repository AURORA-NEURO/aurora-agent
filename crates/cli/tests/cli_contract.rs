//! The CLI contract of blueprint 40.13.
//!
//! Each invariant in that contract is asserted here rather than documented and hoped for:
//! `--json` is parseable with nothing else on stdout, `--dry-run` has no effects, exit codes
//! follow the published matrix, and artifacts are byte-identical to the reference runtime.

use bioprism_devplat::{run_workbench, StudioSession, WorkbenchRequest};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_bioprism");

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn fixture(relative: &str) -> PathBuf {
    let mut path = repo_root();
    path.push("fixtures");
    path.push("fiber-v0.1");
    path.push(relative);
    path
}

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("bioprism-cli-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch dir");
    path
}

fn run(arguments: &[&str]) -> Output {
    Command::new(BIN)
        .args(arguments)
        .output()
        .expect("cli binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("process reported an exit code")
}

fn canonical(path: &Path) -> String {
    let text = std::fs::read_to_string(path).expect("readable json");
    let value: Value = serde_json::from_str(&text).expect("valid json");
    bioprism_ids::to_canonical_string(&value).expect("canonicalisable")
}

fn world() -> String {
    fixture("radiogenomic_world.json").display().to_string()
}

fn query() -> String {
    fixture("leakage_query.json").display().to_string()
}

#[test]
fn help_exits_zero_and_publishes_the_exit_code_matrix() {
    let output = run(&["--help"]);
    assert_eq!(code(&output), 0);
    let text = stdout(&output);
    for expected in [
        "0  ok",
        "1  assertion_failed",
        "2  usage",
        "3  invalid_input",
        "4  compile_failed",
        "5  io",
        "6  conflict",
        "7  policy_denied",
        "8  indeterminate",
        "9  stale",
        "Not a medical device",
    ] {
        assert!(text.contains(expected), "help must document {expected:?}");
    }
}

#[test]
fn help_publishes_a_retry_decision_against_every_failure_code_and_none_against_the_two_verdicts() {
    let text = stdout(&run(&["--help"]));
    let table: Vec<&str> = text
        .lines()
        .filter(|line| {
            line.starts_with("  ")
                && line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
        })
        .collect();
    assert_eq!(
        table.len(),
        10,
        "the help table lost or gained a row: {table:?}"
    );

    for line in &table[..2] {
        for decision in ["terminal", "retryable_after_change", "retryable_as_is"] {
            assert!(
                !line.contains(decision),
                "a verdict code publishes a retry decision: {line:?}"
            );
        }
    }
    for line in &table[2..] {
        assert!(
            ["terminal", "retryable_after_change", "retryable_as_is"]
                .iter()
                .any(|decision| line.ends_with(decision)),
            "a failure code publishes no retry decision, so a script cannot branch on it: {line:?}"
        );
    }
}

#[test]
fn json_mode_emits_exactly_one_document_and_no_progress_noise() {
    let output = run(&[
        "--json",
        "context",
        "explain",
        "--world",
        &world(),
        "--query",
        &query(),
    ]);
    assert_eq!(code(&output), 0);
    let text = stdout(&output);
    let parsed: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("stdout was not a single JSON document: {e}\n{text}"));
    assert_eq!(parsed["ok"], Value::Bool(true));
    assert_eq!(
        parsed["backend"],
        Value::String("backward_factor_slice_reference".into())
    );
    assert!(
        output.stderr.is_empty(),
        "json mode must not write to stderr on success"
    );
}

#[test]
fn dry_run_writes_nothing() {
    let directory = scratch("dry-run");
    let certificate = directory.join("cert.json");
    let output = run(&[
        "--json",
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
        "--certificate-out",
        &certificate.display().to_string(),
        "--dry-run",
    ]);
    assert_eq!(code(&output), 0);

    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["dry_run"], Value::Bool(true));
    assert_eq!(parsed["artifacts"][0]["written"], Value::Bool(false));
    assert!(!certificate.exists(), "--dry-run must not create files");
    assert_eq!(
        std::fs::read_dir(&directory).unwrap().count(),
        0,
        "--dry-run left files behind"
    );
}

#[test]
fn compiled_artifacts_are_byte_identical_to_the_reference() {
    let directory = scratch("parity");
    let certificate = directory.join("cert.json");
    let section = directory.join("section.json");

    let output = run(&[
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
        "--certificate-out",
        &certificate.display().to_string(),
        "--section-out",
        &section.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        canonical(&certificate),
        canonical(&fixture("golden/reference_certificate.json")),
        "certificate diverged from the reference runtime"
    );
    assert_eq!(
        canonical(&section),
        canonical(&fixture("golden/reference_section.json")),
        "decision section diverged from the reference runtime"
    );
}

#[test]
fn verify_accepts_a_sound_certificate_and_rejects_a_tampered_one() {
    let directory = scratch("verify");
    let certificate = directory.join("cert.json");
    run(&[
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
        "--certificate-out",
        &certificate.display().to_string(),
    ]);

    let good = run(&[
        "context",
        "verify",
        "--certificate",
        &certificate.display().to_string(),
    ]);
    assert_eq!(code(&good), 0);

    let mut document: Value =
        serde_json::from_str(&std::fs::read_to_string(&certificate).unwrap()).unwrap();
    document["selected_facts"]
        .as_array_mut()
        .unwrap()
        .push(Value::String("fact.smuggled".into()));
    let tampered = directory.join("tampered.json");
    std::fs::write(&tampered, serde_json::to_string_pretty(&document).unwrap()).unwrap();

    let bad = run(&[
        "--json",
        "context",
        "verify",
        "--certificate",
        &tampered.display().to_string(),
    ]);
    assert_eq!(
        code(&bad),
        1,
        "a tampered certificate must fail the assertion"
    );
    let parsed: Value = serde_json::from_str(&stdout(&bad)).unwrap();
    assert_eq!(parsed["ok"], Value::Bool(false));
    assert!(parsed["verification"]
        .as_str()
        .unwrap()
        .contains("digest mismatch"));
}

#[test]
fn fail_on_invalid_gates_ci_without_changing_the_compile() {
    let permissive = run(&[
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
    ]);
    assert_eq!(
        code(&permissive),
        0,
        "an invalid split is a finding, not a crash"
    );

    let gated = run(&[
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
        "--fail-on-invalid",
    ]);
    assert_eq!(code(&gated), 1);
}

#[test]
fn world_validate_passes_on_the_golden_world() {
    let output = run(&["--json", "world", "validate", "--world", &world()]);
    assert_eq!(code(&output), 0);
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["errors"], Value::from(0));
    assert_eq!(parsed["counts"]["facts"], Value::from(761));
    assert_eq!(
        parsed["world_sha256"],
        Value::String("b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5".into())
    );
}

#[test]
fn world_show_reports_the_distractor_population() {
    let output = run(&["--json", "world", "show", "--world", &world()]);
    assert_eq!(code(&output), 0);
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["fact_tags"]["exploratory"], Value::from(750));
    assert_eq!(parsed["fact_tags"]["protected"], Value::from(9));
    assert_eq!(
        parsed["factor_kinds"]["exploratory_summary"],
        Value::from(750)
    );
}

#[test]
fn bad_invocations_exit_two() {
    for arguments in [
        vec!["context", "compile", "--world", &world()],
        vec!["nonsense", "thing"],
        vec!["world"],
        vec![
            "context",
            "compile",
            "--world",
            &world(),
            "--query",
            &query(),
            "--profile",
            "banana",
        ],
        vec![
            "context",
            "explain",
            "--world",
            &world(),
            "--query",
            &query(),
            "--bogus",
            "x",
        ],
    ] {
        let output = run(&arguments);
        assert_eq!(code(&output), 2, "expected usage error for {arguments:?}");
    }
}

#[test]
fn a_malformed_world_exits_three_and_a_missing_file_exits_five() {
    let directory = scratch("errors");

    let broken = directory.join("broken.json");
    std::fs::write(&broken, r#"{"schema_version":"fiber-world/0.9"}"#).unwrap();
    let output = run(&[
        "world",
        "validate",
        "--world",
        &broken.display().to_string(),
    ]);
    assert_eq!(code(&output), 3, "schema rejection is invalid_input");

    let missing = directory.join("absent.json");
    let output = run(&[
        "--json",
        "world",
        "validate",
        "--world",
        &missing.display().to_string(),
    ]);
    assert_eq!(code(&output), 5, "an unreadable file is an io error");
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["error"]["retryable"], Value::Bool(true));
    assert_eq!(
        parsed["error"]["retryability"],
        Value::String("retryable_as_is".into())
    );

    let not_json = directory.join("not.json");
    std::fs::write(&not_json, "this is not json").unwrap();
    let output = run(&[
        "world",
        "validate",
        "--world",
        &not_json.display().to_string(),
    ]);
    assert_eq!(code(&output), 3);
}

#[test]
fn indexing_a_second_world_into_an_occupied_store_exits_six_and_says_it_is_terminal() {
    let directory = scratch("store-identity");
    let store = directory.join("index");

    let first = run(&[
        "world",
        "index",
        "--world",
        &world(),
        "--store",
        &store.display().to_string(),
    ]);
    assert_eq!(
        code(&first),
        0,
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let again = run(&[
        "world",
        "index",
        "--world",
        &world(),
        "--store",
        &store.display().to_string(),
    ]);
    assert_eq!(
        code(&again),
        0,
        "re-indexing the same world is the operation this command is for"
    );

    let other = directory.join("other.json");
    std::fs::write(
        &other,
        r#"{"schema_version":"fiber-world/0.1","world_id":"world.somebody_elses","facts":[],"factors":[],"events":[]}"#,
    )
    .unwrap();
    let rebind = run(&[
        "--json",
        "world",
        "index",
        "--world",
        &other.display().to_string(),
        "--store",
        &store.display().to_string(),
    ]);
    assert_eq!(
        code(&rebind),
        6,
        "rebinding a store to a second world is a conflict"
    );
    let parsed: Value = serde_json::from_str(&stdout(&rebind)).unwrap();
    assert_eq!(parsed["error"]["kind"], Value::String("conflict".into()));
    assert_eq!(parsed["error"]["retryable"], Value::Bool(false));
    assert_eq!(
        parsed["error"]["retryability"],
        Value::String("terminal".into())
    );
}

#[test]
fn a_store_written_under_another_schema_exits_nine_and_says_the_identical_command_may_be_resent() {
    let directory = scratch("stale-store");
    let store = directory.join("index");
    let built = run(&[
        "world",
        "index",
        "--world",
        &world(),
        "--store",
        &store.display().to_string(),
    ]);
    assert_eq!(
        code(&built),
        0,
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let manifest_path = store.join("manifest.json");
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["schema_version"] = Value::String("bioprism-store/0.0".into());
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let output = run(&[
        "--json",
        "context",
        "explain",
        "--world",
        &store.display().to_string(),
        "--query",
        &query(),
    ]);
    assert_eq!(
        code(&output),
        9,
        "an index built under another schema is stale, not malformed"
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["error"]["kind"], Value::String("stale".into()));
    assert_eq!(
        parsed["error"]["retryable"],
        Value::Bool(true),
        "the caller re-indexes and re-sends the identical command, so the code must not say terminal"
    );
}

#[test]
fn every_human_mode_command_prints_a_reproducible_follow_up() {
    for arguments in [
        vec!["world", "show", "--world", &world()],
        vec!["world", "validate", "--world", &world()],
        vec![
            "context",
            "explain",
            "--world",
            &world(),
            "--query",
            &query(),
        ],
        vec![
            "context",
            "compile",
            "--world",
            &world(),
            "--query",
            &query(),
        ],
    ] {
        let output = run(&arguments);
        let text = stdout(&output);
        assert!(
            text.contains("\nNext: bioprism "),
            "no follow-up command printed for {arguments:?}:\n{text}"
        );
    }
}

#[test]
fn workflow_portfolio_json_mode_preserves_no_dispatch_posture() {
    let directory = scratch("workflow-portfolio");
    let requests = directory.join("requests.json");
    std::fs::write(
        &requests,
        r#"{
          "requests": [{
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "cli-portfolio",
            "goal": "exercise the bounded portfolio handoff",
            "steps": [{"id": "capability", "tool": "workspace_capabilities", "arguments": {}}]
          }]
        }"#,
    )
    .expect("write portfolio request");
    let output = run(&[
        "--json",
        "workflow",
        "portfolio",
        "--requests",
        &requests.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_str(&stdout(&output)).expect("portfolio JSON");
    assert_eq!(report["workflow"], "domain_workflow_portfolio");
    assert_eq!(report["portfolio_ready"], true);
    assert_eq!(report["preflight"]["status"], "matched");
    assert_eq!(report["dispatch"], "not_started");
    assert_eq!(report["execution"], "not_started");
    assert_eq!(report["items"].as_array().map(Vec::len), Some(1));
}

#[test]
fn workflow_portfolio_verify_replays_a_retained_report_without_dispatch() {
    let directory = scratch("workflow-portfolio-verify");
    let requests = directory.join("requests.json");
    let portfolio = directory.join("portfolio.json");
    let replay_requests = directory.join("replay-requests.json");
    std::fs::write(
        &requests,
        r#"{
          "requests": [{
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "cli-portfolio-verify",
            "goal": "replay the bounded portfolio handoff",
            "steps": [{"id": "capability", "tool": "workspace_capabilities", "arguments": {}}]
          }]
        }"#,
    )
    .expect("write portfolio request");
    let planned = run(&[
        "--json",
        "workflow",
        "portfolio",
        "--requests",
        &requests.display().to_string(),
    ]);
    assert_eq!(code(&planned), 0, "{}", stdout(&planned));
    std::fs::write(&portfolio, stdout(&planned)).expect("write retained portfolio");
    std::fs::write(
        &replay_requests,
        r#"[
          {
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "cli-portfolio-verify",
            "goal": "replay the bounded portfolio handoff",
            "steps": [{"id": "capability", "tool": "workspace_capabilities", "arguments": {}}]
          }
        ]"#,
    )
    .expect("write replay requests");
    let output = run(&[
        "--json",
        "workflow",
        "portfolio-verify",
        "--portfolio",
        &portfolio.display().to_string(),
        "--replay-requests",
        &replay_requests.display().to_string(),
        "--require-replay",
    ]);
    assert_eq!(code(&output), 0, "{}", stdout(&output));
    let report: Value = serde_json::from_str(&stdout(&output)).expect("portfolio verify JSON");
    assert_eq!(report["workflow"], "domain_workflow_portfolio_verify");
    assert_eq!(report["valid"], true);
    assert_eq!(report["verification_status"], "verified");
    assert_eq!(report["summary"]["replay_matched_count"], 1);
    assert_eq!(report["preflight"]["status"], "matched");
    assert_eq!(report["items"][0]["status"], "verified");
    assert_eq!(report["dispatch"], "not_started");
    assert_eq!(report["execution"], "not_started");
}

#[test]
fn workbench_verify_replays_a_retained_report_without_execution() {
    let directory = scratch("workbench-verify");
    let session_path = directory.join("session.json");
    let report_path = directory.join("report.json");
    let session = StudioSession {
        session_id: "cli-workbench".into(),
        owner: "cli-test".into(),
        goal: "verify retained authoring evidence".into(),
        environment_digest: None,
        artifacts: Vec::new(),
        cells: Vec::new(),
        changes: Vec::new(),
        policy: Default::default(),
    };
    let retained = run_workbench(&WorkbenchRequest {
        session: session.clone(),
        dashboard: None,
        ci: None,
    })
    .expect("build retained workbench report");
    std::fs::write(&session_path, serde_json::to_vec_pretty(&session).unwrap()).unwrap();
    std::fs::write(&report_path, serde_json::to_vec_pretty(&retained).unwrap()).unwrap();
    let output = run(&[
        "--json",
        "workbench",
        "verify",
        "--session",
        &session_path.display().to_string(),
        "--report",
        &report_path.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_str(&stdout(&output)).expect("workbench verification JSON");
    assert_eq!(report["workflow"], "developer_workbench_verify");
    assert_eq!(report["valid"], true);
    assert_eq!(report["status"], "verified");
    assert_eq!(report["execution"], "not_started");
    assert_eq!(report["network_access"], "not_started");
}

/// Instantiate a one-step documentation workflow through the CLI itself and retain the
/// instantiation artifact, so autopilot tests drive exactly what an operator would.
fn instantiated_workflow(directory: &Path, mission_id: &str) -> PathBuf {
    let steps = directory.join("steps.json");
    std::fs::write(
        &steps,
        r#"[{"id": "capability", "tool": "workspace_capabilities", "arguments": {}}]"#,
    )
    .expect("write steps");
    let output = run(&[
        "--json",
        "workflow",
        "instantiate",
        "--workflow",
        "documentation_and_knowledge",
        "--mission-id",
        mission_id,
        "--goal",
        "drive the bounded capability handoff",
        "--steps",
        &steps.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let instantiation = directory.join("instantiation.json");
    std::fs::write(&instantiation, stdout(&output)).expect("retain instantiation");
    instantiation
}

fn autopilot_grant(directory: &Path, allowed_tool: &str) -> PathBuf {
    let grant = directory.join("grant.json");
    std::fs::write(
        &grant,
        format!(r#"{{"allowed_tools": ["{allowed_tool}"], "max_attempts": 2}}"#),
    )
    .expect("write grant");
    grant
}

fn instantiated_mission(directory: &Path, mission_id: &str) -> Value {
    let path = instantiated_workflow(directory, mission_id);
    let instantiation: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read instantiation"))
            .expect("valid instantiation");
    instantiation["mission"].clone()
}

fn write_goal_request(directory: &Path, name: &str, request: &Value) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(request).expect("serialize goal request"),
    )
    .expect("write goal request");
    path
}

fn run_goal_step(request: &Path, recovery_dir: &Path) -> Output {
    run(&[
        "--json",
        "autopilot",
        "goal-step",
        "--request",
        &request.display().to_string(),
        "--recovery-dir",
        &recovery_dir.display().to_string(),
    ])
}

fn run_goal_step_with_report(request: &Path, recovery_dir: &Path, report_out: &Path) -> Output {
    run(&[
        "--json",
        "autopilot",
        "goal-step",
        "--request",
        &request.display().to_string(),
        "--recovery-dir",
        &recovery_dir.display().to_string(),
        "--report-out",
        &report_out.display().to_string(),
    ])
}

#[test]
fn help_documents_every_autopilot_subcommand_and_names_the_grant_as_the_only_authority() {
    let text = stdout(&run(&["--help"]));
    for expected in [
        "autopilot grant-template",
        "autopilot run     --instantiation <path> --grant <path> [--report-out <path>]",
        "autopilot resume  --instantiation <path> --grant <path> --recovery-dir <dir>",
        "autopilot goal-step --request <path> --recovery-dir <dir> [--report-out <path>]",
        "autopilot goal-verify [--report <path>] [--checkpoint <path>]",
        "autopilot verify  --report <path>",
        "comes only from an explicit grant document",
        "Exit 1 reports a completed drive",
        "Exit 1 if the report does not verify",
    ] {
        assert!(text.contains(expected), "help must document {expected:?}");
    }
}

#[test]
fn autopilot_grant_template_json_mode_emits_exactly_one_bare_grant_document() {
    let output = run(&["--json", "autopilot", "grant-template"]);
    assert_eq!(code(&output), 0);
    let text = stdout(&output);
    let template: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("stdout was not a single JSON document: {e}\n{text}"));
    assert!(
        template.get("ok").is_none(),
        "the --json template is the bare grant object, directly usable as --grant"
    );
    serde_json::from_value::<bioprism_autopilot::AutonomyGrant>(template)
        .expect("the emitted template must satisfy the grant validator unedited");
    assert!(
        output.stderr.is_empty(),
        "json mode must not write to stderr on success"
    );

    let human = stdout(&run(&["autopilot", "grant-template"]));
    assert!(human.contains("\nNext: bioprism "));
    assert!(
        human.contains("never re-sent"),
        "the commented template must explain the terminal-retry posture"
    );
}

#[test]
fn autopilot_run_dry_run_dispatches_nothing_writes_nothing_and_labels_itself_no_dispatch() {
    let directory = scratch("autopilot-dry-run");
    let instantiation = instantiated_workflow(&directory, "autopilot-dry");
    let grant = autopilot_grant(&directory, "workspace_capabilities");
    let report_out = directory.join("autopilot-report.json");
    let recovery_dir = directory.join("autopilot-state");
    let files_before = std::fs::read_dir(&directory).unwrap().count();

    let output = run(&[
        "--json",
        "autopilot",
        "run",
        "--instantiation",
        &instantiation.display().to_string(),
        "--grant",
        &grant.display().to_string(),
        "--report-out",
        &report_out.display().to_string(),
        "--recovery-dir",
        &recovery_dir.display().to_string(),
        "--dry-run",
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("dry-run JSON");
    assert_eq!(parsed["ok"], Value::Bool(true));
    assert_eq!(parsed["dry_run"], Value::Bool(true));
    assert_eq!(parsed["no_dispatch"], Value::Bool(true));
    assert_eq!(parsed["dispatch"], "not_started");
    assert_eq!(parsed["writes"], "none");
    assert_eq!(parsed["planned_first_action"]["action"], "dispatch_full");
    assert_eq!(parsed["planned_first_action"]["attempt_index"], 1);
    assert_eq!(
        parsed["planned_first_action"]["mission"]["policy"]["execute"],
        Value::Bool(true),
        "the previewed mission must show the policy the grant would apply"
    );
    assert!(!report_out.exists(), "--dry-run must not write the report");
    assert_eq!(
        std::fs::read_dir(&directory).unwrap().count(),
        files_before,
        "--dry-run left files behind"
    );
}

#[test]
fn autopilot_run_with_a_grant_that_does_not_cover_the_mission_tool_exits_seven() {
    let directory = scratch("autopilot-denied");
    let instantiation = instantiated_workflow(&directory, "autopilot-denied");
    let grant = autopilot_grant(&directory, "some_other_tool");
    let output = run(&[
        "--json",
        "autopilot",
        "run",
        "--instantiation",
        &instantiation.display().to_string(),
        "--grant",
        &grant.display().to_string(),
        "--dry-run",
    ]);
    assert_eq!(
        code(&output),
        7,
        "a well-formed mission the grant does not authorise is a policy refusal"
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["ok"], Value::Bool(false));
    assert_eq!(
        parsed["error"]["kind"],
        Value::String("policy_denied".into())
    );
}

#[test]
fn autopilot_run_drives_a_real_mission_to_success_and_verify_rejects_a_tampered_report() {
    let directory = scratch("autopilot-run");
    let instantiation = instantiated_workflow(&directory, "autopilot-live");
    let grant = autopilot_grant(&directory, "workspace_capabilities");
    let report_out = directory.join("autopilot-report.json");
    let recovery_dir = directory.join("autopilot-state");

    let output = run(&[
        "--json",
        "autopilot",
        "run",
        "--instantiation",
        &instantiation.display().to_string(),
        "--grant",
        &grant.display().to_string(),
        "--report-out",
        &report_out.display().to_string(),
        "--recovery-dir",
        &recovery_dir.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "stdout: {}\nstderr: {}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("run JSON");
    assert_eq!(parsed["ok"], Value::Bool(true));
    assert_eq!(parsed["final_status"], "succeeded");
    assert_eq!(parsed["attempts_used"], 1);
    assert_eq!(
        parsed["report"]["attempts"][0]["reconciliation_status"]["completion"], "complete",
        "success must rest on a retained reconciliation, never inference"
    );
    assert!(
        report_out.exists(),
        "the run must retain its report artifact"
    );
    assert_eq!(parsed["recovery"]["generation"], 1);
    let checkpoint_path = recovery_dir.join("checkpoint-000001.json");
    let private_attempt_path = recovery_dir.join("attempt-000001.private.json");
    assert!(
        checkpoint_path.exists(),
        "the digest-only checkpoint is retained"
    );
    assert!(
        private_attempt_path.exists(),
        "rehydration material is kept separately"
    );
    let checkpoint: Value =
        serde_json::from_str(&std::fs::read_to_string(&checkpoint_path).unwrap()).unwrap();
    assert_eq!(checkpoint["secret_material"], "never_returned");
    assert!(checkpoint.get("mission").is_none());
    assert!(checkpoint.get("report").is_none());
    let private_attempt: Value =
        serde_json::from_str(&std::fs::read_to_string(&private_attempt_path).unwrap()).unwrap();
    assert!(private_attempt["attempt"].get("mission").is_some());
    assert!(private_attempt["attempt"].get("report").is_some());

    let resumed_report = directory.join("autopilot-resumed-report.json");
    let resumed = run(&[
        "--json",
        "autopilot",
        "resume",
        "--instantiation",
        &instantiation.display().to_string(),
        "--grant",
        &grant.display().to_string(),
        "--recovery-dir",
        &recovery_dir.display().to_string(),
        "--report-out",
        &resumed_report.display().to_string(),
    ]);
    assert_eq!(
        code(&resumed),
        0,
        "resume stdout: {}\nresume stderr: {}",
        stdout(&resumed),
        String::from_utf8_lossy(&resumed.stderr)
    );
    let resumed_json: Value = serde_json::from_str(&stdout(&resumed)).unwrap();
    assert_eq!(resumed_json["workflow"], "autopilot_resume");
    assert_eq!(resumed_json["resumed"], true);
    assert_eq!(resumed_json["attempts_used"], 1);
    assert_eq!(resumed_json["final_status"], "succeeded");

    let mut checkpoint: Value =
        serde_json::from_str(&std::fs::read_to_string(&checkpoint_path).unwrap()).unwrap();
    checkpoint["base_mission_id"] = Value::String("edited-checkpoint".into());
    std::fs::write(
        &checkpoint_path,
        serde_json::to_string_pretty(&checkpoint).unwrap(),
    )
    .unwrap();
    let refused_resume_report = directory.join("refused-resume-report.json");
    let refused_resume = run(&[
        "--json",
        "autopilot",
        "resume",
        "--instantiation",
        &instantiation.display().to_string(),
        "--grant",
        &grant.display().to_string(),
        "--recovery-dir",
        &recovery_dir.display().to_string(),
        "--report-out",
        &refused_resume_report.display().to_string(),
    ]);
    assert_ne!(code(&refused_resume), 0);
    assert!(!refused_resume_report.exists());

    let good = run(&[
        "autopilot",
        "verify",
        "--report",
        &report_out.display().to_string(),
    ]);
    assert_eq!(code(&good), 0, "{}", stdout(&good));

    let mut document: Value =
        serde_json::from_str(&std::fs::read_to_string(&report_out).unwrap()).unwrap();
    document["base_mission_id"] = Value::String("smuggled".into());
    let tampered = directory.join("tampered.json");
    std::fs::write(&tampered, serde_json::to_string_pretty(&document).unwrap()).unwrap();

    let bad = run(&[
        "--json",
        "autopilot",
        "verify",
        "--report",
        &tampered.display().to_string(),
    ]);
    assert_eq!(
        code(&bad),
        1,
        "a tampered autopilot report must fail the assertion"
    );
    let parsed: Value = serde_json::from_str(&stdout(&bad)).unwrap();
    assert_eq!(parsed["ok"], Value::Bool(false));
    assert_eq!(parsed["valid"], Value::Bool(false));
    assert_eq!(parsed["digest_match"], Value::Bool(false));
}

#[test]
fn the_extended_profile_is_selectable_and_reports_sufficiency() {
    let output = run(&[
        "--json",
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
        "--profile",
        "extended",
    ]);
    assert_eq!(code(&output), 0);
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["profile"], Value::String("extended".into()));
    assert_eq!(parsed["supports_sufficiency_claim"], Value::Bool(true));
    assert_ne!(
        parsed["certificate_sha256"],
        Value::String("c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4".into()),
        "the extended profile hashes different bytes"
    );
}

fn write_research_request(directory: &Path, points: &str, extras: &str) -> PathBuf {
    let request = directory.join("request.json");
    let body = format!(
        r#"{{
  "research_id": "cli-contract-run",
  "question": "Does the compiled section stay admissible under fifty distractors?",
  "family": "discriminating",
  "distractor_points": [{points}],
  "seed": 7{extras}
}}"#
    );
    std::fs::write(&request, body).expect("request written");
    request
}

#[test]
fn research_template_json_emits_one_bare_document_that_validates_as_a_request() {
    let output = run(&["--json", "research", "template"]);
    assert_eq!(code(&output), 0);
    let text = stdout(&output);
    let parsed: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("stdout was not a single JSON document: {e}\n{text}"));
    assert!(
        parsed.get("ok").is_none() && parsed.get("workflow").is_none(),
        "--json must print the bare request object, not an envelope: {parsed}"
    );
    assert_eq!(parsed["family"], Value::String("discriminating".into()));
    let request: bioprism_research::ResearchRequest = serde_json::from_value(parsed)
        .expect("the printed template must itself pass request validation");
    assert_eq!(request.distractor_points(), &[50, 250, 750]);
}

#[test]
fn research_dry_run_prints_the_planned_protocol_and_writes_nothing() {
    let directory = scratch("research-dry-run");
    let request = write_research_request(
        &directory,
        "50, 250",
        ",\n  \"run_sweep\": true,\n  \"run_mutation\": true,\n  \"run_minimize\": true",
    );
    let out_dir = directory.join("out");
    let output = run(&[
        "--json",
        "research",
        "run",
        "--request",
        &request.display().to_string(),
        "--out-dir",
        &out_dir.display().to_string(),
        "--dry-run",
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["dry_run"], Value::Bool(true));
    assert_eq!(parsed["no_dispatch"], Value::Bool(true));
    assert_eq!(parsed["writes"], Value::String("none".into()));
    assert_eq!(
        parsed["step_count"],
        Value::Number(10.into()),
        "anchor + 2 x (generate, compile, compare) + sweep + mutate + minimize"
    );
    assert_eq!(
        parsed["planned_protocol"]["steps"][0]["kind"],
        Value::String("anchor_reference_fixture".into())
    );
    assert!(!out_dir.exists(), "--dry-run must not create the out-dir");
    assert_eq!(
        std::fs::read_dir(&directory).unwrap().count(),
        1,
        "--dry-run left files beside the request"
    );
}

#[test]
fn an_invalid_research_request_exits_three_naming_the_rule() {
    let directory = scratch("research-invalid");
    let request = write_research_request(&directory, "", "");
    let output = run(&[
        "research",
        "run",
        "--request",
        &request.display().to_string(),
        "--out-dir",
        &directory.join("out").display().to_string(),
    ]);
    assert_eq!(code(&output), 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("at least one point"),
        "the refusal must name the rule, not just the field: {stderr}"
    );
}

#[test]
fn research_run_writes_a_verifiable_dossier_and_a_tampered_one_exits_one() {
    let directory = scratch("research-run");
    let request = write_research_request(&directory, "50", "");
    let out_dir = directory.join("out");
    let output = run(&[
        "--json",
        "research",
        "run",
        "--request",
        &request.display().to_string(),
        "--out-dir",
        &out_dir.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(parsed["ok"], Value::Bool(true));
    assert!(
        parsed["findings_total"].as_u64().unwrap_or(0) >= 2,
        "a completed run derives at least the anchor and comparison findings: {parsed}"
    );

    let dossier = out_dir.join("dossier.json");
    let report = out_dir.join("REPORT.md");
    assert!(dossier.is_file(), "run must write dossier.json");
    assert!(report.is_file(), "run must write REPORT.md");
    let figures: Vec<_> = std::fs::read_dir(out_dir.join("figures"))
        .expect("run must write a figures directory")
        .map(|entry| entry.expect("figure entry").file_name())
        .collect();
    assert_eq!(
        figures.len() as u64,
        parsed["figures"].as_u64().expect("figures count"),
        "every rendered figure lands in figures/"
    );
    let report_text = std::fs::read_to_string(&report).expect("report readable");
    for figure in &figures {
        assert!(
            report_text.contains(&format!("./figures/{}", figure.to_string_lossy())),
            "REPORT.md must link {figure:?} under ./figures/"
        );
    }
    assert!(
        report_text.contains("## Limitations"),
        "the report must carry its limitations block"
    );

    let verified = run(&[
        "research",
        "verify",
        "--dossier",
        &dossier.display().to_string(),
    ]);
    assert_eq!(
        code(&verified),
        0,
        "{}",
        String::from_utf8_lossy(&verified.stdout)
    );

    let text = std::fs::read_to_string(&dossier).expect("dossier readable");
    let tampered_text = text.replacen("fifty", "sixty", 1);
    assert_ne!(
        text, tampered_text,
        "the tamper must actually change a byte"
    );
    std::fs::write(&dossier, tampered_text).expect("tampered dossier written");
    let tampered = run(&[
        "--json",
        "research",
        "verify",
        "--dossier",
        &dossier.display().to_string(),
    ]);
    assert_eq!(code(&tampered), 1, "a tampered dossier must exit 1");
    let verdict: Value = serde_json::from_str(&stdout(&tampered)).unwrap();
    assert_eq!(verdict["ok"], Value::Bool(false));
    assert_eq!(verdict["valid"], Value::Bool(false));
    assert_eq!(verdict["digest_match"], Value::Bool(false));
}

#[test]
fn autopilot_goal_step_runs_and_resumes_without_replaying_or_exposing_private_reports() {
    let directory = scratch("autopilot-goal-resume");
    let first_mission = instantiated_mission(&directory, "goal-cycle-one");
    let second_mission = instantiated_mission(&directory, "goal-cycle-two");
    let recovery_dir = directory.join("goal-state");
    let first_request = write_goal_request(
        &directory,
        "goal-step-one.json",
        &json!({
            "goal_id": "goal-cli-resume",
            "grant": {"allowed_tools": ["workspace_capabilities"], "max_attempts": 2},
            "budget": {"max_cycles": 3, "max_total_dispatches": 6},
            "decision": {"kind": "run_mission", "mission": first_mission},
        }),
    );
    let first_report_path = directory.join("first-goal-report.json");
    let first = run_goal_step_with_report(&first_request, &recovery_dir, &first_report_path);
    assert_eq!(
        code(&first),
        0,
        "stdout: {}\nstderr: {}",
        stdout(&first),
        String::from_utf8_lossy(&first.stderr)
    );
    let first_json: Value = serde_json::from_str(&stdout(&first)).expect("first goal-step JSON");
    assert_eq!(first_json["workflow"], "autopilot_goal_step");
    assert_eq!(first_json["goal_status"], "stopped");
    assert_eq!(first_json["dispatch_started"], true);
    assert_eq!(first_json["dispatches_this_call"], 1);
    assert_eq!(first_json["recovery"]["state"], "safe_stop_checkpointed");
    assert_eq!(first_json["report"]["total_dispatches"], 1);
    assert_eq!(first_json["checkpoint_generation"], 1);
    assert!(first_json.get("cycle_autopilot_reports").is_none());
    assert!(first_json["report"]["cycles"][0].get("mission").is_none());
    assert!(first_json["report"]["cycles"][0]
        .get("autopilot_report")
        .is_none());

    let written_report: Value = serde_json::from_slice(
        &std::fs::read(&first_report_path).expect("goal report artifact was written"),
    )
    .expect("goal report artifact is JSON");
    assert_eq!(written_report, first_json["report"]);
    let first_private = recovery_dir.join("cycle-report-000001.private.json");
    assert!(
        first_private.is_file(),
        "the raw mission report is retained privately"
    );
    let private_text = std::fs::read_to_string(first_private).unwrap();
    assert!(private_text.contains("autopilot_report"));
    assert!(private_text.contains("workspace_capabilities"));

    let second_request = write_goal_request(
        &directory,
        "goal-step-two.json",
        &json!({
            "goal_id": "goal-cli-resume",
            "grant": {"allowed_tools": ["workspace_capabilities"], "max_attempts": 2},
            "decision": {"kind": "run_mission", "mission": second_mission},
        }),
    );
    let second = run_goal_step(&second_request, &recovery_dir);
    assert_eq!(
        code(&second),
        0,
        "stdout: {}\nstderr: {}",
        stdout(&second),
        String::from_utf8_lossy(&second.stderr)
    );
    let second_json: Value = serde_json::from_str(&stdout(&second)).expect("second goal-step JSON");
    assert_eq!(second_json["resumed"], true);
    assert_eq!(second_json["goal_status"], "stopped");
    assert_eq!(second_json["dispatch_started"], true);
    assert_eq!(second_json["dispatches_this_call"], 1);
    assert_eq!(second_json["report"]["cycles"].as_array().unwrap().len(), 2);
    assert_eq!(second_json["report"]["total_dispatches"], 2);
    assert_eq!(second_json["checkpoint_generation"], 2);
    assert!(recovery_dir
        .join("cycle-report-000002.private.json")
        .is_file());
    assert!(recovery_dir.join("checkpoint-000002.json").is_file());

    let second_report_path = directory.join("second-goal-report.json");
    let second_checkpoint_path = directory.join("second-goal-checkpoint.json");
    std::fs::write(
        &second_report_path,
        serde_json::to_vec_pretty(&second_json["report"]).unwrap(),
    )
    .unwrap();
    std::fs::write(
        &second_checkpoint_path,
        serde_json::to_vec_pretty(&second_json["checkpoint"]).unwrap(),
    )
    .unwrap();

    let valid = run(&[
        "--json",
        "autopilot",
        "goal-verify",
        "--report",
        &second_report_path.display().to_string(),
        "--checkpoint",
        &second_checkpoint_path.display().to_string(),
    ]);
    assert_eq!(
        code(&valid),
        0,
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    let valid_json: Value = serde_json::from_str(&stdout(&valid)).unwrap();
    assert_eq!(valid_json["ok"], true);
    assert_eq!(valid_json["checkpoint_verification"]["generation"], 2);

    let report_only = run(&[
        "--json",
        "autopilot",
        "goal-verify",
        "--report",
        &first_report_path.display().to_string(),
    ]);
    assert_eq!(
        code(&report_only),
        0,
        "{}",
        String::from_utf8_lossy(&report_only.stderr)
    );
    let report_only_json: Value = serde_json::from_str(&stdout(&report_only)).unwrap();
    assert_eq!(report_only_json["ok"], true);

    let checkpoint_only = run(&[
        "--json",
        "autopilot",
        "goal-verify",
        "--checkpoint",
        &second_checkpoint_path.display().to_string(),
    ]);
    assert_eq!(
        code(&checkpoint_only),
        0,
        "{}",
        String::from_utf8_lossy(&checkpoint_only.stderr)
    );
    let checkpoint_only_json: Value = serde_json::from_str(&stdout(&checkpoint_only)).unwrap();
    assert_eq!(checkpoint_only_json["ok"], true);

    let mismatched = run(&[
        "--json",
        "autopilot",
        "goal-verify",
        "--report",
        &first_report_path.display().to_string(),
        "--checkpoint",
        &second_checkpoint_path.display().to_string(),
    ]);
    assert_ne!(
        code(&mismatched),
        0,
        "a valid but unrelated report must not bind"
    );

    let tampered_recovery = directory.join("tampered-goal-state");
    std::fs::create_dir_all(&tampered_recovery).unwrap();
    for entry in std::fs::read_dir(&recovery_dir).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), tampered_recovery.join(entry.file_name())).unwrap();
    }
    let private_path = tampered_recovery.join("cycle-report-000001.private.json");
    let mut private_record: Value =
        serde_json::from_slice(&std::fs::read(&private_path).unwrap()).unwrap();
    private_record["record_digest"] = json!("0".repeat(64));
    std::fs::write(
        &private_path,
        serde_json::to_vec_pretty(&private_record).unwrap(),
    )
    .unwrap();
    let tampered_resume = run_goal_step(&second_request, &tampered_recovery);
    assert_ne!(
        code(&tampered_resume),
        0,
        "a modified private report must prevent checkpoint rehydration"
    );
    assert!(
        !tampered_recovery.join("pending.json").exists(),
        "failed rehydration must stop before dispatch intent is written"
    );
}

#[test]
fn autopilot_goal_completion_is_terminal_and_cannot_resume() {
    let directory = scratch("autopilot-goal-complete");
    let mission = instantiated_mission(&directory, "goal-completion-cycle");
    let recovery_dir = directory.join("goal-state");
    let first_request = write_goal_request(
        &directory,
        "goal-step-one.json",
        &json!({
            "goal_id": "goal-cli-complete",
            "grant": {"allowed_tools": ["workspace_capabilities"], "max_attempts": 2},
            "budget": {"max_cycles": 2, "max_total_dispatches": 4},
            "decision": {"kind": "run_mission", "mission": mission},
        }),
    );
    let first = run_goal_step(&first_request, &recovery_dir);
    assert_eq!(
        code(&first),
        0,
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let complete_request = write_goal_request(
        &directory,
        "goal-complete.json",
        &json!({
            "goal_id": "goal-cli-complete",
            "grant": {"allowed_tools": ["workspace_capabilities"], "max_attempts": 2},
            "decision": {
                "kind": "complete",
                "evaluator_id": "reviewed-evidence",
                "evidence_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            },
        }),
    );
    let complete = run_goal_step(&complete_request, &recovery_dir);
    assert_eq!(
        code(&complete),
        0,
        "stdout: {}\nstderr: {}",
        stdout(&complete),
        String::from_utf8_lossy(&complete.stderr)
    );
    let complete_json: Value = serde_json::from_str(&stdout(&complete)).unwrap();
    assert_eq!(complete_json["goal_status"], "completed");
    assert_eq!(complete_json["goal_complete"], true);
    assert_eq!(complete_json["dispatch_started"], false);
    assert!(recovery_dir.join("terminal.json").is_file());

    let retry = run_goal_step(&complete_request, &recovery_dir);
    assert_ne!(code(&retry), 0, "terminal goals must refuse resumption");
    assert!(
        stdout(&retry).contains("terminal and cannot resume")
            || String::from_utf8_lossy(&retry.stderr).contains("terminal and cannot resume"),
        "terminal refusal should explain why resumption stopped"
    );
}

#[test]
fn autopilot_goal_cannot_claim_completion_before_any_mission_or_create_recovery_state() {
    let directory = scratch("autopilot-goal-empty-completion");
    let recovery_dir = directory.join("goal-state");
    let request = write_goal_request(
        &directory,
        "goal-complete.json",
        &json!({
            "goal_id": "goal-no-evidence",
            "grant": {"allowed_tools": ["workspace_capabilities"], "max_attempts": 2},
            "decision": {
                "kind": "complete",
                "evaluator_id": "reviewed-evidence",
                "evidence_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            },
        }),
    );
    let refused = run_goal_step(&request, &recovery_dir);
    assert_ne!(code(&refused), 0, "empty goals cannot complete");
    assert!(
        stdout(&refused).contains("before a mission produces evidence")
            || String::from_utf8_lossy(&refused.stderr)
                .contains("before a mission produces evidence"),
        "the refusal should state the missing evidence prerequisite"
    );
    assert!(
        !recovery_dir.exists(),
        "an invalid completion assertion must not leave an identity-only recovery directory"
    );
}
