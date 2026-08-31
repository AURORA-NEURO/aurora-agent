#![allow(clippy::all)]

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
//! Two populations were demonstrably missing from the subtraction and are now computed
//! structurally, one per relation that can carry an omission to a target.
//!
//! The first is the backward slice. A fact shadowed by a later fact providing the same variable has
//! a backward dependency path to the target whenever the slice needs that variable, so its omission
//! is a document-order tiebreak and not a proof; it used to be published in the zero group with a
//! bound of `0.0`. [`bioprism_world::WorldSource::shadowed_provider_ids`] makes it visible per
//! variable, the compiler enumerates it — the population is bounded by the compiled region, so
//! naming its members costs nothing the design forbids — and it is classified
//! [`bioprism_section::InfluenceClass::Unknown`], which voids the sufficiency claim.
//! [`bioprism_section::ProvenUnreachable`] is the type that forces the subtraction to be named at
//! the point the zero count is minted.
//!
//! The second is scope membership, and it is the one the first relation looks like it subsumes. The
//! argument that it does runs: under `fiber-world/0.1` a fact's only edge into the factor graph is
//! the single variable it provides; the facts providing a needed variable are exactly the winner
//! plus its displaced providers, both nameable in one index lookup per needed variable, and both
//! already classified; so what is left provides no *needed* variable. All of that holds. What does
//! not hold is the last step, that providing no needed variable is the same as having no path. A
//! factor may declare several outputs and the backward slice enters it through one of them, so a
//! *sibling* output of a selected factor never becomes a needed variable — while
//! [`bioprism_backends::QueryRegion::from_world_slice`] puts that same variable in that same
//! factor's scope, and [`influence`] treats scope membership as exactly the relation that makes a
//! withholding perturbable rather than zero-influence, reporting its absence as
//! `NotPosable::OutsideCompiledRegion` and documenting that as not zero influence. A certificate
//! calling such a fact provably irrelevant would contradict the region it ships beside.
//!
//! So the compiler reads the compiled region's factor scopes — the same [`bioprism_backends::QueryRegion`]
//! it hands to [`influence`], rather than a second derivation of what a factor's scope is — and any
//! omitted fact providing a carried variable the slice did not need is classified
//! [`bioprism_section::InfluenceClass::Unknown`] with the variable and the carrying factor named in
//! the reason. Both providers are examined, not just the displaced ones: nothing selected any
//! provider of a variable outside the needed set, so the tiebreak winner is omitted too and was the
//! fact actually being published with a bound of `0.0`. The work is bounded by the region's scopes
//! and two index lookups per carried variable, so the pass stays output-sensitive and no traversal
//! of the omitted corpus is introduced. When no region could be built the pass has nothing to read,
//! and a compile in that state declines the proof and says the region is missing rather than
//! falling back to the answer the pass exists to prevent.
//!
//! What none of that establishes is that the remainder is now proven per fact. It is still a
//! remainder, and a population nobody has thought of would still land in it silently. The honest
//! statement of the current guarantee is: no omission the compiler can *see* a path to the
//! decision for — by dependency or by scope — is classified zero.
//!
//! Neither the temporal cut nor the policy screen can deposit a fact in the proven remainder. The
//! reason is narrower than the one that suggests itself, and the wider one is false: the selection
//! is not keyed on the needed set alone. It is the needed set's providers *unioned with the
//! protected closure*, so a fact carrying one of the query's protected tags is selected whatever
//! variable it provides, sibling outputs included, and the cut can then remove it. What survives
//! without that precondition is the weaker half of the argument, and it is the half that carries
//! the guarantee: every fact either pass removes is named explicitly into
//! [`bioprism_section::InfluenceClass::InaccessibleByPolicy`] or into the bounded or deferred
//! group, so none of them is left for the remainder to absorb.
//!
//! Which class a carried sibling output lands in does depend on that precondition. Carrying no
//! protected tag, nothing selects it — selection is otherwise keyed on the needed set and a
//! sibling output is never needed — so it is reported as unknown-because-carried even when an
//! unreleased event governs it, naming the structural reason the compiler checked rather than one
//! it never evaluated. Protected, the same fact is selected, the cut removes it, and it is
//! reported as [`bioprism_section::InfluenceClass::DeferredAcquisition`] instead. The policy screen
//! has no third answer here: a sibling output's provider reaches the screen only through the
//! protected closure, and a protected fact the screen would withhold is
//! [`crate::policy::PolicyViolation::ProtectedClosureWithheld`] — a refusal of the whole compile
//! rather than a removal. The two labels differ; both void the sufficiency claim, which is the
//! property the certificate rests on.
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
//! a delivered fact — on a needed variable or on one only the region carries, because two passes
//! reading the same collision may not reach two verdicts about it — and one identifier reported for
//! the displaced providers of two needed variables, which
//! [`bioprism_section::ProvenUnreachable::from_classified`] rejects as a fact named twice.

pub mod closure;
pub mod compile;
pub mod error;
pub mod federated_analysis_control_plane;
pub mod federated_continual_fibration_integrity_contract_model;
pub mod federated_continual_fibration_integrity_inference;
pub mod federated_continual_fibration_integrity_research_copilot;
pub mod federated_continual_fibration_integrity_workflow_fabric;
pub mod federated_execution_interoperability;
pub mod federated_protocol_simulation_assurance;
pub mod federated_resource_workbench;
pub mod fibration_integrity_support;
pub mod influence;
pub mod local_fibration_integrity_contract_model;
pub mod local_fibration_integrity_inference;
pub mod local_fibration_integrity_research_copilot;
pub mod local_fibration_integrity_workflow_fabric;
pub mod mechanism_assurance;
pub mod mechanism_contract_model;
pub mod mechanism_gateway;
pub mod multimodal_fibration_integrity_contract_model;
pub mod multimodal_fibration_integrity_inference;
pub mod multimodal_fibration_integrity_research_copilot;
pub mod multimodal_fibration_integrity_workflow_fabric;
pub mod oracle;
pub mod plan;
pub mod policy;
pub mod qir;
pub mod research_context;
pub mod resource_workbench;
pub mod retrieval_assurance;
pub mod semantic_parity_assurance;
pub mod slice;
pub mod temporal;
pub mod throughput_fibration_integrity_contract_model;
pub mod throughput_fibration_integrity_inference;
pub mod throughput_fibration_integrity_research_copilot;
pub mod throughput_fibration_integrity_workflow_fabric;

pub use federated_analysis_control_plane::{
    admit_federated_analysis, capability_manifest as federated_analysis_control_manifest,
    FederatedAnalysisCandidate8, FederatedAnalysisControlError, FederatedAnalysisControlReceipt,
    FederatedAnalysisControlRequest, CONTENT_TYPE as FEDERATED_ANALYSIS_CONTENT_TYPE,
    CONTRACT_VERSION as FEDERATED_ANALYSIS_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_ANALYSIS_FEATURE_ID, INPUT_SCHEMA as FEDERATED_ANALYSIS_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_ANALYSIS_OUTPUT_SCHEMA,
};

pub use compile::{
    compile, compile_with_oracle, AdaptiveAcquisitionTrace, CompileOutput, CompileTrace,
    PassReceipt, RateDistortionTrace, UnprovenRemainder,
};
pub use error::FiberError;
pub use federated_continual_fibration_integrity_contract_model::*;
pub use federated_continual_fibration_integrity_inference::*;
pub use federated_continual_fibration_integrity_research_copilot::*;
pub use federated_continual_fibration_integrity_workflow_fabric::*;
pub use federated_execution_interoperability::{
    assure as assure_federated_execution_interoperability,
    capability_manifest as federated_execution_interoperability_manifest,
    ExecutionArtifactCandidate, ExecutionInteroperabilityEnvelope, ExecutionInteroperabilityError,
    ExecutionInteroperabilityRequest,
    CONTRACT_VERSION as FEDERATED_EXECUTION_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_EXECUTION_INTEROPERABILITY_FEATURE_ID,
    INPUT_SCHEMA as FEDERATED_EXECUTION_INTEROPERABILITY_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_EXECUTION_INTEROPERABILITY_OUTPUT_SCHEMA,
};
pub use federated_protocol_simulation_assurance::{
    assure as assure_federated_protocol_simulation,
    capability_manifest as federated_protocol_simulation_manifest, PeerProtocolSummary,
    ProtocolDraft, ProtocolSimulationAssuranceError, ProtocolSimulationReport,
    CONTRACT_VERSION as FEDERATED_PROTOCOL_SIMULATION_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_PROTOCOL_SIMULATION_FEATURE_ID,
    INPUT_SCHEMA as FEDERATED_PROTOCOL_SIMULATION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_PROTOCOL_SIMULATION_OUTPUT_SCHEMA,
};
pub use federated_resource_workbench::{
    federated_resource_workbench_manifest, qualify_federated_resources,
    FederatedResourceCandidate5, FederatedResourceDiscoveryRequest7, FederatedResourceDisposition,
    FederatedResourceWorkbenchError, FederatedResourceWorkbenchReceipt8,
    CONTENT_TYPE as FEDERATED_RESOURCE_CONTENT_TYPE,
    CONTRACT_VERSION as FEDERATED_RESOURCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RESOURCE_FEATURE_ID, INPUT_SCHEMA as FEDERATED_RESOURCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_RESOURCE_OUTPUT_SCHEMA,
};
pub use fibration_integrity_support::{
    certify as certify_fibration_integrity, manifest as fibration_integrity_manifest, FiberRegion4,
    FibrationIntegrityArtifact4, FibrationIntegrityCard7, FibrationIntegrityError,
    FibrationIntegrityRequest4, BOUNDARY as FIBRATION_INTEGRITY_BOUNDARY,
    CONTENT_TYPE as FIBRATION_INTEGRITY_CONTENT_TYPE,
};
pub use influence::{CorrespondenceCheck, NotPosable, WithheldSplit, WithholdingAnalysis};
pub use local_fibration_integrity_contract_model::*;
pub use local_fibration_integrity_inference::*;
pub use local_fibration_integrity_research_copilot::*;
pub use local_fibration_integrity_workflow_fabric::*;
pub use mechanism_assurance::{
    assure as assure_mechanisms, capability_manifest as mechanism_assurance_manifest,
    AssuranceDisposition, CandidateState, MechanismAssuranceError, MechanismCandidate,
    MechanismPortfolio, MechanismQuestion,
    CONTRACT_VERSION as MECHANISM_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_ASSURANCE_FEATURE_ID,
};
pub use mechanism_contract_model::{
    mechanism_contract_model_manifest, model_mechanism_contract, MechanismContractCandidate,
    MechanismContractDisposition, MechanismContractError, MechanismPortfolioContract,
    MechanismQuestionContract, CONTRACT_VERSION as MECHANISM_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_CONTRACT_MODEL_FEATURE_ID,
    INPUT_SCHEMA as MECHANISM_CONTRACT_MODEL_INPUT_SCHEMA,
    OUTPUT_SCHEMA as MECHANISM_CONTRACT_MODEL_OUTPUT_SCHEMA,
};
pub use mechanism_gateway::{
    admit_mechanism_gateway, MechanismGatewayDisposition, MechanismGatewayError,
    MechanismGatewayReceipt, MechanismGatewayRequest,
    CONTRACT_VERSION as MECHANISM_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_GATEWAY_FEATURE_ID,
};
pub use multimodal_fibration_integrity_contract_model::*;
pub use multimodal_fibration_integrity_inference::*;
pub use multimodal_fibration_integrity_research_copilot::*;
pub use multimodal_fibration_integrity_workflow_fabric::*;
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
pub use semantic_parity_assurance::{
    assure_semantic_parity, semantic_parity_assurance_manifest, FiberParityCase,
    FiberParityFixture, FiberParityWitness, ParityDisposition, SemanticParityError,
    CONTRACT_VERSION as SEMANTIC_PARITY_CONTRACT_VERSION, FEATURE_ID as SEMANTIC_PARITY_FEATURE_ID,
    INPUT_SCHEMA as SEMANTIC_PARITY_INPUT_SCHEMA, OUTPUT_SCHEMA as SEMANTIC_PARITY_OUTPUT_SCHEMA,
};
pub use slice::{backward_slice, Slice};
pub use temporal::{temporal_cut, TemporalCut};
pub use throughput_fibration_integrity_contract_model::*;
pub use throughput_fibration_integrity_inference::*;
pub use throughput_fibration_integrity_research_copilot::*;
pub use throughput_fibration_integrity_workflow_fabric::*;
