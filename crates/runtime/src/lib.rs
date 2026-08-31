//! BioPRISM execution runtime: the layer that runs a trial and can prove what it did.
//!
//! Implements blueprint §05 (execution runtime): the run orchestrator (05.02), the executor
//! provider interface (05.03), the WorldTape and state ledger (05.04), fork/replay/suffix execution
//! (05.05), filesystem and service virtualization (05.06), network/time/randomness virtualization
//! (05.07), effects, permissions and the secret broker (05.08), and the budget controller (05.09).
//!
//! The organising idea is a single narrow door. Everything nondeterministic a run can touch — the
//! clock, entropy, the network, files, processes, services, models, irreversible actions — passes
//! through [`Host::perform`], and every passage is sealed into a hash-chained [`WorldTape`]. From
//! that one constraint the rest follows:
//!
//! - **A run can be replayed** because the tape holds every answer the run did not compute itself.
//! - **A replay cannot cheat** because [`ReplayHost`] holds a tape and a cursor and *no live world*;
//!   an effect the tape does not record has nothing to fall through to.
//! - **Two runs can be compared at a state** because "the state after step n" is a digest, so
//!   forking at a decision point (05.05) is an identity, not a resemblance.
//! - **A fork cannot repeat a side effect** because its inherited prefix is copied, never
//!   re-performed. The world is not asked again, so it cannot answer again.
//! - **Nothing degrades quietly.** Undeclared effects are refused, unavailable providers error
//!   instead of falling back, exhausted budgets abort instead of truncating, and simulated outcomes
//!   are labelled on the tape as simulated.
//!
//! Where this sits relative to `bioprism-weave`: Weave is coordination — who said what to whom, who
//! owes whom, how authority attenuates. This is execution — what was read, written, fetched, spent.
//! Both keep append-only hash-chained records, and they are separate records on purpose: they are
//! appended by different subsystems, read by different audiences, and a merged log would be
//! replayable by neither.
//!
//! What this crate deliberately does **not** do: it does not score, it does not schedule against a
//! real queue, it does not open a socket or a real file, and it does not run containers. The last
//! two are the same refusal — the subprocess and container providers of 05.03 are declared and
//! return `ProviderUnavailable`, because a plan that asked for container isolation and silently got
//! a thread produces results that are wrong in a way no downstream analysis can detect.
//!
//! ```
//! use bioprism_ids::RunId;
//! use bioprism_runtime::{
//!     Clock, EffectKind, EffectPolicy, Host, InProcessWorld, Randomness, RecordingHost, ReplayHost,
//! };
//!
//! let policy = EffectPolicy::evaluation_default()
//!     .declaring([EffectKind::ClockNow, EffectKind::ClockSleep, EffectKind::RandomBytes]);
//!
//! fn program(host: &mut dyn Host) -> Result<String, bioprism_runtime::RuntimeError> {
//!     host.sleep(250)?;
//!     Ok(format!("{}:{}", host.now_millis()?, host.random_hex(4)?))
//! }
//!
//! let run = RunId::parse("run-1").unwrap();
//! let mut recording = RecordingHost::new(run, InProcessWorld::new().with_seed(7), policy);
//! let live = program(&mut recording).unwrap();
//! let tape = recording.into_tape();
//!
//! // The tape survives a round trip through JSON, and the replay agrees byte for byte.
//! let reloaded = bioprism_runtime::WorldTape::from_json(&tape.to_json().unwrap()).unwrap();
//! let mut replay = ReplayHost::new(reloaded);
//! assert_eq!(program(&mut replay).unwrap(), live);
//! ```

pub mod budget;
pub mod effect;
pub mod error;
pub mod fork;
pub mod host;
pub mod interpretation_assurance;
pub mod orchestrator;
pub mod provider;
pub mod research_run;
pub mod replay_audit;
pub mod workflow_execution;
pub mod workflow_batch;
pub mod sandbox;
pub mod seam;
pub mod secret;
pub mod tape;
pub mod context_compilation_contract;
pub mod federated_knowledge_representation_assurance;

pub use budget::{
    Accounting, BudgetController, BudgetPlan, BudgetWarning, ChargeStatus, Limit, RuntimeResource,
};
pub use effect::{
    Authorization, DecisionOutcome, Effect, EffectClass, EffectKind, EffectOutcome, EffectPolicy,
    EffectRequest, MaterializationPolicy, NetworkMode, PolicyDecision, Provenance,
};
pub use error::RuntimeError;
pub use fork::{
    cache_reuse, compare_suffixes, fork_tape, observable_state, open_suffix, CachePrefixKey,
    MatchedComparison, ObservedState, ObservedStep, ReuseStatus, SuffixHost,
    OBSERVABLE_STATE_VERSION,
};
pub use host::{EffectSource, Host, RecordingHost, ReplayHost};
pub use interpretation_assurance::{
    assure_interpretation, interpretation_assurance_manifest, EvidenceBackedResult4,
    InterpretationArtifact7, InterpretationAssuranceError, InterpretationCandidate4,
    InterpretationEvidenceState, InteractiveInterpretation7,
    CONTRACT_VERSION as INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_ASSURANCE_FEATURE_ID,
};
pub use orchestrator::{
    AggregationPolicy, AttemptId, AttemptRecord, LifecycleEvent, RetryClass, RunState,
    Termination, TerminationReason, Trial, TrialId,
};
pub use provider::{
    Artifact, Capabilities, ContainerProvider, ExecutionPlan, ExecutorProvider, InProcessProvider,
    StateHandle, SubprocessProvider,
};
pub use context_compilation_contract::{
    capability_manifest as context_compilation_capability_manifest,
    compile as compile_context_contract,
    compile_json as compile_context_contract_json,
    CertifiedDecisionSection, ContextContractError, ContextContractReceipt, DecisionQuery,
    FactBinding, CONTRACT_VERSION as CONTEXT_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_CONTRACT_FEATURE_ID, INPUT_SCHEMA as CONTEXT_CONTRACT_INPUT_SCHEMA,
    OUTPUT_SCHEMA as CONTEXT_CONTRACT_OUTPUT_SCHEMA,
};
pub use research_run::{
    ResearchExecutionSession, ResearchReplayBundle, ResearchRuntimeError,
    FEATURE_CONTRACT_VERSION as RESEARCH_FEATURE_CONTRACT_VERSION,
    FEATURE_ID as RESEARCH_FEATURE_ID,
};
pub use federated_knowledge_representation_assurance::{
    assure_knowledge_representation as assure_runtime_knowledge_representation,
    knowledge_representation_assurance_manifest as runtime_knowledge_representation_assurance_manifest,
    KnowledgeEvidenceState as RuntimeKnowledgeEvidenceState,
    KnowledgePeer4 as RuntimeKnowledgePeer4,
    ResearchClaim4 as RuntimeResearchClaim4,
    ScopedResearchClaims4 as RuntimeScopedResearchClaims4,
    TypedKnowledgeWorld7 as RuntimeTypedKnowledgeWorld7,
    KnowledgeRepresentationAssuranceError as RuntimeKnowledgeRepresentationAssuranceError,
    FEATURE_ID as RUNTIME_KNOWLEDGE_REPRESENTATION_FEATURE_ID,
    CONTRACT_VERSION as RUNTIME_KNOWLEDGE_REPRESENTATION_CONTRACT_VERSION,
};
pub use replay_audit::{
    audit_replay, replay_audit_manifest, ReplayAuditError, ReplayAuditRequest,
    ReplayAuditReceipt, ReplayAuditStatus,
};
pub use workflow_execution::{
    execute_workflow, workflow_execution_manifest, WorkflowAction, WorkflowExecutionError,
    WorkflowExecutionMode, WorkflowExecutionReceipt, WorkflowExecutionRequest,
    WorkflowExecutionStatus, FEATURE_CONTRACT_VERSION as WORKFLOW_EXECUTION_FEATURE_VERSION,
    FEATURE_ID as WORKFLOW_EXECUTION_FEATURE_ID,
};
pub use workflow_batch::{
    execute_workflow_batch, workflow_batch_manifest, WorkflowBatchDisposition,
    WorkflowBatchEntry, WorkflowBatchError, WorkflowBatchMode, WorkflowBatchReceipt,
    WorkflowBatchRequest, FEATURE_ID as WORKFLOW_BATCH_FEATURE_ID,
    FEATURE_VERSION as WORKFLOW_BATCH_FEATURE_VERSION,
};
pub use sandbox::{Fault, FileChange, InProcessWorld};
pub use seam::{Clock, ExternalActions, Network, Randomness, Sandbox};
pub use secret::{Capability, SecretBroker, SecretRef};
pub use tape::{
    Artifacts, Checkpoint, RestorationDeclaration, TapeEntry, TapeLineage, WorldTape,
};
