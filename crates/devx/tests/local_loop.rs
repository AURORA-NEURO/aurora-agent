//! The local loop held as a contract.
//!
//! These tests pin the two properties a contributor relies on: that the expensive invalidation is
//! declared rather than discovered, and that an undeclared path is refused rather than answered
//! with silence.

use bioprism_devx::devloop::{
    invalidated_by, validate_contract, workspace_contract, ArtifactKind, ChangeUnit,
    SURFACE_CANONICAL_SERIALISATION,
};
use bioprism_devx::error::LoopError;
use bioprism_docgraph::fixture::{repository_doc_graph, repository_routes};
use bioprism_docgraph::DocGraph;

fn no_graph() -> DocGraph {
    DocGraph::new()
}

#[test]
fn the_workspace_contract_validates() {
    assert!(validate_contract(&workspace_contract()).is_ok());
    assert_eq!(workspace_contract().len(), 5);
}

#[test]
fn a_canonical_serialisation_change_invalidates_the_cross_language_parity_fixtures() {
    for path in ["crates/ids/src/canonical.rs", "crates/ids/src/hash.rs"] {
        let set = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::source(path),
            &no_graph(),
            &[],
        )
        .expect("the surface is declared");
        assert_eq!(set.surfaces, vec![SURFACE_CANONICAL_SERIALISATION]);
        assert!(
            set.invalidates_cross_language_parity(),
            "{path} did not invalidate parity, which is the lesson this contract exists to teach"
        );
    }
}

#[test]
fn the_parity_entry_is_the_only_one_that_cannot_be_re_established_inside_this_workspace() {
    let set = invalidated_by(
        &workspace_contract(),
        &ChangeUnit::source("crates/ids/src/canonical.rs"),
        &no_graph(),
        &[],
    )
    .expect("declared");
    let external: Vec<&str> = set
        .entries
        .iter()
        .filter(|e| e.kind.needs_an_external_implementation())
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(external.len(), 1);
    assert!(external[0].contains("reference-profile"));
}

#[test]
fn a_certificate_schema_change_also_reaches_parity_but_a_manifest_change_does_not() {
    let contract = workspace_contract();
    let schema = invalidated_by(
        &contract,
        &ChangeUnit::source("crates/section/src/certificate.rs"),
        &no_graph(),
        &[],
    )
    .expect("declared");
    let manifest = invalidated_by(
        &contract,
        &ChangeUnit::source("crates/section/src/omission.rs"),
        &no_graph(),
        &[],
    )
    .expect("declared");
    assert!(schema.invalidates_cross_language_parity());
    assert!(!manifest.invalidates_cross_language_parity());
    assert!(manifest.invalidates_certificate_digests());
}

#[test]
fn a_path_outside_every_declared_surface_is_refused_by_name() {
    let error = invalidated_by(
        &workspace_contract(),
        &ChangeUnit::source("crates/devx/src/lib.rs"),
        &no_graph(),
        &[],
    )
    .expect_err("this crate's own lib.rs is not a declared surface");
    match error {
        LoopError::UnownedSubject { subject } => {
            assert_eq!(subject, "crates/devx/src/lib.rs");
        }
        other => panic!("expected UnownedSubject, got {other:?}"),
    }
}

#[test]
fn a_change_to_the_transcribed_exit_code_registry_invalidates_the_transcription() {
    let set = invalidated_by(
        &workspace_contract(),
        &ChangeUnit::source("crates/cli/src/exit.rs"),
        &no_graph(),
        &[],
    )
    .expect("declared");
    assert!(set
        .of_kind(ArtifactKind::Fixture)
        .iter()
        .any(|e| e.name.contains("shipped_exit_codes")));
    assert!(set
        .entries
        .iter()
        .any(|e| e.because.contains("dependency set")));
}

#[test]
fn a_documentation_change_produces_the_same_closure_docgraph_would() {
    let graph = repository_doc_graph();
    let routes = repository_routes();
    for module in graph.node_ids().take(5) {
        let set = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::doc(module.as_str()),
            &graph,
            &routes,
        )
        .expect("the module is in the corpus");
        let report = set.doc_impact.as_ref().expect("report attached");
        assert_eq!(&report.changed, module);
        for hop in &report.affected {
            assert!(
                set.names().contains(hop.module.as_str()),
                "{} was reached by docgraph and dropped here",
                hop.module.as_str()
            );
        }
    }
}

#[test]
fn every_documentation_entry_records_the_edge_and_depth_it_was_reached_by() {
    let graph = repository_doc_graph();
    let routes = repository_routes();
    let module = graph
        .node_ids()
        .next()
        .expect("the fixture has modules")
        .clone();
    let set = invalidated_by(
        &workspace_contract(),
        &ChangeUnit::doc(module.as_str()),
        &graph,
        &routes,
    )
    .expect("in corpus");
    for entry in &set.entries {
        assert_eq!(entry.kind, ArtifactKind::DocBundle);
        assert!(entry.because.contains("depth") || entry.because.contains("route"));
    }
}

#[test]
fn every_declared_surface_states_why_it_is_drawn_where_it_is_drawn() {
    for surface in workspace_contract() {
        assert!(
            surface.rationale.len() > 40,
            "{} states no rationale",
            surface.id
        );
        assert!(!surface.owns.is_empty());
        assert!(!surface.invalidates.is_empty());
    }
}

#[test]
fn no_two_surfaces_claim_the_same_path() {
    let contract = workspace_contract();
    let mut owned: Vec<&str> = contract
        .iter()
        .flat_map(|s| s.owns.iter().map(String::as_str))
        .collect();
    let before = owned.len();
    owned.sort_unstable();
    owned.dedup();
    assert_eq!(
        before,
        owned.len(),
        "a path claimed twice would make the invalidation set depend on surface order"
    );
}

#[test]
fn the_invalidation_set_is_deterministic_across_repeated_calls() {
    let contract = workspace_contract();
    let first = invalidated_by(
        &contract,
        &ChangeUnit::source("crates/ids/src/canonical.rs"),
        &no_graph(),
        &[],
    )
    .expect("declared");
    let second = invalidated_by(
        &contract,
        &ChangeUnit::source("crates/ids/src/canonical.rs"),
        &no_graph(),
        &[],
    )
    .expect("declared");
    assert_eq!(first, second);
}
