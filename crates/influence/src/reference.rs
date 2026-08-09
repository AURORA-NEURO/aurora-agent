//! The worked example, and the measurement that comes out against the crate.
//!
//! `AGENTS.md`: "If a measurement disagrees with the thesis, that is the measurement we publish."
//! This module runs the crate against the shipped reference world — 761 facts, 756 factors, the
//! split-integrity query the whole workspace is calibrated on — and the headline number is zero.
//!
//! ## Why zero
//!
//! Every method in this crate needs one of two things: a factor's potential, or a caller-stated
//! perturbation range. The `fiber-world/0.1` wire schema carries neither. A factor document
//! declares `inputs`, `outputs`, `kind`, `scope`, `tags` and `cost`; there is no field in which a
//! potential could be written, and no field in which an uncertainty range could be declared. A
//! region sliced out of that world is *structural* — `bioprism-backends` says so in its own docs
//! and refuses to execute one — so the removal ratio range, which is read off a factor's own
//! entries, does not exist to be read.
//!
//! The consequence for the omission manifest is exact and worth stating plainly: **on the shipped
//! reference world, no omitted group moves from `Unknown` to `Bounded`, because no potential
//! exists to bound.** This is a schema gap, not an effort gap, and it is the same gap
//! `bioprism-backends` names when it declines to implement worst-case-optimal joins.
//!
//! ## What the crate can still say about this world
//!
//! Three things, and the difference between them matters:
//!
//! 1. **Under a caller-stated range**, every factor in the region gets a real bound with no
//!    execution at all — [`crate::BoundMethod::DynamicRange`] never touches a table when the range
//!    is stated. This is the integration path that would actually work today: a query that
//!    declares "this assay is known to within ±10%" gets `Bounded` groups. The world declares no
//!    such ranges, so the number below is what a *hypothetical* declaration would buy, and it is
//!    labelled as such.
//! 2. **Under the all-ones valuation** that [`bioprism_backends::QueryRegion::with_uniform_tables`]
//!    supplies, every bound is exactly `0.0` and every one of them is worthless. A uniform
//!    potential has dynamic range one, so of course removing it moves nothing; the region reports
//!    the fabrication in its assumptions and this module refuses to count those bounds as a
//!    result.
//! 3. **Under removal against the world as shipped**, nothing. [`crate::UnknownReason::NoFactorTable`],
//!    six times.
//!
//! ## And the same answer from the 43.11 pass
//!
//! [`mod@crate::interpret`] adds an abstract-domain registry, a join, a widening, a narrowing schedule
//! and a bound for cyclic regions that no other method here can bound. It changes this measurement
//! by exactly nothing, and the reason is worth stating in the same sentence as the capability: an
//! abstract interpretation abstracts *something*, and `fiber-world/0.1` declares no potential for it
//! to abstract. Every one of the six region factors comes back `⊤` in
//! [`crate::domains::SupportDomain`] — recorded in [`ReferenceMeasurement::support_abstraction`], so
//! it is a measurement rather than an assertion — and the analysis refuses on the hypothesis rather
//! than reading a bound of one off `⊤`.
//!
//! That refusal is the load-bearing part. `⊤` reads out as a bound of one, a bound of one is
//! `Bounded`, and `Bounded` supports a sufficiency claim. An interpreter that ran anyway would turn
//! six honestly unknown groups into six that promise nothing and are formally sufficient, which is
//! not closing the gap the crate exists to close but widening it.
//!
//! ## The 750 exploratory factors
//!
//! The world's 756 factors are 750 `exploratory_summary` factors, five `deterministic_rule`
//! factors and one `decision_rule`. The backward slice from `split_integrity_status` reaches six
//! of them. The other 750 are omitted with `InfluenceClass::Zero` and a bound of `0.0` — which
//! `bioprism-fiber` already emits and which this crate corroborates from the numeric side via
//! [`crate::analysis::structural_zero`]. That is not new capability; it is the part of the
//! reference certificate that was already correct.

use crate::analysis::InfluenceAnalyzer;
use crate::bound::InfluenceEstimate;
use crate::error::InfluenceError;
use crate::perturbation::Perturbation;
use bioprism_backends::{CardinalityPolicy, QueryRegion};
use bioprism_world::World;

pub const REFERENCE_WORLD_JSON: &str =
    include_str!("../../../fixtures/fiber-v0.1/radiogenomic_world.json");

/// The target of the shipped `audit-split-integrity-v1` query.
pub const REFERENCE_TARGET: &str = "split_integrity_status";

/// The tolerance the hypothetical-declaration column uses: ±10%.
pub const HYPOTHETICAL_TOLERANCE: f64 = 0.10;

/// One factor's outcome under one perturbation class.
#[derive(Debug, Clone, PartialEq)]
pub struct FactorFinding {
    pub factor: String,
    pub arity: usize,
    pub estimate: InfluenceEstimate,
}

impl FactorFinding {
    pub fn bounded_at(&self) -> Option<f64> {
        self.estimate.bound().map(|bound| bound.value())
    }
}

/// Everything this crate can say about the shipped reference world.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceMeasurement {
    pub world_id: String,
    pub total_facts: usize,
    pub total_factors: usize,
    /// Factors the backward slice from the target reaches.
    pub region_factors: usize,
    pub region_variables: usize,
    /// Facts the slice does not compile, i.e. the omitted population.
    pub omitted_facts: usize,
    /// Factors the slice does not reach; every one of them is structurally zero-influence.
    pub unreached_factors: usize,
    /// Removal against the world exactly as shipped. The headline.
    pub removal: Vec<FactorFinding>,
    /// What a caller-declared ±10% range would buy. Hypothetical: the world declares no ranges.
    pub hypothetical_stated_range: Vec<FactorFinding>,
    /// The all-ones valuation, retained only so the reader can see that it produces `0.0` and why
    /// that is not a result.
    pub uniform_valuation: Vec<FactorFinding>,
    /// The 43.11 pass under removal against the world exactly as shipped.
    ///
    /// Separate from `removal` even though both come back unknown, because they come back unknown
    /// for clauses a reader should be able to tell apart: the other methods have no ratio range to
    /// read, and this one has no potential to abstract.
    pub abstract_interpretation: Vec<FactorFinding>,
    /// Every region factor's potential abstracted in [`crate::domains::SupportDomain`].
    ///
    /// The measurement behind the sentence "there is nothing to abstract". Each element is `⊤` of
    /// the factor's table length, meaning every entry is `Either` — neither known to be zero nor
    /// known to be positive.
    pub support_abstraction: Vec<(String, crate::domains::Support)>,
}

impl ReferenceMeasurement {
    /// How many region factors carry a bound under removal against the world as shipped.
    pub fn bounded_under_removal(&self) -> usize {
        self.removal
            .iter()
            .filter(|finding| finding.estimate.is_bounded())
            .count()
    }

    pub fn bounded_under_hypothetical_range(&self) -> usize {
        self.hypothetical_stated_range
            .iter()
            .filter(|finding| finding.estimate.is_bounded())
            .count()
    }

    /// How many region factors the 43.11 pass bounds against the world as shipped.
    ///
    /// Zero, and the crate ships that number rather than the capability's press release.
    pub fn bounded_under_abstract_interpretation(&self) -> usize {
        self.abstract_interpretation
            .iter()
            .filter(|finding| finding.estimate.is_bounded())
            .count()
    }

    /// Whether every region factor abstracts to `⊤` — nothing known about any entry.
    pub fn every_factor_abstracts_to_top(&self) -> bool {
        self.support_abstraction.iter().all(|(_, element)| {
            element.signs().is_some_and(|signs| {
                !signs.is_empty()
                    && signs
                        .iter()
                        .all(|sign| *sign == crate::domains::EntrySign::Either)
            })
        })
    }

    /// The single bound every factor gets under a stated ±10% range.
    ///
    /// It is the same number for every factor because [`crate::BoundMethod::DynamicRange`] reads
    /// only the range, and that is exactly the method's weakness: it does not distinguish a factor
    /// adjacent to the target from one six hops away. On a region with potentials,
    /// [`crate::contraction`] does.
    pub fn hypothetical_bound(&self) -> Option<f64> {
        self.hypothetical_stated_range
            .first()
            .and_then(FactorFinding::bounded_at)
    }

    /// A line a certificate reader can act on.
    pub fn headline(&self) -> String {
        format!(
            "reference world {}: {} facts, {} factors, slice reaches {} factors over {} variables; {} of {} region factors are bounded under removal ({} carry no potential), {} would be bounded under a declared ±{:.0}% range at {:.6}; the 43.11 abstract-interpretation pass bounds {} of them, because every factor abstracts to top",
            self.world_id,
            self.total_facts,
            self.total_factors,
            self.region_factors,
            self.region_variables,
            self.bounded_under_removal(),
            self.region_factors,
            self.region_factors - self.bounded_under_removal(),
            self.bounded_under_hypothetical_range(),
            HYPOTHETICAL_TOLERANCE * 100.0,
            self.hypothetical_bound().unwrap_or(f64::NAN),
            self.bounded_under_abstract_interpretation(),
        )
    }
}

/// Loads the shipped reference world.
pub fn reference_world() -> Result<World, InfluenceError> {
    let raw = serde_json::from_str(REFERENCE_WORLD_JSON).map_err(|source| {
        InfluenceError::ReferenceWorldUnreadable {
            message: source.to_string(),
        }
    })?;
    World::from_json(raw).map_err(|source| InfluenceError::ReferenceWorldUnreadable {
        message: source.to_string(),
    })
}

/// The structural region the split-integrity query compiles to.
pub fn reference_region(world: &World) -> Result<QueryRegion, InfluenceError> {
    QueryRegion::from_world_slice(
        world,
        "reference/split_integrity",
        [REFERENCE_TARGET],
        &CardinalityPolicy::default(),
    )
    .map_err(|source| InfluenceError::PerturbedRegionRejected {
        message: source.to_string(),
    })
}

/// Runs the crate against the reference world and reports what it found.
pub fn measure() -> Result<ReferenceMeasurement, InfluenceError> {
    let world = reference_world()?;
    let region = reference_region(&world)?;
    let analyzer = InfluenceAnalyzer::default();

    let removal_class = Perturbation::Removal;
    let stated_class = Perturbation::relative_tolerance(HYPOTHETICAL_TOLERANCE)?;

    let mut removal = Vec::new();
    let mut hypothetical_stated_range = Vec::new();
    let mut abstract_interpretation = Vec::new();
    for factor in region.factors() {
        let subject = vec![factor.id().to_string()];
        abstract_interpretation.push(FactorFinding {
            factor: factor.id().to_string(),
            arity: factor.arity(),
            estimate: match crate::interpret::interpret_with_standard_domains(
                &region,
                &subject,
                &removal_class,
            )? {
                Ok(interpretation) => InfluenceEstimate::Bounded(interpretation.bound),
                Err(reason) => InfluenceEstimate::Unknown(reason),
            },
        });
        removal.push(FactorFinding {
            factor: factor.id().to_string(),
            arity: factor.arity(),
            estimate: analyzer
                .analyse_factor(&region, factor.id(), &removal_class)?
                .estimate,
        });
        hypothetical_stated_range.push(FactorFinding {
            factor: factor.id().to_string(),
            arity: factor.arity(),
            estimate: analyzer
                .analyse_factor(&region, factor.id(), &stated_class)?
                .estimate,
        });
    }

    let valued = region
        .clone()
        .with_uniform_tables()
        .map_err(|source| InfluenceError::PerturbedRegionRejected {
            message: source.to_string(),
        })?;
    let mut uniform_valuation = Vec::new();
    for factor in valued.factors() {
        uniform_valuation.push(FactorFinding {
            factor: factor.id().to_string(),
            arity: factor.arity(),
            estimate: analyzer
                .analyse_factor(&valued, factor.id(), &removal_class)?
                .estimate,
        });
    }

    let provenance = region.provenance();
    Ok(ReferenceMeasurement {
        world_id: world.world_id.to_string(),
        total_facts: provenance.total_fact_count,
        total_factors: provenance.total_factor_count,
        region_factors: region.factors().len(),
        region_variables: region.variable_count(),
        omitted_facts: provenance
            .total_fact_count
            .saturating_sub(provenance.compiled_fact_count),
        unreached_factors: provenance
            .total_factor_count
            .saturating_sub(region.factors().len()),
        removal,
        hypothetical_stated_range,
        uniform_valuation,
        abstract_interpretation,
        support_abstraction: crate::interpret::region_support(&region),
    })
}
