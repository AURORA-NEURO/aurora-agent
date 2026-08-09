//! The 43.11 pass: abstract the region, solve, and read a bound off the post-fixpoint.
//!
//! This is where [`crate::registry`], [`crate::domains`], [`crate::solver`] and [`crate::gibbs`]
//! become one thing a compiler could call. The shape is the standard one for an abstract
//! interpretation and each step is somebody else's module:
//!
//! 1. **Abstract.** [`crate::gibbs::comparison_system`] turns the region and the perturbation into
//!    `C` and `b`, refusing with a named clause when a hypothesis fails. The positivity clause is
//!    discharged through [`crate::domains::SupportDomain`], the finite domain whose only job it is.
//! 2. **Select a domain.** The registry is asked for the domain the analysis will run in. An
//!    unregistered name is [`crate::DomainError::UnregisteredDomain`]; a registered name this
//!    module has no transformer for is [`crate::DomainError::NoTransformerForDomain`]. Neither is a
//!    bound and neither is silent.
//! 3. **Solve.** [`crate::solver::solve_from`] runs the scheduled join, widening and narrowing over
//!    [`crate::domains::ProductDomain`] of the selected domain, one coordinate per region variable.
//! 4. **Read out.** The upper endpoints at the free variables, summed and capped at one.
//!
//! ## Why the read-out is sound whatever phase produced it
//!
//! The solver returns a post-fixpoint: `F(v) ⊑ v`, that is `h ≥ b + C·h` on upper endpoints.
//! Iterating that inequality gives `h ≥ Σ_{k<n} Cᵏ b + Cⁿ h ≥ Σ_{k<n} Cᵏ b` for every `n`, and
//! since every entry is non-negative the limit gives `h ≥ D b`. So any post-fixpoint dominates the
//! quantity the comparison theorem bounds the influence by — which is what makes widening's answer
//! usable rather than merely terminating, and what makes narrowing an optimisation rather than a
//! correction.
//!
//! ## The one thing this must never do
//!
//! Report a vacuous bound where the crate would otherwise have reported `Unknown`. A factor with no
//! declared potential abstracts to `⊤` in every domain here, `⊤` reads out as one, and a bound of
//! one is `Bounded` — so an interpreter that ran anyway would convert six honestly unknown groups
//! into six groups that formally support a sufficiency claim and promise nothing. The hypothesis
//! check in [`crate::gibbs`] is what prevents it: a region without potentials fails clause two and
//! comes back [`crate::UnknownReason::NoFactorTable`], never `Bounded(1.0)`. The suite asserts it,
//! because it is the difference between this crate closing a gap and this crate widening one.

use crate::bound::{Approximation, BoundMethod, InfluenceBound, InfluenceMetric};
use crate::domain::{AbstractDomain, DomainError, DomainId, FactClass};
use crate::domains::displacement::{self, Displacement, DisplacementDomain, DISPLACEMENT_DOMAIN};
use crate::domains::product::ProductDomain;
use crate::domains::support::Support;
use crate::error::{InfluenceError, UnknownReason};
use crate::gibbs::{self, ComparisonSystem};
use crate::perturbation::Perturbation;
use crate::registry::DomainRegistry;
use crate::solver::{self, Convergence, RefinementSchedule};
use bioprism_backends::QueryRegion;

/// Everything the pass computed, not only the number it reports.
#[derive(Debug, Clone, PartialEq)]
pub struct AbstractInterpretation {
    /// The domain the fixed point was computed in, as it was looked up in the registry.
    pub domain: DomainId,
    /// Which phase produced the post-fixpoint, and therefore how much was traded away.
    pub convergence: Convergence,
    pub joins: usize,
    pub widenings: usize,
    pub narrowings: usize,
    /// `‖C‖_∞`. The closer to one, the slower the descending phase closes the gap.
    pub contraction: f64,
    /// The post-fixpoint, one element per region variable, in site order.
    pub state: Vec<(String, Displacement)>,
    /// The bound the read-out produced.
    pub bound: InfluenceBound,
}

impl AbstractInterpretation {
    /// Whether precision was traded for termination on the way to this bound.
    ///
    /// A caller that cannot distinguish a bound reached by join from one reached by widening has
    /// lost the only thing widening costs. This is that distinction, and
    /// [`AbstractInterpretation::bound`] carries it onto a certificate as a distinct
    /// [`BoundMethod`] so it survives leaving this type.
    pub fn widened(&self) -> bool {
        self.convergence.traded_precision_for_termination()
    }
}

/// The method a bound gets, given how its fixed point was reached.
fn method_for(convergence: Convergence) -> BoundMethod {
    match convergence {
        Convergence::Join => BoundMethod::AbstractInterpretation,
        Convergence::Widening | Convergence::WideningThenNarrowing => {
            BoundMethod::WidenedAbstractInterpretation
        }
    }
}

/// Runs the 43.11 pass for one factor's perturbation.
///
/// The outer `Result` is a malformed request or a registry misuse; the inner one is a well-formed
/// question whose hypotheses this method does not have.
pub fn interpret(
    registry: &DomainRegistry,
    domain_id: &DomainId,
    region: &QueryRegion,
    factor_ids: &[String],
    perturbation: &Perturbation,
    schedule: RefinementSchedule,
) -> Result<Result<AbstractInterpretation, UnknownReason>, InfluenceError> {
    let selected = registry.get(domain_id).map_err(InfluenceError::Domain)?;
    if selected.abstracts() != FactClass::AnswerDisplacement || domain_id.as_str() != DISPLACEMENT_DOMAIN
    {
        return Err(InfluenceError::Domain(DomainError::NoTransformerForDomain {
            id: domain_id.clone(),
        }));
    }

    let system = match gibbs::comparison_system(region, factor_ids, perturbation)? {
        Ok(system) => system,
        Err(reason) => return Ok(Err(reason)),
    };

    let inner = DisplacementDomain;
    let product = ProductDomain::new(inner, system.sites().len());
    let seed: Vec<Displacement> = vec![
        Displacement::exactly(0.0).expect("zero is an admissible displacement");
        system.sites().len()
    ];

    let fixed_point = solver::solve_from(&product, seed, |state| transfer(&system, state), schedule)
        .map_err(InfluenceError::Domain)?;

    let value = read_out(&system, &fixed_point.value);
    let state: Vec<(String, Displacement)> = system
        .sites()
        .iter()
        .cloned()
        .zip(fixed_point.value.iter().copied())
        .collect();

    let bound = InfluenceBound::new(
        value,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        method_for(fixed_point.reached_by),
        Approximation::ConservativeUpperBound,
        format!(
            "{} perturbation of {} factor(s) {factor_ids:?} in region {:?}; Dobrushin comparison over {} site(s) with contraction {}, solved in domain {domain_id} by {} ({} join(s), {} widening(s), {} narrowing(s)), post-fixpoint {}",
            perturbation.class_name(),
            factor_ids.len(),
            region.label(),
            system.sites().len(),
            system.contraction(),
            fixed_point.reached_by.as_str(),
            fixed_point.joins,
            fixed_point.widenings,
            fixed_point.narrowings,
            product.render(&fixed_point.value),
        ),
    )?;

    Ok(Ok(AbstractInterpretation {
        domain: domain_id.clone(),
        convergence: fixed_point.reached_by,
        joins: fixed_point.joins,
        widenings: fixed_point.widenings,
        narrowings: fixed_point.narrowings,
        contraction: system.contraction(),
        state,
        bound,
    }))
}

/// The same pass against [`DomainRegistry::standard`], for a caller with no registry of its own.
pub fn interpret_with_standard_domains(
    region: &QueryRegion,
    factor_ids: &[String],
    perturbation: &Perturbation,
) -> Result<Result<AbstractInterpretation, UnknownReason>, InfluenceError> {
    let registry = DomainRegistry::standard().map_err(InfluenceError::Domain)?;
    interpret(
        &registry,
        &DomainId::new(DISPLACEMENT_DOMAIN),
        region,
        factor_ids,
        perturbation,
        RefinementSchedule::default(),
    )
}

/// The abstract counterpart of [`ComparisonSystem::apply`].
///
/// `u ↦ b ⊕ Σ_j C_ij ⊗ u_j`, with `⊕` and `⊗` the transformers of
/// [`crate::domains::displacement`]. Soundness is exactly `f(γ(a)) ⊆ γ(f#(a))` for that concrete
/// `f`, and the suite checks it that way — by applying both and testing membership — rather than by
/// inspecting the arithmetic.
pub fn transfer(system: &ComparisonSystem, state: &[Displacement]) -> Vec<Displacement> {
    system
        .interdependence()
        .iter()
        .zip(system.perturbation())
        .map(|(row, base)| {
            let mut accumulated =
                Displacement::at_most(*base).unwrap_or(Displacement::Range { lo: 0.0, hi: 1.0 });
            for (coefficient, value) in row.iter().zip(state) {
                if *coefficient == 0.0 {
                    continue;
                }
                accumulated = displacement::add(accumulated, displacement::scale(*value, *coefficient));
            }
            accumulated
        })
        .collect()
}

/// The bound the post-fixpoint licenses: the free variables' upper endpoints, summed and capped.
///
/// Summing rather than maximising is what the oscillation term in the comparison theorem asks for
/// when the answer is a joint marginal over several free variables. With one free variable — every
/// query in this workspace's fixtures — the sum is that one coordinate.
fn read_out(system: &ComparisonSystem, state: &[Displacement]) -> f64 {
    system
        .free_positions()
        .iter()
        .map(|position| state[*position].total_variation_bound())
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

/// Every factor's potential abstracted in [`crate::domains::SupportDomain`].
///
/// Exposed because it is the measurement [`crate::reference`] needs: on the shipped reference world
/// every entry of every one of the six region factors comes back `Either`, which is `⊤`, which is
/// the whole reason no bound follows. An abstraction of nothing is still an abstraction, and saying
/// so with a domain is more checkable than saying so in prose.
pub fn region_support(region: &QueryRegion) -> Vec<(String, Support)> {
    region
        .factors()
        .iter()
        .map(|factor| {
            let element = match factor.table() {
                Some(table) => Support::of_table(table),
                None => Support::unknown(entries_of(region, factor.scope())),
            };
            (factor.id().to_string(), element)
        })
        .collect()
}

fn entries_of(region: &QueryRegion, scope: &[String]) -> usize {
    scope.iter().fold(1usize, |entries, name| {
        entries.saturating_mul(region.cardinality_of(name).unwrap_or(1))
    })
}
