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

pub mod closure;
pub mod compile;
pub mod error;
pub mod influence;
pub mod oracle;
pub mod plan;
pub mod policy;
pub mod qir;
pub mod research_context;
pub mod resource_workbench;
pub mod retrieval_assurance;
pub mod slice;
pub mod temporal;

pub use compile::{
    compile, AdaptiveAcquisitionTrace, CompileOutput, CompileTrace, PassReceipt,
    RateDistortionTrace,
};
pub use error::FiberError;
pub use influence::{CorrespondenceCheck, NotPosable, WithheldSplit, WithholdingAnalysis};
pub use plan::{PlanEvaluation, PortfolioOutcome, RegionStatistics, DELIVERING_BACKEND};
pub use policy::{PolicyEnvelope, PolicyOutcome, PolicyScreen, PolicyViolation};
pub use qir::{
    AdaptiveAcquisitionContract, Budgets, DecisionContract, DecisionSense, Query,
    RateDistortionContract, ACCEPTED_QUERY_SCHEMA_VERSIONS, QUERY_ADAPTIVE_FIELD_PATHS,
    QUERY_ADAPTIVE_SCHEMA_VERSION, QUERY_DECISION_FIELD_PATHS, QUERY_DECISION_SCHEMA_VERSION,
    QUERY_FIELD_PATHS, QUERY_RATE_DISTORTION_FIELD_PATHS, QUERY_RATE_DISTORTION_SCHEMA_VERSION,
    QUERY_SCHEMA_VERSION, REFERENCE_GOAL,
};
pub use research_context::{
    compile_research_context, research_context_manifest, ResearchContextError,
    ResearchContextReceipt, ResearchContextRequest,
};
pub use resource_workbench::{
    discover_resources, resource_workbench_manifest, QualifiedResource, QualifiedResourceSet,
    ResourceAvailability, ResourceCandidate, ResourceDiscoveryDisposition, ResourceNeed,
    ResourceOmission, ResourceWorkbenchError, FEATURE_ID as RESOURCE_WORKBENCH_FEATURE_ID,
    FEATURE_VERSION as RESOURCE_WORKBENCH_FEATURE_VERSION,
};
pub use retrieval_assurance::{
    assure_federated_retrieval, FederatedRetrievalAssuranceReceipt,
    FederatedRetrievalAssuranceRequest, RetrievalAssuranceDisposition, RetrievalAssuranceError,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID,
};
pub use slice::{backward_slice, Slice};
pub use temporal::{temporal_cut, TemporalCut};
