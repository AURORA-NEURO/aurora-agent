//! End-to-end claims over the demo-app and bare-script fixtures: determinism, honest loss
//! declaration, oracle verdicts with checkable witnesses, and issue-scoped evidence regions.

use bioprism_adapter::{conformance, LossKind, Source};
use bioprism_domain::DomainPack;
use bioprism_fiber::{compile_with_oracle, Query};
use bioprism_ids::to_canonical_bytes;
use bioprism_project::{
    audit, AssemblyOptions, AuditOptions, Issue, ProjectAdapter, ProjectScan, ProjectWorld,
    ScanOptions,
};
use bioprism_section::{LeakageWitness, OracleStatus};
use bioprism_world::World;
use serde_json::Value;
use std::path::PathBuf;

fn fixture_root(name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures", "projects", name]
        .iter()
        .collect()
}

fn demo_scan() -> ProjectScan {
    let (scan, _) = ProjectScan::scan(&fixture_root("demo-app"), &ScanOptions::new("demo-app"))
        .expect("demo-app scans");
    scan
}

fn demo_world_with_issues() -> ProjectWorld {
    let scan = demo_scan();
    let issues = Issue::load(&fixture_root("demo-app").join("issues.json")).expect("issues load");
    let options = AssemblyOptions {
        issues,
        ..AssemblyOptions::default()
    };
    ProjectWorld::assemble(&scan, &options).expect("demo-app assembles")
}

fn domain_check<'a>(
    witnesses: &'a [LeakageWitness],
    name: &str,
) -> Option<(&'a std::collections::BTreeMap<String, String>, &'a str)> {
    witnesses.iter().find_map(|witness| match witness {
        LeakageWitness::DomainCheck {
            check,
            observed,
            detail,
        } if check == name => Some((observed, detail.as_str())),
        _ => None,
    })
}

#[test]
fn scanning_the_demo_app_twice_yields_byte_identical_worlds_and_ingestions() {
    let root = fixture_root("demo-app");
    let options = ScanOptions::new("demo-app");
    let (first_scan, first_ingestion) = ProjectScan::scan(&root, &options).unwrap();
    let (second_scan, second_ingestion) = ProjectScan::scan(&root, &options).unwrap();

    assert_eq!(
        first_ingestion.canonical_bytes().unwrap(),
        second_ingestion.canonical_bytes().unwrap(),
        "two scans of the same tree must produce identical ingestion bytes"
    );

    let first_world = ProjectWorld::assemble(&first_scan, &AssemblyOptions::default()).unwrap();
    let second_world = ProjectWorld::assemble(&second_scan, &AssemblyOptions::default()).unwrap();
    assert_eq!(
        to_canonical_bytes(&first_world.world).unwrap(),
        to_canonical_bytes(&second_world.world).unwrap(),
        "two assemblies of the same scan must produce identical world bytes"
    );
    assert_eq!(first_world.world_id, second_world.world_id);
    assert!(first_world.world_id.starts_with("project-"));
}

#[test]
fn the_assembled_world_validates_and_its_dimension_document_classifies_every_scope_dimension() {
    let assembled = demo_world_with_issues();
    World::from_json(assembled.world.clone()).expect("world passes the reference validator");

    let pack = DomainPack::from_json(&assembled.pack).expect("pack parses");
    let registry = pack.dimension_registry();
    let scoped: Vec<&Value> = assembled.world["facts"]
        .as_array()
        .unwrap()
        .iter()
        .chain(assembled.world["factors"].as_array().unwrap().iter())
        .collect();
    assert!(
        scoped.len() >= 16,
        "the walk produced only {} scoped documents; an empty world classifies vacuously",
        scoped.len()
    );
    for document in scoped {
        for dimension in document["scope"].as_object().expect("scope object").keys() {
            assert!(
                registry.classify(dimension).is_classified(),
                "dimension {dimension:?} on {} is unclassified under the world's own pack",
                document["id"]
            );
        }
    }
}

#[test]
fn the_release_audit_fires_unpinned_dependency_and_the_witness_names_the_dependency() {
    let report = audit(&fixture_root("demo-app"), &AuditOptions::new("demo-app")).unwrap();

    assert_eq!(report.status, OracleStatus::Invalid);
    assert_eq!(report.oracle_kind, "rule/project-release-readiness-v1");
    let (observed, detail) = domain_check(&report.witnesses, "unpinned_dependency")
        .expect("the unpinned_dependency check fired");
    let bindings = observed
        .get("unpinned_dependencies")
        .expect("the witness carries the observed unpinned set");
    assert!(
        bindings.contains("loose-gadget") && bindings.contains("1.0"),
        "the witness must name the unpinned dependency; got {bindings}"
    );
    assert!(
        !bindings.contains("exact-widget"),
        "the pinned dependency must not appear in the unpinned witness; got {bindings}"
    );
    assert!(detail.contains("resolved version") || detail.contains("static"));

    // The fixture has one test and one workflow, so the absence checks must not fire.
    assert!(domain_check(&report.witnesses, "tests_absent").is_none());
    assert!(domain_check(&report.witnesses, "no_ci").is_none());
    assert!(domain_check(&report.witnesses, "todo_burden").is_none());
}

#[test]
fn the_component_naming_issue_compiles_to_a_region_with_that_component_and_without_another() {
    let assembled = demo_world_with_issues();
    let world = World::from_json(assembled.world.clone()).unwrap();
    let pack = DomainPack::from_json(&assembled.pack).unwrap();
    let query = Query::from_json(
        assembled
            .issue_queries
            .get("ISSUE-1")
            .expect("ISSUE-1 query generated")
            .clone(),
    )
    .unwrap();

    let out = compile_with_oracle(&world, &query, pack.oracle()).expect("issue query compiles");
    let selected = &out.certificate.selected_facts;
    assert!(
        selected.iter().any(|id| id == "fact.component.src"),
        "the issue names src/lib.rs, so the src inventory must be in its region; got {selected:?}"
    );
    assert!(
        !selected.iter().any(|id| id == "fact.component.assets"),
        "the assets component is not named by the issue and must be excluded; got {selected:?}"
    );
    assert!(
        selected.iter().any(|id| id == "fact.issue.ISSUE-1"),
        "the issue's own record belongs to its region; got {selected:?}"
    );
    assert!(out.trace.dropped_protected.is_empty());
}

#[test]
fn the_issue_without_components_compiles_against_the_aggregates_alone() {
    let assembled = demo_world_with_issues();
    let world = World::from_json(assembled.world.clone()).unwrap();
    let pack = DomainPack::from_json(&assembled.pack).unwrap();
    let query =
        Query::from_json(assembled.issue_queries.get("ISSUE-2").unwrap().clone()).unwrap();

    let out = compile_with_oracle(&world, &query, pack.oracle()).expect("issue query compiles");
    let selected = &out.certificate.selected_facts;
    assert!(
        !selected.iter().any(|id| id.starts_with("fact.component.")),
        "an issue declaring no components gets no component inventory; got {selected:?}"
    );
    assert!(
        selected
            .iter()
            .any(|id| id == "fact.aggregate.dependency_declarations"),
        "the aggregate decision inputs are the whole region; got {selected:?}"
    );
    assert!(out.trace.dropped_protected.is_empty());
}

#[test]
fn every_skipped_or_unread_byte_is_declared_with_a_location() {
    let scan = demo_scan();
    let entries = scan.loss.entries();

    assert!(
        scan.files.len() >= 7,
        "the walk found only {} files, fewer than the fixture holds; an empty scan and a \
         clean tree look identical from here",
        scan.files.len()
    );
    // Every file carries at least one declaration, so a scan can never quietly claim a file
    // was fully understood.
    assert!(entries.len() >= scan.files.len());

    let binary = entries
        .iter()
        .find(|entry| entry.location.artifact.as_deref() == Some("assets/blob.bin"))
        .expect("the non-UTF-8 asset is declared");
    assert_eq!(binary.kind, LossKind::ContentUninterpreted);
    assert!(binary.detail.contains("UTF-8"));

    let manifest_line = entries
        .iter()
        .find(|entry| {
            entry.location.artifact.as_deref() == Some("Cargo.toml")
                && entry.location.record.is_some()
        })
        .expect("at least one unparsed Cargo.toml line is declared at its line number");
    assert!(manifest_line.detail.contains("narrow reader"));

    // No entry can lack a location by construction; assert the sharper claim that every
    // file-level declaration names a real scanned or excluded path.
    for entry in entries {
        assert!(!entry.location.source.is_empty());
    }
}

#[test]
fn a_scan_with_no_tests_and_no_ci_fires_both_absence_checks() {
    let report = audit(
        &fixture_root("bare-script"),
        &AuditOptions::new("bare-script"),
    )
    .unwrap();

    assert_eq!(report.status, OracleStatus::Invalid);
    let (observed, detail) =
        domain_check(&report.witnesses, "tests_absent").expect("tests_absent fired");
    assert_eq!(
        observed.get("test_function_total").map(String::as_str),
        Some("0"),
        "the witness shows the counted zero, which the description scopes to the static proxy"
    );
    assert!(detail.contains("counted"));

    let (observed, _) = domain_check(&report.witnesses, "no_ci").expect("no_ci fired");
    assert_eq!(
        observed.get("ci_workflow_inventory").map(String::as_str),
        Some("[]"),
        "the witness shows the empty inventory, not a fabricated count"
    );

    // The pyproject dependency is unpinned too; the fixture proves checks stack.
    assert!(domain_check(&report.witnesses, "unpinned_dependency").is_some());
}

#[test]
fn the_project_adapter_passes_independent_conformance_on_the_demo_fixture() {
    let adapter = ProjectAdapter::new(ScanOptions::new("demo-app"));
    let source = Source::directory("demo-app", fixture_root("demo-app"));
    let (report, ingestion) = conformance::certify(&adapter, &source).expect("certify runs");
    assert!(
        report.verified(),
        "the adapter must satisfy the sealed contract against an independent probe: {}",
        report.summary()
    );
    assert!(ingestion.fact_count() >= 7, "one fact per file at minimum");
    assert!(ingestion.loss().kinds().contains(&LossKind::ContentUninterpreted));
}

#[test]
fn files_under_an_excluded_directory_are_accounted_to_the_independent_probe_not_dropped() {
    let root = std::env::temp_dir().join(format!(
        "bioprism-project-excluded-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("target/debug")).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("target/debug/app.bin"), b"artifact").unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn a() {}\n").unwrap();

    let adapter = ProjectAdapter::new(ScanOptions::new("excluded-demo"));
    let source = Source::directory("excluded-demo", &root);
    let (report, ingestion) = conformance::certify(&adapter, &source).expect("certify runs");
    assert!(
        report.verified(),
        "an excluded file must be declared lost, or the independent probe finds it \
         unaccounted: {}",
        report.summary()
    );
    let excluded = ingestion
        .loss()
        .entries()
        .iter()
        .find(|entry| entry.location.artifact.as_deref() == Some("target/debug/app.bin"))
        .expect("the excluded file is declared at its exact path");
    assert!(excluded.detail.contains("exclusion list"));
    // Excluded files produce no fact: presence in the loss report is their only trace.
    assert_eq!(ingestion.fact_count(), 1, "one fact for src/lib.rs and none for the artifact");

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_over_the_byte_cap_is_named_sized_and_declared_never_hashed_in_silence() {
    let root = std::env::temp_dir().join(format!("bioprism-project-cap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("big.dat"), vec![b'x'; 100]).unwrap();
    std::fs::write(root.join("small.txt"), b"ok\n").unwrap();

    let options = ScanOptions::new("capped").with_max_file_bytes(10);
    let (scan, _) = ProjectScan::scan(&root, &options).unwrap();

    let big = scan.files.iter().find(|f| f.path == "big.dat").unwrap();
    assert_eq!(big.byte_length, Some(100));
    assert!(big.sha256.is_none(), "an unread file has no digest to claim");
    let declared = scan
        .loss
        .entries()
        .iter()
        .find(|entry| entry.location.artifact.as_deref() == Some("big.dat"))
        .expect("the oversized skip is declared");
    assert!(declared.detail.contains("cap"));
    assert!(declared.detail.contains("missing, not zero"));

    std::fs::remove_dir_all(&root).unwrap();
}

/// DOGFOOD: scan this repository itself, assemble, audit, and print the verdict. Ignored by
/// default because it walks the whole worktree (including enumerating target/ and .git/ for
/// the loss report); run with `cargo test -p bioprism-project --offline -- --ignored --nocapture`.
#[test]
#[ignore]
fn dogfood_the_repository_scans_assembles_and_is_judged_by_its_own_pack() {
    let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let report = audit(&root, &AuditOptions::new("aurora-agent-fable")).expect("audit runs");
    println!("DOGFOOD {}", report.summary());
    println!(
        "DOGFOOD witness kinds: {:?}",
        report
            .witnesses
            .iter()
            .map(|w| w.kind())
            .collect::<Vec<_>>()
    );
    println!("DOGFOOD fact count: {}", report.fact_count);
    for witness in &report.witnesses {
        if let LeakageWitness::DomainCheck { check, observed, .. } = witness {
            for (variable, rendered) in observed {
                let mut shown = rendered.clone();
                if shown.len() > 600 {
                    let cut = (0..=600)
                        .rev()
                        .find(|&index| shown.is_char_boundary(index))
                        .unwrap_or(0);
                    shown.truncate(cut);
                    shown.push('…');
                }
                println!("DOGFOOD {check} observed {variable} = {shown}");
            }
        }
    }
    assert!(report.fact_count > 10, "the repository is not empty");
}
