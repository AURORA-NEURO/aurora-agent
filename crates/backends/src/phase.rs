//! Where elimination stops paying: a measured phase boundary.
//!
//! Blueprint 43.48: "make structural hardness visible and empirically map backend crossovers rather
//! than claiming that a richer representation removes combinatorial complexity", and its first
//! invariant, "heuristic success is not presented as worst-case tractability". This module runs the
//! experiment rather than asserting the conclusion.
//!
//! ## The family
//!
//! [`band_region`] builds a *band graph*: `n` variables, and one factor over each window of `k+1`
//! consecutive variables. The window width is the knob. Eliminating from the far end gives induced
//! width exactly `k`, so `k = 1` is a chain, `k = n-1` is a single clique factor over everything,
//! and every value in between is reachable — with the number of variables, the domain size and the
//! potentials held fixed. 43.48 asks for "parameter-controlled generators" and this is the smallest
//! one that isolates width from size.
//!
//! ## What is measured
//!
//! Semiring operations executed, by both backends, on the same tables through the same kernel. Not
//! seconds: 43.48 requires a phase diagram to state its hardware context, and an operation count
//! has none to state. The consequence is that this diagram bounds *arithmetic*, not latency —
//! cache behaviour and allocation are invisible to it, and both favour the schedule with smaller
//! intermediates, which is elimination. The measurement is therefore conservative in elimination's
//! disfavour at low width and says nothing about constant factors.
//!
//! Every point also checks that the two backends returned the same answer. A speedup measured
//! against a wrong result is worthless, and the entries are integers below `2^53` so the comparison
//! is exact rather than tolerant.
//!
//! ## The boundary this crate actually measures
//!
//! Ten variables, domain three, sum-product, seed `0x5EED`, pinned by the tests:
//!
//! ```text
//!   w    VE ops   direct ops   speedup   VE peak
//!   1       237       590490   2491.52         3
//!   3      1695       472392    278.70        27
//!   5     10929       354294     32.42       243
//!   7     59043       236196      4.00      2187
//!   8    118092       177147      1.50      6561
//!   9    177141       118098      0.67     19683
//! ```
//!
//! Elimination stops executing fewer operations at induced width **9**, which for ten variables is
//! the maximum possible width: the region has become one clique factor over everything and the two
//! schedules are the same traversal, so elimination loses by its bookkeeping. The portfolio's cost
//! model leaves at width **8**, one step earlier, because it charges the peak intermediate that
//! elimination holds and streaming enumeration does not.
//!
//! The honest reading is that this is not a cliff and barely a crossover. Elimination's advantage
//! is `d^(n-w-1)` and decays smoothly from three orders of magnitude to nothing; it never becomes a
//! liability worth more than a small constant, because at maximum width the two plans degenerate to
//! the same computation. Anyone hoping for a dramatic phase boundary between these two backends
//! will not find one, and the useful conclusion is the negative half of 43.18's claim: what buys
//! the three orders of magnitude at low width is width, and nothing about the region's size
//! predicts it.

use crate::backend::QueryBackend;
use crate::direct::DirectMaterialization;
use crate::elimination::VariableElimination;
use crate::error::Declined;
use crate::estimate::Budget;
use crate::order::OrderStrategy;
use crate::portfolio::Portfolio;
use crate::region::{QueryRegion, RegionFactor};
use crate::semiring::Semiring;
use bioprism_section::Backend;

/// Largest table entry generated. Keeps every intermediate an exact integer in `f64`.
const MAX_POTENTIAL: u64 = 4;

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// A width-controlled region: `variables` variables over a domain of `domain` values, with one
/// factor per window of `width + 1` consecutive variables.
///
/// The induced width of the resulting primal graph is exactly `width`. Variable `v0` is free, so
/// the answer is a vector rather than a scalar and the free scope does not itself dominate memory.
///
/// Panics if `width` is zero or not below `variables`, or if `domain` is below two: those are
/// programming errors in a benchmark generator, not runtime conditions.
pub fn band_region(variables: usize, width: usize, domain: usize, seed: u64) -> QueryRegion {
    assert!(variables >= 2, "a band needs at least two variables");
    assert!(
        width >= 1 && width < variables,
        "band width must lie in 1..variables"
    );
    assert!(domain >= 2, "a domain of one value makes every width free");

    let name = |index: usize| format!("v{index:02}");
    let mut builder = QueryRegion::builder(format!("band-n{variables}-w{width}-d{domain}"))
        .semiring(Semiring::SumProduct)
        .free(name(0));
    for index in 0..variables {
        builder = builder.variable(name(index), domain);
    }

    let mut state = seed;
    for last in width..variables {
        let scope: Vec<String> = (last - width..=last).map(name).collect();
        let entries = domain.pow(scope.len() as u32);
        let table: Vec<f64> = (0..entries)
            .map(|_| (splitmix64(&mut state) % MAX_POTENTIAL + 1) as f64)
            .collect();
        builder = builder.factor(RegionFactor::with_table(
            format!("band.{last:02}"),
            scope,
            table,
        ));
    }

    builder
        .assumption("synthetic width-controlled family; potentials are pseudo-random small integers and carry no evidential content".to_string())
        .build()
        .expect("the band generator produces a well-formed region")
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhasePoint {
    /// Window width requested from the generator.
    pub requested_width: usize,
    /// Induced width the order search actually achieved.
    pub induced_width: usize,
    pub elimination_ops: u64,
    pub direct_ops: u64,
    pub elimination_peak_entries: u64,
    pub direct_peak_entries: u64,
    /// `direct_ops / elimination_ops`. Below `1.0` means enumeration was cheaper.
    pub measured_speedup: f64,
    /// The same ratio taken from the estimates, before either backend ran.
    pub predicted_speedup: f64,
    /// Which backend the portfolio picked from the estimate alone.
    pub portfolio_choice: Backend,
    /// Whether the two backends returned identical answers.
    pub answers_agree: bool,
}

impl PhasePoint {
    pub fn elimination_wins(&self) -> bool {
        self.measured_speedup > 1.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseDiagram {
    pub variables: usize,
    pub domain: usize,
    pub points: Vec<PhasePoint>,
    /// Smallest induced width at which elimination stopped executing fewer operations than
    /// enumeration. `None` means it never stopped over the range swept.
    pub crossover_width: Option<usize>,
    /// Smallest induced width at which the portfolio's cost model abandoned elimination. This is
    /// generally below `crossover_width` because the cost model charges peak memory, which
    /// elimination pays and streaming enumeration does not.
    pub portfolio_crossover_width: Option<usize>,
    /// Scope and caveats. 43.37 requires a performance claim to carry the workload family it
    /// applies to.
    pub notes: Vec<String>,
}

impl PhaseDiagram {
    pub fn point(&self, width: usize) -> Option<&PhasePoint> {
        self.points
            .iter()
            .find(|point| point.requested_width == width)
    }

    pub fn all_answers_agree(&self) -> bool {
        self.points.iter().all(|point| point.answers_agree)
    }

    /// A plain-text rendering, for the structural diagnostics CLI 43.37 asks for.
    pub fn report(&self) -> String {
        let mut out = format!(
            "phase diagram: band family, {} variables, domain {}, sum-product\n",
            self.variables, self.domain
        );
        out.push_str("  w  induced   VE ops    direct ops   speedup  VE peak   portfolio\n");
        for point in &self.points {
            out.push_str(&format!(
                "{:3}  {:7}  {:9}  {:11}  {:7.2}  {:8}  {}\n",
                point.requested_width,
                point.induced_width,
                point.elimination_ops,
                point.direct_ops,
                point.measured_speedup,
                point.elimination_peak_entries,
                point.portfolio_choice.as_str()
            ));
        }
        match self.crossover_width {
            Some(width) => out.push_str(&format!(
                "measured crossover: elimination stops winning at induced width {width}\n"
            )),
            None => out.push_str(
                "measured crossover: none; elimination executed fewer operations at every width swept\n",
            ),
        }
        match self.portfolio_crossover_width {
            Some(width) => out.push_str(&format!(
                "portfolio crossover: the cost model falls back to direct materialisation at induced width {width}\n"
            )),
            None => out.push_str("portfolio crossover: none over the range swept\n"),
        }
        for note in &self.notes {
            out.push_str("note: ");
            out.push_str(note);
            out.push('\n');
        }
        out
    }
}

/// Sweeps induced width from 1 to `variables - 1` and measures both backends at each point.
///
/// Returns the first refusal if either backend declines: a diagram with a hole in it would
/// misreport the boundary.
pub fn sweep_induced_width(
    variables: usize,
    domain: usize,
    seed: u64,
) -> Result<PhaseDiagram, Declined> {
    let budget = Budget::default()
        .with_max_induced_width(variables)
        .with_max_peak_entries(f64::INFINITY)
        .with_max_ops(f64::INFINITY);
    let elimination = VariableElimination::new(OrderStrategy::MinFill).with_budget(budget);
    let direct = DirectMaterialization::new().with_budget(budget);
    let portfolio = Portfolio::reference_with(budget);

    let mut points = Vec::with_capacity(variables.saturating_sub(1));
    for width in 1..variables {
        let region = band_region(variables, width, domain, seed);

        let eliminated = elimination.execute(&region)?;
        let enumerated = direct.execute(&region)?;
        let elimination_estimate = elimination.estimate(&region)?;
        let direct_estimate = direct.estimate(&region)?;
        let selection = portfolio.select(&region)?;

        let elimination_ops = eliminated.receipt().observed_ops();
        let direct_ops = enumerated.receipt().observed_ops();

        points.push(PhasePoint {
            requested_width: width,
            induced_width: eliminated.receipt().induced_width.unwrap_or(width),
            elimination_ops,
            direct_ops,
            elimination_peak_entries: eliminated.receipt().observed_peak_entries,
            direct_peak_entries: enumerated.receipt().observed_peak_entries,
            measured_speedup: ratio(direct_ops as f64, elimination_ops as f64),
            predicted_speedup: ratio(
                direct_estimate.predicted_ops(),
                elimination_estimate.predicted_ops(),
            ),
            portfolio_choice: selection.chosen,
            answers_agree: eliminated.agrees_exactly_with(&enumerated),
        });
    }

    let crossover_width = points
        .iter()
        .find(|point| !point.elimination_wins())
        .map(|point| point.induced_width);
    let portfolio_crossover_width = points
        .iter()
        .find(|point| point.portfolio_choice == Backend::DirectMaterialization)
        .map(|point| point.induced_width);

    Ok(PhaseDiagram {
        variables,
        domain,
        points,
        crossover_width,
        portfolio_crossover_width,
        notes: vec![
            "operations executed, not wall clock; no hardware context is claimed".to_string(),
            "one synthetic family (uniform domain, uniform band structure, dense potentials); nothing here transfers to skewed or sparse factors".to_string(),
            "direct materialisation streams the joint space and allocates only the answer, so elimination is compared against a baseline that already has the memory advantage".to_string(),
        ],
    })
}

fn ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator <= 0.0 {
        1.0
    } else {
        numerator / denominator
    }
}
