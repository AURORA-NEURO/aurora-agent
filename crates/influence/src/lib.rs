//! Sound numeric influence bounds.
//!
//! Every Context Certificate this workspace emits carries one sentence, verbatim, from
//! `REFERENCE_LIMITATION` in `bioprism-fiber`:
//!
//! > Reference slicer uses dependency reachability and protected tags; it does not yet implement
//! > sheaf cohomology, FAQ-width optimization, abstract interpretation, or formal influence bounds.
//!
//! This crate closes the last clause, and only the last clause. It does not implement sheaf
//! cohomology and it does not implement FAQ-width optimization; see [`INTEGRATION_NOTE`] for what
//! the limitation string should say afterwards and for the part of it that stays true.
//!
//! ## The gap being closed
//!
//! [`bioprism_section::InfluenceClass`] has five variants and `bioprism-fiber` constructs two of
//! them. It can say "this evidence provably cannot matter" ([`bioprism_section::InfluenceClass::Zero`])
//! and "I deferred acquiring it" ([`bioprism_section::InfluenceClass::DeferredAcquisition`]). It
//! cannot say "this evidence can move the answer by at most ε", because nothing in the workspace
//! computes an ε. `bioprism-examples` records the consequence as a blocked claim,
//! `bounded_influence_omission`: *"nothing computes a numeric influence bound, so no group is ever
//! `Bounded`."*
//!
//! ## What makes a bound worth having
//!
//! Soundness. The true influence must never exceed the reported bound, because a certificate
//! carrying an unsound bound licenses omitting evidence that mattered — strictly worse than
//! carrying no bound at all. Three consequences run through every module here:
//!
//! - **Every approximation errs upward, and says so in the type.** [`Approximation`] is a field on
//!   [`InfluenceBound`], not a remark in a doc comment, because a doc comment does not travel on a
//!   certificate.
//! - **A bound that cannot be computed is not a bound of infinity.** It is
//!   [`InfluenceEstimate::Unknown`], which carries no `f64` at all, so there is no representation
//!   in which "nobody could check" and "the answer is large" are the same state. `AGENTS.md` calls
//!   this out as non-negotiable and the type system enforces it here.
//! - **Soundness is tested against ground truth.** [`bruteforce`] perturbs small worlds
//!   exhaustively and the suite asserts `true_influence ≤ reported_bound` across a generated
//!   family. A bound nobody checked against brute force is a number, not a guarantee.
//!
//! ## The measure, in one line
//!
//! Influence is total-variation distance between *normalised* answers over the query's free
//! variables. Normalising is what makes a factor of constant potential register as exactly zero
//! influence — which is correct — and what keeps the measure inside `[0, 1]` so it composes along
//! a path without accumulating units. See [`measure`] for the cost of that choice.
//!
//! ## The three methods
//!
//! | Method | Argument | Needs | Exact? |
//! |---|---|---|---|
//! | [`BoundMethod::DynamicRange`] | the lemma of [`ratio`] applied to one factor's ratio range | a table, or a stated range | no |
//! | [`BoundMethod::ChainContraction`] | Dobrushin coefficients multiplied along the path | a recognised Markov chain | no |
//! | [`BoundMethod::ExactRemoval`] | run the query twice and subtract | tables everywhere, a willing backend | yes |
//!
//! [`InfluenceAnalyzer`] runs whichever apply and keeps the smallest — the minimum of two sound
//! upper bounds is sound — while recording what every method said, so the slack is visible rather
//! than inferred.
//!
//! Nothing here reimplements variable elimination. The elimination engine, the order search, the
//! width budget and the typed refusal all live in `bioprism-backends`; this crate supplies two
//! regions and subtracts two answers. A second implementation would be a second thing to keep in
//! parity, and this workspace has paid for cross-implementation parity once already.
//!
//! ## The measurement that comes out against the crate
//!
//! On the shipped reference world, **zero omitted groups move from `Unknown` to `Bounded`**. The
//! `fiber-world/0.1` schema declares a factor's signature and never its potential, and it has no
//! field in which a perturbation range could be declared, so no method's precondition is met and
//! all six region factors report [`UnknownReason::NoFactorTable`]. That is a schema gap rather
//! than an effort gap, and it ships as the finding. [`mod@reference`] has the full run, including what
//! a caller-declared range *would* buy and why the all-ones valuation's uniform `0.0` is not a
//! result.
//!
//! ## What is not implemented
//!
//! See [`NOT_IMPLEMENTED`]. The short version: no Shapley-style attribution, no probabilistic or
//! sampled bounds, no refinement lattice, no decision-loss metric, no branching or loopy
//! contraction, and no additive or structural perturbation class.
//!
//! ## Where §43 names a technique without specifying it
//!
//! 43.28 requires the compiler to "compute exact or conservative influence bounds" and lists
//! "metric, method, confidence, and validity scope" in the runtime contract — and specifies none
//! of the four. It requires step 3, "propagate bounds through the decision algebra", and gives no
//! composition rule. It lists "bound tightness" in the evaluation program and defines no tightness
//! statistic. 43.11 gives the soundness condition `f(γ(a)) ⊆ γ(f#(a))` and names no concrete
//! domain. Every one of those is a decision of this implementation, made in the open in the module
//! that makes it, and none of them is presented as specification.

pub mod analysis;
pub mod bound;
pub mod bruteforce;
pub mod contraction;
pub mod error;
pub mod exact;
pub mod manifest;
pub mod measure;
pub mod perturbation;
pub mod perturbed;
pub mod ratio;
pub mod reference;
pub mod rng;
pub mod smallworld;

pub use analysis::{
    chain_of, dynamic_range_bound, structural_zero, InfluenceAnalysis, InfluenceAnalyzer,
    MethodOutcome,
};
pub use bound::{Approximation, BoundMethod, InfluenceBound, InfluenceEstimate, InfluenceMetric};
pub use bruteforce::{maximum_influence, BruteForceResult, MAX_PERTURBATION_VERTICES};
pub use contraction::{dobrushin_coefficients, ChainStructure};
pub use error::{InfluenceError, UnknownReason};
pub use exact::{exact_group_removal_influence, exact_removal_influence};
pub use manifest::{omission_group, omission_group_from_analysis, summarise, BoundedSummary};
pub use measure::{total_variation, AnswerDistribution};
pub use perturbation::Perturbation;
pub use ratio::{union_bound, RatioRange};
pub use reference::{measure as measure_reference_world, ReferenceMeasurement};
pub use smallworld::{Family, SmallWorldSpec};

/// Capabilities this crate does not have, named so their absence is a limitation rather than a lie.
///
/// `AGENTS.md`: "A missing capability that is stated is a limitation; one that is implied to exist
/// is a lie."
pub const NOT_IMPLEMENTED: &[(&str, &str)] = &[
    (
        "attribution over subsets",
        "every bound is the influence of a perturbation *given the rest of the region*. Nothing decomposes a joint influence into per-factor shares, Shapley or otherwise, and 43.28 does not ask for one.",
    ),
    (
        "probabilistic or sampled bounds",
        "every method is a deterministic worst-case argument, so the only honest confidence is one. A bound holding with probability 1-δ would need a different type; adding a confidence field to InfluenceBound before that type exists would invite a sampled method to fill it while the certificate still read `Bounded`.",
    ),
    (
        "the refinement lattice of 43.11",
        "the ratio-range interval is a sound abstract domain with a sound transformer, but there is no domain registry, no join or widening, and no refinement scheduler. Abstract interpretation in the sense 43.11 means is not implemented and the limitation string must keep saying so.",
    ),
    (
        "a decision-loss metric",
        "43.10 and 43.12 are defined relative to permitted actions and a decision loss, and `fiber-query/0.1` declares neither. A bound on the change in an answer distribution is not a bound on the change in a decision's cost, and this crate does not claim to convert one into the other.",
    ),
    (
        "branching and loopy contraction",
        "the Dobrushin argument extends to trees unchanged and is not implemented for them. It does *not* extend to cycles: the coefficient of a cycle is not the product of its edges, so there is no drop-in generalisation to write.",
    ),
    (
        "additive and structural perturbation classes",
        "`phi +/- delta` induces no bounded multiplicative reweighting when an entry approaches zero, so the lemma does not apply; and adding a factor the world does not declare changes the region's scope, which every method here assumes fixed. The second is exactly the incomplete-factor-graph caveat the reference certificate already carries.",
    ),
    (
        "max-product influence",
        "the measure normalises, and a max-product answer is not a distribution. Influence on a most-probable-explanation value needs a different functional.",
    ),
];

/// Exactly what `bioprism-fiber` would have to change, and what stays true afterwards.
///
/// This crate owns no code outside `crates/influence`. Integration is a separate step; this is the
/// note that makes it mechanical.
///
/// ## The five changes
///
/// 1. **Dependency.** Add `bioprism-influence` to `crates/fiber/Cargo.toml`. The link direction is
///    safe: this crate depends on `world`, `section` and `backends`, and on nothing in `fiber`.
///
/// 2. **A region.** `compile()` already has the `WorldSource` and the targets, so it can call
///    [`bioprism_backends::QueryRegion::from_world_slice`] with the same targets it passes to
///    `backward_slice`. `bioprism-backends` documents that the two slicers agree by construction —
///    a factor enters when it produces a needed variable — so this introduces no second notion of
///    what the query reaches.
///
/// 3. **A pass.** Insert `influence_bounds` after `plan_selection` and before the certificate is
///    assembled, emitting a `PassReceipt` whose `retained` is the number of groups that came back
///    `Bounded` and whose note carries the worst informative bound. Use
///    [`InfluenceAnalyzer::structural_only`] there: the exact method executes the query, and
///    compile cost is supposed to track the compiled region rather than the corpus (43.34). The
///    exact method belongs in audit and evaluation paths, not in compile.
///
/// 4. **The manifest.** `build_manifest` currently takes three counts. It would take the analyses
///    as well and call [`crate::manifest::omission_group_from_analysis`], which produces exactly
///    the [`bioprism_section::OmissionGroup`] shape — including `bound: Some(ε)` — so no schema
///    changes anywhere.
///
/// 5. **The deferred group must not simply be promoted.** A temporally withheld fact whose
///    influence is bounded is *both* deferred and bounded, and `OmissionGroup` carries one class.
///    Emit two groups: members with an informative bound in a `Bounded` group whose reason string
///    records that they were also withheld at the cut, and the rest left in the
///    `DeferredAcquisition` group. Collapsing them into one `Bounded` group would silently drop
///    the "may become available later" fact that the refinement frontier is built from.
///
/// ## Which line of the limitation becomes false
///
/// The clause `formal influence bounds` — and it becomes false *conditionally*, which the
/// replacement string must say. Proposed:
///
/// > Reference slicer uses dependency reachability and protected tags; it does not yet implement
/// > sheaf cohomology, FAQ-width optimization, or abstract interpretation. Formal influence bounds
/// > are computed where a factor valuation or a declared perturbation range exists, and reported
/// > as unknown influence otherwise.
///
/// ## What stays true, and must
///
/// - **Sheaf cohomology (43.06).** Untouched. Nothing here computes an obstruction class, and
///   `fiber-world/0.1` carries no cover to compute one over.
/// - **FAQ-width optimization (43.18, 43.19).** Untouched *by this crate*. It exists in
///   `bioprism-backends` and `bioprism-fiber` does not call it; that is a wiring gap between two
///   other crates and closing it is not this crate's claim to make.
/// - **Abstract interpretation (43.11).** Stays true. [`ratio::RatioRange`] is a sound interval
///   abstraction with a sound transformer, which is one ingredient of 43.11 and not the module:
///   there is no domain registry, no join, no widening and no refinement scheduler. Claiming
///   abstract interpretation on the strength of one interval domain would be exactly the
///   overstatement this workspace exists to avoid.
/// - **And the honest caveat on the win itself.** On the world as shipped the new pass emits no
///   bound at all. The capability is real, tested and sound; the reference certificate will not
///   change until a world carries potentials or a query declares ranges. A limitation string that
///   shrinks while the certificate it appears on gains nothing would be worse than one that stays.
///
/// ## The blocked claim in `bioprism-examples`
///
/// `Property::BoundedInfluenceOmission`'s blocker reads *"bioprism-fiber emits only
/// `InfluenceClass::Zero` and `DeferredAcquisition`; nothing computes a numeric influence bound,
/// so no group is ever `Bounded`."* After integration the second half is false and the first half
/// is still true on this world. The accurate replacement blocker is that `fiber-world/0.1`
/// declares factor signatures and never potentials, so the computed bound is `Unknown` for every
/// group in the shipped fixture — a schema gap, with a named fixture that would unblock it.
pub const INTEGRATION_NOTE: &str = concat!(
    "bioprism-fiber would: (1) depend on bioprism-influence; ",
    "(2) build a QueryRegion with QueryRegion::from_world_slice over the same targets it passes to backward_slice; ",
    "(3) run an `influence_bounds` pass after plan_selection using InfluenceAnalyzer::structural_only, so compile does not execute the query it is compiling; ",
    "(4) pass the analyses into build_manifest and construct groups with omission_group_from_analysis; ",
    "(5) split the deferred group rather than promoting it, so a temporally withheld fact that is also bounded does not lose its refinement-frontier entry. ",
    "The clause `formal influence bounds` then becomes false, conditionally: bounds exist where a factor valuation or a declared perturbation range exists, and are reported as unknown influence otherwise. ",
    "Sheaf cohomology, FAQ-width optimization and abstract interpretation all remain true of the reference slicer, ",
    "and on the world as shipped the new pass emits no bound at all, because fiber-world/0.1 declares factor signatures and never potentials."
);
