//! Claims about repair plans and acceptance reports over `fixtures/projects/demo-app`.
//!
//! Every test names the property it defends. The two that matter most are the ones that come out
//! *against* the tool: an unchanged tree verifies as `NotMet`, and an absent variable is reported
//! `NotEvaluable` rather than as a failed criterion.

use bioprism_domain::{DomainPack, Predicate};
use bioprism_fiber::{compile_with_oracle, Query};
use bioprism_ids::to_canonical_bytes;
use bioprism_project::{AssemblyOptions, Issue, ProjectScan, ProjectWorld, ScanOptions};
use bioprism_repair::{
    plan_for_issue, verify, verify_successor, AcceptanceReport, DeclaredItem, EvidenceBinding,
    ItemKind, ItemStatus, Origin, Outcome, PlanOptions, RepairError, RepairPlan, RepairPlanDraft,
    Succession, CRITERIA_ARE_NOT_PROOF, REGION_EVIDENCE_REMOVED,
};
use bioprism_section::ContextCertificate;
use bioprism_world::World;
use std::path::{Path, PathBuf};

fn fixture_root(name: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "projects",
        name,
    ]
    .iter()
    .collect()
}

/// Assembles a project world from a tree, carrying the demo fixture's issues.
fn assemble(root: &Path, project: &str) -> ProjectWorld {
    let (scan, _) = ProjectScan::scan(root, &ScanOptions::new(project)).expect("tree scans");
    let issues =
        Issue::load(&fixture_root("demo-app").join("issues.json")).expect("issues.json loads");
    ProjectWorld::assemble(
        &scan,
        &AssemblyOptions {
            issues,
            ..AssemblyOptions::default()
        },
    )
    .expect("tree assembles")
}

fn demo() -> ProjectWorld {
    assemble(&fixture_root("demo-app"), "demo-app")
}

/// The world, its pack, and the compiled region for one issue.
fn compiled(assembled: &ProjectWorld, issue_id: &str) -> (World, DomainPack, ContextCertificate) {
    let world = World::from_json(assembled.world.clone()).expect("world validates");
    let pack = DomainPack::from_json(&assembled.pack).expect("pack parses");
    let query = Query::from_json(
        assembled
            .issue_queries
            .get(issue_id)
            .unwrap_or_else(|| panic!("{issue_id} query generated"))
            .clone(),
    )
    .expect("query parses");
    let out = compile_with_oracle(&world, &query, pack.oracle()).expect("issue query compiles");
    (world, pack, out.certificate)
}

fn plan_for(assembled: &ProjectWorld, issue_id: &str, options: &PlanOptions) -> RepairPlan {
    let (world, pack, certificate) = compiled(assembled, issue_id);
    plan_for_issue(&world, &pack, issue_id, &certificate, options).expect("plan generates")
}

/// A declared criterion that reads the `src` component inventory with a non-total predicate, so
/// its status is `Met` while `src` exists and `NotEvaluable` once it does not.
fn src_inventory_nonempty() -> DeclaredItem {
    DeclaredItem::new(
        "src_inventory_nonempty",
        "The scanner still reports an inventory for the src component.",
        Predicate::Nonempty {
            variable: "component_src_inventory".to_string(),
        },
    )
    .with_rationale("Declared by the test author to exercise a non-total predicate.")
}

fn temp_tree(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "bioprism-repair-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// Copies the demo fixture into `into`, skipping any path whose relative form starts with one of
/// `omit`. Deterministic: the walk sorts its entries.
fn copy_fixture(into: &Path, omit: &[&str]) {
    fn walk(from: &Path, to: &Path, prefix: &str, omit: &[&str]) {
        std::fs::create_dir_all(to).expect("destination directory");
        let mut entries: Vec<_> = std::fs::read_dir(from)
            .expect("fixture readable")
            .map(|entry| entry.expect("entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            let name = entry
                .file_name()
                .expect("named entry")
                .to_string_lossy()
                .to_string();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if omit.iter().any(|skip| relative.starts_with(skip)) {
                continue;
            }
            if entry.is_dir() {
                walk(&entry, &to.join(&name), &relative, omit);
            } else {
                std::fs::copy(&entry, to.join(&name)).expect("file copies");
            }
        }
    }
    walk(&fixture_root("demo-app"), into, "", omit);
}

#[test]
fn a_plan_binds_the_region_it_was_planned_from_and_refuses_to_verify_against_a_different_world() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());

    let elsewhere = temp_tree("other-world");
    copy_fixture(&elsewhere, &["assets"]);
    let other = assemble(&elsewhere, "demo-app");
    let other_world = World::from_json(other.world.clone()).unwrap();
    assert_ne!(
        assembled.world_id, other.world_id,
        "the modified tree must assemble to a different world, or this test proves nothing"
    );

    let report = verify(&plan, &other_world);
    match &report {
        AcceptanceReport::Stale {
            expected_world_id,
            found_world_id,
            ..
        } => {
            assert_eq!(expected_world_id, &assembled.world_id);
            assert_eq!(found_world_id, &other.world_id);
        }
        other => panic!("expected a stale report, got {other:?}"),
    }
    assert!(
        report.items().is_empty(),
        "staleness is decided before evaluation: a stale report carries no item status"
    );
    assert!(
        report.outcome().is_none(),
        "a stale report has no verdict rather than a neutral one"
    );
    assert!(
        report
            .limitations()
            .iter()
            .any(|line| line.contains("not a verdict about this plan")),
        "the stale report must say why nothing was evaluated: {:?}",
        report.limitations()
    );

    std::fs::remove_dir_all(&elsewhere).unwrap();
}

#[test]
fn a_plan_with_no_falsifier_is_refused_at_construction() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    assert!(
        !plan.falsifiers().is_empty(),
        "the generated plan is the positive control for this test"
    );

    let draft = RepairPlanDraft {
        issue_id: plan.issue_id().to_string(),
        goal: plan.goal().to_string(),
        evidence_binding: EvidenceBinding {
            world_id: plan.evidence_binding().world_id.clone(),
            world_sha256: plan.evidence_binding().world_sha256.clone(),
            region_fact_ids: plan.evidence_binding().region_fact_ids.clone(),
            query_sha256: plan.evidence_binding().query_sha256.clone(),
        },
        criteria: plan.criteria().to_vec(),
        obligations: Vec::new(),
        falsifiers: Vec::new(),
        limitations: vec![CRITERIA_ARE_NOT_PROOF.to_string()],
    };
    match RepairPlan::admit(draft) {
        Err(RepairError::NoFalsifier { issue }) => assert_eq!(issue, "ISSUE-1"),
        other => panic!("a plan with no falsifier must be refused, got {other:?}"),
    }
}

#[test]
fn a_criterion_whose_variable_vanished_is_not_evaluable_rather_than_unmet() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![src_inventory_nonempty()],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);

    let (world, _, _) = compiled(&assembled, "ISSUE-1");
    let before = verify(&plan, &world);
    assert_eq!(
        before.item("src_inventory_nonempty").map(|i| &i.status),
        Some(&ItemStatus::Met),
        "the declared criterion holds while src exists, so its later status is a real change"
    );

    let repaired = temp_tree("src-deleted");
    copy_fixture(&repaired, &["src"]);
    let after_world = World::from_json(assemble(&repaired, "demo-app").world.clone()).unwrap();
    let succession = Succession::declare(
        "the test author",
        "the demo-app tree with src/ removed is the successor of the planned tree",
    )
    .unwrap();
    let report = verify_successor(&plan, &after_world, &succession);

    let item = report
        .item("src_inventory_nonempty")
        .expect("the declared criterion is reported");
    match &item.status {
        ItemStatus::NotEvaluable(obstruction) => {
            assert_eq!(obstruction.variable, "component_src_inventory");
            assert!(
                obstruction.reason.contains("absent"),
                "the obstruction must say the variable is absent: {}",
                obstruction.reason
            );
        }
        other => panic!(
            "a criterion over a variable the new world does not carry must be NotEvaluable, \
             never Unmet; got {other:?}"
        ),
    }

    std::fs::remove_dir_all(&repaired).unwrap();
}

#[test]
fn a_met_falsifier_outranks_unmet_and_unevaluable_criteria() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![src_inventory_nonempty()],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);

    let repaired = temp_tree("falsified");
    copy_fixture(&repaired, &["src"]);
    let after_world = World::from_json(assemble(&repaired, "demo-app").world.clone()).unwrap();
    let succession =
        Succession::declare("the test author", "src/ was removed from the planned tree").unwrap();
    let report = verify_successor(&plan, &after_world, &succession);

    assert_eq!(
        report.item(REGION_EVIDENCE_REMOVED).map(|i| &i.status),
        Some(&ItemStatus::Met),
        "removing the component the issue names is exactly what the derived falsifier watches for"
    );
    assert_eq!(
        report
            .item("component_present:src")
            .map(|i| &i.status),
        Some(&ItemStatus::Unmet),
        "the presence criterion is total, so a vanished component fails determinately"
    );
    assert!(
        matches!(
            report.item("src_inventory_nonempty").map(|i| &i.status),
            Some(ItemStatus::NotEvaluable(_))
        ),
        "the third status must also be present, or this test does not exercise the ordering"
    );
    assert_eq!(
        report.outcome(),
        Some(Outcome::Falsified),
        "a met falsifier outranks both an unmet criterion and an unevaluable one: {}",
        report.summary()
    );

    std::fs::remove_dir_all(&repaired).unwrap();
}

#[test]
fn the_report_names_every_items_status_including_the_obstruction_that_stopped_it() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![src_inventory_nonempty()],
        declared_obligations: vec![DeclaredItem::new(
            "a_test_covers_the_component",
            "The src component's inventory reports at least one counted test function.",
            Predicate::NumberAtLeast {
                variable: "test_function_total".to_string(),
                minimum: 1.0,
            },
        )],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);

    let repaired = temp_tree("full-report");
    copy_fixture(&repaired, &["src"]);
    let after_world = World::from_json(assemble(&repaired, "demo-app").world.clone()).unwrap();
    let succession = Succession::declare("the test author", "src/ removed").unwrap();
    let report = verify_successor(&plan, &after_world, &succession);

    let planned: Vec<&str> = plan
        .criteria()
        .iter()
        .map(|item| item.name.as_str())
        .chain(plan.obligations().iter().map(|item| item.name.as_str()))
        .chain(plan.falsifiers().iter().map(|item| item.name.as_str()))
        .collect();
    assert!(
        planned.len() >= 5,
        "the plan must carry criteria, an obligation and a falsifier for this claim to bite: \
         {planned:?}"
    );
    for name in &planned {
        assert!(
            report.item(name).is_some(),
            "every declared item must appear in the report with its own status; {name} did not"
        );
    }
    assert_eq!(report.items().len(), planned.len());

    let blocked: Vec<&str> = report
        .items()
        .iter()
        .filter(|item| item.status.obstruction().is_some())
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(
        blocked,
        vec!["src_inventory_nonempty"],
        "the report must name which item could not run"
    );
    assert!(
        report.summary().contains("component_src_inventory"),
        "even the one-line summary names the variable that stopped a check: {}",
        report.summary()
    );
    assert!(
        report
            .missing_region_facts()
            .contains(&"fact.component.src".to_string()),
        "the report names the bound region facts that no longer exist: {:?}",
        report.missing_region_facts()
    );

    std::fs::remove_dir_all(&repaired).unwrap();
}

#[test]
fn a_derived_criterion_is_marked_derived_and_a_declared_one_declared_and_neither_absorbs_the_other()
{
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![src_inventory_nonempty()],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);

    let derived: Vec<&str> = plan
        .criteria()
        .iter()
        .filter(|item| item.origin == Origin::Derived)
        .map(|item| item.name.as_str())
        .collect();
    let declared: Vec<&str> = plan
        .criteria()
        .iter()
        .filter(|item| item.origin == Origin::Declared)
        .map(|item| item.name.as_str())
        .collect();
    assert!(
        derived.contains(&"check_cleared:unpinned_dependency"),
        "the fired release check must yield a derived criterion: {derived:?}"
    );
    assert_eq!(declared, vec!["src_inventory_nonempty"]);
    assert!(
        !derived.contains(&"src_inventory_nonempty"),
        "a caller's criterion must never be recorded as the tool's inference"
    );
    assert_eq!(
        plan.criteria().len(),
        derived.len() + declared.len(),
        "every criterion carries exactly one origin"
    );

    let collision = PlanOptions {
        declared_criteria: vec![DeclaredItem::new(
            "check_cleared:unpinned_dependency",
            "A human restating the derived criterion under its own name.",
            Predicate::Exists {
                variable: "unpinned_dependencies".to_string(),
            },
        )],
        ..PlanOptions::default()
    };
    let (world, pack, certificate) = compiled(&assembled, "ISSUE-1");
    match plan_for_issue(&world, &pack, "ISSUE-1", &certificate, &collision) {
        Err(RepairError::DuplicateItemName { name }) => {
            assert_eq!(name, "check_cleared:unpinned_dependency")
        }
        other => panic!(
            "a declared item reusing a derived item's name must be refused rather than merged \
             into or over it; got {other:?}"
        ),
    }
}

#[test]
fn verifying_the_unchanged_demo_app_against_its_own_plan_reports_not_met() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    let (world, _, _) = compiled(&assembled, "ISSUE-1");

    let report = verify(&plan, &world);
    assert_eq!(
        report.outcome(),
        Some(Outcome::NotMet),
        "nothing was repaired, so the tool must not congratulate the tree: {}",
        report.summary()
    );
    assert_eq!(
        report
            .item("check_cleared:unpinned_dependency")
            .map(|i| &i.status),
        Some(&ItemStatus::Unmet),
        "the check that fired when the plan was made still fires"
    );
    assert_eq!(
        report
            .item("component_present:src")
            .map(|i| &i.status),
        Some(&ItemStatus::Met),
        "the component the issue names is still there"
    );
    assert_eq!(
        report.item(REGION_EVIDENCE_REMOVED).map(|i| &i.status),
        Some(&ItemStatus::Unmet),
        "no decisive variable vanished, so the falsifier does not hold"
    );
    assert!(
        report
            .limitations()
            .iter()
            .any(|line| line == CRITERIA_ARE_NOT_PROOF),
        "the plan's mandatory limitation rides on the report verbatim"
    );
    assert!(
        report
            .limitations()
            .iter()
            .any(|line| line.contains("does not state that the issue is resolved")),
        "the report adds its own refusal to claim resolution: {:?}",
        report.limitations()
    );
}

#[test]
fn obligations_stay_out_of_the_outcome_and_are_reported_on_their_own_axis() {
    let assembled = demo();
    let (world, _, _) = compiled(&assembled, "ISSUE-1");

    let without = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    assert_eq!(
        verify(&without, &world).admissibility(),
        Some(bioprism_repair::Admissibility::Undeclared),
        "a plan with no obligations has declared no prerequisite, which is not the same as one \
         that holds"
    );

    let violated = PlanOptions {
        declared_obligations: vec![DeclaredItem::new(
            "no_unpinned_dependency_before_the_change",
            "The tree declares no unpinned dependency before the change is made.",
            Predicate::Not {
                predicate: Box::new(Predicate::Nonempty {
                    variable: "unpinned_dependencies".to_string(),
                }),
            },
        )],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &violated);
    let report = verify(&plan, &world);
    assert_eq!(
        report.admissibility(),
        Some(bioprism_repair::Admissibility::Violated),
        "an unmet obligation is reported as a violated prerequisite"
    );
    assert_eq!(
        report.outcome(),
        Some(Outcome::NotMet),
        "and it does not change the achievement verdict, which is decided by criteria alone"
    );
    assert_eq!(
        report.items_of(ItemKind::Obligation).count(),
        1,
        "the obligation is still individually reported"
    );
    assert!(
        report
            .limitations()
            .iter()
            .any(|line| line.contains("Obligations are not in the outcome")),
        "the report says the outcome excludes obligations rather than leaving it to be inferred"
    );
}

#[test]
fn an_issue_declaring_no_component_still_derives_a_check_criterion_and_declares_the_gap() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-2", &PlanOptions::default());

    let names: Vec<&str> = plan
        .criteria()
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, vec!["check_cleared:unpinned_dependency"]);
    assert_eq!(plan.goal(), "Audit release readiness");
    assert!(
        plan.limitations()
            .iter()
            .any(|line| line.contains("declares no obligations")),
        "a plan with no prerequisites says so: {:?}",
        plan.limitations()
    );
}

#[test]
fn a_plan_document_round_trips_through_its_strict_parser() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![src_inventory_nonempty()],
        declared_obligations: vec![DeclaredItem::new(
            "docs_exist",
            "The scan reports at least one document.",
            Predicate::CountAtLeast {
                variable: "doc_inventory".to_string(),
                minimum: 1,
            },
        )],
        declared_falsifiers: vec![DeclaredItem::new(
            "dependencies_all_gone",
            "The tree declares no dependency at all, so the check was cleared by deletion.",
            Predicate::Not {
                predicate: Box::new(Predicate::Nonempty {
                    variable: "dependency_declarations".to_string(),
                }),
            },
        )],
        limitations: vec!["Authored for the round-trip test.".to_string()],
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);

    let document = plan.to_json().expect("plan serialises");
    let reparsed = RepairPlan::from_json(&document).expect("plan parses back");
    assert_eq!(reparsed, plan);
    assert_eq!(
        to_canonical_bytes(&reparsed.to_json().unwrap()).unwrap(),
        to_canonical_bytes(&document).unwrap()
    );
}

#[test]
fn a_plan_document_with_an_undeclared_key_is_refused_rather_than_ignored() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    let mut document = plan.to_json().unwrap();
    document
        .as_object_mut()
        .unwrap()
        .insert("severity".to_string(), serde_json::json!("high"));

    match RepairPlan::from_json(&document) {
        Err(RepairError::Document(message)) => assert!(
            message.contains("severity"),
            "the refusal must name the undeclared key: {message}"
        ),
        other => panic!("an undeclared key must be refused, got {other:?}"),
    }
}

#[test]
fn a_plan_whose_body_was_edited_after_minting_fails_its_own_content_derived_id() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    let mut document = plan.to_json().unwrap();
    document["goal"] = serde_json::json!("Something the issue never said");

    match RepairPlan::from_json(&document) {
        Err(RepairError::PlanIdMismatch { declared, derived }) => {
            assert_eq!(declared, plan.plan_id());
            assert_ne!(derived, declared);
        }
        other => panic!("an edited body must not keep its id, got {other:?}"),
    }
}

#[test]
fn an_acceptance_report_round_trips_through_its_strict_parser() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![src_inventory_nonempty()],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);
    let (world, _, _) = compiled(&assembled, "ISSUE-1");

    let fresh = verify(&plan, &world);
    let reparsed = AcceptanceReport::from_json(&fresh.to_json()).expect("report parses back");
    assert_eq!(reparsed, fresh);

    let repaired = temp_tree("report-roundtrip");
    copy_fixture(&repaired, &["src"]);
    let after_world = World::from_json(assemble(&repaired, "demo-app").world.clone()).unwrap();

    let stale = verify(&plan, &after_world);
    assert!(matches!(stale, AcceptanceReport::Stale { .. }));
    assert_eq!(
        AcceptanceReport::from_json(&stale.to_json()).expect("stale report parses back"),
        stale
    );

    let succession = Succession::declare("the test author", "src/ removed").unwrap();
    let blocked = verify_successor(&plan, &after_world, &succession);
    let blocked_again =
        AcceptanceReport::from_json(&blocked.to_json()).expect("blocked report parses back");
    assert_eq!(blocked_again, blocked);
    assert!(
        blocked_again
            .item("src_inventory_nonempty")
            .and_then(|item| item.status.obstruction())
            .is_some(),
        "the obstruction survives the round trip; a status that loses its reason is a status \
         that hides which check did not run"
    );

    std::fs::remove_dir_all(&repaired).unwrap();
}

#[test]
fn a_report_claiming_not_evaluable_without_an_obstruction_is_refused() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    let (world, _, _) = compiled(&assembled, "ISSUE-1");
    let mut document = verify(&plan, &world).to_json();
    document["items"][0]["status"] = serde_json::json!("not_evaluable");

    match AcceptanceReport::from_json(&document) {
        Err(RepairError::Document(message)) => assert!(
            message.contains("obstruction"),
            "the refusal must say the obstruction is the point of the third value: {message}"
        ),
        other => panic!("an unexplained NotEvaluable must be refused, got {other:?}"),
    }
}

#[test]
fn a_plan_document_declaring_no_falsifier_is_refused_by_the_reader_not_only_by_the_constructor() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    let mut document = plan.to_json().unwrap();
    document["falsifiers"] = serde_json::json!([]);

    match RepairPlan::from_json(&document) {
        Err(RepairError::NoFalsifier { issue }) => assert_eq!(issue, "ISSUE-1"),
        other => panic!(
            "the admissibility gate must not be routable around by handing the parser a document \
             instead of a draft; got {other:?}"
        ),
    }
}

#[test]
fn a_report_whose_declared_outcome_its_own_items_do_not_produce_is_refused() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![DeclaredItem::new(
            "ghost_component_inventory_nonempty",
            "A component the tree does not carry reports a non-empty inventory.",
            Predicate::Nonempty {
                variable: "component_ghost_inventory".to_string(),
            },
        )
        .with_rationale("Declared to put one unevaluable status on the report.")],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);
    let (world, _, _) = compiled(&assembled, "ISSUE-1");
    let report = verify(&plan, &world);
    assert_eq!(report.outcome(), Some(Outcome::Underdetermined));

    let mut document = report.to_json();
    document["outcome"] = serde_json::json!("met");
    match AcceptanceReport::from_json(&document) {
        Err(RepairError::Document(message)) => assert!(
            message.contains("met") && message.contains("underdetermined"),
            "the refusal must name both what the document claimed and what its items say: \
             {message}"
        ),
        other => panic!(
            "a report reading \"met\" beside an item that could not run is the aggregate this \
             crate exists to make unsayable, however it was authored; got {other:?}"
        ),
    }
}

#[test]
fn a_world_providing_one_variable_from_two_facts_has_the_collapse_named_on_the_report() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());

    let mut raw = assembled.world.clone();
    let facts = raw["facts"].as_array_mut().expect("the world carries facts");
    let mut shadow = facts
        .iter()
        .find(|fact| fact["provides"] == serde_json::json!("unpinned_dependencies"))
        .expect("the aggregate the derived criterion reads")
        .clone();
    shadow["id"] = serde_json::json!("fact.aggregate.unpinned_dependencies.shadow");
    shadow["value"] = serde_json::json!([]);
    facts.push(shadow);
    let shadowed_world = World::from_json(raw).expect(
        "the world reader accepts a shadowed variable, which is why the verifier has to say so",
    );

    let succession = Succession::declare(
        "the test author",
        "a second fact providing unpinned_dependencies was appended",
    )
    .unwrap();
    let report = verify_successor(&plan, &shadowed_world, &succession);
    assert!(
        report.limitations().iter().any(|line| line
            .contains("more than one fact")
            && line.contains("unpinned_dependencies")),
        "a criterion evaluated against whichever of two values document order left standing must \
         say so, not read as a check against the world: {:?}",
        report.limitations()
    );
}

#[test]
fn a_succession_declared_for_the_planned_world_itself_is_not_reported_as_a_different_world() {
    let assembled = demo();
    let plan = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    let (world, _, _) = compiled(&assembled, "ISSUE-1");

    let succession =
        Succession::declare("the test author", "this is the very tree it was planned from")
            .unwrap();
    let report = verify_successor(&plan, &world, &succession);
    match &report {
        AcceptanceReport::Evaluated {
            binding_matches,
            limitations,
            ..
        } => {
            assert!(*binding_matches);
            assert!(
                !limitations
                    .iter()
                    .any(|line| line.contains("is not the world this plan was bound to")),
                "a report may not state the opposite of its own binding_matches field: {limitations:?}"
            );
            assert!(
                limitations
                    .iter()
                    .any(|line| line.contains("it was not relied on")),
                "the declaration was still made, so the report records it rather than dropping \
                 it: {limitations:?}"
            );
        }
        other => panic!("expected an evaluated report, got {other:?}"),
    }
}

#[test]
fn planning_twice_from_the_same_world_yields_byte_identical_documents() {
    let first = plan_for(&demo(), "ISSUE-1", &PlanOptions::default());
    let second = plan_for(&demo(), "ISSUE-1", &PlanOptions::default());

    assert_eq!(first.plan_id(), second.plan_id());
    assert_eq!(
        to_canonical_bytes(&first.to_json().unwrap()).unwrap(),
        to_canonical_bytes(&second.to_json().unwrap()).unwrap(),
        "two plans made from the same world and options must be byte-identical"
    );
    assert!(
        first.plan_id().starts_with("repair-ISSUE-1-"),
        "the id names its issue: {}",
        first.plan_id()
    );
}

#[test]
fn every_predicate_kind_a_plan_can_carry_survives_the_domain_readers_round_trip() {
    let kinds = vec![
        Predicate::Exists {
            variable: "v".into(),
        },
        Predicate::Missing {
            variable: "v".into(),
        },
        Predicate::Nonempty {
            variable: "v".into(),
        },
        Predicate::Equals {
            variable: "v".into(),
            value: serde_json::json!({"a": [1, 2]}),
        },
        Predicate::NotEquals {
            variable: "v".into(),
            value: serde_json::json!(null),
        },
        Predicate::Contains {
            variable: "v".into(),
            value: serde_json::json!("x"),
        },
        Predicate::NumberAtLeast {
            variable: "v".into(),
            minimum: 50.0,
        },
        Predicate::NumberBelow {
            variable: "v".into(),
            maximum: 1.5,
        },
        Predicate::StringBefore {
            variable: "v".into(),
            than: "2020".into(),
        },
        Predicate::StringAfter {
            variable: "v".into(),
            than: "2020".into(),
        },
        Predicate::HasKey {
            variable: "v".into(),
            key: "k".into(),
        },
        Predicate::CountAtLeast {
            variable: "v".into(),
            minimum: 3,
        },
        Predicate::Not {
            predicate: Box::new(Predicate::Nonempty {
                variable: "v".into(),
            }),
        },
        Predicate::AllOf {
            predicates: vec![
                Predicate::Exists {
                    variable: "a".into(),
                },
                Predicate::Missing {
                    variable: "b".into(),
                },
            ],
        },
        Predicate::AnyOf {
            predicates: vec![Predicate::Exists {
                variable: "a".into(),
            }],
        },
    ];

    for predicate in kinds {
        let document = bioprism_repair::predicate_to_json(&predicate)
            .unwrap_or_else(|error| panic!("{predicate:?} must be writable: {error}"));
        let back = bioprism_repair::predicate_from_json(&document)
            .unwrap_or_else(|error| panic!("{predicate:?} must parse back: {error}"));
        assert_eq!(back, predicate);
    }

    assert!(
        bioprism_repair::predicate_to_json(&Predicate::NumberAtLeast {
            variable: "v".into(),
            minimum: f64::NAN,
        })
        .is_err(),
        "a non-finite bound would be encoded as null and parse back as an absent threshold, so \
         it is refused instead"
    );
}

#[test]
fn an_unevaluable_criterion_outranks_an_unmet_one_so_a_reader_is_not_told_the_failures_are_all() {
    let assembled = demo();
    let options = PlanOptions {
        declared_criteria: vec![DeclaredItem::new(
            "ghost_component_inventory_nonempty",
            "A component the tree does not carry reports a non-empty inventory.",
            Predicate::Nonempty {
                variable: "component_ghost_inventory".to_string(),
            },
        )],
        ..PlanOptions::default()
    };
    let plan = plan_for(&assembled, "ISSUE-1", &options);
    let (world, _, _) = compiled(&assembled, "ISSUE-1");
    let report = verify(&plan, &world);

    assert_eq!(
        report
            .item("check_cleared:unpinned_dependency")
            .map(|i| &i.status),
        Some(&ItemStatus::Unmet),
        "a determinate failure is present, so this test discriminates the two orderings"
    );
    assert!(
        matches!(
            report.item("ghost_component_inventory_nonempty").map(|i| &i.status),
            Some(ItemStatus::NotEvaluable(_))
        ),
        "and so is a criterion that could not run"
    );
    assert_eq!(
        report.outcome(),
        Some(Outcome::Underdetermined),
        "NotMet would presuppose the criteria were all checked, and one was not: {}",
        report.summary()
    );
    assert_eq!(
        report.item(REGION_EVIDENCE_REMOVED).map(|i| &i.status),
        Some(&ItemStatus::Unmet),
        "an absent variable outside the plan's decisive region is an obstruction, not a \
         falsification: the falsifier watches only what the derived criteria reason from"
    );
}

#[test]
fn a_succession_cannot_be_declared_without_a_declarant_and_a_statement() {
    assert!(Succession::declare("", "src/ was removed").is_err());
    assert!(Succession::declare("the test author", "   ").is_err());
    let declared = Succession::declare("the test author", "src/ was removed").unwrap();
    assert_eq!(declared.declared_by(), "the test author");
}

#[test]
fn a_region_certificate_compiled_from_another_world_cannot_bind_a_plan() {
    let assembled = demo();
    let (world, pack, _) = compiled(&assembled, "ISSUE-1");

    let elsewhere = temp_tree("foreign-region");
    copy_fixture(&elsewhere, &["assets"]);
    let other = assemble(&elsewhere, "demo-app");
    let (_, _, foreign_certificate) = compiled(&other, "ISSUE-1");

    match plan_for_issue(
        &world,
        &pack,
        "ISSUE-1",
        &foreign_certificate,
        &PlanOptions::default(),
    ) {
        Err(RepairError::RegionWorldMismatch { expected, found }) => {
            assert_eq!(expected, assembled.world_id);
            assert_eq!(found, other.world_id);
        }
        other => panic!(
            "a plan bound to a region compiled from another world is bound to nothing; got \
             {other:?}"
        ),
    }

    std::fs::remove_dir_all(&elsewhere).unwrap();
}

#[test]
fn a_derived_falsifier_that_watches_only_unconditional_aggregates_says_it_has_no_teeth() {
    let assembled = demo();
    let aggregate_only = plan_for(&assembled, "ISSUE-2", &PlanOptions::default());
    assert_eq!(
        aggregate_only
            .falsifiers()
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec![REGION_EVIDENCE_REMOVED],
        "ISSUE-2 declares no component, so its decisive set is aggregates alone"
    );
    assert!(
        aggregate_only
            .limitations()
            .iter()
            .any(|line| line.contains("unlikely ever to hold")),
        "a falsifier that cannot realistically fire is not the same as a falsifier, and the plan \
         must say which it has: {:?}",
        aggregate_only.limitations()
    );

    let with_component = plan_for(&assembled, "ISSUE-1", &PlanOptions::default());
    assert!(
        !with_component
            .limitations()
            .iter()
            .any(|line| line.contains("unlikely ever to hold")),
        "an issue whose component inventory can genuinely vanish must not carry the warning, or \
         it is boilerplate rather than a finding"
    );
}
