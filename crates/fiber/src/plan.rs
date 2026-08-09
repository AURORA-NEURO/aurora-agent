//! The physical backend portfolio, consulted rather than assumed.
//!
//! Blueprint 43.36 selects a plan `π* = argmin_{π ∈ Π(L_q)} ρ(C(π; θ))` from a portfolio, and
//! 43.37 requires the conditions under which FIBER *does not* compress or accelerate to be
//! first-class outputs. Until this module existed the compiler wrote
//! [`bioprism_section::Backend::BackwardFactorSliceReference`] and a `None` fallback into every
//! certificate as literals, so the plan block was a constant wearing the shape of a result.
//! `bioprism-backends` has held the portfolio, the cost model and the typed refusal the whole
//! time; what was missing was the call. This module makes it.
//!
//! ## What the call finds
//!
//! A real argmin. On the shipped reference world the compiled region is six factors over
//! seventeen variables, and the portfolio costs InsideOut bucket elimination at `2.1439e3`
//! semiring operations against a direct-materialisation baseline of `3.7240e6` — a predicted
//! speedup of 1737x — with no member declining. That number is new information and it is reported,
//! on [`PlanEvaluation`].
//!
//! ## Why the argmin does not reach the certificate
//!
//! Because not one of those plans can run. [`bioprism_backends::Portfolio::execute`] over the same
//! region returns [`bioprism_backends::Declined::MissingFactorTable`]: `fiber-world/0.1` declares
//! a factor's `inputs`, `outputs`, `kind`, `scope`, `tags` and `cost` and has no field in which a
//! potential could be written, so a region sliced out of *any* world this engine can load carries
//! signatures and no tables. The elimination estimate is a costing of an evaluation that cannot be
//! performed.
//!
//! [`bioprism_section::PlanDescriptor`] is a receipt for the plan that produced the delivered
//! section. Writing `faq_inside_out` into it would name an engine that provably never ran and
//! provably could not have — a worse falsehood than the literal it replaced, and one a consumer
//! could not detect, because the certificate carries no evidence of execution. So the descriptor
//! keeps naming the backward factor slice, which did run, and the portfolio's verdict is reported
//! on [`crate::CompileTrace::plan`] — the same place, and for the same reason, that
//! [`crate::PolicyOutcome`] is reported rather than added to the hashed bytes.
//!
//! This is why the change moves no wire format and needs no bump of
//! `fiber-context-certificate/0.1`. The claim is put to `bioprism-governance` rather than asserted,
//! in `tests/plan_governance.rs`.
//!
//! ## What is still not implemented here
//!
//! - **Execution.** The pass costs and never runs, because compile cost must track the compiled
//!   region rather than the corpus (43.34), and because a compiler that evaluates the query it is
//!   compiling has stopped being a compiler.
//! - **Five of the seven backends.** `bioprism-backends` implements bucket elimination and direct
//!   materialisation and states, in its own crate documentation, why worst-case-optimal joins,
//!   tensor networks, decision diagrams and incremental views are absent rather than stubbed. This
//!   module inherits that portfolio exactly and registers no member of its own.
//! - **[`bioprism_section::FallbackReason::ProtectedClosureDominates`] and
//!   [`bioprism_section::FallbackReason::PolicyFragmentation`].** Both are conditions of the
//!   *slice* rather than of the portfolio, 43.37 names the codes without giving either a trigger,
//!   and nothing in the workspace constructs them. Named here so the gap is a line item rather
//!   than a silence.

use bioprism_backends::{
    CardinalityPolicy, Declined, Portfolio, QueryRegion, RegionError, Selection,
};
use bioprism_section::{Backend, PlanDescriptor};
use bioprism_world::WorldSource;

/// The engine that produces a Decision Section.
///
/// Named once, here, so the certificate's `backend` field has a single origin and a reader can
/// find the argument for it. 43.36's portfolio does not contain this member: every member of
/// [`Portfolio::reference`] is a sum-product evaluator over a valuated region, and the backward
/// factor slice selects evidence instead of evaluating anything — which is why it needs no
/// potentials and they do.
pub const DELIVERING_BACKEND: Backend = Backend::BackwardFactorSliceReference;

/// The compiled region's own statistics.
///
/// 43.18: "never use total graph size as the primary complexity predictor." Every figure here is
/// query-specific; the world's size sits beside them on the plan descriptor, and the pair is the
/// comparison the blueprint asks a reader to be able to make.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionStatistics {
    pub variables: usize,
    pub factors: usize,
    /// Size of the joint assignment space the conservative plan would enumerate.
    pub joint_entries: f64,
    /// Share of region variables whose domain size was defaulted rather than read off a fact.
    pub assumed_cardinality_fraction: f64,
}

/// What the portfolio said about the compiled region.
#[derive(Debug, Clone, PartialEq)]
pub enum PortfolioOutcome {
    /// At least one member costed the region inside its budgets, and this is the argmin.
    ///
    /// Boxed because [`Selection`] carries every candidate's full
    /// [`bioprism_backends::Estimate`], while the enum's other two variants are a pointer wide.
    Costed(Box<Selection>),
    /// Not one member could cost the region inside its budgets, direct materialisation included.
    ///
    /// 43.48: "impossible contracts return infeasible, not fabricated output." The compile still
    /// succeeds — the section was assembled by the backward slice, which needs no plan — but the
    /// region it delivers has no costed physical evaluation behind it at all.
    NoAdmissiblePlan(Declined),
    /// The compiled region could not be built, so nothing was costed.
    ///
    /// Reported rather than raised. A portfolio consultation is an observation *about* a compiled
    /// region; letting it fail a compile that the closure, the slice, the policy screen, the
    /// temporal cut and the oracle all completed would turn a diagnostic into a gate, and would
    /// make the set of compilable worlds depend on a cost model.
    RegionRejected(RegionError),
}

/// The pass's full record, including the part that cannot reach the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanEvaluation {
    /// Absent exactly when no region could be built.
    pub region: Option<RegionStatistics>,
    pub outcome: PortfolioOutcome,
    /// The refusal every portfolio member returns when asked to *execute* this region, if any.
    ///
    /// Derived from the region rather than obtained by calling
    /// [`bioprism_backends::Portfolio::execute`], because the pass must not run the query it is
    /// costing. `None` would mean a valuated region reached the compiler, which no `fiber-world/0.1`
    /// world can produce.
    pub execution_refusal: Option<Declined>,
}

impl PlanEvaluation {
    /// The backend the portfolio would choose if the region could be run.
    pub fn argmin(&self) -> Option<Backend> {
        match &self.outcome {
            PortfolioOutcome::Costed(selection) => Some(selection.chosen),
            PortfolioOutcome::NoAdmissiblePlan(_) | PortfolioOutcome::RegionRejected(_) => None,
        }
    }

    /// `baseline_cost / cost` for the argmin. Below `1.0` means the conservative plan was cheaper.
    pub fn predicted_speedup(&self) -> Option<f64> {
        match &self.outcome {
            PortfolioOutcome::Costed(selection) => Some(selection.predicted_speedup),
            PortfolioOutcome::NoAdmissiblePlan(_) | PortfolioOutcome::RegionRejected(_) => None,
        }
    }

    /// Whether any portfolio member could execute the compiled region.
    ///
    /// False on every world `fiber-world/0.1` can state, once the region has a factor at all. That
    /// is the finding, not a defect, and it is the whole reason [`Self::argmin`] stays off the
    /// certificate.
    pub fn is_executable(&self) -> bool {
        self.execution_refusal.is_none()
    }

    /// The wire plan: what actually produced the delivered section.
    ///
    /// `backend` is [`DELIVERING_BACKEND`] and `fallback` is `None`, and the two must agree about
    /// which plan they describe or the block is incoherent. [`bioprism_section::Fallback`] answers
    /// "why did the compiler fall back instead of using a compressing backend"; the plan named
    /// here did not fall back, it ran, and on the reference world it delivered eleven of 761
    /// facts. The portfolio's own [`Selection::fallback`] is about a sum-product plan the compiler
    /// never adopted, so putting it in this field would describe one plan in the `backend` key and
    /// a different one in the `fallback` key.
    ///
    /// The counts come from the slice rather than from the region on purpose. The two slicers
    /// agree on which factors are selected — `bioprism-backends` documents that they must — but
    /// they measure arity differently: [`crate::slice::max_selected_arity`] reports a factor's
    /// declared input count, while [`bioprism_backends::QueryRegion::max_factor_arity`] reports the
    /// size of its deduplicated input-plus-output scope, one larger on the reference world's widest
    /// factor. Taking the region's figure would move the certificate digest for a change of
    /// definition nobody asked for.
    pub fn descriptor(
        &self,
        compiled_factor_count: usize,
        compiled_fact_count: usize,
        total_factor_count: usize,
        total_fact_count: usize,
        max_selected_factor_arity: usize,
    ) -> PlanDescriptor {
        PlanDescriptor {
            backend: DELIVERING_BACKEND,
            compiled_factor_count,
            compiled_fact_count,
            total_factor_count,
            total_fact_count,
            max_selected_factor_arity,
            fallback: None,
        }
    }

    /// One line for the pass receipt, carrying what the wire cannot.
    pub fn receipt_note(&self, fact_selection_ratio: f64) -> String {
        let portfolio = match &self.outcome {
            PortfolioOutcome::Costed(selection) => format!(
                "portfolio costed {} candidate(s), argmin {} at {:.4e} against a baseline of {:.4e} ({:.1}x)",
                selection.candidates.len(),
                selection.chosen.as_str(),
                selection.cost,
                selection.baseline_cost,
                selection.predicted_speedup,
            ),
            PortfolioOutcome::NoAdmissiblePlan(declined) => {
                format!("portfolio admitted no plan: {declined}")
            }
            PortfolioOutcome::RegionRejected(error) => {
                format!("no region could be built to cost: {error}")
            }
        };
        let executable = match &self.execution_refusal {
            None => "every costed plan is runnable over this region".to_string(),
            Some(declined) => format!("no costed plan is runnable: {declined}"),
        };
        format!(
            "backend {} retained {fact_selection_ratio:.4} of facts; {portfolio}; {executable}",
            DELIVERING_BACKEND.as_str(),
        )
    }
}

/// Slices the region the query reaches.
///
/// The targets are the ones handed to [`crate::backward_slice`], so both slicers see the same
/// query. `bioprism-backends` reimplements the slice rather than importing it, because a physical
/// backend must be costable from a region alone with no compiler in the link; the crate documents
/// that the two agree by construction, a factor entering when it produces a needed variable.
///
/// Built once per compile and handed to both the portfolio and [`crate::influence`], so the two
/// passes cannot disagree about what the query reached.
pub fn compile_region<'a, S, I>(
    source: &S,
    label: &str,
    targets: I,
) -> Result<QueryRegion, RegionError>
where
    S: WorldSource + ?Sized,
    I: IntoIterator<Item = &'a str>,
{
    QueryRegion::from_world_slice(source, label, targets, &CardinalityPolicy::default())
}

/// Asks the portfolio to cost the compiled region.
pub fn evaluate(region: Result<&QueryRegion, &RegionError>) -> PlanEvaluation {
    let region = match region {
        Ok(region) => region,
        Err(error) => {
            return PlanEvaluation {
                region: None,
                outcome: PortfolioOutcome::RegionRejected(error.clone()),
                execution_refusal: None,
            }
        }
    };

    let outcome = match Portfolio::reference().select(region) {
        Ok(selection) => PortfolioOutcome::Costed(Box::new(selection)),
        Err(declined) => PortfolioOutcome::NoAdmissiblePlan(declined),
    };
    let execution_refusal = execution_refusal(region, &outcome);

    PlanEvaluation {
        region: Some(RegionStatistics {
            variables: region.variable_count(),
            factors: region.factors().len(),
            joint_entries: region.joint_entries(),
            assumed_cardinality_fraction: region.assumed_cardinality_fraction(),
        }),
        outcome,
        execution_refusal,
    }
}

/// The refusal a portfolio member would return if this region were handed to `execute`.
///
/// A region whose factors carry no potential is costable and not runnable, and
/// [`Declined::MissingFactorTable`] is the vocabulary `bioprism-backends` already uses to say so.
/// Reconstructing it here rather than provoking it keeps the pass free of execution: the condition
/// is a property of the region's factors, readable without touching a semiring.
///
/// A region with no factors at all is reported as runnable, which is vacuously true and reachable:
/// a query whose target no factor produces compiles to an empty region.
fn execution_refusal(region: &QueryRegion, outcome: &PortfolioOutcome) -> Option<Declined> {
    let backend = match outcome {
        PortfolioOutcome::Costed(selection) => selection.chosen,
        PortfolioOutcome::NoAdmissiblePlan(_) | PortfolioOutcome::RegionRejected(_) => {
            Backend::DirectMaterialization
        }
    };
    region
        .factors()
        .iter()
        .find(|factor| factor.table().is_none())
        .map(|factor| Declined::MissingFactorTable {
            backend: backend.as_str(),
            factor: factor.id().to_string(),
        })
}
