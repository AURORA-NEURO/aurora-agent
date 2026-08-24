//! `project plan` and `project verify`: the repair surface of blueprint 40.13's CLI slice.
//!
//! `bioprism-repair` derives a checkable repair plan for one declared issue and later reports
//! which of that plan's declared criteria held. These tests pin the CLI's side of it, and the
//! three that matter most are the ones that come out *against* the tool: verifying an unrepaired
//! tree reports `not_met` rather than congratulating it, a plan checked against a world it was not
//! planned from reports `stale` and evaluates nothing at all, and a criterion that could not be
//! evaluated exits on its own code rather than joining the determinate failures.
//!
//! Every assertion runs against `fixtures/projects/demo-app`, which declares one unpinned
//! dependency (`loose-gadget`), a `src` component, and two issues — ISSUE-1 naming `src/lib.rs`
//! and ISSUE-2 naming nothing.

use bioprism_repair::{Origin, RepairPlan};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_bioprism");

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn fixture(project: &str) -> String {
    let mut path = repo_root();
    path.push("fixtures");
    path.push("projects");
    path.push(project);
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

/// A scratch directory outside the scanned tree.
///
/// Deliberately not under `fixtures/`: every byte there is scanned, so a temporary file written
/// beside the fixture would change the world id the plan binds and make these tests report
/// staleness for a reason nobody intended.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("bioprism-cli-repair-{name}"));
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

/// Plans ISSUE-1 into `out`, returning the parsed `--json` document.
fn plan_issue_one(out: &Path, extra: &[&str]) -> Value {
    let mut arguments: Vec<String> = [
        "--json",
        "project",
        "plan",
        "--root",
        &fixture("demo-app"),
        "--issues",
        &demo_issues(),
        "--issue",
        "ISSUE-1",
        "--out",
        &out.display().to_string(),
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
        "planning failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(&stdout(&output)).expect("plan JSON")
}

fn verify_json(root: &str, plan: &Path, extra: &[&str]) -> (i32, Value) {
    let mut arguments: Vec<String> = [
        "--json",
        "project",
        "verify",
        "--root",
        root,
        "--plan",
        &plan.display().to_string(),
    ]
    .iter()
    .map(|argument| argument.to_string())
    .collect();
    arguments.extend(extra.iter().map(|argument| argument.to_string()));

    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let output = run(&borrowed);
    let parsed = serde_json::from_str(&stdout(&output)).unwrap_or_else(|error| {
        panic!(
            "verify emitted no JSON document ({error}); stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    // 40.13's one-document rule is checked on every verify these tests run, not once. Verify is
    // the first command here that reports a completed run under a non-zero status, and a
    // non-zero status is exactly where a diagnostic tends to leak onto stderr and turn a
    // pipeable document into a document plus a warning.
    assert!(
        output.stderr.is_empty(),
        "--json mode must emit one document on stdout and nothing else, whatever the exit code; \
         stderr carried: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (code(&output), parsed)
}

#[test]
fn a_written_plan_is_bound_to_the_world_and_region_it_was_planned_from() {
    let directory = scratch("bound-plan");
    let out = directory.join("plan.json");
    let reported = plan_issue_one(&out, &[]);

    let document: Value =
        serde_json::from_slice(&std::fs::read(&out).expect("the plan is on disk")).expect("JSON");
    let plan = RepairPlan::from_json(&document)
        .expect("the written document must satisfy the crate's own strict parser");

    assert_eq!(plan.issue_id(), "ISSUE-1");
    assert_eq!(reported["plan_id"], Value::from(plan.plan_id()));
    assert!(
        plan.plan_id().starts_with("repair-ISSUE-1-"),
        "the id names the issue it plans for: {}",
        plan.plan_id()
    );

    let binding = plan.evidence_binding();
    assert_eq!(
        reported["world_id"],
        Value::from(binding.world_id.as_str()),
        "the reported world and the bound world must be the same world"
    );
    assert!(
        binding.world_id.starts_with("project-"),
        "the binding names a project world: {}",
        binding.world_id
    );
    assert_eq!(
        binding.world_sha256.len(),
        64,
        "the binding carries a sha256 digest of the world it was planned from, not a label"
    );
    assert!(
        binding
            .region_fact_ids
            .contains(&"fact.component.src".to_string()),
        "ISSUE-1 declares src/lib.rs, so that component belongs to the region the plan binds: \
         {:?}",
        binding.region_fact_ids
    );
    assert!(
        binding
            .region_fact_ids
            .contains(&"fact.issue.ISSUE-1".to_string()),
        "the issue's own record belongs to the bound region: {:?}",
        binding.region_fact_ids
    );

    assert!(
        !plan.falsifiers().is_empty(),
        "a plan that could never be shown to be the wrong plan is not a plan"
    );
    assert!(
        plan.limitations()
            .iter()
            .any(|line| line.contains("not proof that the issue is resolved")),
        "the mandatory limitation must ride on the written document: {:?}",
        plan.limitations()
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// One field name may not mean a count in one `project` document and a list of ids in the next.
///
/// `project audit` reports an issue's region as `region` with `region_facts` beside it as the
/// count, and `docs/PROJECT_MODELING.md` records that as this surface's convention. A caller who
/// learned it there and indexed `region_facts` on a plan would get a list where it expected a
/// number — a divergence that shows up only at the point of use, and only for whoever wrote the
/// script. The list travels under the plan document's own field name, which is also what the
/// `repair_plan` MCP tool returns, so one name means one thing on both surfaces.
#[test]
fn project_plan_names_the_region_list_and_its_count_the_way_project_audit_already_does() {
    let directory = scratch("region-field-names");
    let out = directory.join("plan.json");
    let reported = plan_issue_one(&out, &[]);

    let ids = reported["region_fact_ids"]
        .as_array()
        .expect("the bound region's fact ids travel as a list under region_fact_ids");
    assert!(
        ids.contains(&Value::from("fact.component.src")),
        "the list must be the region the plan bound, not an empty placeholder: {ids:?}"
    );
    assert_eq!(
        reported["region_facts"],
        Value::from(ids.len()),
        "region_facts is the count on this surface, exactly as project audit emits it"
    );

    let audit = run(&[
        "--json",
        "project",
        "audit",
        "--root",
        &fixture("demo-app"),
        "--issues",
        &demo_issues(),
    ]);
    let audited: Value = serde_json::from_str(&stdout(&audit)).expect("audit JSON");
    let issue = &audited["issues"]["ISSUE-1"];
    assert!(
        issue["region_facts"].is_number(),
        "the claim is the agreement between the two commands, so the audit's own shape is read \
         rather than assumed: {issue}"
    );
    assert_eq!(
        issue["region_facts"],
        Value::from(ids.len()),
        "the same issue, the same tree, the same region: the two commands must not disagree about \
         how big it is"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn verifying_an_unrepaired_tree_reports_not_met_and_never_that_the_issue_is_resolved() {
    let directory = scratch("unchanged");
    let out = directory.join("plan.json");
    plan_issue_one(&out, &[]);

    let (status, parsed) = verify_json(
        &fixture("demo-app"),
        &out,
        &["--issues", &demo_issues()],
    );
    let report = &parsed["report"];

    assert_eq!(
        report["verdict"],
        Value::from("evaluated"),
        "the tree is the one the plan was bound to, so it is checked rather than refused"
    );
    assert_eq!(
        report["outcome"],
        Value::from("not_met"),
        "nothing was repaired, so the tool must not congratulate the tree: {report}"
    );
    assert_eq!(
        status, 1,
        "a determinate adverse verdict is a completed run whose checked property does not hold"
    );
    assert_eq!(
        report["admissibility"],
        Value::from("undeclared"),
        "a plan declaring no prerequisite has declared none, which is not the same as one holding"
    );

    let items = report["items"].as_array().expect("item list");
    let status_of = |name: &str| -> &Value {
        items
            .iter()
            .find(|item| item["name"] == *name)
            .unwrap_or_else(|| panic!("{name} is not on the report: {items:?}"))
    };
    assert_eq!(
        status_of("check_cleared:unpinned_dependency")["status"],
        Value::from("unmet"),
        "the release check that fired when the plan was made still fires"
    );
    assert_eq!(
        status_of("component_present:src")["status"],
        Value::from("met"),
        "the component the issue declares is still there"
    );
    assert_eq!(
        status_of("region_evidence_removed")["status"],
        Value::from("unmet"),
        "no decisive variable vanished, so the falsifier does not hold"
    );
    for item in items {
        assert!(
            item["origin"] == *"derived" || item["origin"] == *"declared",
            "every reported item carries exactly one origin: {item}"
        );
    }

    let limitations = report["limitations"].as_array().expect("limitations");
    assert!(
        limitations
            .iter()
            .any(|line| line.as_str().unwrap_or_default()
                .contains("does not state that the issue is resolved")),
        "the report must refuse the claim the whole command could be mistaken for: {limitations:?}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_plan_verified_against_a_different_root_exits_stale_and_evaluates_nothing() {
    let directory = scratch("stale-root");
    let out = directory.join("plan.json");
    let planned = plan_issue_one(&out, &[]);

    let (status, parsed) = verify_json(&fixture("bare-script"), &out, &[]);
    let report = &parsed["report"];

    assert_eq!(
        status, 9,
        "a stale plan is not a failed verification: nothing was checked, and re-reading the tree \
         is the whole remedy — {report}"
    );
    assert_eq!(report["verdict"], Value::from("stale"));
    assert_eq!(
        report["expected_world_id"], planned["world_id"],
        "the report must name the world the plan was planned from"
    );
    assert_ne!(
        report["found_world_id"], report["expected_world_id"],
        "if the two worlds were the same this test would prove nothing"
    );
    assert!(
        report["items"].is_null() && report["outcome"].is_null(),
        "staleness is decided before evaluation, so a stale report carries no item status and no \
         verdict rather than neutral ones: {report}"
    );
    assert!(
        report["limitations"]
            .as_array()
            .expect("limitations")
            .iter()
            .any(|line| line
                .as_str()
                .unwrap_or_default()
                .contains("not a verdict about this plan")),
        "the stale report must say why nothing was evaluated: {report}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn verifying_the_planned_tree_without_the_issues_it_was_planned_from_is_stale_not_a_verdict() {
    let directory = scratch("stale-issues");
    let out = directory.join("plan.json");
    plan_issue_one(&out, &[]);

    let (status, parsed) = verify_json(&fixture("demo-app"), &out, &[]);
    assert_eq!(
        status, 9,
        "the same tree assembled without its issue declarations is a different world, and a \
         verdict computed against it would not be a verdict about this plan: {}",
        parsed["report"]
    );
    assert_eq!(parsed["report"]["verdict"], Value::from("stale"));

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_declared_criterion_that_cannot_be_evaluated_exits_eight_rather_than_joining_the_failures() {
    let directory = scratch("underdetermined");
    let out = directory.join("plan.json");
    let declarations = directory.join("declared.json");
    std::fs::write(
        &declarations,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "bioprism-repair-declarations/0.1",
            "criteria": [{
                "name": "ghost_component_inventory_nonempty",
                "statement": "A component the tree does not carry reports a non-empty inventory.",
                "predicate": { "kind": "nonempty", "variable": "component_ghost_inventory" },
                "rationale": "Declared to exercise a criterion no scan of this tree can evaluate."
            }]
        }))
        .unwrap(),
    )
    .expect("declarations written");

    plan_issue_one(
        &out,
        &["--criteria", &declarations.display().to_string()],
    );
    let (status, parsed) = verify_json(
        &fixture("demo-app"),
        &out,
        &["--issues", &demo_issues()],
    );
    let report = &parsed["report"];

    let items = report["items"].as_array().expect("item list");
    let blocked = items
        .iter()
        .find(|item| item["name"] == *"ghost_component_inventory_nonempty")
        .expect("the declared criterion is reported");
    assert_eq!(blocked["status"], Value::from("not_evaluable"));
    assert_eq!(
        blocked["obstruction"]["variable"],
        Value::from("component_ghost_inventory"),
        "the third status exists to name what stopped the check: {blocked}"
    );
    assert!(
        items
            .iter()
            .any(|item| item["status"] == *"unmet"),
        "a determinate failure must also be present, or this test does not discriminate the two \
         exit codes: {items:?}"
    );

    assert_eq!(
        report["outcome"],
        Value::from("underdetermined"),
        "not_met would presuppose the criteria were all checked, and one was not"
    );
    assert_eq!(
        status, 8,
        "exit 1 would tell a script that clearing the listed failures is the whole remaining \
         distance to a pass, which is false while a criterion never ran: {report}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_declared_criterion_is_never_recorded_as_the_generators_own_inference() {
    let directory = scratch("origins");
    let out = directory.join("plan.json");
    let declarations = directory.join("declared.json");
    std::fs::write(
        &declarations,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "bioprism-repair-declarations/0.1",
            "criteria": [{
                "name": "src_inventory_nonempty",
                "statement": "The scanner still reports an inventory for the src component.",
                "predicate": { "kind": "nonempty", "variable": "component_src_inventory" },
                "rationale": "Declared by the operator, who is accountable for it."
            }],
            "obligations": [{
                "name": "a_test_is_counted_somewhere",
                "statement": "The scan counts at least one test function in the tree.",
                "predicate": { "kind": "number_at_least", "variable": "test_function_total", "minimum": 1 }
            }],
            "limitations": ["Authored for the origin test."]
        }))
        .unwrap(),
    )
    .expect("declarations written");

    plan_issue_one(
        &out,
        &["--criteria", &declarations.display().to_string()],
    );
    let plan = RepairPlan::from_json(
        &serde_json::from_slice(&std::fs::read(&out).expect("plan on disk")).expect("JSON"),
    )
    .expect("plan parses");

    let origin = |name: &str| -> Origin {
        plan.criteria()
            .iter()
            .find(|item| item.name == name)
            .map(|item| item.origin)
            .unwrap_or_else(|| panic!("{name} is not among the plan's criteria"))
    };
    assert_eq!(
        origin("src_inventory_nonempty"),
        Origin::Declared,
        "the operator's criterion must never borrow the authority of an inference"
    );
    assert_eq!(
        origin("check_cleared:unpinned_dependency"),
        Origin::Derived,
        "and the generator's inference must never be reported as somebody's claim"
    );
    assert_eq!(
        plan.obligations().len(),
        1,
        "a declared obligation reaches the plan: {:?}",
        plan.obligations()
    );
    assert!(
        plan.limitations()
            .iter()
            .any(|line| line == "Authored for the origin test."),
        "the author's own limitations are appended, never replacing the generator's: {:?}",
        plan.limitations()
    );

    let (_status, parsed) = verify_json(
        &fixture("demo-app"),
        &out,
        &["--issues", &demo_issues()],
    );
    assert_eq!(
        parsed["report"]["admissibility"],
        Value::from("held"),
        "the declared obligation is reported on its own axis, not folded into the outcome"
    );
    assert_eq!(
        parsed["report"]["outcome"],
        Value::from("not_met"),
        "and a held obligation does not move the achievement verdict"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_dry_run_plan_reports_the_write_it_would_perform_and_creates_no_file() {
    let directory = scratch("dry-run");
    let out = directory.join("nested").join("plan.json");
    let parsed = plan_issue_one(&out, &["--dry-run"]);

    assert_eq!(parsed["dry_run"], Value::Bool(true));
    let artifacts = parsed["artifacts"].as_array().expect("artifact list");
    assert_eq!(artifacts.len(), 1, "one plan means one planned write");
    assert_eq!(
        artifacts[0]["written"],
        Value::Bool(false),
        "a dry run declares the plan and performs none of it: {}",
        artifacts[0]
    );
    assert!(
        artifacts[0]["bytes"].as_u64().expect("byte count") > 0,
        "the plan must state how many bytes the write would produce: {}",
        artifacts[0]
    );
    assert!(
        parsed["plan"]["plan_id"].is_string(),
        "the document the write would have produced still reaches the caller, so --dry-run is a \
         preview rather than a refusal: {parsed}"
    );

    let left_behind: Vec<String> = std::fs::read_dir(&directory)
        .expect("scratch dir readable")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into())
        .collect();
    assert!(
        left_behind.is_empty(),
        "--dry-run must have no undeclared effects, and creating the parent directory is an \
         effect: {left_behind:?}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn planning_for_an_issue_the_declarations_do_not_carry_names_the_file_rather_than_the_tree() {
    let directory = scratch("unknown-issue");
    let out = directory.join("plan.json");
    let output = run(&[
        "--json",
        "project",
        "plan",
        "--root",
        &fixture("demo-app"),
        "--issues",
        &demo_issues(),
        "--issue",
        "ISSUE-404",
        "--out",
        &out.display().to_string(),
    ]);

    assert_eq!(
        code(&output),
        3,
        "the operator edits the flag or the issues file, so re-sending unchanged cannot succeed"
    );
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("error envelope");
    assert_eq!(parsed["ok"], Value::Bool(false));
    assert_eq!(parsed["error"]["kind"], Value::from("invalid_input"));
    let message = parsed["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("ISSUE-404") && message.contains("ISSUE-1"),
        "the refusal must name both what was asked for and what is actually declared: {message}"
    );
    assert!(
        !out.exists(),
        "a refused plan must not leave a document behind"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn the_human_verify_report_prints_every_item_with_its_status_and_the_obstruction_that_stopped_it() {
    let directory = scratch("human");
    let out = directory.join("plan.json");
    let declarations = directory.join("declared.json");
    std::fs::write(
        &declarations,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "bioprism-repair-declarations/0.1",
            "criteria": [{
                "name": "ghost_component_inventory_nonempty",
                "statement": "A component the tree does not carry reports a non-empty inventory.",
                "predicate": { "kind": "nonempty", "variable": "component_ghost_inventory" },
                "rationale": "Declared to force one unevaluable status into the printed report."
            }]
        }))
        .unwrap(),
    )
    .expect("declarations written");
    plan_issue_one(
        &out,
        &["--criteria", &declarations.display().to_string()],
    );

    let output = run(&[
        "project",
        "verify",
        "--root",
        &fixture("demo-app"),
        "--plan",
        &out.display().to_string(),
        "--issues",
        &demo_issues(),
    ]);
    let text = stdout(&output);

    assert!(
        text.contains("underdetermined (admissibility undeclared)"),
        "the outcome and the separate admissibility axis lead the report:\n{text}"
    );
    for expected in [
        "criterion  derived   unmet          check_cleared:unpinned_dependency",
        "criterion  derived   met            component_present:src",
        "criterion  declared  not_evaluable  ghost_component_inventory_nonempty",
        "falsifier  derived   unmet          region_evidence_removed",
    ] {
        assert!(
            text.contains(expected),
            "every item needs its own line carrying kind, origin and status; missing {expected:?} \
             in:\n{text}"
        );
    }
    assert!(
        text.contains("blocked: component_ghost_inventory"),
        "an unevaluable item must name the variable that stopped it, or the third status is just \
         a shrug:\n{text}"
    );
    assert!(
        text.contains("does not state that the issue is resolved"),
        "the human report carries the same refusal the JSON one does:\n{text}"
    );
    assert!(
        text.contains("\nNext: bioprism "),
        "40.13 requires a reproducible follow-up command in human mode:\n{text}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_declarations_document_with_an_undeclared_field_is_refused_rather_than_ignored() {
    let directory = scratch("strict-declarations");
    let out = directory.join("plan.json");
    let declarations = directory.join("declared.json");
    std::fs::write(
        &declarations,
        serde_json::to_vec_pretty(&json!({
            "schema_version": "bioprism-repair-declarations/0.1",
            "falsifier": [{
                "name": "typo_in_the_key",
                "statement": "The author meant falsifiers and wrote falsifier.",
                "predicate": { "kind": "missing", "variable": "component_src_inventory" }
            }]
        }))
        .unwrap(),
    )
    .expect("declarations written");

    let output = run(&[
        "--json",
        "project",
        "plan",
        "--root",
        &fixture("demo-app"),
        "--issues",
        &demo_issues(),
        "--issue",
        "ISSUE-1",
        "--criteria",
        &declarations.display().to_string(),
        "--out",
        &out.display().to_string(),
    ]);

    assert_eq!(code(&output), 3);
    let parsed: Value = serde_json::from_str(&stdout(&output)).expect("error envelope");
    let message = parsed["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("falsifier"),
        "the refusal must name the key the author has to fix, because silently ignoring it would \
         produce a plan whose missing falsifier the author would then be blamed for: {message}"
    );

    let _ = std::fs::remove_dir_all(&directory);
}
