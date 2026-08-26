//! Sound numeric influence bounds.
//!
//! Every Context Certificate this workspace emits carries one sentence, verbatim, from
//! `REFERENCE_LIMITATION` in `bioprism-fiber`:
//!
//! > Reference slicer uses dependency reachability and protected tags; it does not yet implement
//! > sheaf cohomology, FAQ-width optimization, abstract interpretation, or formal influence bounds.
//!
//! This crate closes the last clause, and builds what `abstract interpretation` names: an
//! abstract-domain registry with join, widening and a bounded refinement schedule, sound against
//! brute force. It does not implement sheaf cohomology and it does not implement FAQ-width
//! optimization. Neither clause it touches comes out of the string unconditionally, and
//! [`INTEGRATION_NOTE`] is where the conditions are written down — including the two reasons
//! `abstract interpretation` stays true today, one of which is that the reference slicer does not
//! yet call any of this.
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
//! ## The four methods
//!
//! | Method | Argument | Needs | Exact? |
//! |---|---|---|---|
//! | [`BoundMethod::DynamicRange`] | the lemma of [`ratio`] applied to one factor's ratio range | a table, or a stated range | no |
//! | [`BoundMethod::ChainContraction`] | Dobrushin coefficients multiplied along the path | a recognised Markov chain | no |
//! | [`BoundMethod::AbstractInterpretation`] | the 43.11 fixed point of [`mod@interpret`] over Dobrushin's comparison theorem | strictly positive tables and `‖C‖_∞ < 1` | no |
//! | [`BoundMethod::ExactRemoval`] | run the query twice and subtract | tables everywhere, a willing backend | yes |
//!
//! [`InfluenceAnalyzer`] runs whichever apply and keeps the smallest — the minimum of two sound
//! upper bounds is sound — while recording what every method said, so the slack is visible rather
//! than inferred.
//!
//! ## What 43.11 asked for, and what is here
//!
//! Blueprint 43.11 states the soundness condition `f(γ(a)) ⊆ γ(f#(a))` and names no domain, no
//! lattice, no widening operator and no refinement strategy. This crate supplies:
//!
//! - **A domain trait and a registry.** [`AbstractDomain`] declares the lattice operations and the
//!   [`FactClass`] a domain is entitled to abstract; [`DomainRegistry`] holds them and refuses both
//!   a duplicate registration and an abstract value from a foreign domain. Two of the shipped
//!   domains are intervals of non-negative reals with identical arithmetic and unrelated meanings,
//!   which is exactly the confusion the tagging exists to make impossible.
//! - **Join, proved against concretisation.** [`domains::SupportDomain`] is finite and its universe
//!   realises every sign pattern, so `γ(a) ∪ γ(b) ⊆ γ(a ⊔ b)` is checked over every equivalence
//!   class `γ` distinguishes. On the interval domains the same law is a falsification search over a
//!   grid, and [`EnumerableConcretisation::universe_is_complete`] is what keeps the two apart.
//! - **Widening, with something for it to do.** [`gibbs`] computes the Neumann series
//!   `Σ_n Cⁿ b` of Dobrushin's comparison theorem, which bounds influence on a factor graph of *any*
//!   shape — cycles included, which is the class [`contraction`] refuses on principle. Its Kleene
//!   chain never reaches its limit, so the solver widens, and [`Convergence`] records that it did.
//! - **A refinement scheduler for the part 43.11 determines.** [`RefinementSchedule`] budgets the
//!   joins before widening and the descending steps after it. The other reading of "refinement" —
//!   recomputing in a more precise *domain* — needs an ordering on domains that 43.11 does not
//!   supply, and is refused rather than invented. See [`NOT_IMPLEMENTED`].
//!
//! Sound against ground truth on 185 factor removals across five structural families, and it
//! changes the reference-world measurement by nothing at all; the next section says why.
//!
//! Nothing here reimplements variable elimination. The elimination engine, the order search, the
//! width budget and the typed refusal all live in `bioprism-backends`; this crate supplies two
//! regions and subtracts two answers. The 43.11 pass does not reimplement it either: it never
//! executes the query at all, and reads its coefficients off single-site conditionals, which are a
//! product over one variable's neighbours rather than an elimination. A second implementation would
//! be a second thing to keep in parity, and this workspace has paid for cross-implementation parity
//! once already.
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
//! The 43.11 pass does not change that number, and the reason is the same one: an abstract
//! interpretation abstracts something, and every one of the six region factors abstracts to `⊤`
//! because there is no potential to read. The pass therefore refuses on its hypothesis rather than
//! reading a bound of one off `⊤` — which matters, because `⊤` reads out as one, a bound of one is
//! `Bounded`, and six vacuous `Bounded` groups would formally support the sufficiency claim that
//! six honest `Unknown` groups correctly void. A registry does not fix a schema gap. Zero is still
//! the finding.
//!
//! ## What is not implemented
//!
//! See [`NOT_IMPLEMENTED`]. The short version: no Shapley-style attribution, no probabilistic or
//! sampled bounds, no ordering on domains and so no domain-level refinement, no relational domain,
//! no decision-loss metric, no branching contraction, and no additive or structural perturbation
//! class.
//!
//! ## Where §43 names a technique without specifying it
//!
//! 43.28 requires the compiler to "compute exact or conservative influence bounds" and lists
//! "metric, method, confidence, and validity scope" in the runtime contract — and specifies none
//! of the four. It requires step 3, "propagate bounds through the decision algebra", and gives no
//! composition rule. It lists "bound tightness" in the evaluation program and defines no tightness
//! statistic. 43.11 gives the soundness condition `f(γ(a)) ⊆ γ(f#(a))` and names no concrete
//! domain, no lattice, no widening operator, and no meaning for the refinement its scheduler is
//! supposed to schedule. Every one of those is a decision of this implementation, made in the open
//! in the module that makes it, and none of them is presented as specification.

pub mod analysis;
pub mod bound;
pub mod bruteforce;
pub mod contraction;
pub mod domain;
pub mod domains;
pub mod error;
pub mod exact;
pub mod federated_continual_interpretation_gateway;
pub mod gibbs;
pub mod interpret;
pub mod manifest;
pub mod measure;
pub mod perturbation;
pub mod perturbed;
pub mod ratio;
pub mod reference;
pub mod registry;
pub mod rng;
pub mod smallworld;
pub mod solver;

pub use analysis::{
    chain_of, dynamic_range_bound, structural_zero, InfluenceAnalysis, InfluenceAnalyzer,
    MethodOutcome,
};
pub use domain::{
    laws, AbstractDomain, DomainError, DomainId, EnumerableConcretisation, FactClass,
};
pub use domains::{
    Displacement, DisplacementDomain, ProductDomain, RatioInterval, RatioIntervalDomain, Support,
    SupportDomain,
};
pub use gibbs::{comparison_system, ComparisonSystem, MAX_CONDITIONAL_CONFIGURATIONS};
pub use interpret::{
    interpret, interpret_with_standard_domains, region_support, AbstractInterpretation,
};
pub use registry::{AbstractValue, DomainRegistry, ErasedDomain};
pub use solver::{
    ascend_by_join_only, join_iterates, solve, Convergence, FixedPoint, RefinementSchedule,
};
pub use bound::{Approximation, BoundMethod, InfluenceBound, InfluenceEstimate, InfluenceMetric};
pub use bruteforce::{maximum_influence, BruteForceResult, MAX_PERTURBATION_VERTICES};
pub use contraction::{dobrushin_coefficients, ChainStructure};
pub use error::{InfluenceError, UnknownReason};
pub use exact::{exact_group_removal_influence, exact_removal_influence};
pub use federated_continual_interpretation_gateway::{
    federated_continual_interpretation_manifest, run_federated_continual_interpretation,
    EvidenceBackedResult4, FederatedInterpretationError, GatewayDisposition,
    InfluenceObservation, InterpretationClaim, InterpretationFactor, InterpretationVariable,
    InteractiveInterpretation, MethodObservation, PeerCapability, InterpretationView,
    CONTRACT_VERSION as FEDERATED_CONTINUAL_INTERPRETATION_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTINUAL_INTERPRETATION_FEATURE_ID,
    INPUT_SCHEMA as FEDERATED_CONTINUAL_INTERPRETATION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_CONTINUAL_INTERPRETATION_OUTPUT_SCHEMA,
};
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
        "an ordering on domains, and therefore domain-level refinement",
        "43.11 asks for a refinement scheduler and does not say what refinement refines. The iterate is refined: `RefinementSchedule` budgets joins before widening and descending steps after it. The *domain* is not. Recomputing a result that stayed at top in a more precise abstraction — a disjunctive completion, a partition, a relational domain over several sites at once — needs an ordering on abstractions that 43.11 neither supplies nor implies, and `DomainRegistry::abstracting` deliberately returns an unordered list rather than inventing one.",
    ),
    (
        "a relational domain, and therefore any correlation between sites",
        "every domain here is non-relational: the product domain carries one independent element per variable and cannot express `u_a + u_b <= 1`. That is the standard first loss of precision in an interval analysis, and on the Dobrushin accumulation it shows up as the union bound over paths that a relational domain would tighten.",
    ),
    (
        "an open transformer table",
        "the registry is open — a caller may register a domain of its own and the lattice operations will be dispatched to it — and the *analysis* is not. `interpret` builds its transformer for one domain of `FactClass::AnswerDisplacement` and returns `DomainError::NoTransformerForDomain` for any other registered member of that class. A new domain is therefore usable for its own lattice and not yet as a target for the Gibbs comparison analysis.",
    ),
    (
        "branching contraction",
        "the Dobrushin *coefficient* argument of `contraction` extends to trees unchanged and is not implemented for them. The comparison theorem of `gibbs` covers trees and cycles alike, so the gap is a tighter bound on a class that already has a looser one, not an unbounded class.",
    ),
    (
        "a decision-loss metric",
        "43.10 and 43.12 are defined relative to permitted actions and a decision loss, and `fiber-query/0.1` declares neither. A bound on the change in an answer distribution is not a bound on the change in a decision's cost, and this crate does not claim to convert one into the other.",
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
/// Two clauses, `formal influence bounds` and `abstract interpretation`, and both become false
/// *conditionally* and only after change (3). The replacement string has to say the condition,
/// because on the world as shipped both capabilities return unknown influence. Proposed:
///
/// > Reference slicer uses dependency reachability and protected tags; it does not yet implement
/// > sheaf cohomology or FAQ-width optimization. Abstract interpretation runs over a registry of
/// > four abstract domains with join, widening and a bounded narrowing schedule, and formal
/// > influence bounds are computed, wherever a factor valuation or a declared perturbation range
/// > exists; both are reported as unknown influence otherwise, which on this world is every
/// > omitted group.
///
/// The trailing clause is not decoration. A limitation string that shrank by two clauses while the
/// certificate it appears on gained no bound would be a worse sentence than the one it replaced,
/// and this workspace's whole claim is that it does not write those.
///
/// ## What stays true, and must
///
/// - **Sheaf cohomology (43.06).** Untouched. Nothing here computes an obstruction class, and
///   `fiber-world/0.1` carries no cover to compute one over.
/// - **FAQ-width optimization (43.18, 43.19).** Untouched *by this crate*. It exists in
///   `bioprism-backends` and `bioprism-fiber` does not call it; that is a wiring gap between two
///   other crates and closing it is not this crate's claim to make.
/// - **Abstract interpretation (43.11).** Stays true *of the reference slicer*, and for a reason
///   that has changed. The previous reason was that the ingredients did not exist: one interval
///   domain with one transformer is not a registry, a join, a widening and a refinement scheduler.
///   They exist now — [`registry::DomainRegistry`] with three domains and a product construction,
///   [`domains`] with a join proved against concretisation on the finite one, [`solver`] with a
///   widening that terminates a chain no number of joins reaches and a narrowing schedule that
///   recovers what it gave away, and [`mod@interpret`] running all of it over a soundness argument that
///   covers cyclic regions [`contraction`] refuses. The remaining reason is narrower and harder:
///
///   1. **`bioprism-fiber` does not call it.** The limitation string is a sentence about what the
///      reference slicer does, and until the pass of change (3) runs, the slicer does not do this.
///      No amount of capability in a crate `fiber` does not invoke makes that sentence false.
///   2. **There is nothing in this world to abstract.** Every one of the six region factors
///      abstracts to `⊤` in [`domains::SupportDomain`], because `fiber-world/0.1` declares no
///      potentials. The pass refuses on that hypothesis rather than reading a bound of one off `⊤`.
///
///   The half of `bioprism-fiber`'s own deferral reason that says *"43.11 requires an abstract-domain
///   registry absent from fiber-world/0.1"* is now misattributed and should be restated when the
///   pass lands: a registry is a compiler-side table of abstractions, not a wire-schema field, and
///   one exists. What is absent from `fiber-world/0.1` is the potential, and that is the accurate
///   blocker — the same one `bioprism-examples` should carry for `bounded_influence_omission`.
///
///   After change (3) the accurate clause is conditional in the same shape as the one for formal
///   influence bounds, and the proposed replacement string below says so.
/// - **And the honest caveat on the win itself.** On the world as shipped the new pass emits no
///   bound at all. The capability is real, tested and sound; the reference certificate will not
///   change until a world carries potentials or a query declares ranges. A limitation string that
///   shrinks while the certificate it appears on gains nothing would be worse than one that stays.
///
/// ## The two blocked claims in `bioprism-examples`
///
/// `Property::BoundedInfluenceOmission`'s blocker reads *"bioprism-fiber emits only
/// `InfluenceClass::Zero` and `DeferredAcquisition`; nothing computes a numeric influence bound,
/// so no group is ever `Bounded`."* After integration the second half is false and the first half
/// is still true on this world. The accurate replacement blocker is that `fiber-world/0.1`
/// declares factor signatures and never potentials, so the computed bound is `Unknown` for every
/// group in the shipped fixture — a schema gap, with a named fixture that would unblock it.
///
/// `Property::AbstractInterpretation`'s blocker reads *"fiber-world/0.1 carries no abstract-domain
/// registry; bioprism-fiber lists abstract_interpretation as a deferred pass on every compile."*
/// Its second half is exactly true and is the operative one. Its first half is now wrong twice
/// over: a registry is not a thing a wire schema carries, and one exists, in
/// [`registry::DomainRegistry`], with a join proved against concretisation, a widening that
/// terminates a chain no number of joins reaches, and a soundness suite against brute force. The
/// accurate replacement blocker is the same schema gap as above — `fiber-world/0.1` declares no
/// potentials, so every factor abstracts to `⊤` and the pass refuses rather than reporting the
/// vacuous bound `⊤` would read out as.
pub const INTEGRATION_NOTE: &str = concat!(
    "bioprism-fiber would: (1) depend on bioprism-influence; ",
    "(2) build a QueryRegion with QueryRegion::from_world_slice over the same targets it passes to backward_slice; ",
    "(3) run an `influence_bounds` pass after plan_selection using InfluenceAnalyzer::structural_only, so compile does not execute the query it is compiling; the 43.11 abstract-interpretation pass runs inside that analyzer and executes nothing either; ",
    "(4) pass the analyses into build_manifest and construct groups with omission_group_from_analysis; ",
    "(5) split the deferred group rather than promoting it, so a temporally withheld fact that is also bounded does not lose its refinement-frontier entry. ",
    "The clauses `formal influence bounds` and `abstract interpretation` then become false, conditionally: bounds exist where a factor valuation or a declared perturbation range exists, and are reported as unknown influence otherwise. ",
    "Sheaf cohomology and FAQ-width optimization remain true of the reference slicer. ",
    "Until step (3) lands, `abstract interpretation` also remains true of it, because the limitation string describes what the slicer does and the slicer does not yet call the pass. ",
    "And on the world as shipped the pass emits no bound at all, because fiber-world/0.1 declares factor signatures and never potentials, so every factor abstracts to top."
);
