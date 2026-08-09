//! Numeric influence bounds on withheld evidence.
//!
//! Blueprint 43.28 requires the compiler to "compute exact or conservative influence bounds" so
//! that an omission can be classified as [`bioprism_section::InfluenceClass::Bounded`] — non-zero
//! influence with a stated ceiling — rather than as unknown. `bioprism-influence` built the
//! bounds, proved them against brute force, and wrote `INTEGRATION_NOTE` naming the five changes
//! this crate would need. This module is items 2 through 5 of that note.
//!
//! ## The split, which is the part that is easy to get wrong
//!
//! A fact withheld at the temporal cut is *deferred*: it may become readable later, and the
//! refinement frontier is built from that fact. A fact whose influence is bounded is *bounded*.
//! A withheld fact whose influence is bounded is both, and [`bioprism_section::OmissionGroup`]
//! carries one class. So the deferred group is **split** rather than promoted: members with an
//! informative bound move into a `Bounded` group whose reason records that they were also withheld
//! at the cut, and the rest stay `DeferredAcquisition`. Promoting the whole group would drop the
//! "may become available later" fact that
//! [`bioprism_section::DecisionSection::refinement_frontier`] exists to carry, and a consumer
//! reading only the manifest would conclude no retry could help.
//!
//! ## Two independent reasons nothing is bounded on any world this engine can load
//!
//! **The schema declares no potentials.** `fiber-world/0.1` gives a factor `inputs`, `outputs`,
//! `kind`, `scope`, `tags` and `cost`. Every implemented method needs either a factor's own
//! entries or a caller-stated perturbation range, and neither the world nor `fiber-query/0.2` has
//! a field for one, so every analysis returns
//! [`bioprism_influence::UnknownReason::NoFactorTable`]. `bioprism-influence` measured exactly
//! this and published it: zero of six region factors bounded on the shipped reference world. This
//! pass reproduces that result from inside the compiler rather than taking it on trust.
//!
//! **The perturbation is a proxy, and the pass says so before it would matter.** Withholding a
//! fact removes an *observation* of a variable. The compiled region has no object for an
//! observation — [`bioprism_backends::QueryRegion::from_world_slice`] builds factors from the
//! world's factor documents and reads a fact only for a domain size — so the closest expressible
//! event is removing the factors whose scope contains that variable, which is a perturbation of
//! the *rule* rather than of the *evidence*. [`CorrespondenceCheck`] is the necessary condition
//! this pass can actually check: every subject factor must produce only variables that are
//! themselves withheld, so that the removal touches no delivered evidence. It is a necessary
//! condition and not a proof of equivalence, and it is a gate rather than a remark precisely so
//! that a world which one day carries potentials cannot silently open a `Bounded` group on an
//! unproven correspondence.
//!
//! ## What this pass does not do
//!
//! - It does not bound the structurally unreachable group. Those omissions are
//!   [`bioprism_section::InfluenceClass::Zero`] on a scope argument that needs no number, and
//!   `bioprism-influence`'s own reference module says corroborating them numerically "is not new
//!   capability".
//! - It does not bound the policy-withheld group. No amount of waiting or bounding produces a fact
//!   consent forbids, and [`bioprism_section::InfluenceClass::InaccessibleByPolicy`] is the class
//!   that says so.
//! - It does not touch the certificate's `limitations` string. The clause `formal influence
//!   bounds` stays, because it stays true of every world `fiber-world/0.1` can state, and
//!   `INTEGRATION_NOTE` is explicit that "a limitation string that shrinks while the certificate it
//!   appears on gains nothing would be worse than one that stays".
//! - It does not execute. [`bioprism_influence::InfluenceAnalyzer::structural_only`] forbids the
//!   exact method from running the query the compiler is compiling (43.34).

use crate::temporal::TemporalCut;
use bioprism_backends::QueryRegion;
use bioprism_influence::manifest::is_informative;
use bioprism_influence::{
    omission_group_from_analysis, InfluenceAnalysis, InfluenceAnalyzer, InfluenceError,
    Perturbation,
};
use bioprism_section::OmissionGroup;
use bioprism_world::WorldSource;
use std::collections::BTreeSet;
use thiserror::Error;

/// The perturbation class a withholding is modelled as.
///
/// Removal is the class the omission manifest needs: an omitted fact is one the compiler did not
/// deliver, and the honest counterfactual is the region evaluated as if that evidence had never
/// been acquired.
const WITHHOLDING_CLASS: Perturbation = Perturbation::Removal;

/// Why a withheld fact's influence could not be put to the analyser at all.
///
/// Distinct from [`bioprism_influence::UnknownReason`], which is a well-formed question no
/// implemented method may answer. These are questions that could not be *asked*.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum NotPosable {
    /// No factor in the compiled region mentions the withheld variable.
    ///
    /// The fact reached the selection through the protected closure rather than through a
    /// dependency path, so the region has no image of it to perturb. Not zero influence: the
    /// oracle reads the fact's value directly and the region's answer distribution is a different
    /// functional from the decision.
    #[error("no factor in the compiled region has variable {variable:?} in scope, so the withholding has no image in the region to perturb")]
    OutsideCompiledRegion { variable: String },

    /// The analyser rejected the question as malformed.
    #[error(transparent)]
    Analyser(#[from] InfluenceError),
}

/// Whether removing the subject factors is a perturbation of withheld evidence only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrespondenceCheck {
    /// Every subject factor produces only variables that are themselves withheld at the cut.
    OnlyWithheldEvidence,
    /// A subject factor also produces a variable the section delivered, so removing it would bound
    /// a different event than the withholding.
    TouchesDeliveredEvidence { factor: String, variable: String },
}

impl CorrespondenceCheck {
    pub fn holds(&self) -> bool {
        matches!(self, CorrespondenceCheck::OnlyWithheldEvidence)
    }
}

/// One withheld fact, the region factors standing in for it, and what came back.
#[derive(Debug, Clone, PartialEq)]
pub struct WithholdingAnalysis {
    pub fact_id: String,
    pub variable: String,
    /// Region factors whose scope contains the withheld variable, sorted.
    pub subject_factors: Vec<String>,
    pub correspondence: CorrespondenceCheck,
    pub outcome: Result<InfluenceAnalysis, NotPosable>,
}

impl WithholdingAnalysis {
    /// Whether this member has earned a place in the `Bounded` group.
    ///
    /// Three conditions, all necessary. The analysis must have produced a bound; the bound must be
    /// informative, because a sound bound of `1.0` permits every answer and a manifest full of
    /// vacuous bounds is formally sufficient and practically empty; and the correspondence check
    /// must hold, because a bound on the wrong event is not a bound.
    pub fn is_bounded(&self, group: &OmissionGroup) -> bool {
        self.correspondence.holds() && is_informative(group)
    }
}

/// The deferred group, split.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WithheldSplit {
    /// Withheld facts with an informative bound, in certificate order.
    pub bounded: Vec<String>,
    /// Withheld facts that stay [`bioprism_section::InfluenceClass::DeferredAcquisition`].
    pub deferred: Vec<String>,
    /// The joint analysis covering every member of `bounded`, absent when that list is empty.
    ///
    /// A group's `bound` field bounds the omission of the *whole* group, so it is computed by
    /// [`InfluenceAnalyzer::analyse_group`] over the union of the members' subject factors rather
    /// than by taking a maximum over per-member bounds, which would not be an upper bound on a
    /// joint perturbation.
    pub joint: Option<InfluenceAnalysis>,
    /// Every per-member analysis, bounded or not, so a reader can see what was attempted.
    pub attempted: Vec<WithholdingAnalysis>,
}

impl WithheldSplit {
    /// The `Bounded` group this split licenses, if any.
    ///
    /// The reason string carries both halves: `INTEGRATION_NOTE` item 5 requires that a
    /// withheld-and-bounded fact keep the record of having been withheld, because the refinement
    /// frontier still names it and a reader of the manifest alone must not conclude otherwise.
    pub fn bounded_group(&self) -> Option<OmissionGroup> {
        let joint = self.joint.as_ref()?;
        let mut group = omission_group_from_analysis(
            joint,
            self.bounded.len(),
            self.bounded.iter().take(3).cloned(),
        );
        group.reason = format!(
            "governed by an event not yet available at the decision cut, and {}",
            group.reason
        );
        Some(group)
    }

    /// How many members of the deferred group moved to `Bounded`.
    pub fn promoted(&self) -> usize {
        self.bounded.len()
    }
}

/// Bounds the influence of every temporally withheld fact and splits the deferred group.
///
/// `withheld` is the compiler's list of facts removed at the cut, in certificate order; the split
/// preserves that order so the manifest a reader sees is the order the compiler decided.
///
/// Returns an empty split when there is no region — a compile whose region could not be built has
/// nothing to perturb — which keeps a costing failure from changing the influence classification
/// of anything.
pub fn split_withheld<S: WorldSource + ?Sized>(
    source: &S,
    region: Option<&QueryRegion>,
    withheld: &[String],
    cut: &TemporalCut,
) -> WithheldSplit {
    let mut split = WithheldSplit::default();
    if withheld.is_empty() {
        return split;
    }
    let Some(region) = region else {
        split.deferred = withheld.to_vec();
        return split;
    };

    let analyzer = InfluenceAnalyzer::default().structural_only();
    let mut bounded_subjects: BTreeSet<String> = BTreeSet::new();

    for fact_id in withheld {
        let Some(fact) = source.fact(fact_id) else {
            split.deferred.push(fact_id.clone());
            continue;
        };
        let variable = fact.provides.as_str().to_string();
        let subject_factors = subject_factors(region, &variable);
        let correspondence = check_correspondence(source, &subject_factors, cut);

        let outcome = if subject_factors.is_empty() {
            Err(NotPosable::OutsideCompiledRegion {
                variable: variable.clone(),
            })
        } else {
            analyzer
                .analyse_group(region, &subject_factors, &WITHHOLDING_CLASS)
                .map_err(NotPosable::from)
        };

        let analysis = WithholdingAnalysis {
            fact_id: fact_id.clone(),
            variable,
            subject_factors: subject_factors.clone(),
            correspondence,
            outcome,
        };

        let earns_a_bound = match &analysis.outcome {
            Ok(result) => {
                let group = omission_group_from_analysis(result, 1, Vec::new());
                analysis.is_bounded(&group)
            }
            Err(_) => false,
        };
        if earns_a_bound {
            split.bounded.push(fact_id.clone());
            bounded_subjects.extend(subject_factors);
        } else {
            split.deferred.push(fact_id.clone());
        }
        split.attempted.push(analysis);
    }

    if !bounded_subjects.is_empty() {
        let subjects: Vec<String> = bounded_subjects.into_iter().collect();
        match analyzer.analyse_group(region, &subjects, &WITHHOLDING_CLASS) {
            Ok(joint) => split.joint = Some(joint),
            Err(_) => {
                split.deferred.append(&mut split.bounded);
                split.deferred.sort();
            }
        }
    }

    split
}

/// Region factors whose scope contains the withheld variable.
fn subject_factors(region: &QueryRegion, variable: &str) -> Vec<String> {
    region
        .factors()
        .iter()
        .filter(|factor| factor.scope().iter().any(|name| name == variable))
        .map(|factor| factor.id().to_string())
        .collect()
}

/// Whether removing the subject factors perturbs withheld evidence and nothing else.
///
/// An empty subject list holds vacuously; the caller has already classified that case as
/// [`NotPosable::OutsideCompiledRegion`], so a vacuous pass here cannot admit anything.
fn check_correspondence<S: WorldSource + ?Sized>(
    source: &S,
    subject_factors: &[String],
    cut: &TemporalCut,
) -> CorrespondenceCheck {
    for factor_id in subject_factors {
        let Some(factor) = source.factor(factor_id) else {
            continue;
        };
        for output in &factor.outputs {
            if cut.is_accessible(output.as_str()) {
                return CorrespondenceCheck::TouchesDeliveredEvidence {
                    factor: factor_id.clone(),
                    variable: output.as_str().to_string(),
                };
            }
        }
    }
    CorrespondenceCheck::OnlyWithheldEvidence
}
