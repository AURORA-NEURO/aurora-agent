//! The catalogue as an artefact: enumerable, deterministic, and honest about both columns.
//!
//! `crates/examples` established the pattern these tests defend — a registry that printed only its
//! passing entries would be a worse artefact than no registry, because it would look like
//! completeness. So the catalogue reports what its worlds make exercisable *and* what stays
//! blocked, and these tests check that the second column is non-empty and reasoned.

use bioprism_bioworlds::catalog::{
    post_treatment_underdetermination, trial_eligibility_temporal_firewall, SliceCatalog,
    NON_PROTECTED_TEMPORAL_WITHHOLDING, UNDERDETERMINED_ABSTENTION,
};
use bioprism_bioworlds::{BioWorldError, SliceCatalog as Catalog};

#[test]
fn every_shipped_slice_holds_every_check_it_declares() {
    let report = SliceCatalog::standard()
        .expect("catalogue builds")
        .run_all()
        .expect("catalogue runs");
    assert!(
        report.holds(),
        "failing slices: {:#?}",
        report
            .failing()
            .iter()
            .map(|slice| (&slice.slice_id, &slice.failures))
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_catalogue_ships_four_slices_two_of_which_are_controls() {
    let catalogue = SliceCatalog::standard().expect("catalogue builds");
    assert_eq!(catalogue.len(), 4);
    let controls = catalogue
        .ids()
        .iter()
        .filter(|id| id.contains("control"))
        .count();
    assert_eq!(controls, 2, "the controls are what make the other two mean anything");
}

#[test]
fn a_duplicate_slice_id_is_a_typed_error_rather_than_a_shadowed_entry() {
    let slice = trial_eligibility_temporal_firewall().expect("builds");
    let duplicate = trial_eligibility_temporal_firewall().expect("builds");
    let refused = Catalog::from_slices(vec![slice, duplicate]);
    assert!(matches!(refused, Err(BioWorldError::DuplicateSlice(_))));
}

#[test]
fn an_unknown_slice_id_is_a_typed_error() {
    let catalogue = SliceCatalog::standard().expect("catalogue builds");
    assert!(matches!(
        catalogue.get("no-such-slice"),
        Err(BioWorldError::UnknownSlice(_))
    ));
}

#[test]
fn the_catalogue_report_recomputes_its_own_digest() {
    let report = SliceCatalog::standard()
        .expect("catalogue builds")
        .run_all()
        .expect("catalogue runs");
    assert!(report.digest_is_intact());
    for slice in &report.slices {
        assert!(slice.digest_is_intact(), "{} digest is stale", slice.slice_id);
    }
}

#[test]
fn two_runs_of_the_catalogue_produce_the_same_digest() {
    let first = SliceCatalog::standard()
        .expect("builds")
        .run_all()
        .expect("runs");
    let second = SliceCatalog::standard()
        .expect("builds")
        .run_all()
        .expect("runs");
    assert_eq!(first.digest, second.digest);
    assert_eq!(first, second);
}

#[test]
fn tampering_with_a_slice_report_breaks_its_digest() {
    let mut report = SliceCatalog::standard()
        .expect("builds")
        .run_all()
        .expect("runs");
    report.slices[0].findings.push("an added claim".into());
    assert!(!report.digest_is_intact());
}

#[test]
fn every_slice_cites_at_least_one_blueprint_module() {
    for slice in SliceCatalog::standard().expect("builds").slices() {
        assert!(
            !slice.blueprint_modules.is_empty(),
            "{} cites no blueprint module",
            slice.id
        );
        assert!(!slice.claim.is_empty());
        assert!(!slice.checks.is_empty(), "{} asserts nothing", slice.id);
    }
}

#[test]
fn both_world_shaped_backlog_properties_appear_in_the_still_blocked_column_with_a_reason() {
    let report = SliceCatalog::standard()
        .expect("builds")
        .run_all()
        .expect("runs");
    assert!(report
        .still_blocked
        .contains(&NON_PROTECTED_TEMPORAL_WITHHOLDING.to_string()));
    assert!(report
        .still_blocked
        .contains(&UNDERDETERMINED_ABSTENTION.to_string()));

    for slice in &report.slices {
        for blocked in &slice.still_blocked {
            assert!(
                blocked.reason.len() > 40,
                "{} records {} as blocked with no usable reason",
                slice.slice_id,
                blocked.property_id
            );
        }
    }
}

#[test]
fn the_temporal_property_is_reported_as_exercisable_and_still_not_demonstrated() {
    let slice = trial_eligibility_temporal_firewall().expect("builds");
    assert!(slice
        .makes_exercisable
        .contains(&NON_PROTECTED_TEMPORAL_WITHHOLDING.to_string()));
    assert!(
        slice
            .still_blocked
            .iter()
            .any(|blocked| blocked.property_id == NON_PROTECTED_TEMPORAL_WITHHOLDING),
        "a world cannot demonstrate a property that needs a compile; both columns must name it"
    );
}

/// This crate's `still_blocked` entry is true of *this crate* and no longer true of the workspace.
///
/// The distinction is worth keeping rather than deleting the entry. `bioprism-fiber` is
/// deliberately not a dependency here, so these worlds are characterised structurally and this
/// crate genuinely still cannot demonstrate the property — that is what `still_blocked` records.
/// What changed is that `crates/examples` now can, using a world spec this crate's findings argued
/// for. A reader comparing the two crates should see why they disagree rather than concluding one
/// of them is stale.
#[test]
fn what_this_crate_still_cannot_do_is_now_done_by_a_crate_that_compiles() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/src/catalog.rs"
    ))
    .expect("the examples catalogue is readable");

    assert!(
        source.contains("unprotected-temporal-withholding-v1"),
        "the slice that answers this crate's still_blocked entry is gone; either the claim          regressed or the slice was renamed, and this crate's own account of the gap is now wrong"
    );
}

#[test]
fn the_unfavourable_control_ships_its_finding_rather_than_hiding_it() {
    let catalogue = SliceCatalog::standard().expect("builds");
    let control = catalogue
        .get("trial-eligibility-firewall-reference-shaped-control")
        .expect("the control is registered");
    assert!(
        control
            .findings
            .iter()
            .any(|finding| finding.starts_with("UNFAVOURABLE")),
        "the control's separating depth is the unflattering result and must be stated"
    );
}

#[test]
fn neither_control_claims_to_make_any_property_exercisable() {
    let catalogue = SliceCatalog::standard().expect("builds");
    for id in catalogue.ids().into_iter().filter(|id| id.contains("control")) {
        let control = catalogue.get(id).expect("registered");
        assert!(
            control.makes_exercisable.is_empty(),
            "{id} is a control; crediting it with unlocking a claim would be the inflation this \
             catalogue exists to avoid"
        );
    }
}

#[test]
fn every_query_shape_serialises_to_the_fiber_query_wire_schema() {
    for (id, document) in SliceCatalog::standard().expect("builds").query_documents() {
        assert_eq!(
            document.get("schema_version").and_then(|v| v.as_str()),
            Some("fiber-query/0.1"),
            "{id} does not emit a document a compiler could read"
        );
        assert!(document.get("targets").and_then(|v| v.as_array()).is_some());
        assert!(document.get("decision_time").is_some());
    }
}

#[test]
fn the_rendered_report_names_every_slice_and_its_verdict() {
    let report = SliceCatalog::standard()
        .expect("builds")
        .run_all()
        .expect("runs");
    let rendered = report.render();
    for slice in &report.slices {
        assert!(rendered.contains(&slice.slice_id));
    }
    assert!(rendered.contains("still blocked"));
}

#[test]
fn running_one_slice_by_id_matches_that_slice_inside_the_full_run() {
    let catalogue = SliceCatalog::standard().expect("builds");
    let single = catalogue
        .run("post-treatment-underdetermination")
        .expect("runs");
    let full = catalogue.run_all().expect("runs");
    let inside = full
        .slices
        .iter()
        .find(|slice| slice.slice_id == "post-treatment-underdetermination")
        .expect("present");
    assert_eq!(&single, inside);
}

#[test]
fn the_underdetermination_slice_declares_the_hypotheses_it_leaves_live() {
    let report = post_treatment_underdetermination()
        .expect("builds")
        .run()
        .expect("runs");
    assert_eq!(report.hypotheses.live_hypotheses.len(), 3);
    assert!(report.hypotheses.is_underdetermined());
    assert!(report.holds());
}
