//! The physical backend portfolio, and the reason its answer stays off the certificate.
//!
//! Blueprint 43.36 and 43.37. `bioprism-examples` recorded the blocked claim
//! `backend_portfolio_selection` as *"bioprism-fiber hard-codes
//! Backend::BackwardFactorSliceReference and leaves PlanDescriptor::fallback None on every
//! compile; the other six backends have no implementation to select"*, and `bioprism-cookbook`
//! sharpened it to a missing call rather than a missing capability. The call is now made. These
//! tests pin what it found, including the part that comes out against the claim.

use bioprism_fiber::{compile, PlanEvaluation, PortfolioOutcome, Query, DELIVERING_BACKEND};
use bioprism_section::{Backend, CertificateProfile};
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

fn reference_compile() -> bioprism_fiber::CompileOutput {
    let world = World::from_json(fixture("radiogenomic_world.json")).expect("world loads");
    let query = Query::from_json(fixture("leakage_query.json")).expect("query loads");
    compile(&world, &query).expect("compiles")
}

fn selection(evaluation: &PlanEvaluation) -> &bioprism_backends::Selection {
    match &evaluation.outcome {
        PortfolioOutcome::Costed(selection) => selection,
        other => panic!("expected a costed portfolio outcome, got {other:?}"),
    }
}

/// The pass runs, and the argmin is a real one rather than the single member fiber used to name.
#[test]
fn the_portfolio_is_consulted_on_every_compile_and_returns_a_costed_argmin() {
    let out = reference_compile();
    let evaluation = &out.trace.plan;

    let selection = selection(evaluation);
    assert_eq!(selection.chosen, Backend::FaqInsideOut);
    assert_eq!(evaluation.argmin(), Some(Backend::FaqInsideOut));
    assert_eq!(selection.candidates.len(), 3);
    assert!(
        selection.candidates.iter().all(|c| c.outcome.is_ok()),
        "no member should decline to cost the reference region"
    );
    assert!(
        evaluation.predicted_speedup().expect("a costed plan") > 1000.0,
        "predicted speedup was {:?}",
        evaluation.predicted_speedup()
    );

    let region = evaluation.region.expect("a region was built");
    assert_eq!(region.factors, 6);
    assert_eq!(region.variables, 17);
}

/// The measurement that comes out against the claim: the argmin cannot be run.
///
/// `fiber-world/0.1` declares a factor's signature and never its potential, so every region this
/// compiler can build is structural, and every member of the portfolio is a sum-product evaluator.
/// This is the same schema gap `bioprism-backends` names when it declines to implement
/// worst-case-optimal joins and `bioprism-influence` names when it bounds nothing.
#[test]
fn no_portfolio_member_can_execute_a_region_whose_factors_carry_no_potential() {
    let out = reference_compile();
    let evaluation = &out.trace.plan;

    assert!(!evaluation.is_executable());
    let refusal = evaluation
        .execution_refusal
        .as_ref()
        .expect("the region carries no tables");
    assert!(
        matches!(refusal, bioprism_backends::Declined::MissingFactorTable { .. }),
        "expected a missing-table refusal, got {refusal:?}"
    );
    assert!(refusal.to_string().contains("carries no table"));
}

/// The certificate names the engine that ran, not the engine that would have been cheapest.
///
/// Writing `faq_inside_out` here would name a plan that provably never executed and provably could
/// not have, and a consumer holding only the certificate could not tell. `backend` and `fallback`
/// describe one plan between them: the backward factor slice, which ran and compressed.
#[test]
fn the_certificate_names_the_delivering_backend_rather_than_the_unrunnable_argmin() {
    let out = reference_compile();
    let plan = &out.certificate.plan;

    assert_eq!(plan.backend, DELIVERING_BACKEND);
    assert_eq!(plan.backend, Backend::BackwardFactorSliceReference);
    assert_ne!(
        Some(plan.backend),
        out.trace.plan.argmin(),
        "the wire backend must not be the portfolio's argmin while that argmin cannot run"
    );
    assert!(plan.fallback.is_none());

    let json = out
        .certificate
        .to_json(CertificateProfile::Reference)
        .expect("certificate serialises");
    assert_eq!(json["plan"]["backend"], "backward_factor_slice_reference");
    assert_eq!(json["plan"]["fallback"], Value::Null);
}

/// The arity the wire reports is the slice's definition, not the region's.
///
/// Both are correct measurements of different things — declared input count against deduplicated
/// input-plus-output scope — and swapping one for the other would have moved the parity digest for
/// a change of definition nobody asked for.
#[test]
fn the_plan_counts_come_from_the_slice_because_the_two_slicers_measure_arity_differently() {
    let out = reference_compile();
    assert_eq!(out.certificate.plan.max_selected_factor_arity, 5);
    assert_eq!(out.certificate.plan.compiled_factor_count, 6);
    assert_eq!(
        out.certificate.plan.compiled_factor_count,
        out.trace.plan.region.expect("a region was built").factors,
        "the two slicers must agree about which factors the query reaches"
    );
}

/// A region nothing can cost is reported, not raised.
///
/// 43.48 wants an impossible contract to return infeasible rather than fabricated output, and
/// 43.37 wants the condition to be a first-class output. Both are satisfied on the trace. What
/// must not happen is a compile failing because a cost model declined: the closure, the slice, the
/// policy screen, the cut and the oracle all completed, and the section they produced is valid.
#[test]
fn a_region_no_member_can_cost_is_reported_as_infeasible_without_failing_the_compile() {
    let world = World::from_json(reference_example("wide_region_world.json")).expect("world loads");
    let query = Query::from_json(reference_example("wide_region_query.json")).expect("query loads");
    let out = compile(&world, &query).expect("an uncostable region still compiles");

    let declined = match &out.trace.plan.outcome {
        PortfolioOutcome::NoAdmissiblePlan(declined) => declined,
        other => panic!("expected the portfolio to admit no plan, got {other:?}"),
    };
    assert!(declined.to_string().contains("declined"));
    assert_eq!(out.trace.plan.argmin(), None);
    assert_eq!(out.trace.plan.predicted_speedup(), None);

    assert_eq!(out.certificate.plan.backend, DELIVERING_BACKEND);
    assert!(
        out.certificate.plan.fallback.is_none(),
        "the delivering backend ran, so the plan block is not a fallback"
    );

    let json = out
        .certificate
        .to_json(CertificateProfile::Reference)
        .expect("certificate serialises");
    assert_eq!(
        json["plan"],
        serde_json::json!({
            "backend": "backward_factor_slice_reference",
            "compiled_factor_count": 1,
            "compiled_fact_count": 12,
            "total_factor_count": 1,
            "total_fact_count": 12,
            "max_selected_factor_arity": 12,
            "fallback": Value::Null,
        }),
        "the CPython reference has no portfolio and must still emit these bytes; \
         reference/fiber_runtime/tests/test_fiber_runtime.py pins the same block"
    );
}

/// The receipt carries what the wire cannot.
#[test]
fn the_plan_receipt_names_the_argmin_and_the_reason_it_is_not_runnable() {
    let out = reference_compile();
    let receipt = out
        .trace
        .passes
        .iter()
        .find(|pass| pass.name == "plan_selection")
        .expect("the pass emits a receipt");

    assert!(receipt.note.contains("backend backward_factor_slice_reference"));
    assert!(receipt.note.contains("argmin faq_inside_out"));
    assert!(receipt.note.contains("no costed plan is runnable"));
}

/// The sibling output of a multi-output factor is produced, not consumed, so it is not needed.
///
/// Every fixture that existed when the two slicers were first claimed to "agree by construction"
/// gave each factor exactly one output, and on such a world the claim is trivially true: a
/// factor's outputs are already needed by the time it is selected. `factor.joint_readout` emits
/// `risk_score` and `calibration_drift` together and only `risk_score` is reached backwards from
/// the target, so `calibration_drift` has no consumer in the slice and `factor.drift_model`, which
/// produces it alone, is not part of the compiled program. A region that walked backwards from
/// `calibration_drift` would select three factors while the certificate reported two, and
/// `compiled_factor_count` would stop counting the factor documents the section actually carries.
///
/// The count is the CPython reference's: `reference/fiber_runtime/fiber_compile.py` writes
/// `len(selected_factor_ids)` from a slice that pushes a factor's inputs and never its outputs.
#[test]
fn a_multi_output_factor_does_not_drag_its_siblings_producers_into_the_compiled_region() {
    let world = World::from_json(reference_example("multi_output_world.json")).expect("world loads");
    let query = Query::from_json(reference_example("multi_output_query.json")).expect("query loads");
    let out = compile(&world, &query).expect("compiles");

    assert_eq!(
        out.certificate.selected_factors,
        vec!["factor.claim_support", "factor.joint_readout"],
        "factor.drift_model produces only the sibling output and no needed variable"
    );
    assert_eq!(out.certificate.plan.compiled_factor_count, 2);
    assert_eq!(
        out.certificate.plan.compiled_factor_count,
        out.certificate.selected_factors.len(),
        "compiled_factor_count counts the factor documents the section delivers"
    );
    assert_eq!(
        out.certificate.plan.compiled_factor_count,
        out.trace.plan.region.expect("a region was built").factors,
        "the two slicers must agree about which factors the query reaches"
    );
}

/// A variable a selected factor emits still needs a domain size.
///
/// Bringing the region's traversal into line with the slice must not strip `calibration_drift`
/// from the region: it sits in `factor.joint_readout`'s scope, elimination has to size it, and a
/// region whose factor mentions a variable it never declared is rejected by its own builder. The
/// variable is in the region and is *not* an entry point for further backward walking, and those
/// are separate facts about it.
#[test]
fn a_sibling_output_stays_a_region_variable_even_though_it_is_not_a_slicing_frontier() {
    let world = World::from_json(reference_example("multi_output_world.json")).expect("world loads");
    let region = bioprism_backends::QueryRegion::from_world_slice(
        &world,
        "multi-output",
        ["split_integrity_status"],
        &bioprism_backends::CardinalityPolicy::default(),
    )
    .expect("region builds");

    let variables: Vec<&str> = region.variables().collect();
    assert_eq!(
        variables,
        vec![
            "assay_batch",
            "calibration_drift",
            "cohort_id",
            "risk_score",
            "split_integrity_status"
        ]
    );
    assert_eq!(
        region.cardinality_source("calibration_drift"),
        Some(bioprism_backends::CardinalitySource::Assumed),
        "no fact provides the sibling output, so its domain size is the policy default"
    );
    assert!(
        !variables.contains(&"scanner_id"),
        "scanner_id is reachable only through factor.drift_model, which the slice does not select"
    );
}
