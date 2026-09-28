//! The gate between evidence and surface, and the standing it produces. These are the invariants
//! that stop a document about a Python package from being reported as verified by a Rust test.

use bioprism_cookbook::{standard_cookbook, CrateName, Workspace};
use bioprism_devplat::audit::{catalogues_are_disjoint, findings, recipes_are_all_in_tree};
use bioprism_devplat::claim::{ApiClaim, ApiName, Evidence};
use bioprism_devplat::surface::{Locale, Surface, SurfaceKind};
use bioprism_devplat::walkthrough::{
    recheck, standard_walkthroughs, Standing, Step, Walkthrough, WalkthroughId,
};
use bioprism_devplat::DevPlatReport;

fn devplat() -> Surface {
    Surface::rust(&CrateName::parse("bioprism-devplat").expect("valid")).expect("workspace crate")
}

fn python() -> Surface {
    Surface::foreign(SurfaceKind::PythonPackage, "prism_sdk").expect("well formed")
}

fn name(value: &str) -> ApiName {
    ApiName::parse(value).expect("non-empty")
}

#[test]
fn locale_belongs_to_the_artifact_address_not_its_language() {
    let rust = devplat();
    let action = Surface::in_repository(
        SurfaceKind::GitHubAction,
        ".github/actions/autonomous-run/action.yml",
    )
    .expect("a checked-in action file is a local surface");
    let hosted = Surface::foreign(SurfaceKind::GitHubAction, "hosted consumer workflow")
        .expect("a hosted workflow is external");

    assert_eq!(rust.locale(), Locale::InRepository);
    assert!(rust.is_falsifiable_here());
    assert_eq!(action.locale(), Locale::InRepository);
    assert!(action.is_falsifiable_here());
    assert_eq!(hosted.locale(), Locale::OutsideRepository);
    assert!(!hosted.is_falsifiable_here());
}

#[test]
fn local_non_rust_surfaces_require_normalized_relative_paths() {
    for path in [
        "../outside/action.yml",
        "/absolute/action.yml",
        "a\\b",
        "C:/a.yml",
        "a//b",
    ] {
        assert!(
            Surface::in_repository(SurfaceKind::GitHubAction, path).is_err(),
            "accepted unsafe local artifact path `{path}`"
        );
    }
    assert!(Surface::in_repository(SurfaceKind::RustCrate, "bioprism-devplat").is_err());
}

#[test]
fn surface_wire_preserves_locality_and_reads_legacy_foreign_addresses() {
    let local = Surface::in_repository(
        SurfaceKind::GitHubAction,
        ".github/actions/autonomous-run/action.yml",
    )
    .expect("checked-in action");
    let value = serde_json::to_value(&local).expect("serialize surface");
    assert_eq!(value["locale"], "in_repository");
    let round_trip: Surface = serde_json::from_value(value).expect("locality survives the wire");
    assert_eq!(round_trip, local);

    let legacy: Surface = serde_json::from_value(serde_json::json!({
        "kind": "git_hub_action",
        "artifact": "aurora-neuro/prism-action@v1"
    }))
    .expect("older wire records default to the kind's former locale");
    assert_eq!(legacy.locale(), Locale::OutsideRepository);

    assert!(serde_json::from_value::<Surface>(serde_json::json!({
        "kind": "rust_crate",
        "artifact": "bioprism-devplat",
        "locale": "outside_repository"
    }))
    .is_err());
}

#[test]
fn an_in_tree_action_claim_is_falsifiable_and_rechecked_against_its_file() {
    let surface = Surface::in_repository(
        SurfaceKind::GitHubAction,
        ".github/actions/autonomous-run/action.yml",
    )
    .expect("checked-in composite action");
    let claim = ApiClaim::about(name("using"), surface)
        .resolved_in(".github/actions/autonomous-run/action.yml")
        .seal()
        .expect("local action claims can carry local evidence");
    let walkthrough = Walkthrough::draft(
        WalkthroughId::parse("local-autonomous-action").expect("valid id"),
        "Check the action entrypoint contract.",
        claim.surface().clone(),
    )
    .step(Step::naming("Read the action metadata.", claim))
    .seal()
    .expect("one checkable step");
    let workspace = Workspace::here().expect("the workspace opens");

    assert!(matches!(
        walkthrough.standing(),
        Standing::CheckableHere { claims: 1 }
    ));
    assert!(matches!(
        recheck(&walkthrough, &workspace).as_slice(),
        [(api, Evidence::ResolvedInTree { .. })] if api == "using"
    ));
}

#[cfg(unix)]
#[test]
fn rechecking_refuses_evidence_that_escapes_through_a_symlink() {
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the epoch")
        .as_nanos();
    let temporary = std::env::temp_dir().join(format!(
        "bioprism-devplat-symlink-{}-{nonce}",
        std::process::id()
    ));
    let root = temporary.join("workspace");
    std::fs::create_dir_all(&root).expect("create temporary workspace");
    std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .expect("write temporary workspace manifest");
    let outside = temporary.join("outside.rs");
    std::fs::write(&outside, "fn call() {}\n").expect("write external evidence fixture");
    symlink(&outside, root.join("link.rs")).expect("create symlink outside the workspace");

    let workspace = Workspace::open(&root).expect("open temporary workspace");
    let surface =
        Surface::in_repository(SurfaceKind::GitHubAction, "link.rs").expect("local action surface");
    let claim = ApiClaim::about(name("call"), surface)
        .resolved_in("link.rs")
        .seal()
        .expect("relative evidence path");
    let walkthrough = Walkthrough::draft(
        WalkthroughId::parse("symlink-evidence").expect("valid id"),
        "Check a claimed local symbol.",
        claim.surface().clone(),
    )
    .step(Step::naming("Read the symbol.", claim))
    .seal()
    .expect("one checkable step");

    assert!(matches!(
        recheck(&walkthrough, &workspace).as_slice(),
        [(_, Evidence::AbsentFromTree)]
    ));
    std::fs::remove_dir_all(&temporary).expect("remove temporary workspace");
}

#[test]
fn a_surface_cannot_be_built_without_naming_its_artifact() {
    assert!(Surface::foreign(SurfaceKind::HttpApi, "   ").is_err());
    assert!(Surface::foreign(SurfaceKind::TypeScriptPackage, "").is_err());
    assert!(Surface::foreign(SurfaceKind::HttpApi, " api").is_err());
    assert!(Surface::foreign(SurfaceKind::HttpApi, "api\u{0000}").is_err());
}

#[test]
fn an_in_repository_surface_must_name_a_workspace_crate() {
    assert!(Surface::rust(&CrateName::parse("serde").expect("valid")).is_err());
    assert!(Surface::foreign(SurfaceKind::RustCrate, "serde").is_err());
    assert!(Surface::foreign(SurfaceKind::RustCrate, "bioprism-devplat").is_ok());
}

#[test]
fn a_foreign_surface_cannot_carry_in_tree_evidence() {
    let resolved = ApiClaim::about(name("prism.compiler.mine"), python())
        .resolved_in("crates/devplat/src/lib.rs")
        .seal();
    assert!(resolved.is_err(), "a python api was reported as found here");
    let absent = ApiClaim::about(name("prism.compiler.mine"), python())
        .absent()
        .seal();
    assert!(absent.is_err(), "a python api was reported as missing here");
}

#[test]
fn an_in_tree_surface_cannot_be_excused_as_outside_the_tree() {
    let claim = ApiClaim::about(name("bioprism_devplat::render"), devplat())
        .outside("we did not get around to it")
        .seal();
    assert!(claim.is_err());
}

#[test]
fn an_unverifiable_claim_without_a_reason_is_refused() {
    assert!(ApiClaim::about(name("prism.lab.compare"), python())
        .outside("   ")
        .seal()
        .is_err());
    assert!(ApiClaim::about(name("prism.lab.compare"), python())
        .seal()
        .is_err());
}

#[test]
fn a_resolved_claim_must_name_a_safe_repository_relative_file() {
    assert!(ApiClaim::about(name("bioprism_devplat::render"), devplat())
        .resolved_in("")
        .seal()
        .is_err());
    for file in [
        "../outside.rs",
        "/outside.rs",
        "C:/outside.rs",
        "crates\\devplat\\src\\lib.rs",
        "crates//devplat/src/lib.rs",
        "./crates/devplat/src/lib.rs",
    ] {
        assert!(
            ApiClaim::about(name("bioprism_devplat::render"), devplat())
                .resolved_in(file)
                .seal()
                .is_err(),
            "accepted evidence path `{file}` outside the normalized workspace-relative form"
        );
    }
}

#[test]
fn claim_names_and_evidence_metadata_are_canonical() {
    assert!(ApiName::parse(" prism.compiler.mine").is_err());
    assert!(ApiName::parse("prism.compiler.mine\u{0000}").is_err());
    assert!(ApiClaim::about(name("bioprism_devplat::render"), devplat())
        .resolved_in(" crates/devplat/src/lib.rs")
        .seal()
        .is_err());
    assert!(ApiClaim::about(name("prism.compiler.mine"), python())
        .outside(" outside this repository")
        .seal()
        .is_err());

    let forged = serde_json::json!({
        "api": " prism.compiler.mine",
        "surface": { "kind": "python_package", "artifact": "prism_sdk" },
        "evidence": { "evidence": "outside_tree", "reason": "not in this repository" }
    });
    assert!(serde_json::from_value::<ApiClaim>(forged).is_err());
}

#[test]
fn the_gate_runs_again_when_a_claim_arrives_over_the_wire() {
    let good = ApiClaim::about(name("bioprism_devplat::render"), devplat())
        .resolved_in("crates/devplat/src/report.rs")
        .seal()
        .expect("valid");
    let text = serde_json::to_string(&good).expect("serialises");
    let back: ApiClaim = serde_json::from_str(&text).expect("round trips");
    assert_eq!(back, good);

    let forged = serde_json::json!({
        "api": "prism.compiler.mine",
        "surface": { "kind": "python_package", "artifact": "prism_sdk" },
        "evidence": { "evidence": "resolved_in_tree", "file": "crates/devplat/src/lib.rs" }
    });
    assert!(
        serde_json::from_value::<ApiClaim>(forged).is_err(),
        "the wire form bypassed the gate"
    );
}

#[test]
fn evidence_is_three_valued_and_outside_is_neither_support_nor_refutation() {
    let outside = Evidence::OutsideTree {
        reason: "not in this repository".to_string(),
    };
    assert!(!outside.supports_the_document());
    assert!(!outside.refutes_the_document());
    assert!(Evidence::AbsentFromTree.refutes_the_document());
    assert!(Evidence::ResolvedInTree {
        file: "a".to_string()
    }
    .supports_the_document());
}

#[test]
fn a_walkthrough_that_names_nothing_is_refused() {
    let draft = Walkthrough::draft(
        WalkthroughId::parse("all-prose").expect("valid"),
        "Explain the platform.",
        devplat(),
    )
    .step(Step::narrating("Read the overview.", "it is orientation"));
    assert!(draft.seal().is_err());
}

#[test]
fn narration_must_say_why_it_names_nothing() {
    let claim = ApiClaim::about(name("bioprism_devplat::render"), devplat())
        .resolved_in("crates/devplat/src/report.rs")
        .seal()
        .expect("valid");
    let draft = Walkthrough::draft(
        WalkthroughId::parse("half-prose").expect("valid"),
        "Render a report.",
        devplat(),
    )
    .step(Step::naming("Render.", claim))
    .step(Step::narrating("Think about it.", "  "));
    assert!(draft.seal().is_err());
}

#[test]
fn a_walkthrough_needs_a_goal_and_at_least_one_step() {
    assert!(Walkthrough::draft(
        WalkthroughId::parse("empty").expect("valid"),
        "  ",
        devplat()
    )
    .seal()
    .is_err());
    assert!(Walkthrough::draft(
        WalkthroughId::parse("stepless").expect("valid"),
        "Do a thing.",
        devplat()
    )
    .seal()
    .is_err());
}

#[test]
fn a_walkthrough_identifier_is_kebab_case() {
    assert!(WalkthroughId::parse("python-sdk-quickstart").is_ok());
    assert!(WalkthroughId::parse("Python-SDK").is_err());
    assert!(WalkthroughId::parse("-leading").is_err());
    assert!(WalkthroughId::parse("trailing-").is_err());
    assert!(WalkthroughId::parse("with space").is_err());
}

#[test]
fn standing_is_derived_from_the_claims_and_cannot_be_declared() {
    let book = standard_walkthroughs().expect("the catalogue seals");
    let find = |id: &str| {
        book.iter()
            .find(|w| w.id().as_str() == id)
            .unwrap_or_else(|| panic!("`{id}` is in the catalogue"))
            .standing()
    };
    assert!(matches!(
        find("python-sdk-quickstart"),
        Standing::EntirelyOutside { .. }
    ));
    assert!(matches!(
        find("one-evidence-state-report"),
        Standing::CheckableHere { .. }
    ));
    assert!(matches!(
        find("mcp-agent-quickstart"),
        Standing::PartlyOutside {
            here: 2,
            outside: 1
        }
    ));
}

#[test]
fn the_two_documents_the_section_writes_out_are_entirely_outside_this_repository() {
    let book = standard_walkthroughs().expect("the catalogue seals");
    let outside: Vec<&str> = book
        .iter()
        .filter(|w| w.documents_absent_artifact())
        .map(|w| w.id().as_str())
        .collect();
    assert_eq!(outside, vec!["python-sdk-quickstart", "ci-regression-gate"]);
}

#[test]
fn guarded_and_unguarded_claims_partition_every_document() {
    for walkthrough in standard_walkthroughs().expect("the catalogue seals") {
        let standing = walkthrough.standing();
        assert_eq!(
            standing.guarded_claims() + standing.unguarded_claims(),
            walkthrough.claims().len(),
            "`{}` loses a claim between the two counts",
            walkthrough.id().as_str()
        );
    }
}

#[test]
fn every_in_tree_claim_in_the_catalogue_resolves_against_the_working_tree() {
    let workspace = Workspace::here().expect("the workspace opens");
    for walkthrough in standard_walkthroughs().expect("the catalogue seals") {
        for (api, evidence) in recheck(&walkthrough, &workspace) {
            assert!(
                !matches!(evidence, Evidence::AbsentFromTree),
                "`{api}` in `{}` no longer exists",
                walkthrough.id().as_str()
            );
        }
    }
}

#[test]
fn rechecking_leaves_a_foreign_claim_exactly_as_it_was() {
    let workspace = Workspace::here().expect("the workspace opens");
    let book = standard_walkthroughs().expect("the catalogue seals");
    let python_doc = book
        .iter()
        .find(|w| w.id().as_str() == "python-sdk-quickstart")
        .expect("in the catalogue");
    for (_, evidence) in recheck(python_doc, &workspace) {
        assert!(matches!(evidence, Evidence::OutsideTree { .. }));
    }
}

#[test]
fn narration_share_is_exact_rather_than_rounded_through_a_float() {
    let book = standard_walkthroughs().expect("the catalogue seals");
    let ci = book
        .iter()
        .find(|w| w.id().as_str() == "ci-regression-gate")
        .expect("in the catalogue");
    assert_eq!(ci.steps().len(), 3);
    assert_eq!(ci.narration_permille(), 333);
}

#[test]
fn the_walkthrough_catalogue_shares_no_identifier_with_the_cookbook() {
    let cookbook = standard_cookbook().expect("the cookbook seals");
    let book = standard_walkthroughs().expect("the catalogue seals");
    assert!(catalogues_are_disjoint(&cookbook, &book).is_empty());
}

#[test]
fn every_crate_the_cookbook_names_is_a_crate_of_this_workspace() {
    let cookbook = standard_cookbook().expect("the cookbook seals");
    let workspace = Workspace::here().expect("the workspace opens");
    assert!(
        recipes_are_all_in_tree(&cookbook, &workspace).is_empty(),
        "the recipe type is supposed to make this impossible"
    );
}

#[test]
fn the_report_digest_is_a_function_of_the_catalogue_alone() {
    let first = DevPlatReport::of(&standard_walkthroughs().expect("seals")).expect("builds");
    let second = DevPlatReport::of(&standard_walkthroughs().expect("seals")).expect("builds");
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.modules_classified(), 20);
    assert_eq!(first.implemented.len(), 5);
    assert_eq!(first.not_implemented.len(), 15);
}

#[test]
fn the_report_counts_guarded_and_unguarded_claims_separately() {
    let report = DevPlatReport::of(&standard_walkthroughs().expect("seals")).expect("builds");
    assert!(
        report.unguarded_claims > 0,
        "a catalogue with no foreign claims would not need this crate"
    );
    assert!(report.guarded_claims > 0);
}

#[test]
fn a_finding_names_a_remedy_with_an_observable_consequence() {
    let workspace = Workspace::here().expect("the workspace opens");
    let book = standard_walkthroughs().expect("the catalogue seals");
    let found = findings(&book, &workspace);
    assert_eq!(
        found.len(),
        2,
        "one per entirely-outside document, and no stale claims"
    );
    for finding in found {
        assert!(!finding.invariant.trim().is_empty());
        assert!(!finding.observed.trim().is_empty());
        assert!(
            !finding.remedy.verified_by.trim().is_empty(),
            "a remedy with no observable consequence has no stopping condition"
        );
    }
}
