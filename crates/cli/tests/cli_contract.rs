//! The CLI contract of blueprint 40.13.
//!
//! Each invariant in that contract is asserted here rather than documented and hoped for:
//! `--json` is parseable with nothing else on stdout, `--dry-run` has no effects, exit codes
//! follow the published matrix, and artifacts are byte-identical to the reference runtime.

use bioprism_devplat::{run_workbench, StudioSession, WorkbenchRequest};
use serde_json::Value;
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
