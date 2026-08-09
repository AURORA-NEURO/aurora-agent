//! Blueprint 43.36, 43.37 and 43.48: what gets chosen, what gets refused, and where the advantage
//! actually ends.
//!
//! The tests that matter most here are the negative ones. 43.37's invariant is that "fallback is
//! not counted as optimized success", which is only enforceable if falling back is observable — so
//! a [`Selection`] landing on direct materialisation is required to carry a reason, and that reason
//! is required to survive onto the [`bioprism_section::PlanDescriptor`] rather than being absorbed
//! into a cheerful summary.

use bioprism_backends::{
    band_region, sweep_induced_width, Budget, ComputedRegion, CostModel, Declined,
    DirectMaterialization, Estimate, OrderStrategy, Portfolio, QueryBackend, QueryRegion,
    RegionFactor, VariableElimination,
};
use bioprism_section::{Backend, FallbackReason};

/// A backend that costs well and then refuses to run, to exercise 43.36's fallback chain.
///
/// Nothing in the crate behaves this way; a real backend that could not run would decline at
/// `estimate`. The point is that the portfolio must survive one that does not, because a physical
/// engine can discover a resource limit only once it starts filling tables.
struct EstimatesWellRunsBadly;

impl QueryBackend for EstimatesWellRunsBadly {
    fn backend(&self) -> Backend {
        Backend::TensorNetwork
    }

    fn method(&self) -> String {
        "test double: reports an implausibly cheap plan, then declines at execution".to_string()
    }

    fn estimate(&self, region: &QueryRegion) -> Result<Estimate, Declined> {
        let mut estimate = DirectMaterialization::new().estimate(region)?;
        estimate.backend = self.backend();
        estimate.method = self.method();
        estimate.predicted_multiply_ops = 1.0;
        estimate.predicted_aggregate_ops = 0.0;
        estimate.predicted_peak_entries = 1.0;
        Ok(estimate)
    }

    fn execute(&self, _region: &QueryRegion) -> Result<ComputedRegion, Declined> {
        Err(Declined::WorkAboveBudget {
            backend: "tensor_network",
            predicted_ops: 1.0,
            budget_ops: 0.0,
        })
    }
}

/// A region with no exploitable structure: one factor over everything.
fn dense_region(variables: usize, domain: usize) -> QueryRegion {
    band_region(variables, variables - 1, domain, 0xD1)
}

#[test]
fn the_portfolio_chooses_elimination_on_a_narrow_region() {
    let region = band_region(12, 2, 3, 0x1A);
    let selection = Portfolio::reference().select(&region).unwrap();

    assert_eq!(selection.chosen, Backend::FaqInsideOut);
    assert!(selection.fallback.is_none());
    assert!(!selection.is_fallback());
    assert!(
        selection.predicted_speedup > 100.0,
        "speedup was {}",
        selection.predicted_speedup
    );
    assert_eq!(selection.estimate.induced_width, Some(2));
}

#[test]
fn the_portfolio_falls_back_to_direct_materialisation_and_records_why() {
    let region = dense_region(8, 2);
    let selection = Portfolio::reference().select(&region).unwrap();

    assert_eq!(selection.chosen, Backend::DirectMaterialization);
    let fallback = selection.fallback.as_ref().expect("a fallback is recorded");
    assert_eq!(fallback.reason, FallbackReason::NoPredictedAdvantage);
    assert!(
        fallback.detail.contains("direct-materialisation baseline"),
        "detail was {:?}",
        fallback.detail
    );
    assert!(selection
        .candidates
        .iter()
        .any(|candidate| candidate.backend == Backend::FaqInsideOut && candidate.cost.is_some()));
}

#[test]
fn the_fallback_reaches_the_plan_descriptor_rather_than_being_swallowed() {
    let region = dense_region(8, 2);
    let selection = Portfolio::reference().select(&region).unwrap();
    let plan = selection.plan_descriptor(&region);

    assert_eq!(plan.backend, Backend::DirectMaterialization);
    let fallback = plan.fallback.expect("the plan carries the fallback");
    assert_eq!(fallback.reason, FallbackReason::NoPredictedAdvantage);

    let narrow = band_region(12, 2, 3, 0x1A);
    let optimized = Portfolio::reference()
        .select(&narrow)
        .unwrap()
        .plan_descriptor(&narrow);
    assert_eq!(optimized.backend, Backend::FaqInsideOut);
    assert!(optimized.fallback.is_none());
}

#[test]
fn a_high_width_refusal_reaches_the_plan_descriptor_as_high_compiled_width() {
    let region = dense_region(9, 2);
    let budget = Budget::default().with_max_induced_width(3);
    let selection = Portfolio::reference_with(budget).select(&region).unwrap();

    assert_eq!(selection.chosen, Backend::DirectMaterialization);
    let fallback = selection.fallback.as_ref().unwrap();
    assert_eq!(fallback.reason, FallbackReason::HighCompiledWidth);
    assert!(fallback.detail.contains("induced width 8"));
    assert!(selection
        .candidates
        .iter()
        .filter(|candidate| candidate.backend == Backend::FaqInsideOut)
        .all(|candidate| candidate.outcome.is_err()));
}

#[test]
fn elimination_declines_rather_than_exceeding_its_memory_or_width_budget() {
    let region = band_region(10, 6, 4, 0xC0);

    let memory = VariableElimination::default()
        .with_budget(Budget::default().with_max_peak_entries(64.0))
        .estimate(&region)
        .unwrap_err();
    assert!(matches!(memory, Declined::PeakMemoryAboveBudget { .. }));
    assert_eq!(memory.fallback_reason(), FallbackReason::HighCompiledWidth);

    let width = VariableElimination::default()
        .with_budget(Budget::default().with_max_induced_width(2))
        .estimate(&region)
        .unwrap_err();
    assert!(matches!(width, Declined::WidthAboveBudget { .. }));
    assert_eq!(width.fallback_reason(), FallbackReason::HighCompiledWidth);

    let work = VariableElimination::default()
        .with_budget(Budget::default().with_max_ops(4.0))
        .estimate(&region)
        .unwrap_err();
    assert!(matches!(work, Declined::WorkAboveBudget { .. }));
    assert_eq!(
        work.fallback_reason(),
        FallbackReason::BackendExecutionFailure
    );
}

#[test]
fn every_refusal_carries_the_backend_that_made_it_and_a_stable_fallback_code() {
    let refusals = [
        Declined::WidthAboveBudget {
            backend: "faq_inside_out",
            induced_width: 30,
            budget: 24,
        },
        Declined::PeakMemoryAboveBudget {
            backend: "faq_inside_out",
            predicted_entries: 1e9,
            budget_entries: 1e6,
        },
        Declined::WorkAboveBudget {
            backend: "direct_materialization",
            predicted_ops: 1e12,
            budget_ops: 1e10,
        },
        Declined::MissingFactorTable {
            backend: "faq_inside_out",
            factor: "factor.identity_check".into(),
        },
        Declined::NoPredictedAdvantage {
            backend: "faq_inside_out",
            predicted_cost: 10.0,
            baseline_cost: 11.0,
            required_speedup: 1.25,
        },
    ];

    for refusal in &refusals {
        assert!(!refusal.backend_name().is_empty());
        assert!(refusal.to_string().contains("declined"));
    }

    assert_eq!(
        refusals[0].fallback_reason(),
        FallbackReason::HighCompiledWidth
    );
    assert_eq!(
        refusals[2].fallback_reason(),
        FallbackReason::BackendExecutionFailure
    );
    assert_eq!(
        refusals[4].fallback_reason(),
        FallbackReason::NoPredictedAdvantage
    );
}

#[test]
fn the_portfolio_reports_infeasible_rather_than_fabricating_a_plan() {
    let region = band_region(10, 5, 4, 0x99);
    let starved = Budget::default().with_max_ops(4.0);

    let declined = Portfolio::reference_with(starved)
        .select(&region)
        .unwrap_err();
    assert!(matches!(declined, Declined::WorkAboveBudget { .. }));
}

#[test]
fn an_optimizing_backend_is_chosen_even_when_enumeration_is_itself_infeasible() {
    let region = band_region(60, 1, 2, 0x2C);
    let selection = Portfolio::reference().select(&region).unwrap();

    assert!(DirectMaterialization::new().estimate(&region).is_err());
    assert_eq!(selection.chosen, Backend::FaqInsideOut);
    assert!(selection.fallback.is_none());
    assert_eq!(selection.baseline_cost, f64::INFINITY);
}

#[test]
fn a_refusal_at_execution_time_is_replaced_by_the_conservative_plan_and_recorded() {
    let region = band_region(8, 3, 2, 0x4D);
    let mut portfolio = Portfolio::conservative();
    portfolio.push(Box::new(EstimatesWellRunsBadly));

    let selected = portfolio.select(&region).unwrap();
    assert_eq!(selected.chosen, Backend::TensorNetwork);
    assert!(selected.fallback.is_none());

    let (computed, executed) = portfolio.execute(&region).unwrap();
    assert_eq!(executed.chosen, Backend::DirectMaterialization);
    let fallback = executed.fallback.as_ref().expect("the substitution is recorded");
    assert_eq!(fallback.reason, FallbackReason::BackendExecutionFailure);
    assert!(fallback.detail.contains("re-ran the same logical query"));

    let reference = DirectMaterialization::new().execute(&region).unwrap();
    assert!(computed.agrees_exactly_with(&reference));
}

#[test]
fn a_portfolio_with_no_optimizing_backend_still_answers_and_still_says_it_fell_back() {
    let region = band_region(8, 3, 2, 0x4D);
    let (computed, selection) = Portfolio::conservative().execute(&region).unwrap();

    assert_eq!(selection.chosen, Backend::DirectMaterialization);
    let fallback = selection.fallback.as_ref().unwrap();
    assert_eq!(fallback.reason, FallbackReason::NoPredictedAdvantage);
    assert!(fallback.detail.contains("registers no optimizing backend"));

    let reference = VariableElimination::default().execute(&region).unwrap();
    assert!(computed.agrees_exactly_with(&reference));
}

#[test]
fn the_hysteresis_margin_decides_marginal_cases_in_favour_of_the_incumbent_plan() {
    let region = band_region(9, 6, 2, 0x71);
    let elimination = VariableElimination::new(OrderStrategy::MinFill);
    let ratio = DirectMaterialization::new()
        .estimate(&region)
        .map(|estimate| CostModel::default().cost(&estimate))
        .unwrap()
        / elimination
            .estimate(&region)
            .map(|estimate| CostModel::default().cost(&estimate))
            .unwrap();

    let permissive = Portfolio::reference()
        .with_cost_model(CostModel::default().with_minimum_speedup(1.0))
        .select(&region)
        .unwrap();
    let demanding = Portfolio::reference()
        .with_cost_model(CostModel::default().with_minimum_speedup(ratio * 1.5))
        .select(&region)
        .unwrap();

    assert!(ratio > 1.0, "cost ratio was {ratio}");
    assert_eq!(permissive.chosen, Backend::FaqInsideOut);
    assert_eq!(demanding.chosen, Backend::DirectMaterialization);
    assert_eq!(
        demanding.fallback.as_ref().unwrap().reason,
        FallbackReason::NoPredictedAdvantage
    );
}

#[test]
fn charging_memory_more_heavily_abandons_elimination_at_a_lower_width() {
    let region = band_region(10, 7, 3, 0x5EED);

    let cheap_memory = Portfolio::reference().with_cost_model(CostModel::default());
    let dear_memory = Portfolio::reference()
        .with_cost_model(CostModel::default().with_memory_weight(200.0));

    assert_eq!(cheap_memory.cost_model().memory_weight, 4.0);
    assert_eq!(dear_memory.cost_model().memory_weight, 200.0);

    assert_eq!(
        cheap_memory.select(&region).unwrap().chosen,
        Backend::FaqInsideOut
    );
    let switched = dear_memory.select(&region).unwrap();
    assert_eq!(switched.chosen, Backend::DirectMaterialization);
    assert_eq!(
        switched.fallback.as_ref().unwrap().reason,
        FallbackReason::NoPredictedAdvantage
    );
}

#[test]
fn the_plan_descriptor_reports_the_region_and_not_the_world() {
    let region = QueryRegion::builder("tiny")
        .variable("a", 2)
        .variable("b", 2)
        .free("a")
        .factor(RegionFactor::with_table(
            "f",
            vec!["a", "b"],
            vec![1.0, 2.0, 3.0, 4.0],
        ))
        .build()
        .unwrap();

    let plan = Portfolio::reference()
        .select(&region)
        .unwrap()
        .plan_descriptor(&region);

    assert_eq!(plan.compiled_factor_count, 1);
    assert_eq!(plan.total_factor_count, 1);
    assert_eq!(plan.max_selected_factor_arity, 2);
    assert_eq!(plan.factor_selection_ratio(), 1.0);
}

#[test]
fn both_backends_agree_at_every_point_of_the_phase_sweep() {
    let diagram = sweep_induced_width(10, 3, 0x5EED).unwrap();

    assert_eq!(diagram.points.len(), 9);
    assert!(diagram.all_answers_agree());
    for point in &diagram.points {
        assert_eq!(point.induced_width, point.requested_width);
    }
}

#[test]
fn the_measured_phase_boundary_is_reported_as_measured() {
    let diagram = sweep_induced_width(10, 3, 0x5EED).unwrap();
    println!("{}", diagram.report());

    let narrow = diagram.point(1).unwrap();
    let widest = diagram.point(9).unwrap();

    assert!(
        narrow.measured_speedup > 100.0,
        "at width 1 elimination should be orders of magnitude cheaper, measured {}",
        narrow.measured_speedup
    );
    assert!(
        widest.measured_speedup < 1.0,
        "at width n-1 the region is a single clique factor and elimination cannot win, measured {}",
        widest.measured_speedup
    );

    assert_eq!(diagram.crossover_width, Some(9));
    assert_eq!(diagram.portfolio_crossover_width, Some(8));

    assert!(diagram
        .notes
        .iter()
        .any(|note| note.contains("not wall clock")));
    assert!(diagram
        .notes
        .iter()
        .any(|note| note.contains("one synthetic family")));
}

#[test]
fn the_advantage_decays_smoothly_with_width_rather_than_cliffing() {
    let diagram = sweep_induced_width(10, 3, 0x5EED).unwrap();
    let speedups: Vec<f64> = diagram
        .points
        .iter()
        .map(|point| point.measured_speedup)
        .collect();

    for pair in speedups.windows(2) {
        assert!(
            pair[1] < pair[0],
            "speedup should fall monotonically with width, saw {pair:?}"
        );
    }
    assert!(
        speedups[0] / speedups[speedups.len() - 1] > 1.0e3,
        "the sweep should span orders of magnitude, spanned {:?}",
        speedups[0] / speedups[speedups.len() - 1]
    );
}

#[test]
fn the_predicted_speedup_matches_the_measured_one_at_every_swept_point() {
    let diagram = sweep_induced_width(9, 3, 0xBEE).unwrap();
    for point in &diagram.points {
        assert_eq!(
            point.predicted_speedup, point.measured_speedup,
            "estimate and execution disagreed at width {}",
            point.requested_width
        );
    }
}
