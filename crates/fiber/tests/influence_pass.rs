//! Influence bounds on withheld evidence, and the two reasons there are none.
//!
//! Blueprint 43.28. `bioprism-examples` recorded the blocked claim `bounded_influence_omission` as
//! *"bioprism-fiber emits only InfluenceClass::Zero and DeferredAcquisition; nothing computes a
//! numeric influence bound, so no group is ever Bounded"*. Something computes one now. Nothing is
//! bounded anyway, and these tests pin why — because a claim that shrinks without a measurement
//! behind it is worse than the claim it replaced.

use bioprism_fiber::{compile, CorrespondenceCheck, NotPosable, Query};
use bioprism_influence::{InfluenceEstimate, UnknownReason};
use bioprism_section::{CertificateProfile, InfluenceClass};
use bioprism_world::World;
use serde_json::Value;
use std::path::PathBuf;

fn reference_example(name: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "reference",
        "fiber_runtime",
        "examples",
        name,
    ]
    .iter()
    .collect();
    serde_json::from_str(&std::fs::read_to_string(&path).expect("reference example is readable"))
        .expect("reference example is valid JSON")
}

fn fixture(relative: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "fiber-v0.1",
        relative,
    ]
    .iter()
    .collect();
    serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture is readable"))
        .expect("fixture is valid JSON")
}

fn deferred_compile() -> bioprism_fiber::CompileOutput {
    let world =
        World::from_json(reference_example("deferred_evidence_world.json")).expect("world loads");
    let query =
        Query::from_json(reference_example("deferred_evidence_query.json")).expect("query loads");
    let out = compile(&world, &query).expect("compiles");
    assert_eq!(
        out.certificate.omissions.inaccessible_selected_before_cut.len(),
        2,
        "this fixture must withhold evidence at the cut or the checks below prove nothing"
    );
    out
}

/// The shipped world exercises the pass and gains nothing, which is the published measurement.
#[test]
fn the_reference_world_withholds_nothing_so_the_split_is_empty() {
    let world = World::from_json(fixture("radiogenomic_world.json")).expect("world loads");
    let query = Query::from_json(fixture("leakage_query.json")).expect("query loads");
    let out = compile(&world, &query).expect("compiles");

    assert!(out.trace.withheld_influence.attempted.is_empty());
    assert_eq!(out.trace.withheld_influence.promoted(), 0);
    assert_eq!(out.certificate.manifest.count_in(InfluenceClass::Bounded), 0);
    assert_eq!(out.certificate.manifest.count_in(InfluenceClass::Zero), 750);
    assert_eq!(
        out.certificate.digest(CertificateProfile::Reference).unwrap().as_str(),
        "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4"
    );
}

/// A withheld fact inside the compiled region is analysed and comes back unknown, not bounded.
///
/// `fiber-world/0.1` declares a factor's signature and never its potential, so the only method
/// that survives a table-less region — the dynamic-range lemma under a *stated* range — has no
/// range to read either. `bioprism-influence` measured this on the reference world and got zero
/// of six; the compiler reproduces it from the inside.
#[test]
fn a_withheld_fact_in_the_region_is_unknown_because_no_factor_carries_a_potential() {
    let out = deferred_compile();
    let analysis = out
        .trace
        .withheld_influence
        .attempted
        .iter()
        .find(|a| a.fact_id == "fact.future_marker")
        .expect("the withheld marker was analysed");

    assert_eq!(analysis.variable, "future_marker");
    assert_eq!(analysis.subject_factors, vec!["factor.claim_support"]);
    let result = analysis.outcome.as_ref().expect("the question was posable");
    assert!(matches!(
        &result.estimate,
        InfluenceEstimate::Unknown(UnknownReason::NoFactorTable { .. })
    ));
    assert_eq!(result.estimate.bound(), None);
}

/// A withheld fact the region does not mention is unknown, never zero.
///
/// `fact.aside` enters through the protected closure and no factor consumes the variable it
/// provides, so the region has no image of it to perturb. `AGENTS.md`: "provably cannot matter"
/// and "nobody checked" must never share a representation. Classifying this as
/// `InfluenceClass::Zero` — which an empty perturbation group would have done, since perturbing
/// nothing moves nothing — would assert the first while having established only the second.
#[test]
fn a_withheld_fact_outside_the_compiled_region_is_not_analysable_rather_than_zero_influence() {
    let out = deferred_compile();
    let analysis = out
        .trace
        .withheld_influence
        .attempted
        .iter()
        .find(|a| a.fact_id == "fact.aside")
        .expect("the withheld aside was analysed");

    assert!(analysis.subject_factors.is_empty());
    match analysis.outcome.as_ref().expect_err("no question was posable") {
        NotPosable::OutsideCompiledRegion { variable } => assert_eq!(variable, "aside_marker"),
        other => panic!("expected an outside-region refusal, got {other:?}"),
    }
    assert!(out.trace.withheld_influence.bounded.is_empty());
    assert_eq!(out.certificate.manifest.count_in(InfluenceClass::Zero), 0);
}

/// The correspondence gate is checked, and it does not hold here.
///
/// Removing `factor.claim_support` would also remove the rule that produces the delivered target,
/// so a bound on that removal would bound a different event than the withholding. The gate is a
/// type rather than a comment so that a world which one day carries potentials cannot open a
/// `Bounded` group on an unproven correspondence.
#[test]
fn the_correspondence_gate_refuses_a_subject_factor_that_also_produces_delivered_evidence() {
    let out = deferred_compile();
    let analysis = out
        .trace
        .withheld_influence
        .attempted
        .iter()
        .find(|a| a.fact_id == "fact.future_marker")
        .expect("the withheld marker was analysed");

    match &analysis.correspondence {
        CorrespondenceCheck::TouchesDeliveredEvidence { factor, variable } => {
            assert_eq!(factor, "factor.claim_support");
            assert_eq!(variable, "split_integrity_status");
        }
        other => panic!("expected the gate to refuse, got {other:?}"),
    }
    assert!(!analysis.correspondence.holds());
}

/// Nothing is promoted, so the deferred group survives whole and keeps its frontier entry.
///
/// `INTEGRATION_NOTE` item 5: a withheld-and-bounded fact must not lose its refinement-frontier
/// entry, so the deferred group is split rather than promoted. Here nothing moves, and the test
/// records both halves of the invariant — the manifest still classes both facts as deferred, and
/// the section still offers the move that would discharge them.
#[test]
fn a_withheld_fact_stays_deferred_and_keeps_its_refinement_frontier_entry() {
    let out = deferred_compile();

    assert_eq!(out.trace.withheld_influence.promoted(), 0);
    assert_eq!(out.trace.withheld_influence.deferred.len(), 2);
    assert!(out.trace.withheld_influence.joint.is_none());
    assert!(out.trace.withheld_influence.bounded_group().is_none());

    assert_eq!(
        out.certificate
            .manifest
            .count_in(InfluenceClass::DeferredAcquisition),
        2
    );
    assert_eq!(out.certificate.manifest.count_in(InfluenceClass::Bounded), 0);
    assert!(!out.certificate.manifest.supports_sufficiency_claim());

    let frontier = &out.section.refinement_frontier;
    assert_eq!(frontier.len(), 1);
    assert_eq!(frontier[0].action, "advance_time_cut_or_use_retrospective_mode");
    assert_eq!(frontier[0].facts, vec!["fact.aside", "fact.future_marker"]);
}

/// The receipt says what was attempted and what came of it.
#[test]
fn the_influence_receipt_reports_zero_bounded_of_the_withheld_population() {
    let out = deferred_compile();
    let receipt = out
        .trace
        .passes
        .iter()
        .find(|pass| pass.name == "influence_bounds")
        .expect("the pass emits a receipt");

    assert_eq!(receipt.retained, 0);
    assert_eq!(
        receipt.note,
        "0 of 2 withheld fact(s) bounded, 0 group(s) informative, worst informative bound none"
    );
}

/// The limitation string does not shrink, because nothing it claims became false.
///
/// `INTEGRATION_NOTE`: "a limitation string that shrinks while the certificate it appears on gains
/// nothing would be worse than one that stays". The clause `formal influence bounds` is still an
/// accurate description of what a reader of *this* certificate got.
#[test]
fn the_certificate_still_declares_that_it_carries_no_formal_influence_bound() {
    let out = deferred_compile();
    assert_eq!(out.certificate.limitations.len(), 1);
    assert!(out.certificate.limitations[0].contains("formal influence bounds"));
}
