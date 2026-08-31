//! The nine §40 service contracts as types, and the workspace held against them.
//!
//! Implements blueprint 40.03 (Service and Process Graph), 40.04 (Domain Boundaries and
//! Ownership), 40.18 (BioWorld Builder Service), 40.20 (BioDecision Compiler Service), 40.22
//! (Mutation Runtime and Lineage), 40.23 (Matched Evaluator Runtime), 40.24 (Adaptive Scheduler
//! and Suite Selection), 40.25 (Context Compiler Service Contract) and 40.27 (Registry and Hub
//! Backend), under the conventions of 40.11 (API Conventions and Idempotency) and the taxonomy of
//! 40.36 (Error Taxonomy, Exit Codes and Retry Semantics).
//!
//! # Why this crate exists
//!
//! The workspace already implements most of these services. `bioprism-fiber` is the context
//! compiler, `bioprism-worldgen` the builder, `bioprism-benchcompiler` the decision compiler,
//! `bioprism-mutation` the mutation runtime, `bioprism-prism` the matched evaluator,
//! `bioprism-adaptive` the scheduler, `bioprism-registry` the backend. What had never been done is
//! writing the contracts down as types and checking that the implementations satisfy them.
//!
//! Nobody had checked. So the deliverable is not the types; it is [`audit`], and the answer is that
//! **all nine diverge**, with 59 named divergences between them. That result ships as it is,
//! because the alternative — quietly adjusting the transcription until the code passes — would
//! produce a contract that certifies whatever exists.
//!
//! | § | contract | crates | verdict | divergences |
//! |---|---|---|---|---:|
//! | 40.03 | Service and Process Graph | — | diverges | 2 |
//! | 40.04 | Domain Boundaries and Ownership | — | diverges | 4 |
//! | 40.18 | BioWorld Builder Service | `worldgen`, `world` | diverges | 16 |
//! | 40.20 | BioDecision Compiler Service | `benchcompiler` | diverges | 6 |
//! | 40.22 | Mutation Runtime and Lineage | `mutation` | diverges | 6 |
//! | 40.23 | Matched Evaluator Runtime | `prism`, `evalengine` | diverges | 8 |
//! | 40.24 | Adaptive Scheduler and Suite Selection | `adaptive` | diverges | 5 |
//! | 40.25 | Context Compiler Service Contract | `fiber`, `section` | diverges | 9 |
//! | 40.27 | Registry and Hub Backend | `registry`, `hub` | diverges | 3 |
//!
//! [`audit::markdown_table`] regenerates that table from the code, so it cannot drift from the
//! verdicts. The four kinds of divergence, and which of them are the code's fault, are set out at
//! [`audit`].
//!
//! # Three findings worth reading before the rest
//!
//! **Six of the seven operation contracts put an artifact write inside the service, and no crate in
//! this workspace does it.** "Emit world release", "package and validate cell", "bundle",
//! "validate/sign artifacts", "publish" — every one of these places persistence inside the
//! operation. Every crate here returns its artifacts as values and leaves the write to its caller,
//! which is what lets them be tested without a store and run in the in-process alpha topology 40.03
//! insists on. This is a uniform defect in the contracts, not seven defects in the code.
//!
//! **The nine do not close under their own dependency lists.** 40.18, 40.20, 40.22 and 40.23 each
//! name an oracle dependency; the oracle runtime is 40.21 and is not one of the nine. Four of the
//! eleven services the dependency lists imply answer no contract at all, so six calls in
//! [`workspace::service_graph`] cross a domain boundary with nothing to cross it under.
//!
//! **40.25's most load-bearing input is parsed and discarded.** `bioprism-fiber` reads `policy` off
//! the query into `Query::policy` and no compiler pass touches it. The contract's `policy conflict`
//! failure therefore cannot fire, and its second invariant — learned ranking cannot bypass policy
//! or closure — has nothing to enforce.
//!
//! # How much of §40 is template
//!
//! `docs/COVERAGE.md` records that §40 is the only build-ready section, so it is worth knowing how
//! much of it is content. Measuring a line as boilerplate when its exact text appears in at least
//! half of §40's 45 modules:
//!
//! | | non-empty lines | shared | boilerplate |
//! |---|---:|---:|---:|
//! | §40 entire | 3806 | 1400 | **36.8%** |
//! | these nine | 770 | 293 | **38.1%** |
//! | the other 36 | 3036 | 1107 | 36.5% |
//!
//! So the nine are *slightly denser in template* than the §40 average, not less. The split inside
//! them is the interesting part: 40.03 and 40.04 are hand-written and sit at 28%, while the seven
//! that use the progressive-disclosure template sit between 38.2% and 39.8% — within 1.6 points of
//! each other, which is what a template looks like. Roughly 57 lines per module are unique to it,
//! and those 57 lines are the whole specification: four invariant sentences, four input names, four
//! output names, five failure phrases. Everything a request or response actually needs — field
//! types, cardinalities, identity rules, which failures are retryable — is absent from all seven.
//! §40 is build-ready in the sense that it decides things, not in the sense that you can build from
//! it without deciding more.
//!
//! # What is deliberately not implemented
//!
//! - **No server, no transport, no HTTP, no network.** These are contracts, not endpoints. 40.11
//!   names REST, OpenAPI and server-sent events and none appears here, because 40.03 requires the
//!   same services to run in-process locally, and a contract expressed as HTTP would be false for
//!   that case. Nothing in this crate opens a socket, routes a path, or serialises a response to a
//!   client, and no type here should be read as serving traffic.
//! - **No second classifier.** A contract change is classified by `bioprism-governance` —
//!   [`contract::ContractChange`] computes two schema diffs and takes the worse. The rule that a
//!   change moving a digest is breaking however innocent it looks lives there and is not restated.
//! - **No clock, no retries, no scheduling.** [`error::Retryability`] is the decision; acting on it
//!   belongs to whoever owns the effect.
//! - **No conformance execution.** [`conformance::ServiceReport`] is a reading of source code, not
//!   an observation of a running service. `bioprism-conformance` owns suite execution and is
//!   neither imported nor reimplemented.
//! - **No authentication, authorisation or tenancy.** 40.11 depends on 40.12; 40.12 is not one of
//!   the nine and inventing it would be transcription fiction.
//! - **No signatures.** 40.23 and 40.27 both ask for signed bundles. There is no key material
//!   anywhere in this workspace, so the audit records "attested, not signed" rather than a type
//!   that would imply otherwise.
//!
//! # Where the blueprint is silent
//!
//! Stated in the open so a reader can disagree rather than discover.
//!
//! 40.36 defines error classes and the seven modules list failures as bare noun phrases; nothing
//! connects them, so [`catalog`] binds each phrase to one class and says which bindings are
//! arguable. 40.03 does not distinguish a call from the result returning along it, so
//! [`graph::EdgeKind`] adds the distinction its own alpha diagram needs. 40.04's "does not own"
//! column mixes concerns owned elsewhere with prohibited behaviours, so [`graph::Disclaimer`]
//! splits them and only the first kind is checkable. None of these is in the blueprint; all three
//! were needed to make it consistent with itself.

pub mod audit;
pub mod catalog;
pub mod conformance;
pub mod contract;
pub mod error;
pub mod federation;
pub mod graph;
pub mod implementations;
pub mod mechanism_workbench;
pub mod research;
pub mod research_release;
pub mod research_release_batch;
pub mod research_release_federated_inference_engine;
pub mod topology;
pub mod workspace;

pub use audit::{audit, AuditEntry, AuditSummary};
pub use conformance::{Conformance, ConformanceCheck, ServiceReport};
pub use contract::{
    ContractChange, ContractId, Delivery, Effect, Idempotency, ServiceContract, VERSION_FIELD,
};
pub use error::{ErrorClass, FailureMode, Invalidates, Retryability, ServiceFault, ServicesError};
pub use federation::{
    verify_signed_federation, FederationError, FederationSigner, SignedFederationArtifact,
};
pub use graph::{
    Call, Concern, Disclaimer, Domain, EdgeKind, GraphError, Ownership, ServiceGraph, ServiceId,
    ServiceNode,
};
pub use mechanism_workbench::{
    mechanism_workbench_manifest, run_mechanism_workbench, CandidateState, MechanismCandidate,
    MechanismWorkbenchDisposition, MechanismWorkbenchError, MechanismWorkbenchReport,
    MechanismWorkbenchRequest, CONTRACT_VERSION as MECHANISM_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_WORKBENCH_FEATURE_ID,
};
pub use research::{EvidenceWorkflowResult, ResearchServiceError, ResearchWorkflowService};
pub use research_release::{
    build_research_release, research_release_manifest, verify_research_release,
    ResearchReleaseError, ResearchReleaseReceipt, ResearchReleaseRequest, SignedResearchObject,
    FEATURE_CONTRACT_VERSION as RESEARCH_RELEASE_FEATURE_VERSION,
    FEATURE_ID as RESEARCH_RELEASE_FEATURE_ID,
};
pub use research_release_batch::{
    build_research_release_batch, research_release_batch_manifest, ResearchReleaseBatchDisposition,
    ResearchReleaseBatchEntry, ResearchReleaseBatchError, ResearchReleaseBatchReceipt,
    ResearchReleaseBatchRequest, FEATURE_ID as RESEARCH_RELEASE_BATCH_FEATURE_ID,
    FEATURE_VERSION as RESEARCH_RELEASE_BATCH_FEATURE_VERSION,
};
pub use research_release_federated_inference_engine::{
    federated_publication_release_inference_manifest, infer_federated_publication_release,
    FederatedPublicationReleaseInferenceError,
    FederatedPublicationReleaseInferenceReceipt,
    FederatedPublicationReleaseInferenceRequest,
    PublicationReleaseInferenceCandidate, PublicationReleaseInferenceDecision,
    FEATURE_ID as FEDERATED_PUBLICATION_RELEASE_INFERENCE_FEATURE_ID,
    FEATURE_VERSION as FEDERATED_PUBLICATION_RELEASE_INFERENCE_VERSION,
};
pub use topology::{Deployment, Placement, Topology, TopologyError};

/// The measured share of these nine §40 modules that is template rather than content.
///
/// A line counts as template when its exact text appears in at least half of §40's 45 modules. See
/// the crate docs for the method and the comparison against the section as a whole.
pub const NINE_BOILERPLATE_PERCENT: f64 = 38.1;

/// The same measure over all 45 modules of §40.
pub const SECTION_40_BOILERPLATE_PERCENT: f64 = 36.8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn these_nine_modules_carry_more_template_than_the_section_average_not_less() {
        let excess = NINE_BOILERPLATE_PERCENT - SECTION_40_BOILERPLATE_PERCENT;
        assert!(
            excess > 0.0,
            "the nine were expected to be denser in content than §40 as a whole and measured 1.3 \
             points more template instead; the measurement is what ships"
        );
    }

    #[test]
    fn the_crate_re_exports_every_type_a_caller_needs_to_state_a_verdict() {
        let entry = audit()
            .into_iter()
            .find(|entry| entry.contract == ContractId::ContextCompiler)
            .expect("audited");
        assert_eq!(entry.verdict, Conformance::Diverges);
        assert_eq!(entry.module_id, "40.25");
    }
}
pub mod context_compilation_research_copilot;
pub mod multimodal_interpretation_engine;
pub use context_compilation_research_copilot::{
    compile_context_compilation, context_compilation_research_copilot_manifest,
    CertifiedDecisionSection3, CompilationDisposition, ContextCompilationError,
    ContextCompilationRequest, DecisionQuery4,
    CONTRACT_VERSION as CONTEXT_COMPILATION_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_COMPILATION_COPILOT_FEATURE_ID,
};
pub use multimodal_interpretation_engine::{
    compile_multimodal_interpretation, multimodal_interpretation_engine_manifest,
    EvidenceBackedResult2, InteractiveInterpretation1, InterpretationDisposition,
    InterpretationEngineError, InterpretationRequest2,
    CONTRACT_VERSION as MULTIMODAL_INTERPRETATION_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_INTERPRETATION_FEATURE_ID,
};
