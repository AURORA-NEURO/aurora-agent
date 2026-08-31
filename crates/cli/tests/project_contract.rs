//! The project-modeling surface of blueprint 40.13's CLI slice.
//!
//! `bioprism-project` compiles a software tree into a fiber world and judges its release
//! readiness. These tests pin the CLI's side of that: the emitted documents are consistent with
//! each other (the world validates cleanly under the dimension document emitted beside it), the
//! verdict arrives as checkable witnesses rather than a score, each declared issue's compiled
//! region is on the wire, two scans of one tree agree byte for byte, a root that cannot be read
//! lands on its documented exit code, and `--dry-run` touches nothing.
//!
//! Every assertion runs against `fixtures/projects/demo-app`, which declares one unpinned
//! dependency (`loose-gadget`), one pinned one, a CI workflow, one `#[test]`, one TODO, a
//! non-UTF-8 asset, and two issues — one naming a component, one naming none.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_bioprism");

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn demo_app() -> String {
    let mut path = repo_root();
    path.push("fixtures");
    path.push("projects");
    path.push("demo-app");
    path.display().to_string()
}

fn demo_issues() -> String {
    let mut path = repo_root();
    path.push("fixtures");
    path.push("projects");
    path.push("demo-app");
    path.push("issues.json");
    path.display().to_string()
}

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("bioprism-cli-project-{name}"));
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

/// Ingests the demo app into `directory`, returning the parsed `--json` document.
fn ingest_into(directory: &Path, extra: &[&str]) -> Value {
    let world = directory.join("world.json").display().to_string();
    let pack = directory.join("pack.json").display().to_string();
    let dimensions = directory.join("dimensions.json").display().to_string();
    let mut arguments: Vec<String> = [
        "--json",
        "project",
        "ingest",
        "--root",
        &demo_app(),
        "--issues",
        &demo_issues(),
        "--world-out",
        &world,
        "--pack-out",
        &pack,
        "--dimensions-out",
        &dimensions,
    ]
    .iter()
    .map(|argument| argument.to_string())
    .collect();
    arguments.extend(extra.iter().map(|argument| argument.to_string()));

    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let output = run(&borrowed);
    assert_eq!(
        code(&output),
        0,
        "ingest failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(&stdout(&output)).expect("ingest JSON")
}

fn audit_json() -> (i32, Value) {
    let output = run(&[
        "--json",
        "project",
        "audit",
        "--root",
        &demo_app(),
        "--issues",
        &demo_issues(),
    ]);
    let parsed = serde_json::from_str(&stdout(&output)).expect("audit JSON");
    (code(&output), parsed)
}

#[test]
fn an_ingested_world_validates_under_the_dimension_document_emitted_beside_it() {
    let directory = scratch("validate");
    let ingested = ingest_into(&directory, &[]);
    assert!(
        ingested["facts"].as_u64().expect("fact count") > 0,
        "an empty world would make the validation vacuous: {ingested}"
    );

    let output = run(&[
        "--json",
        "world",
        "validate",
        "--world",
        &directory.join("world.json").display().to_string(),
        "--dimensions",
        &directory.join("dimensions.json").display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        0,
        "the emitted world must satisfy the reference validator: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_str(&stdout(&output)).expect("validate JSON");
    assert_eq!(report["errors"], Value::from(0));
    let unclassified = report["diagnostics"]
        .as_array()
        .expect("diagnostics list")
        .iter()
        .filter(|diagnostic| diagnostic["code"] == "unclassified_scope_dimension")
        .count();
    assert_eq!(
        unclassified, 0,
        "the emitted dimension document must classify every dimension the emitted world's \
         scopes bind, or the two documents disagree about the same world: {report}"
    );
}

#[test]
fn the_audit_verdict_carries_the_fired_unpinned_dependency_witness_naming_the_loose_dependency() {
    let (status, parsed) = audit_json();
    assert_eq!(
        status, 1,
        "an invalid verdict is a completed run whose checked property does not hold: {parsed}"
    );
    assert_eq!(parsed["verdict"]["status"], Value::from("invalid"));
    assert_eq!(
        parsed["verdict"]["oracle_kind"],
        Value::from("rule/project-release-readiness-v1"),
        "the verdict must name the emitted pack's oracle, not the reference one"
    );

    let witnesses = parsed["verdict"]["witnesses"]
        .as_array()
        .expect("witness list");
    let unpinned = witnesses
        .iter()
        .find(|witness| witness["check"] == "unpinned_dependency")
        .unwrap_or_else(|| panic!("no unpinned_dependency witness among {witnesses:?}"));
    assert_eq!(unpinned["type"], Value::from("domain_check"));
    let observed = serde_json::to_string(&unpinned["observed"]).expect("observed bindings");
    assert!(
        observed.contains("loose-gadget"),
        "the witness must name the dependency it fired on so a reader can re-check it by hand: \
         {observed}"
    );
    assert!(
        !observed.contains("exact-widget"),
        "the pinned dependency is not a violation and must not appear in the bindings that \
         justify one: {observed}"
    );
    let detail = unpinned["detail"].as_str().expect("witness detail");
    assert!(
        detail.contains("static manifest scan"),
        "the witness must carry its own static-proxy caveat, because it is quotable away from \
         this document: {detail}"
    );
}

#[test]
fn the_audit_publishes_the_compiled_evidence_region_of_every_declared_issue() {
    let (_status, parsed) = audit_json();
    let issues = parsed["issues"].as_object().expect("issue map");
    assert_eq!(
        issues.len(),
        2,
        "both declared issues must be compiled, not only the resolvable one: {issues:?}"
    );

    let named = &issues["ISSUE-1"];
    let region: Vec<&str> = named["region"]
        .as_array()
        .expect("region fact list")
        .iter()
        .map(|fact| fact.as_str().expect("fact id"))
        .collect();
    assert!(
        region.contains(&"fact.component.src"),
        "the issue declares src/lib.rs, so its region must carry that component's inventory: \
         {region:?}"
    );
    assert!(
        region.contains(&"fact.issue.ISSUE-1"),
        "the issue's own record must be in the region compiled for working it: {region:?}"
    );
    assert_eq!(
        named["region_facts"],
        Value::from(region.len()),
        "the published count must be the length of the published region"
    );
    assert_eq!(
        named["declared_components"],
        serde_json::json!(["src/lib.rs"]),
        "relevance here is declaration, so the declarations travel with the region"
    );

    let undeclared = &issues["ISSUE-2"];
    let undeclared_region: Vec<&str> = undeclared["region"]
        .as_array()
        .expect("region fact list")
        .iter()
        .map(|fact| fact.as_str().expect("fact id"))
        .collect();
    assert_eq!(
        undeclared["declared_components"],
        serde_json::json!([]),
        "an issue naming no component must report an empty declaration, not an absent key"
    );
    assert!(
        !undeclared_region
            .iter()
            .any(|fact| fact.starts_with("fact.component.")),
        "an issue that declares no component gets no component inventory: there is no semantic \
         search to fall back on: {undeclared_region:?}"
    );
}

#[test]
fn the_human_audit_prints_the_verdict_each_witness_the_loss_summary_and_every_issue_region() {
    let output = run(&[
        "project",
        "audit",
        "--root",
        &demo_app(),
        "--issues",
        &demo_issues(),
    ]);
    let text = stdout(&output);
    assert_eq!(
        code(&output),
        1,
        "the demo app declares an unpinned dependency"
    );
    assert!(
        text.contains("judged invalid by rule/project-release-readiness-v1"),
        "the verdict and the oracle that reached it must lead the report:\n{text}"
    );
    assert!(
        text.contains("witness unpinned_dependency"),
        "a human reader gets the checkable witness, not only a status:\n{text}"
    );
    assert!(
        text.contains("observed unpinned_dependencies ="),
        "the bindings the check read must be printed so it can be re-run by hand:\n{text}"
    );
    assert!(
        text.contains("scan loss 11 entries:"),
        "a verdict printed without what the scan skipped is a verdict about a tree nobody \
         scanned:\n{text}"
    );
    assert!(
        text.contains("issue ISSUE-1") && text.contains("fact.component.src"),
        "each issue's compiled region must be printed, not just counted:\n{text}"
    );
    assert!(
        text.contains("\nNext: bioprism "),
        "40.13 requires a reproducible follow-up command in human mode:\n{text}"
    );
}

#[test]
fn two_ingests_of_the_same_tree_write_byte_identical_world_documents() {
    let first_directory = scratch("determinism-first");
    let second_directory = scratch("determinism-second");
    let first = ingest_into(&first_directory, &[]);
    let second = ingest_into(&second_directory, &[]);

    assert_eq!(
        first["world_id"], second["world_id"],
        "the world id is content-derived and must not move between two scans of one tree"
    );
    let first_bytes = std::fs::read(first_directory.join("world.json")).expect("first world");
    let second_bytes = std::fs::read(second_directory.join("world.json")).expect("second world");
    assert_eq!(
        first_bytes, second_bytes,
        "the emitted world must be byte-identical across runs: no clock, no directory iteration \
         order, and no hash-map order may reach the document"
    );
    let first_pack = std::fs::read(first_directory.join("pack.json")).expect("first pack");
    let second_pack = std::fs::read(second_directory.join("pack.json")).expect("second pack");
    assert_eq!(
        first_pack, second_pack,
        "the emitted pack must be stable too"
    );
}

#[test]
fn a_root_that_cannot_be_read_exits_five_and_reports_the_retry_decision() {
    let missing = repo_root()
        .join("fixtures")
        .join("projects")
        .join("no-such-project-tree");
    let output = run(&[
        "--json",
        "project",
        "audit",
        "--root",
        &missing.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        5,
        "a root that cannot be read is a dependency failure, not malformed input"
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("error envelope");
    assert_eq!(parsed["ok"], Value::Bool(false));
    assert_eq!(parsed["error"]["kind"], Value::from("io"));
    assert_eq!(
        parsed["error"]["retryability"],
        Value::from("retryable_as_is"),
        "the path may exist on a re-send, so the identical command may succeed unchanged"
    );
}

#[test]
fn a_dry_run_ingest_reports_every_planned_write_and_creates_no_file() {
    let directory = scratch("dry-run");
    let queries = directory.join("queries");
    let parsed = ingest_into(
        &directory,
        &["--queries-out", &queries.display().to_string(), "--dry-run"],
    );

    assert_eq!(parsed["dry_run"], Value::Bool(true));
    let artifacts = parsed["artifacts"].as_array().expect("artifact list");
    assert_eq!(
        artifacts.len(),
        6,
        "world, pack, dimensions, the release query and one query per issue must all be \
         reported as planned: {artifacts:?}"
    );
    for artifact in artifacts {
        assert_eq!(
            artifact["written"],
            Value::Bool(false),
            "a dry run declares the plan and performs none of it: {artifact}"
        );
        assert!(
            artifact["bytes"].as_u64().expect("byte count") > 0,
            "the plan must state how many bytes each write would produce: {artifact}"
        );
    }

    let left_behind: Vec<String> = std::fs::read_dir(&directory)
        .expect("scratch dir readable")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into())
        .collect();
    assert!(
        left_behind.is_empty(),
        "--dry-run must have no undeclared effects, and creating the output directory is an \
         effect: {left_behind:?}"
    );
}
