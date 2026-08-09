//! Running every applicable method and keeping the tightest.
//!
//! The minimum of two sound upper bounds is a sound upper bound, so the analyzer does not have to
//! choose a method in advance: it runs the ones whose preconditions hold, keeps the smallest
//! number, and records what every method said so a reader can see the slack. When no method's
//! preconditions hold the result is [`crate::InfluenceEstimate::Unknown`] carrying the reasons —
//! never a large number, never infinity.
//!
//! ## The methods, in the order of how much they cost and how much they know
//!
//! | Method | Needs | Exact? |
//! |---|---|---|
//! | [`BoundMethod::DynamicRange`] | the perturbation's ratio range | no, ignores all structure |
//! | [`BoundMethod::ChainContraction`] | a recognised Markov chain | no, ignores within-row structure |
//! | [`BoundMethod::AbstractInterpretation`] | a strictly positive factor graph under Dobrushin's condition | no, sums over all paths including the ones that double back |
//! | [`BoundMethod::ExactRemoval`] | tables everywhere and a willing backend | yes, for removal |
//!
//! The 43.11 method is the only one that accepts a *cycle*, which is the class
//! [`crate::contraction`] refuses on principle rather than for want of effort. It reports itself as
//! [`BoundMethod::WidenedAbstractInterpretation`] whenever it had to widen to terminate, which on
//! this analysis is almost always: conditional dependence between two sites runs both ways, so the
//! interdependence matrix is never nilpotent and the ascending chain never stabilises under join.
//!
//! `DynamicRange` is the only one that survives a region with no potentials, and then only for a
//! *stated* range: a removal bound is read off the factor's own entries and there are none. This
//! asymmetry is the whole story of the reference-world measurement in [`crate::reference`].
//!
//! ## Group composition, and why the order of the steps matters
//!
//! A group is bounded up to three ways and the smallest is kept.
//!
//! The **multiplicative** rule composes the ratio ranges into one range and applies the lemma
//! once. It is order-free, because the joint reweighting of the measure is a product and the
//! lemma is uniform over the base distribution.
//!
//! The **union** rule adds the per-factor structural bounds. This is the triangle inequality along
//! a chain of one-factor-at-a-time perturbations, and it too is order-free for the same reason:
//! each step's bound holds whatever the intermediate distribution happens to be.
//!
//! The **chain-contraction union** adds the per-factor *attenuated* bounds, and there the order is
//! not free. A step's attenuation is a product of Dobrushin coefficients of the kernels downstream
//! of it, and those coefficients are read off the original tables — so the steps must be taken
//! from the *upstream* end, where every downstream kernel is still in its original state when its
//! coefficient is used. Taking them downstream-first would multiply by coefficients of kernels
//! that had already been perturbed, and the result would not be a bound. The implementation sorts
//! by chain position before summing, and refuses the whole rule if any group member is not on the
//! detected chain.

use crate::bound::{
    Approximation, BoundMethod, InfluenceBound, InfluenceEstimate, InfluenceMetric,
};
use crate::contraction::{self, ChainStructure};
use crate::error::{InfluenceError, UnknownReason};
use crate::exact;
use crate::interpret;
use crate::perturbation::Perturbation;
use crate::perturbed;
use crate::ratio::{union_bound, RatioRange};
use bioprism_backends::{Budget, QueryRegion};
use serde::{Deserialize, Serialize};

/// What one method said when it was asked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodOutcome {
    pub method: BoundMethod,
    /// The bound the method produced, if its preconditions held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Why it did not, if they did not. Exactly one of these two fields is populated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declined: Option<UnknownReason>,
}

impl MethodOutcome {
    fn produced(method: BoundMethod, value: f64) -> Self {
        MethodOutcome {
            method,
            value: Some(value),
            declined: None,
        }
    }

    fn refused(method: BoundMethod, reason: UnknownReason) -> Self {
        MethodOutcome {
            method,
            value: None,
            declined: Some(reason),
        }
    }
}

/// The full record of an analysis: the answer, and every argument that was tried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfluenceAnalysis {
    pub subject: Vec<String>,
    pub perturbation: Perturbation,
    pub estimate: InfluenceEstimate,
    pub attempted: Vec<MethodOutcome>,
}

impl InfluenceAnalysis {
    /// How much larger the reported bound is than the exact influence, when both are known.
    ///
    /// `None` when no method was exact, which is the usual case and is not a failure — it is why
    /// [`crate::bruteforce`] exists. A looseness of `0.0` means the structural bound happened to
    /// be tight on this region.
    pub fn looseness(&self) -> Option<f64> {
        let exact = self
            .attempted
            .iter()
            .find(|outcome| outcome.method.is_exact())
            .and_then(|outcome| outcome.value)?;
        let reported = self.estimate.bound()?.value();
        Some(reported - exact)
    }
}

/// The analysis entry point.
///
/// Holds the execution budget so a caller can forbid execution outright — a compiler pass that
/// must not run the query it is compiling can set a zero work budget and still get the structural
/// bounds, which is the mode [`crate::INTEGRATION_NOTE`] recommends for `bioprism-fiber`.
#[derive(Debug, Clone, Copy)]
pub struct InfluenceAnalyzer {
    budget: Budget,
    execute: bool,
}

impl Default for InfluenceAnalyzer {
    fn default() -> Self {
        InfluenceAnalyzer {
            budget: Budget::default(),
            execute: true,
        }
    }
}

impl InfluenceAnalyzer {
    pub fn new(budget: Budget) -> Self {
        InfluenceAnalyzer {
            budget,
            execute: true,
        }
    }

    /// Forbids the exact method from running the query. Structural methods still apply.
    pub fn structural_only(mut self) -> Self {
        self.execute = false;
        self
    }

    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// Bounds the influence of perturbing one factor.
    pub fn analyse_factor(
        &self,
        region: &QueryRegion,
        factor_id: &str,
        perturbation: &Perturbation,
    ) -> Result<InfluenceAnalysis, InfluenceError> {
        let mut attempted = Vec::new();
        let mut best: Option<InfluenceBound> = None;

        match dynamic_range_bound(region, factor_id, perturbation) {
            Ok(bound) => {
                attempted.push(MethodOutcome::produced(
                    BoundMethod::DynamicRange,
                    bound.value(),
                ));
                best = Some(bound);
            }
            Err(reason) => attempted.push(MethodOutcome::refused(BoundMethod::DynamicRange, reason)),
        }

        match contraction::detect(region) {
            Ok(chain) => {
                match contraction::chain_contraction_bound(
                    region,
                    &chain,
                    factor_id,
                    perturbation,
                )? {
                    Ok(bound) => {
                        attempted.push(MethodOutcome::produced(
                            BoundMethod::ChainContraction,
                            bound.value(),
                        ));
                        best = Some(match best {
                            Some(current) => current.tightest(bound),
                            None => bound,
                        });
                    }
                    Err(reason) => attempted
                        .push(MethodOutcome::refused(BoundMethod::ChainContraction, reason)),
                }
            }
            Err(reason) => {
                attempted.push(MethodOutcome::refused(BoundMethod::ChainContraction, reason))
            }
        }

        let subject = vec![factor_id.to_string()];
        self.attempt_abstract_interpretation(
            region,
            &subject,
            perturbation,
            &mut attempted,
            &mut best,
        )?;

        if self.execute {
            if let Perturbation::Removal = perturbation {
                match exact::exact_removal_bound(region, factor_id, self.budget) {
                    Ok(Ok(bound)) => {
                        attempted.push(MethodOutcome::produced(
                            BoundMethod::ExactRemoval,
                            bound.value(),
                        ));
                        best = Some(match best {
                            Some(current) => current.tightest(bound),
                            None => bound,
                        });
                    }
                    Ok(Err(reason)) => {
                        attempted.push(MethodOutcome::refused(BoundMethod::ExactRemoval, reason))
                    }
                    Err(InfluenceError::UntabledFactor { factor }) => attempted.push(
                        MethodOutcome::refused(
                            BoundMethod::ExactRemoval,
                            UnknownReason::NoFactorTable { factor },
                        ),
                    ),
                    Err(other) => return Err(other),
                }
            } else {
                attempted.push(MethodOutcome::refused(
                    BoundMethod::ExactRemoval,
                    UnknownReason::PerturbationClassUnsupported {
                        class: perturbation.class_name().to_string(),
                    },
                ));
            }
        }

        let estimate = match best {
            Some(bound) => InfluenceEstimate::Bounded(bound),
            None => InfluenceEstimate::Unknown(
                attempted
                    .iter()
                    .find_map(|outcome| outcome.declined.clone())
                    .unwrap_or(UnknownReason::NotAnalysed),
            ),
        };

        Ok(InfluenceAnalysis {
            subject,
            perturbation: *perturbation,
            estimate,
            attempted,
        })
    }

    /// Runs the 43.11 pass and records what it said.
    ///
    /// The pass is structural in the sense that matters for a compile: it reads potentials and
    /// never executes the query, so it is available under
    /// [`InfluenceAnalyzer::structural_only`] alongside the other two structural methods. Its
    /// method variant records whether it widened, which is why the outcome is pushed with the
    /// method the interpretation reported rather than with a fixed one.
    fn attempt_abstract_interpretation(
        &self,
        region: &QueryRegion,
        factor_ids: &[String],
        perturbation: &Perturbation,
        attempted: &mut Vec<MethodOutcome>,
        best: &mut Option<InfluenceBound>,
    ) -> Result<(), InfluenceError> {
        let outcome = match interpret::interpret_with_standard_domains(
            region,
            factor_ids,
            perturbation,
        ) {
            Ok(outcome) => outcome,
            Err(InfluenceError::UnknownFactor { factor, .. }) => Err(
                UnknownReason::RegionOutsideMethodClass {
                    method: BoundMethod::AbstractInterpretation.as_str().to_string(),
                    handles: "perturbations of factors the region declares".to_string(),
                    detail: format!("factor {factor:?} is not in the region"),
                },
            ),
            Err(other) => return Err(other),
        };
        match outcome {
            Ok(interpretation) => {
                attempted.push(MethodOutcome::produced(
                    interpretation.bound.method(),
                    interpretation.bound.value(),
                ));
                *best = Some(match best.take() {
                    Some(current) => current.tightest(interpretation.bound),
                    None => interpretation.bound,
                });
            }
            Err(reason) => attempted.push(MethodOutcome::refused(
                BoundMethod::AbstractInterpretation,
                reason,
            )),
        }
        Ok(())
    }

    /// Bounds the joint influence of perturbing a whole group of factors.
    ///
    /// An empty group is bounded by exactly zero: perturbing nothing moves nothing, and reporting
    /// that as `Unknown` would let an empty omission group void a sufficiency claim.
    pub fn analyse_group(
        &self,
        region: &QueryRegion,
        factor_ids: &[String],
        perturbation: &Perturbation,
    ) -> Result<InfluenceAnalysis, InfluenceError> {
        if factor_ids.is_empty() {
            return Ok(InfluenceAnalysis {
                subject: Vec::new(),
                perturbation: *perturbation,
                estimate: InfluenceEstimate::Bounded(structural_zero(
                    "the group is empty; no factor is perturbed",
                )?),
                attempted: vec![MethodOutcome::produced(BoundMethod::StructuralZero, 0.0)],
            });
        }

        let mut attempted = Vec::new();
        let mut best: Option<InfluenceBound> = None;

        let mut composed = RatioRange::identity();
        let mut per_factor: Vec<f64> = Vec::with_capacity(factor_ids.len());
        let mut composition_failed: Option<UnknownReason> = None;
        for id in factor_ids {
            match ratio_range_for(region, id, perturbation) {
                Ok(range) => {
                    composed = composed.compose(range);
                    per_factor.push(range.total_variation_bound());
                }
                Err(reason) => {
                    composition_failed = Some(reason);
                    break;
                }
            }
        }

        match composition_failed {
            Some(reason) => {
                attempted.push(MethodOutcome::refused(BoundMethod::RatioComposition, reason))
            }
            None => {
                let multiplicative = composed.total_variation_bound();
                let additive = union_bound(per_factor.iter().copied());
                let value = multiplicative.min(additive);
                let bound = InfluenceBound::new(
                    value,
                    InfluenceMetric::TotalVariationOnNormalisedAnswer,
                    BoundMethod::RatioComposition,
                    Approximation::ConservativeUpperBound,
                    format!(
                        "{} perturbation of {} factor(s) in region {:?}; multiplicative composition gave {multiplicative}, the union bound gave {additive}",
                        perturbation.class_name(),
                        factor_ids.len(),
                        region.label()
                    ),
                )?;
                attempted.push(MethodOutcome::produced(BoundMethod::RatioComposition, value));
                best = Some(bound);
            }
        }

        self.attempt_abstract_interpretation(
            region,
            factor_ids,
            perturbation,
            &mut attempted,
            &mut best,
        )?;

        match self.chain_union_bound(region, factor_ids, perturbation)? {
            Ok(bound) => {
                attempted.push(MethodOutcome::produced(
                    BoundMethod::ChainContraction,
                    bound.value(),
                ));
                best = Some(match best {
                    Some(current) => current.tightest(bound),
                    None => bound,
                });
            }
            Err(reason) => {
                attempted.push(MethodOutcome::refused(BoundMethod::ChainContraction, reason))
            }
        }

        if self.execute {
            if let Perturbation::Removal = perturbation {
                match exact::exact_group_removal_influence(region, factor_ids, self.budget) {
                    Ok(Ok(value)) => {
                        let bound = InfluenceBound::new(
                            value,
                            InfluenceMetric::TotalVariationOnNormalisedAnswer,
                            BoundMethod::ExactRemoval,
                            Approximation::Exact,
                            format!(
                                "joint removal of {} factor(s) from region {:?}",
                                factor_ids.len(),
                                region.label()
                            ),
                        )?;
                        attempted.push(MethodOutcome::produced(BoundMethod::ExactRemoval, value));
                        best = Some(match best {
                            Some(current) => current.tightest(bound),
                            None => bound,
                        });
                    }
                    Ok(Err(reason)) => {
                        attempted.push(MethodOutcome::refused(BoundMethod::ExactRemoval, reason))
                    }
                    Err(InfluenceError::UntabledFactor { factor }) => attempted.push(
                        MethodOutcome::refused(
                            BoundMethod::ExactRemoval,
                            UnknownReason::NoFactorTable { factor },
                        ),
                    ),
                    Err(other) => return Err(other),
                }
            }
        }

        let estimate = match best {
            Some(bound) => InfluenceEstimate::Bounded(bound),
            None => InfluenceEstimate::Unknown(
                attempted
                    .iter()
                    .find_map(|outcome| outcome.declined.clone())
                    .unwrap_or(UnknownReason::NotAnalysed),
            ),
        };

        Ok(InfluenceAnalysis {
            subject: factor_ids.to_vec(),
            perturbation: *perturbation,
            estimate,
            attempted,
        })
    }

    /// The sum of per-factor attenuated bounds, taken upstream-first.
    ///
    /// Refuses unless every member of the group sits on a detected chain: a group with one member
    /// off the chain has a step whose attenuation is undefined, and summing the rest would bound
    /// a different perturbation than the one that was asked about.
    fn chain_union_bound(
        &self,
        region: &QueryRegion,
        factor_ids: &[String],
        perturbation: &Perturbation,
    ) -> Result<Result<InfluenceBound, UnknownReason>, InfluenceError> {
        let chain = match contraction::detect(region) {
            Ok(chain) => chain,
            Err(reason) => return Ok(Err(reason)),
        };
        let mut ordered: Vec<(usize, &String)> = Vec::with_capacity(factor_ids.len());
        for id in factor_ids {
            match chain.position_of(id) {
                Some(position) => ordered.push((position, id)),
                None => {
                    return Ok(Err(UnknownReason::RegionOutsideMethodClass {
                        method: BoundMethod::ChainContraction.as_str().to_string(),
                        handles: "groups whose every member sits on the detected chain".to_string(),
                        detail: format!("factor {id:?} is not on the chain"),
                    }))
                }
            }
        }
        ordered.sort_by_key(|(position, _)| *position);

        let mut steps = Vec::with_capacity(ordered.len());
        for (_, id) in &ordered {
            match contraction::chain_contraction_bound(region, &chain, id, perturbation)? {
                Ok(bound) => steps.push(bound.value()),
                Err(reason) => return Ok(Err(reason)),
            }
        }
        let value = union_bound(steps.iter().copied());
        Ok(Ok(InfluenceBound::new(
            value,
            InfluenceMetric::TotalVariationOnNormalisedAnswer,
            BoundMethod::ChainContraction,
            Approximation::ConservativeUpperBound,
            format!(
                "{} perturbation of {} chain factor(s), summed upstream-first over positions {:?}",
                perturbation.class_name(),
                ordered.len(),
                ordered
                    .iter()
                    .map(|(position, _)| *position)
                    .collect::<Vec<_>>()
            ),
        )?))
    }
}

/// The ratio range a perturbation of this factor induces on the joint measure.
///
/// A stated multiplicative range needs no table, which is what lets a purely structural region —
/// the ordinary output of slicing a `fiber-world/0.1` world — carry a bound at all. A removal
/// range is read off the factor's own entries, so a table-less factor has no removal bound and
/// says so.
pub fn ratio_range_for(
    region: &QueryRegion,
    factor_id: &str,
    perturbation: &Perturbation,
) -> Result<RatioRange, UnknownReason> {
    match perturbation {
        Perturbation::MultiplicativeRange { range } => Ok(*range),
        Perturbation::Removal => match perturbed::table_of(region, factor_id) {
            Ok(table) => RatioRange::of_removal(table).map_err(|error| {
                UnknownReason::RegionOutsideMethodClass {
                    method: "dynamic_range".to_string(),
                    handles: "factors whose entries are finite and non-negative".to_string(),
                    detail: error.to_string(),
                }
            }),
            Err(InfluenceError::UntabledFactor { factor }) => {
                Err(UnknownReason::NoFactorTable { factor })
            }
            Err(other) => Err(UnknownReason::RegionOutsideMethodClass {
                method: "dynamic_range".to_string(),
                handles: "factors present in the region".to_string(),
                detail: other.to_string(),
            }),
        },
    }
}

/// The lemma applied to one factor's ratio range, with no reference to the rest of the region.
pub fn dynamic_range_bound(
    region: &QueryRegion,
    factor_id: &str,
    perturbation: &Perturbation,
) -> Result<InfluenceBound, UnknownReason> {
    let range = ratio_range_for(region, factor_id, perturbation)?;
    InfluenceBound::new(
        range.total_variation_bound(),
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::DynamicRange,
        Approximation::ConservativeUpperBound,
        format!(
            "{} perturbation of factor {factor_id:?} in region {:?}, ratio range [{}, {}]",
            perturbation.class_name(),
            region.label(),
            range.lo(),
            range.hi()
        ),
    )
    .map_err(|error| UnknownReason::RegionOutsideMethodClass {
        method: "dynamic_range".to_string(),
        handles: "ratio ranges whose bound is a total-variation distance".to_string(),
        detail: error.to_string(),
    })
}

/// A bound of exactly zero, for evidence no dependency path reaches.
///
/// This is the numeric face of [`bioprism_section::InfluenceClass::Zero`]. It is a separate
/// constructor rather than `InfluenceBound::new(0.0, …)` because the argument is different: no
/// perturbation was considered and no table was read, the conclusion follows from the region's
/// scope alone, and a reader of the certificate should be able to tell the two apart.
pub fn structural_zero(validity: impl Into<String>) -> Result<InfluenceBound, InfluenceError> {
    InfluenceBound::new(
        0.0,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::StructuralZero,
        Approximation::Exact,
        validity,
    )
}

/// The chain a region was recognised as, for a caller that wants to inspect the path.
pub fn chain_of(region: &QueryRegion) -> Result<ChainStructure, UnknownReason> {
    contraction::detect(region)
}
