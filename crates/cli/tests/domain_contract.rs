//! The domain-pack surface of blueprint 40.13's CLI slice.
//!
//! A domain pack (`bioprism-domain/0.1`) carries a decision question the reference oracle was not
//! born knowing: a rule oracle, the tags its queries should protect, and the scope dimensions its
//! worlds use. These tests pin the CLI's side of that contract: `--domain` routes the compile to
//! the pack's oracle and names the pack in the output, a malformed pack is refused rather than
//! silently compiled without, the pack's advisories surface what a query fails to honour, and
//! `world validate --dimensions` classifies the pack's dimensions instead of warning on them.

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_bioprism");

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn trade_fixture(file: &str) -> PathBuf {
    let mut path = repo_root();
    path.push("fixtures");
    path.push("domains");
    path.push("trade-surveillance");
    path.push(file);
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

fn world() -> String {
    trade_fixture("world.json").display().to_string()
}

fn query() -> String {
    trade_fixture("query.json").display().to_string()
}

fn pack() -> String {
    trade_fixture("domain.json").display().to_string()
}

#[test]
fn a_domain_pack_routes_the_compile_to_its_rule_oracle_and_names_itself_in_the_output() {
    let output = run(&[
        "--json", "context", "compile", "--world", &world(), "--query", &query(), "--domain",
        &pack(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "an invalid verdict is a finding, not a failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("compile JSON");
    assert_eq!(
        parsed["oracle"]["kind"],
        Value::String("rule/trade-surveillance-v1".into()),
        "the certificate must carry the pack's oracle kind, not the reference one"
    );
    assert_eq!(parsed["oracle"]["status"], Value::String("invalid".into()));
    assert!(
        parsed["oracle"]["witnesses"]
            .as_array()
            .expect("witness list")
            .iter()
            .any(|kind| kind == "domain_check"),
        "the violation must arrive as a checkable domain_check witness: {parsed}"
    );
    assert_eq!(
        parsed["domain"]["name"],
        Value::String("trade-surveillance".into())
    );
    assert_eq!(
        parsed["domain"]["oracle_kind"],
        Value::String("rule/trade-surveillance-v1".into())
    );
    assert_eq!(
        parsed["domain"]["advisories"],
        Value::Array(Vec::new()),
        "the fixture query honours everything the pack declares"
    );
}

#[test]
fn without_the_pack_the_same_world_reads_clean_which_is_exactly_the_gap_packs_exist_to_close() {
    let output = run(&[
        "--json", "context", "compile", "--world", &world(), "--query", &query(),
    ]);
    assert_eq!(code(&output), 0);
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("compile JSON");
    assert_eq!(
        parsed["oracle"]["kind"],
        Value::String("deterministic_split_integrity_v1".into())
    );
    assert_eq!(
        parsed["oracle"]["status"],
        Value::String("valid".into()),
        "the reference oracle does not know this decision and reads the wash trade as clean"
    );
    assert_eq!(parsed["oracle"]["witnesses"], Value::Array(Vec::new()));
}

#[test]
fn a_malformed_domain_pack_exits_three_rather_than_compiling_without_it() {
    let directory = scratch("bad-pack");
    let bad = directory.join("pack.json");
    std::fs::write(&bad, r#"{"schema_version":"bioprism-domain/0.9"}"#).unwrap();
    let output = run(&[
        "--json",
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &query(),
        "--domain",
        &bad.display().to_string(),
    ]);
    assert_eq!(code(&output), 3, "a malformed pack is invalid_input");
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("error envelope");
    assert_eq!(parsed["ok"], Value::Bool(false));
    assert_eq!(parsed["error"]["kind"], Value::String("invalid_input".into()));
    assert_eq!(
        parsed["error"]["retryability"],
        Value::String("terminal".into())
    );
}

#[test]
fn the_advisories_name_the_unprotected_pack_tag_and_point_a_goalless_query_at_the_packs_goal() {
    let directory = scratch("advisories");
    let mut query_document: Value =
        serde_json::from_str(&std::fs::read_to_string(trade_fixture("query.json")).unwrap())
            .unwrap();
    let map = query_document.as_object_mut().unwrap();
    map.remove("goal");
    map.insert("protected_tags".into(), serde_json::json!(["time", "protected"]));
    let stripped = directory.join("query.json");
    std::fs::write(&stripped, serde_json::to_string_pretty(&query_document).unwrap()).unwrap();

    let output = run(&[
        "--json",
        "context",
        "compile",
        "--world",
        &world(),
        "--query",
        &stripped.display().to_string(),
        "--domain",
        &pack(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("compile JSON");
    let advisories: Vec<&str> = parsed["domain"]["advisories"]
        .as_array()
        .expect("advisory list")
        .iter()
        .map(|advisory| advisory.as_str().expect("advisory string"))
        .collect();
    assert!(
        advisories
            .iter()
            .any(|advisory| advisory.contains("\"identity\"")),
        "the dropped protected tag must be named: {advisories:?}"
    );
    assert!(
        advisories
            .iter()
            .any(|advisory| advisory.contains("declares no goal")
                && advisory.contains("wash trading")),
        "a goalless query must be pointed at the pack's declared goal: {advisories:?}"
    );
}

#[test]
fn context_compare_refuses_a_domain_pack_rather_than_half_applying_it() {
    let output = run(&[
        "context", "compare", "--world", &world(), "--query", &query(), "--domain", &pack(),
    ]);
    assert_eq!(code(&output), 2, "unsupported flag combination is a usage error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not supported on context compare"),
        "the refusal must say why instead of just rejecting the flag: {stderr}"
    );
}

#[test]
fn a_dimension_document_clears_the_unclassified_warnings_the_default_registry_reports() {
    let unclassified_count = |parsed: &Value| {
        parsed["diagnostics"]
            .as_array()
            .expect("diagnostics list")
            .iter()
            .filter(|d| d["code"] == "unclassified_scope_dimension")
            .count()
    };

    let bare = run(&["--json", "world", "validate", "--world", &world()]);
    assert_eq!(code(&bare), 0, "unclassified dimensions warn, not error");
    let bare: Value = serde_json::from_str(&stdout(&bare)).expect("validate JSON");
    assert_eq!(bare["dimensions_source"], Value::String("default".into()));
    assert!(
        unclassified_count(&bare) > 0,
        "without the pack's document the domain dimensions must be reported unclassified: {bare}"
    );

    let pack_document: Value =
        serde_json::from_str(&std::fs::read_to_string(trade_fixture("domain.json")).unwrap())
            .unwrap();
    let directory = scratch("dimensions");
    let dimensions = directory.join("dimensions.json");
    std::fs::write(
        &dimensions,
        serde_json::to_string_pretty(&pack_document["scope_dimensions"]).unwrap(),
    )
    .unwrap();

    let classified = run(&[
        "--json",
        "world",
        "validate",
        "--world",
        &world(),
        "--dimensions",
        &dimensions.display().to_string(),
    ]);
    assert_eq!(code(&classified), 0);
    let classified: Value = serde_json::from_str(&stdout(&classified)).expect("validate JSON");
    assert_eq!(
        classified["dimensions_source"],
        Value::String(dimensions.display().to_string()),
        "a clean report must say which classification it is clean under"
    );
    assert_eq!(
        unclassified_count(&classified),
        0,
        "the pack's document classifies every domain dimension: {classified}"
    );
}

#[test]
fn a_malformed_dimension_document_exits_three() {
    let directory = scratch("bad-dimensions");
    let bad = directory.join("dimensions.json");
    std::fs::write(&bad, r#"{"schema_version":"bioprism-scope-dimensions/0.9"}"#).unwrap();
    let output = run(&[
        "--json",
        "world",
        "validate",
        "--world",
        &world(),
        "--dimensions",
        &bad.display().to_string(),
    ]);
    assert_eq!(code(&output), 3);
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("error envelope");
    assert_eq!(parsed["error"]["kind"], Value::String("invalid_input".into()));
}
