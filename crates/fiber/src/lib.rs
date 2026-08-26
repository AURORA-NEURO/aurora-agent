//! The FIBER query compiler.
//!
//! Implements blueprint 43.13 (protected closure), 43.17 (dependency slicing and obligation
//! closure), 43.09 (temporal accessibility), 43.16 (the compiler pipeline), the fragment of 43.33
//! (policy fibers) the v0.1 wire formats can state, the portfolio consultation of 43.36 and 43.37,
//! the influence bounds of 43.28, the deterministic oracle of 43.41, and the bounded adaptive
//! acquisition planner of 43.15, emitting the Decision
//! Section and Context Certificate defined in `bioprism-section`.
//!
//! ```no_run
//! use bioprism_fiber::{compile, Query};
//! use bioprism_world::World;
//!
//! let world = World::from_json(serde_json::from_str(&std::fs::read_to_string("world.json")?)?)?;
//! let query = Query::from_json(serde_json::from_str(&std::fs::read_to_string("query.json")?)?)?;
//! let out = compile(&world, &query)?;
//! println!("{}", out.certificate.digest(bioprism_section::CertificateProfile::Reference)?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! What this engine does *not* do is as important as what it does. It performs no gluing or
//! obstruction analysis or abstract interpretation. The versioned `fiber-query/0.3` boundary
//! carries the explicit permitted actions and decision loss needed by the 43.10 quotient,
//! `fiber-query/0.4` additionally executes the bounded observed-context rate-distortion audit,
//! and `fiber-query/0.5` executes the exact adaptive acquisition policy contract.
//! Older queries continue to report whichever decision-relative pass their wire form cannot state
//! as deferred.
//! Every compile reports the remaining gaps in [`CompileTrace::deferred_passes`] and on the
//! certificate's `limitations`.
//!
//! The [`policy`] pass is the same story told at one level of detail rather than none: it enforces
//! the clause grants the wire formats *can* express and names, in its module documentation, the
//! six 43.33 mechanisms they cannot. `bioprism-policy` is where the full fiber lives, and this
//! crate deliberately does not depend on it.
//!
//! [`plan`] and [`influence`] are the same shape again, and both come out against the engine. The
//! backend portfolio of `bioprism-backends` is now consulted on every compile and its argmin is
//! real — 1737x predicted on the reference world — but no member can *execute* a region whose
//! factors carry no potential, so the certificate keeps naming the backward slice that produced
//! the section. The influence bounds of `bioprism-influence` are computed on every compile and
//! return `Unknown` for the same reason. Both report their finding on [`CompileTrace`]; neither
//! changes a byte of the certificate on any world `fiber-world/0.1` can state, and the modules say
//! so rather than leaving a reader to infer it from a digest that did not move.
//!
//! ## What the zero-influence group is, and what it is not
//!
//! The omitted population is *counted* rather than enumerated — `total_facts - |selection|` — so
//! that compile cost tracks the compiled region rather than the corpus (43.34). A consequence is
//! that [`bioprism_section::InfluenceClass::Zero`] is arrived at as a remainder: every population
//! known not to be zero is subtracted, and what is left is published as provably unable to move the
//! decision. That is sound exactly to the extent that the subtracted populations are exhaustive,
//! and it is where the strongest claim on the certificate is at its weakest.
//!
//! One population was demonstrably missing from the subtraction and is now computed structurally.
//! A fact shadowed by a later fact providing the same variable has a backward dependency path to
//! the target whenever the slice needs that variable, so its omission is a document-order tiebreak
//! and not a proof; it used to be published in the zero group with a bound of `0.0`.
//! [`bioprism_world::WorldSource::shadowed_provider_ids`] makes it visible per variable, the
//! compiler enumerates it — the population is bounded by the compiled region, so naming its members
//! costs nothing the design forbids — and it is classified
//! [`bioprism_section::InfluenceClass::Unknown`], which voids the sufficiency claim.
//! [`bioprism_section::ProvenUnreachable`] is the type that forces the subtraction to be named at
//! the point the zero count is minted.
//!
//! What that does **not** establish is that the remainder is now proven per fact. It is still a
//! remainder, and a population nobody has thought of would still land in it silently. The honest
//! statement of the current guarantee is: no omission the compiler can *see* a dependency path for
//! is classified zero.
//!
//! The argument for reading the remainder as a per-fact proof gets close enough to be worth
//! stating together with the step that defeats it. It runs: under `fiber-world/0.1` a fact's only
//! edge into the factor graph is the single variable it provides; the facts providing a needed
//! variable are exactly the winner plus its displaced providers, both nameable in one index lookup
//! per needed variable, and both already classified — the winner into the policy or deferred group
//! when something removed it, every displaced one into the unproven group; so what is left provides
//! no *needed* variable. All of that holds. What does not hold is the last step, that providing no
//! needed variable is the same as having no path. A factor may declare several outputs and the
//! backward slice enters it through one of them, so a *sibling* output of a selected factor never
//! becomes a needed variable and is never examined here — while
//! [`bioprism_backends::QueryRegion::from_world_slice`] puts that same variable in that same
//! factor's scope, and [`influence`] treats scope membership as exactly the relation that makes a
//! withholding perturbable rather than zero-influence. Compiling
//! `reference/fiber_runtime/examples/multi_output_world.json` with a fact added for
//! `calibration_drift` publishes that fact in the zero group with a bound of `0.0` while
//! `factor.joint_readout` — named on the same certificate as a selected factor — carries
//! `calibration_drift` in its scope. `crates/fiber/tests/omission_accounting.rs` pins that as a
//! known misclassification so it is a recorded limit rather than something to be rediscovered.
//!
//! Two further conditions are load-bearing and neither is checked on the compile path. Fact
//! identifiers must be unique, because every population downstream of the slice is a set of
//! identifiers and two facts under one identifier make that classification not a partition of
//! facts. And the declared factor graph must carry every dependency the decision has, which is the
//! assumption the zero group's own reason string states.
//!
//! What the accounting does earn is arithmetic that is checked rather than assumed.
//! [`bioprism_section::ProvenUnreachable`] takes the classified populations themselves and does
//! every subtraction itself, so a fact named under two reasons or a classification larger than the
//! corpus is a refusal rather than a smaller number, and the compiler publishes the remainder as
//! [`bioprism_section::InfluenceClass::Unknown`] with the refusal on the record.
//! [`CompileTrace::unproven_remainder`] says which check declined, because a certificate carrying no
//! zero group otherwise looks exactly like one whose corpus had nothing to prove.
//!
//! Nor does any of it establish anything about a source whose aggregate disagrees with its own
//! records. `omitted = total_facts - |selection|` is taken from the source, and a source
//! reporting more facts than its indices can name inflates the remainder by the difference, which
//! lands in the proven group and is undetectable without the corpus walk 43.34 forbids. The eager
//! [`bioprism_world::World`] derives count and indices from one vector and cannot disagree with
//! itself; `bioprism_store::LazyWorld` reads the count from a manifest written beside indices it
//! never re-derives, and for that path this is an assumption about the store's integrity rather
//! than a property the compiler checks. Two shapes of that disagreement the compiler *can* see, and
//! both refuse the proof outright rather than shrinking the count: a displaced provider reported
//! under the winner's own identifier, which no identifier-keyed classification can tell apart from
//! a delivered fact, and one identifier reported for the displaced providers of two needed
//! variables, which [`bioprism_section::ProvenUnreachable::from_classified`] rejects as a fact named
//! twice.

pub mod closure;
pub mod compile;
pub mod error;
pub mod influence;
pub mod oracle;
pub mod plan;
pub mod policy;
pub mod qir;
pub mod slice;
pub mod temporal;

pub use compile::{
    compile, compile_with_oracle, AdaptiveAcquisitionTrace, CompileOutput, CompileTrace,
    PassReceipt, RateDistortionTrace, UnprovenRemainder,
};
pub use error::FiberError;
pub use influence::{CorrespondenceCheck, NotPosable, WithheldSplit, WithholdingAnalysis};
pub use oracle::{DecisionOracle, SplitIntegrityOracle, ORACLE_KIND};
pub use plan::{PlanEvaluation, PortfolioOutcome, RegionStatistics, DELIVERING_BACKEND};
pub use policy::{PolicyEnvelope, PolicyOutcome, PolicyScreen, PolicyViolation};
pub use qir::{
    AdaptiveAcquisitionContract, Budgets, DecisionContract, DecisionSense, Query,
    RateDistortionContract, ACCEPTED_QUERY_SCHEMA_VERSIONS, NO_DECLARED_GOAL,
    QUERY_ADAPTIVE_FIELD_PATHS, QUERY_ADAPTIVE_SCHEMA_VERSION, QUERY_DECISION_FIELD_PATHS,
    QUERY_DECISION_SCHEMA_VERSION, QUERY_FIELD_PATHS, QUERY_RATE_DISTORTION_FIELD_PATHS,
    QUERY_RATE_DISTORTION_SCHEMA_VERSION, QUERY_SCHEMA_VERSION, REFERENCE_GOAL,
};
pub use slice::{backward_slice, Slice};
pub use temporal::{temporal_cut, TemporalCut};
