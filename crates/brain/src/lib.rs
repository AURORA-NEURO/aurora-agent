//! A provider-neutral autonomous brain kernel.
//!
//! This crate implements the part of an agent that must remain deterministic and inspectable
//! even when the eventual language model is stochastic: model selection under explicit resource
//! constraints, prompt assembly with omission accounting, bounded DAG planning, and a guarded
//! online bandit ledger. It is the executable kernel for the model-selection and autonomous
//! planning layers named by blueprint sections 09.08, 09.11, 11.18, and 11.20.
//!
//! The kernel deliberately does not open sockets, read environment variables, store provider
//! keys, execute tools, or claim that a model response is correct. Those effects belong at an
//! application-owned runtime boundary. A runtime may use [`bioprism_runtime::SecretBroker`] or
//! the Python SDK's in-memory credential store, then pass only an opaque credential handle and
//! the resulting value-free metadata back here. This separation makes a user-supplied key
//! possible without making the key part of an MCP argument, plan, certificate, or learning state.
//!
//! The learning implementation is an explicit-reward policy layer rather than a claim of
//! reinforcement learning in the statistical sense. It supports deterministic UCB and seeded
//! epsilon-greedy exploration, deterministic Thompson-sampling exploration, updates only from a bounded reward supplied by an evaluator,
//! records failures separately, and never mutates hidden global state. Contextual state is nested
//! under a canonical domain/capability/risk/task-family digest and remains compatible with the
//! legacy global arm ledger as a cold-start prior.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub mod context_compilation;
pub mod evidence_contract_model;
pub mod evidence_operations_control_plane;
pub mod evidence_protocol_adapter;
pub mod evidence_research_copilot;
pub mod evidence_research_workbench;
pub mod evidence_safety_assurance;
pub mod evidence_surveillance;
pub mod evidence_workflow_fabric;
pub mod federated_contract_model;
pub mod federated_evidence_copilot;
pub mod federated_evidence_surveillance;
pub mod federated_evidence_workflow_fabric;
pub mod federated_operations_control_plane;
pub mod federated_protocol_adapter;
pub mod federated_research_workbench;
pub mod federated_retrieval_assurance_harness;
pub mod federated_retrieval_contract_model;
pub mod federated_retrieval_control_plane;
pub mod federated_retrieval_copilot;
pub mod federated_retrieval_protocol_gateway;
pub mod federated_retrieval_synthesis;
pub mod federated_retrieval_workbench;
pub mod federated_retrieval_workflow_fabric;
pub mod federated_safety_assurance;
pub mod high_throughput_evidence_copilot;
pub mod high_throughput_evidence_surveillance;
pub mod high_throughput_evidence_workflow_fabric;
pub mod multimodal_context_compilation;
pub mod throughput_context_compilation;
pub mod federated_context_compilation;
pub mod context_omission_adjudication;
pub mod context_release_admission;
pub mod context_freshness_drift;
pub mod context_uncertainty_envelope;
pub mod context_contradiction_resolution;
pub mod context_dependency_closure;
pub mod context_decision_projection;
pub mod federated_decision_projection;
pub mod context_workflow_fabric;
pub mod multimodal_context_workflow_fabric;
pub mod throughput_context_workflow_fabric;
pub mod federated_context_workflow_fabric;
pub mod context_research_workbench;
pub mod multimodal_context_workbench;
pub mod multimodal_contract_model;
pub mod multimodal_evidence_copilot;
pub mod multimodal_evidence_surveillance;
pub mod multimodal_evidence_workflow_fabric;
pub mod multimodal_operations_control_plane;
pub mod multimodal_protocol_adapter;
pub mod multimodal_research_workbench;
pub mod multimodal_retrieval_assurance_harness;
pub mod multimodal_retrieval_contract_model;
pub mod multimodal_retrieval_control_plane;
pub mod multimodal_retrieval_copilot;
pub mod multimodal_retrieval_protocol_gateway;
pub mod multimodal_retrieval_synthesis;
pub mod multimodal_retrieval_workbench;
pub mod multimodal_retrieval_workflow_fabric;
pub mod multimodal_safety_assurance;
pub mod retrieval_assurance_harness;
pub mod retrieval_contract_model;
pub mod retrieval_federated_control_plane;
pub mod retrieval_protocol_gateway;
pub mod retrieval_research_copilot;
pub mod retrieval_research_workbench;
pub mod retrieval_synthesis;
pub mod retrieval_workflow_fabric;
pub mod throughput_context_workbench;
pub mod federated_context_workbench;
pub mod context_protocol_adapter;
pub mod multimodal_context_protocol;
pub mod throughput_context_protocol;
pub mod federated_context_protocol;
pub mod context_compilation_assurance;
pub mod multimodal_context_compilation_assurance;
pub mod throughput_context_compilation_assurance;
pub mod federated_continual_context_compilation_assurance;
pub mod local_context_compilation_federated_control_plane;
pub mod multimodal_context_compilation_federated_control_plane;
pub mod throughput_context_compilation_federated_control_plane;
pub mod federated_continual_context_compilation_federated_control_plane;
pub mod local_knowledge_representation_inference_engine;
pub mod multimodal_knowledge_representation_inference_engine;
pub mod throughput_knowledge_representation_inference_engine;
pub mod federated_continual_knowledge_representation_inference_engine;
pub mod local_knowledge_representation_contract_model;
pub mod multimodal_knowledge_representation_contract_model;
pub mod throughput_knowledge_representation_contract_model;
pub mod throughput_contract_model;
pub mod throughput_operations_control_plane;
pub mod throughput_protocol_adapter;
pub mod throughput_research_workbench;
pub mod throughput_retrieval_assurance_harness;
pub mod throughput_retrieval_contract_model;
pub mod throughput_retrieval_control_plane;
pub mod throughput_retrieval_copilot;
pub mod throughput_retrieval_protocol_gateway;
pub mod throughput_retrieval_synthesis;
pub mod throughput_retrieval_workbench;
pub mod throughput_retrieval_workflow_fabric;
pub mod throughput_safety_assurance;

pub use context_compilation::{
    compile_research_context, context_compilation_manifest, ContextCompilationDisposition,
    ContextCompilationError, ContextFact, ResearchContextCompilationReceipt,
    ResearchContextCompilationRequest, CONTRACT_VERSION as CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_COMPILATION_FEATURE_ID,
};
pub use evidence_contract_model::{
    evidence_contract_model_manifest, model_evidence_contract, ContractCompatibility,
    ContractDisposition, EvidenceContractModelError, EvidenceContractModelReceipt,
    EvidenceContractModelRequest, CONTRACT_VERSION as EVIDENCE_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_CONTRACT_MODEL_FEATURE_ID,
};
pub use evidence_operations_control_plane::{
    evidence_operations_control_plane_manifest, operate_evidence, EvidenceOperationsError,
    EvidenceOperationsReceipt, EvidenceOperationsRequest, OperationsDisposition,
    CONTRACT_VERSION as EVIDENCE_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_OPERATIONS_CONTROL_PLANE_FEATURE_ID,
};
pub use evidence_protocol_adapter::{
    evidence_protocol_adapter_manifest, serve_evidence_protocol, EvidenceProtocolError,
    EvidenceProtocolReceipt, EvidenceProtocolRequest,
    CONTRACT_VERSION as EVIDENCE_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_PROTOCOL_ADAPTER_FEATURE_ID,
};
pub use evidence_research_copilot::{
    compile_evidence_copilot, evidence_research_copilot_manifest, EvidenceCopilotError,
    EvidenceCopilotReceipt, EvidenceCopilotRequest,
    CONTRACT_VERSION as EVIDENCE_RESEARCH_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_RESEARCH_COPILOT_FEATURE_ID,
};
pub use evidence_research_workbench::{
    compile_evidence_research_workbench, evidence_research_workbench_manifest,
    EvidenceWorkbenchError, EvidenceWorkbenchReceipt, EvidenceWorkbenchRequest,
    CONTRACT_VERSION as EVIDENCE_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use evidence_safety_assurance::{
    evidence_safety_assurance_manifest, verify_evidence_safety, AssuranceVerdict,
    EvidenceAssuranceError, EvidenceAssuranceReceipt,
    CONTRACT_VERSION as EVIDENCE_SAFETY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_SAFETY_ASSURANCE_FEATURE_ID,
};
pub use evidence_surveillance::{
    evidence_surveillance_manifest, surveil_evidence, EvidenceFeedRequest, EvidenceObservation,
    EvidenceSurveillanceDisposition, EvidenceSurveillanceError, QualifiedEvidenceSet,
    CONTRACT_VERSION as EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_SURVEILLANCE_FEATURE_ID,
};
pub use evidence_workflow_fabric::{
    compile_evidence_workflow, evidence_workflow_fabric_manifest, EvidenceWorkflowError,
    EvidenceWorkflowReceipt, EvidenceWorkflowRequest,
    CONTRACT_VERSION as EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use federated_contract_model::{
    federated_contract_model_manifest, model_federated_contract, FederatedContractDisposition,
    FederatedContractModelError, FederatedContractModelReceipt, FederatedContractModelRequest,
    CONTRACT_VERSION as FEDERATED_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTRACT_MODEL_FEATURE_ID,
};
pub use federated_evidence_copilot::{
    compile_federated_evidence_copilot, federated_evidence_research_copilot_manifest,
    FederatedCopilotError, FederatedCopilotReceipt, FederatedCopilotRequest,
    CONTRACT_VERSION as FEDERATED_EVIDENCE_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_EVIDENCE_COPILOT_FEATURE_ID,
};
pub use federated_evidence_surveillance::{
    admit_federated_evidence, federated_evidence_surveillance_manifest,
    FederatedEvidenceDisposition, FederatedEvidenceError, FederatedEvidenceFeedRequest,
    FederatedEvidenceReceipt, CONTRACT_VERSION as FEDERATED_EVIDENCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_EVIDENCE_FEATURE_ID,
};
pub use federated_evidence_workflow_fabric::{
    compile_federated_evidence_workflow, federated_evidence_workflow_fabric_manifest,
    FederatedWorkflowError, FederatedWorkflowReceipt, FederatedWorkflowRequest,
    CONTRACT_VERSION as FEDERATED_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use federated_operations_control_plane::{
    federated_operations_control_plane_manifest, operate_federated_evidence,
    FederatedOperationsDisposition, FederatedOperationsError, FederatedOperationsReceipt,
    FederatedOperationsRequest,
    CONTRACT_VERSION as FEDERATED_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_OPERATIONS_CONTROL_PLANE_FEATURE_ID,
};
pub use federated_protocol_adapter::{
    federated_protocol_adapter_manifest, serve_federated_protocol, FederatedProtocolError,
    FederatedProtocolReceipt, FederatedProtocolRequest,
    CONTRACT_VERSION as FEDERATED_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_PROTOCOL_ADAPTER_FEATURE_ID,
};
pub use federated_research_workbench::{
    compile_federated_research_workbench, federated_research_workbench_manifest,
    FederatedWorkbenchError, FederatedWorkbenchReceipt, FederatedWorkbenchRequest,
    CONTRACT_VERSION as FEDERATED_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use federated_retrieval_assurance_harness::{
    federated_retrieval_assurance_harness_manifest, verify_federated_retrieval_assurance,
    FederatedRetrievalAssuranceError, FederatedRetrievalAssuranceReceipt,
    FederatedRetrievalAssuranceVerdict,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID,
};
pub use federated_retrieval_contract_model::{
    federated_retrieval_contract_model_manifest, model_federated_retrieval_contract,
    FederatedRetrievalContractError, FederatedRetrievalContractReceipt,
    FederatedRetrievalContractRequest,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID,
};
pub use federated_retrieval_control_plane::{
    federated_retrieval_control_plane_manifest, operate_federated_retrieval_control_plane,
    FederatedRetrievalControlPlaneError, FederatedRetrievalControlPlaneReceipt,
    FederatedRetrievalControlPlaneRequest,
    ACTION_ORDER as FEDERATED_RETRIEVAL_CONTROL_ACTION_ORDER,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_CONTROL_PLANE_FEATURE_ID,
};
pub use federated_retrieval_copilot::{
    compile_federated_retrieval_copilot, federated_retrieval_copilot_manifest,
    FederatedRetrievalCopilotError, FederatedRetrievalCopilotReceipt,
    FederatedRetrievalCopilotRequest,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_COPILOT_FEATURE_ID,
};
pub use federated_retrieval_protocol_gateway::{
    compile_federated_retrieval_protocol, federated_retrieval_protocol_gateway_manifest,
    FederatedRetrievalProtocolError, FederatedRetrievalProtocolReceipt,
    FederatedRetrievalProtocolRequest,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_PROTOCOL_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_PROTOCOL_FEATURE_ID,
};
pub use federated_retrieval_synthesis::{
    federated_retrieval_synthesis_manifest, synthesize_federated_retrieval,
    FederatedEvidenceSynthesis, FederatedRetrievalDisposition, FederatedRetrievalError,
    FederatedRetrievalQuery, CONTRACT_VERSION as FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
pub use federated_retrieval_workbench::{
    compile_federated_retrieval_workbench, federated_retrieval_workbench_manifest,
    FederatedRetrievalWorkbenchError, FederatedRetrievalWorkbenchReceipt,
    FederatedRetrievalWorkbenchRequest,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_WORKBENCH_FEATURE_ID,
};
pub use federated_retrieval_workflow_fabric::{
    compile_federated_retrieval_workflow, federated_retrieval_workflow_fabric_manifest,
    FederatedRetrievalWorkflowError, FederatedRetrievalWorkflowReceipt,
    FederatedRetrievalWorkflowRequest,
    CONTRACT_VERSION as FEDERATED_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use federated_safety_assurance::{
    federated_safety_assurance_manifest, verify_federated_safety, FederatedAssuranceError,
    FederatedAssuranceReceipt, FederatedAssuranceVerdict,
    CONTRACT_VERSION as FEDERATED_SAFETY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_SAFETY_ASSURANCE_FEATURE_ID,
};
pub use high_throughput_evidence_copilot::{
    compile_high_throughput_evidence_copilot, high_throughput_evidence_research_copilot_manifest,
    HighThroughputCopilotError, HighThroughputCopilotReceipt, HighThroughputCopilotRequest,
    CONTRACT_VERSION as HIGH_THROUGHPUT_EVIDENCE_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as HIGH_THROUGHPUT_EVIDENCE_COPILOT_FEATURE_ID,
};
pub use high_throughput_evidence_surveillance::{
    admit_high_throughput_evidence, high_throughput_evidence_surveillance_manifest,
    HighThroughputDisposition, HighThroughputEvidenceError, HighThroughputEvidenceFeedRequest,
    HighThroughputEvidenceReceipt, CONTRACT_VERSION as HIGH_THROUGHPUT_EVIDENCE_CONTRACT_VERSION,
    FEATURE_ID as HIGH_THROUGHPUT_EVIDENCE_FEATURE_ID,
};
pub use high_throughput_evidence_workflow_fabric::{
    compile_high_throughput_evidence_workflow, high_throughput_evidence_workflow_fabric_manifest,
    HighThroughputWorkflowError, HighThroughputWorkflowReceipt, HighThroughputWorkflowRequest,
    CONTRACT_VERSION as HIGH_THROUGHPUT_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as HIGH_THROUGHPUT_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use multimodal_context_compilation::{
    compile_multimodal_context, multimodal_context_compilation_manifest,
    MultimodalContextCompilationError, MultimodalContextCompilationReceipt,
    MultimodalContextCompilationRequest, MultimodalContextFact,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID,
};
pub use throughput_context_compilation::{
    compile_throughput_context, throughput_context_compilation_manifest,
    ThroughputContextCompilationError, ThroughputContextCompilationReceipt,
    ThroughputContextCompilationRequest, ThroughputContextItem,
    CONTRACT_VERSION as THROUGHPUT_CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTEXT_COMPILATION_FEATURE_ID,
};
pub use federated_context_compilation::{
    compile_federated_context, federated_context_compilation_manifest,
    FederatedContextCandidate, FederatedContextCompilationError,
    FederatedContextCompilationReceipt, FederatedContextCompilationRequest,
    CONTRACT_VERSION as FEDERATED_CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTEXT_COMPILATION_FEATURE_ID,
};
pub use context_omission_adjudication::{
    adjudicate_context_omissions, context_omission_adjudication_manifest,
    ContextAdjudicationEvidence, ContextOmissionAdjudicationError,
    ContextOmissionAdjudicationReceipt, ContextOmissionAdjudicationRequest,
    CONTRACT_VERSION as CONTEXT_OMISSION_ADJUDICATION_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID,
};
pub use context_release_admission::{
    admit_context_release, context_release_admission_manifest,
    ContextReleaseAdmissionError, ContextReleaseAdmissionReceipt,
    ContextReleaseAdmissionRequest, RELEASE_ACTION,
    CONTRACT_VERSION as CONTEXT_RELEASE_ADMISSION_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_RELEASE_ADMISSION_FEATURE_ID,
};
pub use context_freshness_drift::{
    context_freshness_drift_manifest, evaluate_context_freshness_drift,
    ContextFreshnessDriftError, ContextFreshnessDriftReceipt,
    ContextFreshnessDriftRequest, ContextSnapshot,
    CONTRACT_VERSION as CONTEXT_FRESHNESS_DRIFT_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_FRESHNESS_DRIFT_FEATURE_ID,
};
pub use context_uncertainty_envelope::{
    compile_context_uncertainty_envelope, context_uncertainty_envelope_manifest,
    ContextUncertaintyEnvelopeError, ContextUncertaintyEnvelopeReceipt,
    ContextUncertaintyEnvelopeRequest, ContextUncertaintyObservation,
    CONTRACT_VERSION as CONTEXT_UNCERTAINTY_ENVELOPE_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID,
};
pub use context_contradiction_resolution::{
    compile_context_contradiction_resolution,
    context_contradiction_resolution_manifest,
    ContextContradictionClaim, ContextContradictionResolutionError,
    ContextContradictionResolutionReceipt, ContextContradictionResolutionRequest,
    CONTRACT_VERSION as CONTEXT_CONTRADICTION_RESOLUTION_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID,
};
pub use context_dependency_closure::{
    compile_context_dependency_closure, context_dependency_closure_manifest,
    ContextDependencyClosureError, ContextDependencyClosureReceipt,
    ContextDependencyClosureRequest, ContextDependencyEdge,
    CONTRACT_VERSION as CONTEXT_DEPENDENCY_CLOSURE_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID,
};
pub use context_decision_projection::{
    context_decision_projection_manifest, project_context_to_decision_section,
    ContextDecisionProjectionError, ContextDecisionProjectionReceipt,
    ContextDecisionProjectionRequest,
    CONTRACT_VERSION as CONTEXT_DECISION_PROJECTION_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_DECISION_PROJECTION_FEATURE_ID,
};
pub use federated_decision_projection::{
    federated_decision_projection_manifest, project_federated_decision_section,
    FederatedDecisionProjectionError, FederatedDecisionProjectionReceipt,
    FederatedDecisionProjectionRequest, PeerDecisionAttestation,
    CONTRACT_VERSION as FEDERATED_DECISION_PROJECTION_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_DECISION_PROJECTION_FEATURE_ID,
};
pub use context_workflow_fabric::{
    compile_context_workflow, context_workflow_fabric_manifest, ContextWorkflowError,
    ContextWorkflowReceipt, ContextWorkflowRequest, ContextWorkflowStage,
    CONTRACT_VERSION as CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use multimodal_context_workflow_fabric::{
    compile_multimodal_context_workflow, multimodal_context_workflow_fabric_manifest,
    ModalContextInput, MultimodalContextWorkflowError, MultimodalContextWorkflowReceipt,
    MultimodalContextWorkflowRequest,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use throughput_context_workflow_fabric::{
    compile_throughput_context_workflow, throughput_context_workflow_fabric_manifest,
    ThroughputContextJob, ThroughputContextWorkflowError,
    ThroughputContextWorkflowReceipt, ThroughputContextWorkflowRequest,
    CONTRACT_VERSION as THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use federated_context_workflow_fabric::{
    compile_federated_context_workflow, federated_context_workflow_fabric_manifest,
    FederatedContextWorkflowError, FederatedContextWorkflowPeer,
    FederatedContextWorkflowReceipt, FederatedContextWorkflowRequest,
    CONTRACT_VERSION as FEDERATED_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use context_research_workbench::{
    context_research_workbench_manifest, render_context_workbench,
    ContextWorkbenchError, ContextWorkbenchReceipt, ContextWorkbenchRequest,
    CONTRACT_VERSION as CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use multimodal_context_workbench::{
    multimodal_context_workbench_manifest, render_multimodal_context_workbench,
    MultimodalContextWorkbenchCell, MultimodalContextWorkbenchError, MultimodalContextWorkbenchReceipt,
    MultimodalContextWorkbenchRequest,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_WORKBENCH_FEATURE_ID,
};
pub use multimodal_contract_model::{
    model_multimodal_evidence_contract, multimodal_contract_model_manifest, ModalitySchemaBinding,
    MultimodalContractDisposition, MultimodalContractModelError, MultimodalEvidenceContractReceipt,
    MultimodalEvidenceContractRequest,
    CONTRACT_VERSION as MULTIMODAL_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTRACT_MODEL_FEATURE_ID,
};
pub use multimodal_evidence_copilot::{
    compile_multimodal_evidence_copilot, multimodal_evidence_research_copilot_manifest,
    MultimodalCopilotError, MultimodalCopilotReceipt, MultimodalCopilotRequest,
    CONTRACT_VERSION as MULTIMODAL_EVIDENCE_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_EVIDENCE_COPILOT_FEATURE_ID,
};
pub use multimodal_evidence_surveillance::{
    multimodal_evidence_surveillance_manifest, surveil_multimodal_evidence,
    MultimodalEvidenceDisposition, MultimodalEvidenceError, MultimodalEvidenceFeedRequest,
    QualifiedMultimodalEvidenceSet, CONTRACT_VERSION as MULTIMODAL_EVIDENCE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_EVIDENCE_FEATURE_ID,
};
pub use multimodal_evidence_workflow_fabric::{
    compile_multimodal_evidence_workflow, multimodal_evidence_workflow_fabric_manifest,
    MultimodalWorkflowError, MultimodalWorkflowReceipt, MultimodalWorkflowRequest,
    CONTRACT_VERSION as MULTIMODAL_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use multimodal_operations_control_plane::{
    multimodal_operations_control_plane_manifest, operate_multimodal_evidence,
    MultimodalOperationsDisposition, MultimodalOperationsError, MultimodalOperationsReceipt,
    MultimodalOperationsRequest,
    CONTRACT_VERSION as MULTIMODAL_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_OPERATIONS_CONTROL_PLANE_FEATURE_ID,
};
pub use multimodal_protocol_adapter::{
    multimodal_protocol_adapter_manifest, serve_multimodal_protocol, MultimodalProtocolError,
    MultimodalProtocolReceipt, MultimodalProtocolRequest,
    CONTRACT_VERSION as MULTIMODAL_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_PROTOCOL_ADAPTER_FEATURE_ID,
};
pub use multimodal_research_workbench::{
    compile_multimodal_research_workbench, multimodal_research_workbench_manifest,
    MultimodalWorkbenchError, MultimodalWorkbenchReceipt, MultimodalWorkbenchRequest,
    CONTRACT_VERSION as MULTIMODAL_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use multimodal_retrieval_assurance_harness::{
    multimodal_retrieval_assurance_harness_manifest, verify_multimodal_retrieval_assurance,
    MultimodalRetrievalAssuranceError, MultimodalRetrievalAssuranceReceipt,
    MultimodalRetrievalAssuranceVerdict,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_ASSURANCE_FEATURE_ID,
};
pub use multimodal_retrieval_contract_model::{
    model_multimodal_retrieval_contract, multimodal_retrieval_contract_model_manifest,
    MultimodalRetrievalContractError, MultimodalRetrievalContractReceipt,
    MultimodalRetrievalContractRequest,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID,
};
pub use multimodal_retrieval_control_plane::{
    multimodal_retrieval_control_plane_manifest, operate_multimodal_retrieval_control_plane,
    MultimodalRetrievalControlPlaneError, MultimodalRetrievalControlPlaneReceipt,
    MultimodalRetrievalControlPlaneRequest,
    ACTION_ORDER as MULTIMODAL_RETRIEVAL_CONTROL_ACTION_ORDER,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_CONTROL_PLANE_FEATURE_ID,
};
pub use multimodal_retrieval_copilot::{
    compile_multimodal_retrieval_copilot, multimodal_retrieval_copilot_manifest,
    MultimodalRetrievalCopilotError, MultimodalRetrievalCopilotReceipt,
    MultimodalRetrievalCopilotRequest,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_COPILOT_FEATURE_ID,
};
pub use multimodal_retrieval_protocol_gateway::{
    compile_multimodal_retrieval_protocol, multimodal_retrieval_protocol_gateway_manifest,
    MultimodalRetrievalProtocolError, MultimodalRetrievalProtocolReceipt,
    MultimodalRetrievalProtocolRequest,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_PROTOCOL_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_PROTOCOL_FEATURE_ID,
};
pub use multimodal_retrieval_synthesis::{
    multimodal_retrieval_synthesis_manifest, synthesize_multimodal_retrieval,
    MultimodalEvidenceSynthesis, MultimodalRetrievalError, MultimodalRetrievalQuery,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
pub use multimodal_retrieval_workbench::{
    compile_multimodal_retrieval_workbench, multimodal_retrieval_workbench_manifest,
    MultimodalRetrievalWorkbenchError, MultimodalRetrievalWorkbenchReceipt,
    MultimodalRetrievalWorkbenchRequest,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_WORKBENCH_FEATURE_ID,
};
pub use multimodal_retrieval_workflow_fabric::{
    compile_multimodal_retrieval_workflow, multimodal_retrieval_workflow_fabric_manifest,
    MultimodalRetrievalWorkflowError, MultimodalRetrievalWorkflowReceipt,
    MultimodalRetrievalWorkflowRequest,
    CONTRACT_VERSION as MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use multimodal_safety_assurance::{
    multimodal_safety_assurance_manifest, verify_multimodal_safety, MultimodalAssuranceError,
    MultimodalAssuranceReceipt, MultimodalAssuranceVerdict,
    CONTRACT_VERSION as MULTIMODAL_SAFETY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_SAFETY_ASSURANCE_FEATURE_ID,
};
pub use retrieval_assurance_harness::{
    retrieval_assurance_harness_manifest, verify_retrieval_assurance, RetrievalAssuranceError,
    RetrievalAssuranceReceipt, RetrievalAssuranceVerdict,
    CONTRACT_VERSION as RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_ASSURANCE_FEATURE_ID,
};
pub use retrieval_contract_model::{
    model_retrieval_contract, retrieval_contract_model_manifest, RetrievalContractModelError,
    RetrievalContractModelReceipt, RetrievalContractModelRequest,
    CONTRACT_VERSION as RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_CONTRACT_MODEL_FEATURE_ID,
};
pub use retrieval_federated_control_plane::{
    operate_retrieval_federated_control_plane, retrieval_federated_control_plane_manifest,
    RetrievalFederatedControlPlaneError, RetrievalFederatedControlPlaneReceipt,
    RetrievalFederatedControlPlaneRequest, ACTION_ORDER as RETRIEVAL_CONTROL_ACTION_ORDER,
    CONTRACT_VERSION as RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_CONTROL_PLANE_FEATURE_ID,
};
pub use retrieval_protocol_gateway::{
    compile_retrieval_protocol, retrieval_protocol_gateway_manifest, RetrievalProtocolError,
    RetrievalProtocolReceipt, RetrievalProtocolRequest,
    CONTRACT_VERSION as RETRIEVAL_PROTOCOL_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_PROTOCOL_FEATURE_ID,
};
pub use retrieval_research_copilot::{
    compile_retrieval_copilot, retrieval_research_copilot_manifest, RetrievalCopilotError,
    RetrievalCopilotReceipt, RetrievalCopilotRequest,
    CONTRACT_VERSION as RETRIEVAL_RESEARCH_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_RESEARCH_COPILOT_FEATURE_ID,
};
pub use retrieval_research_workbench::{
    compile_retrieval_research_workbench, retrieval_research_workbench_manifest,
    RetrievalWorkbenchError, RetrievalWorkbenchReceipt, RetrievalWorkbenchRequest,
    CONTRACT_VERSION as RETRIEVAL_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use retrieval_synthesis::{
    retrieval_synthesis_manifest, synthesize_retrieval, EvidenceSynthesis, RetrievalCandidate,
    RetrievalSynthesisError, ScopedRetrievalQuery, SynthesisDisposition,
    CONTRACT_VERSION as RETRIEVAL_SYNTHESIS_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
pub use retrieval_workflow_fabric::{
    compile_retrieval_workflow, retrieval_workflow_fabric_manifest, RetrievalWorkflowError,
    RetrievalWorkflowReceipt, RetrievalWorkflowRequest,
    CONTRACT_VERSION as RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use throughput_context_workbench::{
    render_throughput_context_workbench, throughput_context_workbench_manifest,
    ThroughputContextWorkbenchError, ThroughputContextWorkbenchJob,
    ThroughputContextWorkbenchReceipt, ThroughputContextWorkbenchRequest,
    CONTRACT_VERSION as THROUGHPUT_CONTEXT_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTEXT_WORKBENCH_FEATURE_ID,
};
pub use federated_context_workbench::{
    federated_context_workbench_manifest, render_federated_context_workbench,
    FederatedContextWorkbenchError, FederatedContextWorkbenchPeer,
    FederatedContextWorkbenchReceipt, FederatedContextWorkbenchRequest,
    CONTRACT_VERSION as FEDERATED_CONTEXT_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTEXT_WORKBENCH_FEATURE_ID,
};
pub use context_protocol_adapter::{
    context_protocol_adapter_manifest, serve_context_protocol, ContextProtocolCandidate,
    ContextProtocolError, ContextProtocolReceipt, ContextProtocolRequest,
    CONTRACT_VERSION as CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID, METHOD as CONTEXT_PROTOCOL_METHOD,
    PROTOCOL_VERSION as CONTEXT_PROTOCOL_VERSION, RESPONSE_SCHEMA as CONTEXT_PROTOCOL_RESPONSE_SCHEMA,
    ROUTE as CONTEXT_PROTOCOL_ROUTE,
};
pub use multimodal_context_protocol::{
    multimodal_context_protocol_manifest, serve_multimodal_context_protocol,
    MultimodalContextProtocolCell, MultimodalContextProtocolError,
    MultimodalContextProtocolReceipt, MultimodalContextProtocolRequest,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID,
    METHOD as MULTIMODAL_CONTEXT_PROTOCOL_METHOD,
    PROTOCOL_VERSION as MULTIMODAL_CONTEXT_PROTOCOL_VERSION,
    RESPONSE_SCHEMA as MULTIMODAL_CONTEXT_PROTOCOL_RESPONSE_SCHEMA,
    ROUTE as MULTIMODAL_CONTEXT_PROTOCOL_ROUTE,
};
pub use throughput_context_protocol::{
    throughput_context_protocol_manifest, serve_throughput_context_protocol,
    ThroughputContextProtocolError, ThroughputContextProtocolJob,
    ThroughputContextProtocolReceipt, ThroughputContextProtocolRequest,
    CONTRACT_VERSION as THROUGHPUT_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID,
    METHOD as THROUGHPUT_CONTEXT_PROTOCOL_METHOD,
    PROTOCOL_VERSION as THROUGHPUT_CONTEXT_PROTOCOL_VERSION,
    RESPONSE_SCHEMA as THROUGHPUT_CONTEXT_PROTOCOL_RESPONSE_SCHEMA,
    ROUTE as THROUGHPUT_CONTEXT_PROTOCOL_ROUTE,
};
pub use federated_context_protocol::{
    federated_context_protocol_manifest, serve_federated_context_protocol,
    FederatedContextProtocolError, FederatedContextProtocolPeer,
    FederatedContextProtocolReceipt, FederatedContextProtocolRequest,
    CONTRACT_VERSION as FEDERATED_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID,
    METHOD as FEDERATED_CONTEXT_PROTOCOL_METHOD,
    PROTOCOL_VERSION as FEDERATED_CONTEXT_PROTOCOL_VERSION,
    RESPONSE_SCHEMA as FEDERATED_CONTEXT_PROTOCOL_RESPONSE_SCHEMA,
    ROUTE as FEDERATED_CONTEXT_PROTOCOL_ROUTE,
};
pub use context_compilation_assurance::{
    assure_context_compilation, context_compilation_assurance_manifest,
    ContextAssuranceCandidate, ContextAssuranceVerdict, ContextCompilationAssuranceError,
    ContextCompilationAssuranceReceipt, ContextCompilationAssuranceRequest,
    CONTRACT_VERSION as CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
};
pub use multimodal_context_compilation_assurance::{
    assure_multimodal_context_compilation, multimodal_context_compilation_assurance_manifest,
    MultimodalContextAssuranceCell, MultimodalContextAssuranceError,
    MultimodalContextAssuranceReceipt, MultimodalContextAssuranceRequest,
    MultimodalContextAssuranceVerdict,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
};
pub use throughput_context_compilation_assurance::{
    assure_throughput_context_compilation, throughput_context_compilation_assurance_manifest,
    ThroughputContextAssuranceError, ThroughputContextAssuranceJob,
    ThroughputContextAssuranceReceipt, ThroughputContextAssuranceRequest,
    ThroughputContextAssuranceVerdict,
    CONTRACT_VERSION as THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
};
pub use federated_continual_context_compilation_assurance::{
    assure_federated_continual_context_compilation,
    federated_continual_context_compilation_assurance_manifest,
    FederatedContextAssuranceError, FederatedContextAssurancePeer,
    FederatedContextAssuranceReceipt, FederatedContextAssuranceRequest,
    FederatedContextAssuranceVerdict,
    CONTRACT_VERSION as FEDERATED_CONTINUAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTINUAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
};
pub use local_context_compilation_federated_control_plane::{
    local_context_compilation_federated_control_plane_manifest,
    operate_local_context_compilation, LocalContextControlDisposition,
    LocalContextControlError, LocalContextControlReceipt, LocalContextControlRequest,
    LocalContextControlStage,
    CONTRACT_VERSION as LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
};
pub use multimodal_context_compilation_federated_control_plane::{
    multimodal_context_compilation_federated_control_plane_manifest,
    operate_multimodal_context_compilation, MultimodalContextControlCell,
    MultimodalContextControlDisposition, MultimodalContextControlError,
    MultimodalContextControlReceipt, MultimodalContextControlRequest,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
};
pub use throughput_context_compilation_federated_control_plane::{
    operate_throughput_context_compilation,
    throughput_context_compilation_federated_control_plane_manifest,
    ThroughputContextControlDisposition, ThroughputContextControlError,
    ThroughputContextControlJob, ThroughputContextControlReceipt,
    ThroughputContextControlRequest,
    CONTRACT_VERSION as THROUGHPUT_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
};
pub use federated_continual_context_compilation_federated_control_plane::{
    operate_federated_continual_context_compilation,
    federated_continual_context_compilation_federated_control_plane_manifest,
    FederatedContinualContextControlDisposition, FederatedContinualContextControlError,
    FederatedContinualContextControlPeer, FederatedContinualContextControlReceipt,
    FederatedContinualContextControlRequest,
    CONTRACT_VERSION as FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTINUAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
};
pub use throughput_contract_model::{
    model_throughput_contract, throughput_contract_model_manifest, ThroughputContractDisposition,
    ThroughputContractModelError, ThroughputContractModelReceipt, ThroughputContractModelRequest,
    CONTRACT_VERSION as THROUGHPUT_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_CONTRACT_MODEL_FEATURE_ID,
};
pub use local_knowledge_representation_inference_engine::{
    infer_local_knowledge_representation, local_knowledge_representation_inference_engine_manifest,
    KnowledgeRepresentationClaim, KnowledgeRepresentationDisposition,
    KnowledgeRepresentationError, KnowledgeRepresentationReceipt, KnowledgeRepresentationRequest,
    CONTRACT_VERSION as LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION,
    FEATURE_ID as LOCAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,
};
pub use throughput_operations_control_plane::{
    operate_throughput_evidence, throughput_operations_control_plane_manifest,
    ThroughputOperationsDisposition, ThroughputOperationsError, ThroughputOperationsReceipt,
    ThroughputOperationsRequest,
    CONTRACT_VERSION as THROUGHPUT_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_OPERATIONS_CONTROL_PLANE_FEATURE_ID,
};
pub use multimodal_knowledge_representation_inference_engine::{
    infer_multimodal_knowledge_representation, multimodal_knowledge_representation_inference_engine_manifest,
    MultimodalKnowledgeClaim, MultimodalKnowledgeDisposition, MultimodalKnowledgeError,
    MultimodalKnowledgeReceipt, MultimodalKnowledgeRequest,
    CONTRACT_VERSION as MULTIMODAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,
};
pub use throughput_protocol_adapter::{
    serve_throughput_protocol, throughput_protocol_adapter_manifest, ThroughputProtocolError,
    ThroughputProtocolReceipt, ThroughputProtocolRequest,
    CONTRACT_VERSION as THROUGHPUT_PROTOCOL_ADAPTER_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_PROTOCOL_ADAPTER_FEATURE_ID,
};
pub use throughput_knowledge_representation_inference_engine::{
    infer_throughput_knowledge_representation, throughput_knowledge_representation_inference_engine_manifest,
    ThroughputKnowledgeDisposition, ThroughputKnowledgeError, ThroughputKnowledgeJob,
    ThroughputKnowledgeReceipt, ThroughputKnowledgeRequest,
    CONTRACT_VERSION as THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,
};
pub use throughput_research_workbench::{
    compile_throughput_research_workbench, throughput_research_workbench_manifest,
    ThroughputWorkbenchError as ThroughputResearchWorkbenchError,
    ThroughputWorkbenchReceipt as ThroughputResearchWorkbenchReceipt,
    ThroughputWorkbenchRequest as ThroughputResearchWorkbenchRequest,
    CONTRACT_VERSION as THROUGHPUT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use federated_continual_knowledge_representation_inference_engine::{
    federated_continual_knowledge_representation_inference_engine_manifest,
    infer_federated_continual_knowledge_representation, FederatedKnowledgeDisposition,
    FederatedKnowledgeError, FederatedKnowledgePeer, FederatedKnowledgeReceipt,
    FederatedKnowledgeRequest,
    CONTRACT_VERSION as FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTINUAL_KNOWLEDGE_REPRESENTATION_INFERENCE_ENGINE_FEATURE_ID,
};
pub use local_knowledge_representation_contract_model::{
    local_knowledge_representation_contract_model_manifest,
    model_local_knowledge_representation_contract, KnowledgeContractClaim,
    KnowledgeContractDisposition, KnowledgeContractModelError, KnowledgeContractModelReceipt,
    KnowledgeContractModelRequest,
    CONTRACT_VERSION as LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as LOCAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
};
pub use multimodal_knowledge_representation_contract_model::{
    model_multimodal_knowledge_representation_contract,
    multimodal_knowledge_representation_contract_model_manifest,
    MultimodalKnowledgeContractCell, MultimodalKnowledgeContractDisposition,
    MultimodalKnowledgeContractError, MultimodalKnowledgeContractReceipt,
    MultimodalKnowledgeContractRequest,
    CONTRACT_VERSION as MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
};
pub use throughput_knowledge_representation_contract_model::{
    model_throughput_knowledge_representation_contract,
    throughput_knowledge_representation_contract_model_manifest,
    ThroughputKnowledgeContractDisposition, ThroughputKnowledgeContractError,
    ThroughputKnowledgeContractJob, ThroughputKnowledgeContractReceipt,
    ThroughputKnowledgeContractRequest,
    CONTRACT_VERSION as THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_KNOWLEDGE_REPRESENTATION_CONTRACT_MODEL_FEATURE_ID,
};
pub use throughput_retrieval_assurance_harness::{
    throughput_retrieval_assurance_harness_manifest, verify_throughput_retrieval_assurance,
    ThroughputRetrievalAssuranceError, ThroughputRetrievalAssuranceReceipt,
    ThroughputRetrievalAssuranceVerdict,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_ASSURANCE_FEATURE_ID,
};
pub use throughput_retrieval_contract_model::{
    model_throughput_retrieval_contract, throughput_retrieval_contract_model_manifest,
    ThroughputRetrievalContractError, ThroughputRetrievalContractReceipt,
    ThroughputRetrievalContractRequest,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID,
};
pub use throughput_retrieval_control_plane::{
    operate_throughput_retrieval_control_plane, throughput_retrieval_control_plane_manifest,
    ThroughputRetrievalControlPlaneError, ThroughputRetrievalControlPlaneReceipt,
    ThroughputRetrievalControlPlaneRequest,
    ACTION_ORDER as THROUGHPUT_RETRIEVAL_CONTROL_ACTION_ORDER,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_CONTROL_PLANE_FEATURE_ID,
};
pub use throughput_retrieval_copilot::{
    compile_throughput_retrieval_copilot, throughput_retrieval_copilot_manifest,
    ThroughputRetrievalCopilotError, ThroughputRetrievalCopilotReceipt,
    ThroughputRetrievalCopilotRequest,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_COPILOT_FEATURE_ID,
};
pub use throughput_retrieval_protocol_gateway::{
    compile_throughput_retrieval_protocol, throughput_retrieval_protocol_gateway_manifest,
    ThroughputRetrievalProtocolError, ThroughputRetrievalProtocolReceipt,
    ThroughputRetrievalProtocolRequest,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_PROTOCOL_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_PROTOCOL_FEATURE_ID,
};
pub use throughput_retrieval_synthesis::{
    synthesize_throughput_retrieval, throughput_retrieval_synthesis_manifest,
    ThroughputEvidenceSynthesis, ThroughputRetrievalError, ThroughputRetrievalQuery,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_SYNTHESIS_FEATURE_ID,
};
pub use throughput_retrieval_workbench::{
    compile_throughput_retrieval_workbench, throughput_retrieval_workbench_manifest,
    ThroughputRetrievalWorkbenchError, ThroughputRetrievalWorkbenchReceipt,
    ThroughputRetrievalWorkbenchRequest,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_WORKBENCH_FEATURE_ID,
};
pub use throughput_retrieval_workflow_fabric::{
    compile_throughput_retrieval_workflow, throughput_retrieval_workflow_fabric_manifest,
    ThroughputRetrievalWorkflowError, ThroughputRetrievalWorkflowReceipt,
    ThroughputRetrievalWorkflowRequest,
    CONTRACT_VERSION as THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID,
};
pub use throughput_safety_assurance::{
    throughput_safety_assurance_manifest, verify_throughput_safety, ThroughputAssuranceError,
    ThroughputAssuranceReceipt, ThroughputAssuranceVerdict,
    CONTRACT_VERSION as THROUGHPUT_SAFETY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as THROUGHPUT_SAFETY_ASSURANCE_FEATURE_ID,
};

pub const BRAIN_SCHEMA_VERSION: &str = "bioprism-autonomous-brain/0.1";
pub const MODEL_SELECTION_SCHEMA: &str = "bioprism-brain-model-selection/0.1";
pub const CONTEXTUAL_MODEL_SELECTION_SCHEMA: &str = "bioprism-brain-contextual-model-selection/0.1";
pub const PROMPT_ASSEMBLY_SCHEMA: &str = "bioprism-brain-prompt-assembly/0.1";
pub const PLAN_SCHEMA: &str = "bioprism-brain-plan/0.1";
pub const BANDIT_SCHEMA: &str = "bioprism-brain-bandit/0.1";
pub const LEARNING_EVIDENCE_SCHEMA: &str = "bioprism-brain-learning-evidence/0.1";
pub const PROVIDER_HEALTH_SCHEMA: &str = "bioprism-brain-provider-health/0.1";

const MAX_MODELS: usize = 256;
const MAX_PROMPT_CHUNKS: usize = 512;
const MAX_PLAN_STEPS: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 256;
const MAX_EVALUATOR_ID_BYTES: usize = 256;
const MAX_CONTEXT_LABEL_BYTES: usize = 256;
const MAX_CREDITED_OUTCOMES: usize = 4096;
const MAX_CONTEXTUAL_STATES: usize = 64;

#[derive(Debug, Error)]
pub enum BrainError {
    #[error("{field} must be non-empty")]
    Empty { field: &'static str },
    #[error("{field} is over the {max}-item bound")]
    TooMany { field: &'static str, max: usize },
    #[error("{field} must be finite and within [{min}, {max}]")]
    OutOfRange {
        field: &'static str,
        min: f64,
        max: f64,
    },
    #[error("model selection refused: no eligible model remains")]
    NoEligibleModel,
    #[error("prompt assembly refused: required content exceeds the input-token budget")]
    RequiredPromptDoesNotFit,
    #[error("plan step {step:?} references unknown dependency {dependency:?}")]
    UnknownDependency { step: String, dependency: String },
    #[error("plan contains a dependency cycle")]
    PlanCycle,
    #[error("plan step {step:?} uses a tool that is not allowed")]
    ToolNotAllowed { step: String },
    #[error("bandit arm {0:?} is not present")]
    UnknownArm(String),
    #[error("contextual observation digest does not match the selected context")]
    ContextDigestMismatch,
    #[error("context digest does not match its context identity")]
    ContextIdentityMismatch,
    #[error("contextual bandit updates require context and context_digest together")]
    ContextRequired,
    #[error("contextual observations contain duplicate arm {0:?}")]
    DuplicateContextObservation(String),
    #[error("bandit state contains duplicate contextual state {0:?}")]
    DuplicateContextState(String),
    #[error("bandit state contains duplicate arm {0:?}")]
    DuplicateArm(String),
    #[error("bandit state contains duplicate credited outcome {0:?}")]
    DuplicateCreditedOutcome(String),
    #[error("credited outcome {0:?} was replayed with different evaluator evidence")]
    ConflictingCreditedOutcome(String),
    #[error("bandit reward is outside the configured range")]
    InvalidReward,
    #[error("unsupported bandit strategy {0:?}")]
    InvalidBanditStrategy(String),
    #[error("assessment cannot be both passed and failed")]
    ContradictoryAssessment,
    #[error("invalid provider health posture for {0:?}")]
    InvalidProviderHealth(String),
    #[error("invalid model health evidence for {0:?}")]
    InvalidModelHealth(String),
    #[error("{field} must be a lowercase SHA-256 digest")]
    InvalidDigest { field: &'static str },
    #[error("invalid JSON for digest: {0}")]
    Json(#[from] serde_json::Error),
}

fn non_empty(value: &str, field: &'static str) -> Result<(), BrainError> {
    if value.trim().is_empty() {
        Err(BrainError::Empty { field })
    } else {
        Ok(())
    }
}

fn finite_range(value: f64, field: &'static str, min: f64, max: f64) -> Result<(), BrainError> {
    if value.is_finite() && value >= min && value <= max {
        Ok(())
    } else {
        Err(BrainError::OutOfRange { field, min, max })
    }
}

fn digest<T: Serialize>(value: &T) -> Result<String, BrainError> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_digest_value(value: &str, field: &'static str) -> Result<(), BrainError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(BrainError::InvalidDigest { field });
    }
    Ok(())
}

/// Metadata describing a model that an application has made available to the brain.
///
/// This is not a credential record. `requires_credential` describes the runtime contract while
/// the actual credential remains outside the serialized model catalogue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelDescriptor {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub context_window_tokens: u64,
    pub max_output_tokens: u64,
    /// Normalized quality prior in `[0, 1]`, supplied by the application or evaluator.
    pub quality: f64,
    /// Expected end-to-end latency in milliseconds.
    pub latency_ms: u64,
    /// Cost in integer micro-units per million tokens; the unit is caller-defined but stable
    /// within one selection request.
    pub cost_per_million_tokens: u64,
    /// Availability prior in `[0, 1]`; it is not provider authentication.
    pub reliability: f64,
    #[serde(default)]
    pub requires_credential: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl ModelDescriptor {
    fn validate(&self) -> Result<(), BrainError> {
        non_empty(&self.provider, "provider")?;
        non_empty(&self.model, "model")?;
        finite_range(self.quality, "quality", 0.0, 1.0)?;
        finite_range(self.reliability, "reliability", 0.0, 1.0)
    }

    fn id(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelObservation {
    pub arm_id: String,
    #[serde(default)]
    pub pulls: u64,
    #[serde(default)]
    pub reward_sum: f64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectionWeights {
    #[serde(default = "default_quality_weight")]
    pub quality: f64,
    #[serde(default = "default_reliability_weight")]
    pub reliability: f64,
    #[serde(default = "default_cost_weight")]
    pub cost: f64,
    #[serde(default = "default_latency_weight")]
    pub latency: f64,
    #[serde(default = "default_exploration_weight")]
    pub exploration: f64,
}

fn default_quality_weight() -> f64 {
    0.55
}
fn default_reliability_weight() -> f64 {
    0.25
}
fn default_cost_weight() -> f64 {
    0.10
}
fn default_latency_weight() -> f64 {
    0.10
}
fn default_exploration_weight() -> f64 {
    0.15
}

impl Default for SelectionWeights {
    fn default() -> Self {
        Self {
            quality: default_quality_weight(),
            reliability: default_reliability_weight(),
            cost: default_cost_weight(),
            latency: default_latency_weight(),
            exploration: default_exploration_weight(),
        }
    }
}

impl SelectionWeights {
    fn validate(&self) -> Result<(), BrainError> {
        for (name, value) in [
            ("weights.quality", self.quality),
            ("weights.reliability", self.reliability),
            ("weights.cost", self.cost),
            ("weights.latency", self.latency),
            ("weights.exploration", self.exploration),
        ] {
            finite_range(value, name, 0.0, 100.0)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSelectionRequest {
    pub task: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    pub input_tokens: u64,
    pub requested_output_tokens: u64,
    #[serde(default)]
    pub max_cost_per_million_tokens: Option<u64>,
    #[serde(default)]
    pub max_latency_ms: Option<u64>,
    #[serde(default)]
    pub min_quality: Option<f64>,
    /// Optional normalized rank-separation floor. When supplied, the kernel abstains instead of
    /// selecting a nearly tied candidate. This is a routing-stability gate, not answer accuracy.
    #[serde(default)]
    pub min_selection_confidence: Option<f64>,
    pub models: Vec<ModelDescriptor>,
    #[serde(default)]
    pub observations: Vec<ModelObservation>,
    #[serde(default)]
    pub weights: SelectionWeights,
    /// Runtime-supplied provider posture. Credentials remain outside this request; this map only
    /// carries bounded readiness/circuit metadata so the kernel can refuse unhealthy providers.
    #[serde(default)]
    pub provider_health: BTreeMap<String, ProviderHealth>,
    /// Process or durable transport evidence for one provider/model arm. This is evidence only:
    /// provider registration, credentials, and provider circuits remain the hard gates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_health: BTreeMap<String, ModelHealthEvidence>,
}

fn default_provider_registered() -> bool {
    true
}

fn default_provider_circuit() -> String {
    "closed".into()
}

fn default_provider_credential_ready() -> bool {
    true
}

fn default_provider_eligible() -> bool {
    true
}

/// Value-only runtime posture for one provider. It is deliberately not a credential record and
/// never contains key material, endpoint secrets, or provider response content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderHealth {
    #[serde(default = "default_provider_registered")]
    pub registered: bool,
    #[serde(default = "default_provider_circuit")]
    pub circuit: String,
    #[serde(default)]
    pub consecutive_failures: u64,
    #[serde(default = "default_provider_credential_ready")]
    pub credential_ready: bool,
    #[serde(default = "default_provider_eligible")]
    pub eligible: bool,
}

impl ProviderHealth {
    fn validate(&self, provider: &str) -> Result<(), BrainError> {
        non_empty(provider, "provider_health provider")?;
        non_empty(&self.circuit, "provider_health.circuit")?;
        if !matches!(
            self.circuit.as_str(),
            "closed" | "half_open" | "open" | "unconfigured"
        ) {
            return Err(BrainError::InvalidProviderHealth(provider.to_string()));
        }
        Ok(())
    }
}

/// Bounded, value-only transport evidence for one model arm.
///
/// The application may keep richer health records outside the kernel, but the selection contract
/// accepts only the small projection needed to adapt reliability and latency. `historical` is a
/// single nested projection for durable stores; deeper nesting is rejected to keep validation and
/// resource use predictable. `prior_adjustment_applied` is set by the Python façade after it has
/// already blended this evidence into the model descriptor, preventing a second update when the
/// same request is forwarded to the Rust kernel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelHealthEvidence {
    #[serde(default)]
    pub attempts: u64,
    #[serde(default)]
    pub successes: u64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub success_rate: Option<f64>,
    #[serde(default)]
    pub mean_latency_ms: Option<f64>,
    #[serde(default)]
    pub last_latency_ms: Option<f64>,
    #[serde(default)]
    pub prior_adjustment_applied: bool,
    #[serde(default)]
    pub historical: Option<Box<ModelHealthEvidence>>,
}

const MAX_HEALTH_ATTEMPTS: u64 = 1_000_000_000;
const MAX_HEALTH_LATENCY_MS: f64 = 600_000.0;

impl ModelHealthEvidence {
    fn validate(&self, arm_id: &str) -> Result<(), BrainError> {
        self.validate_at_depth(arm_id, 0)
    }

    fn validate_at_depth(&self, arm_id: &str, depth: u8) -> Result<(), BrainError> {
        let Some((provider, model)) = arm_id.split_once('/') else {
            return Err(BrainError::InvalidModelHealth(arm_id.to_string()));
        };
        // Provider ids are the first path segment; model ids may themselves contain slashes
        // (for example, hosted gateways commonly expose `vendor/model` names).
        if provider.trim().is_empty() || model.trim().is_empty() {
            return Err(BrainError::InvalidModelHealth(arm_id.to_string()));
        }
        if self.attempts > MAX_HEALTH_ATTEMPTS
            || self.successes > MAX_HEALTH_ATTEMPTS
            || self.failures > MAX_HEALTH_ATTEMPTS
            || self.successes > self.attempts
            || self.failures > self.attempts
        {
            return Err(BrainError::InvalidModelHealth(arm_id.to_string()));
        }
        if let Some(success_rate) = self.success_rate {
            finite_range(success_rate, "model_health.success_rate", 0.0, 1.0)?;
        }
        for latency in [self.mean_latency_ms, self.last_latency_ms]
            .into_iter()
            .flatten()
        {
            finite_range(
                latency,
                "model_health.latency_ms",
                0.0,
                MAX_HEALTH_LATENCY_MS,
            )?;
        }
        if depth > 0 && self.historical.is_some() {
            return Err(BrainError::InvalidModelHealth(arm_id.to_string()));
        }
        if let Some(historical) = &self.historical {
            historical.validate_at_depth(arm_id, depth.saturating_add(1))?;
        }
        Ok(())
    }

    fn effective_metrics(&self, model: &ModelDescriptor) -> (f64, f64) {
        if self.prior_adjustment_applied {
            return (model.reliability, model.latency_ms as f64);
        }
        let evidence = self.historical.as_deref().unwrap_or(self);
        if evidence.attempts == 0 {
            return (model.reliability, model.latency_ms as f64);
        }
        let confidence = (evidence.attempts as f64 / 12.0).min(0.75);
        let success_rate = evidence
            .success_rate
            .unwrap_or(evidence.successes as f64 / evidence.attempts as f64);
        let reliability = (1.0 - confidence) * model.reliability + confidence * success_rate;
        let latency = evidence
            .last_latency_ms
            .or(evidence.mean_latency_ms)
            .map(|observed| (1.0 - confidence) * model.latency_ms as f64 + confidence * observed)
            .unwrap_or(model.latency_ms as f64);
        (reliability, latency)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelCandidateScore {
    pub model_id: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
    pub base_score: f64,
    pub exploration_bonus: f64,
    pub score: f64,
    pub observed_pulls: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelSelectionReport {
    pub schema: String,
    pub task: String,
    pub selected_model: Option<ModelDescriptor>,
    pub selected_model_id: Option<String>,
    pub ranking: Vec<ModelCandidateScore>,
    pub eligible_model_count: usize,
    pub selection_confidence: f64,
    pub min_selection_confidence: Option<f64>,
    pub selection_status: String,
    pub decision_digest: String,
    pub does_not_claim: Vec<String>,
}

/// Select a model using hard capability/resource gates followed by a deterministic utility/UCB
/// ranking. Every candidate remains in the report, including rejected models, so the caller can
/// explain why a model was not selected.
pub fn select_model(request: &ModelSelectionRequest) -> Result<ModelSelectionReport, BrainError> {
    non_empty(&request.task, "task")?;
    if request.models.is_empty() {
        return Err(BrainError::NoEligibleModel);
    }
    if request.models.len() > MAX_MODELS {
        return Err(BrainError::TooMany {
            field: "models",
            max: MAX_MODELS,
        });
    }
    if request.provider_health.len() > MAX_MODELS {
        return Err(BrainError::TooMany {
            field: "provider_health",
            max: MAX_MODELS,
        });
    }
    if request.model_health.len() > MAX_MODELS {
        return Err(BrainError::TooMany {
            field: "model_health",
            max: MAX_MODELS,
        });
    }
    for (provider, health) in &request.provider_health {
        health.validate(provider)?;
    }
    for (arm_id, health) in &request.model_health {
        health.validate(arm_id)?;
    }
    request.weights.validate()?;
    if let Some(min_quality) = request.min_quality {
        finite_range(min_quality, "min_quality", 0.0, 1.0)?;
    }
    if let Some(min_selection_confidence) = request.min_selection_confidence {
        finite_range(
            min_selection_confidence,
            "min_selection_confidence",
            0.0,
            1.0,
        )?;
    }

    let mut observations = BTreeMap::new();
    for observation in &request.observations {
        non_empty(&observation.arm_id, "observation.arm_id")?;
        finite_range(
            observation.reward_sum,
            "observation.reward_sum",
            -1e12,
            1e12,
        )?;
        if observations
            .insert(observation.arm_id.clone(), observation)
            .is_some()
        {
            return Err(BrainError::DuplicateArm(observation.arm_id.clone()));
        }
    }
    let mut effective_metrics = BTreeMap::new();
    for model in &request.models {
        model.validate()?;
        let model_id = model.id();
        let metrics = request
            .model_health
            .get(&model_id)
            .map(|health| health.effective_metrics(model))
            .unwrap_or((model.reliability, model.latency_ms as f64));
        effective_metrics.insert(model_id, metrics);
    }
    let max_cost = request
        .models
        .iter()
        .map(|model| model.cost_per_million_tokens)
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let max_latency = effective_metrics
        .values()
        .map(|(_, latency)| *latency)
        .fold(1.0_f64, f64::max);
    let total_pulls = request
        .observations
        .iter()
        .map(|observation| observation.pulls)
        .sum::<u64>();
    let log_total = ((total_pulls + 1) as f64).ln();

    let mut ranking = Vec::with_capacity(request.models.len());
    for model in &request.models {
        let model_id = model.id();
        let (effective_reliability, effective_latency) = effective_metrics
            .get(&model_id)
            .copied()
            .unwrap_or((model.reliability, model.latency_ms as f64));
        let observation = observations.get(&model_id).copied();
        let mut reasons = Vec::new();
        if !model.enabled {
            reasons.push("disabled_by_caller".into());
        }
        if model.context_window_tokens
            < request
                .input_tokens
                .saturating_add(request.requested_output_tokens)
        {
            reasons.push("context_window_too_small".into());
        }
        if model.max_output_tokens < request.requested_output_tokens {
            reasons.push("max_output_tokens_too_small".into());
        }
        if let Some(max_cost) = request.max_cost_per_million_tokens {
            if model.cost_per_million_tokens > max_cost {
                reasons.push("cost_limit_exceeded".into());
            }
        }
        if let Some(max_latency) = request.max_latency_ms {
            if effective_latency > max_latency as f64 {
                reasons.push("latency_limit_exceeded".into());
            }
        }
        if let Some(min_quality) = request.min_quality {
            if model.quality < min_quality {
                reasons.push("quality_floor_not_met".into());
            }
        }
        for capability in &request.required_capabilities {
            if !model.capabilities.iter().any(|item| item == capability) {
                reasons.push(format!("missing_capability:{capability}"));
            }
        }
        if let Some(health) = request.provider_health.get(&model.provider) {
            if !health.registered {
                reasons.push("provider_unregistered".into());
            }
            if !health.credential_ready {
                reasons.push("provider_credential_unready".into());
            }
            if health.circuit == "open" {
                reasons.push("provider_circuit_open".into());
            }
            if !health.eligible {
                reasons.push("provider_health_ineligible".into());
            }
        }
        if observation.is_some_and(|item| item.disabled) {
            reasons.push("disabled_by_learning_policy".into());
        }
        let eligible = reasons.is_empty();
        let pulls = observation.map(|item| item.pulls).unwrap_or(0);
        let mean_reward = observation
            .filter(|item| item.pulls > 0)
            .map(|item| item.reward_sum / item.pulls as f64)
            .unwrap_or(0.0);
        let exploration_bonus = if pulls == 0 {
            request.weights.exploration
        } else {
            request.weights.exploration * (log_total / pulls as f64).sqrt()
        };
        let base_score = request.weights.quality * model.quality
            + request.weights.reliability * effective_reliability
            + request.weights.exploration * mean_reward
            - request.weights.cost * (model.cost_per_million_tokens as f64 / max_cost)
            - request.weights.latency * (effective_latency / max_latency);
        ranking.push(ModelCandidateScore {
            model_id,
            eligible,
            reasons,
            base_score,
            exploration_bonus,
            score: base_score + exploration_bonus,
            observed_pulls: pulls,
        });
    }
    ranking.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.model_id.cmp(&right.model_id))
    });
    let eligible_scores = ranking
        .iter()
        .filter(|candidate| candidate.eligible)
        .collect::<Vec<_>>();
    let eligible_model_count = eligible_scores.len();
    let selection_confidence = match eligible_scores.as_slice() {
        [] => 0.0,
        [_] => 1.0,
        [top, runner_up, ..] => ((top.score - runner_up.score)
            / (1.0 + top.score.abs() + runner_up.score.abs()))
        .clamp(0.0, 1.0),
    };
    let confidence_abstention = request
        .min_selection_confidence
        .is_some_and(|threshold| selection_confidence < threshold);
    let selected_model_id = ranking
        .iter()
        .find(|candidate| candidate.eligible)
        .filter(|_| !confidence_abstention)
        .map(|candidate| candidate.model_id.clone());
    let selected_model = selected_model_id.as_ref().and_then(|id| {
        request
            .models
            .iter()
            .find(|model| model.id() == *id)
            .cloned()
    });
    let selection_status = if selected_model_id.is_some() {
        "selected"
    } else if confidence_abstention {
        "abstained_low_selection_confidence"
    } else {
        "refused_no_eligible_model"
    };
    let mut report = ModelSelectionReport {
        schema: MODEL_SELECTION_SCHEMA.into(),
        task: request.task.clone(),
        selected_model,
        selected_model_id,
        ranking,
        eligible_model_count,
        selection_confidence,
        min_selection_confidence: request.min_selection_confidence,
        selection_status: selection_status.into(),
        decision_digest: String::new(),
        does_not_claim: vec![
            "model quality priors are caller-supplied and are not an evaluation result".into(),
            "selection confidence measures normalized rank separation, not answer correctness"
                .into(),
            "selection does not authenticate a provider or redeem a credential".into(),
            "selection does not execute a model call or verify a future answer".into(),
        ],
    };
    let digest_input = report.clone();
    report.decision_digest = digest(&digest_input)?;
    if report.selected_model_id.is_none() {
        return Ok(report);
    }
    Ok(report)
}

/// Stable, non-secret context labels used to keep online model observations scoped to a domain
/// and risk posture. The raw task remains in the base selection request; this structure is safe to
/// retain in a learning ledger and its digest is the join key for contextual observations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSelectionContext {
    pub domain: String,
    pub capability: String,
    pub risk_class: String,
    #[serde(default)]
    pub task_family: Option<String>,
}

impl ModelSelectionContext {
    fn validate(&self) -> Result<(), BrainError> {
        for (field, value) in [
            ("context.domain", &self.domain),
            ("context.capability", &self.capability),
            ("context.risk_class", &self.risk_class),
        ] {
            non_empty(value, field)?;
            if value.len() > MAX_CONTEXT_LABEL_BYTES {
                return Err(BrainError::TooMany {
                    field,
                    max: MAX_CONTEXT_LABEL_BYTES,
                });
            }
        }
        if let Some(task_family) = &self.task_family {
            non_empty(task_family, "context.task_family")?;
            if task_family.len() > MAX_CONTEXT_LABEL_BYTES {
                return Err(BrainError::TooMany {
                    field: "context.task_family",
                    max: MAX_CONTEXT_LABEL_BYTES,
                });
            }
        }
        Ok(())
    }
}

fn validate_context_binding(
    context_digest: Option<&String>,
    context: Option<&ModelSelectionContext>,
) -> Result<(), BrainError> {
    match (context_digest, context) {
        (None, None) => Ok(()),
        (Some(_), None) | (None, Some(_)) => Err(BrainError::ContextRequired),
        (Some(context_digest), Some(context)) => {
            context.validate()?;
            validate_digest_value(context_digest, "context_digest")?;
            if digest(context)? != *context_digest {
                return Err(BrainError::ContextIdentityMismatch);
            }
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualModelObservation {
    pub context_digest: String,
    pub arm_id: String,
    #[serde(default)]
    pub pulls: u64,
    #[serde(default)]
    pub reward_sum: f64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualModelSelectionRequest {
    pub context: ModelSelectionContext,
    pub base: ModelSelectionRequest,
    #[serde(default)]
    pub observations: Vec<ContextualModelObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualModelSelectionReport {
    pub schema: String,
    pub context: ModelSelectionContext,
    pub context_digest: String,
    pub selection: ModelSelectionReport,
    pub contextual_observations_used: usize,
    pub global_observation_fallbacks: usize,
    pub selection_status: String,
    pub does_not_claim: Vec<String>,
}

/// Select a model with observations scoped to one domain/capability/risk context.
///
/// Exact contextual observations override global observations for the same model arm. Missing
/// contextual observations fall back to the base request's global observation, so a new domain is
/// exploratory without erasing useful system-wide history. The server retains no hidden state.
pub fn select_model_contextual(
    request: &ContextualModelSelectionRequest,
) -> Result<ContextualModelSelectionReport, BrainError> {
    request.context.validate()?;
    let context_digest = digest(&request.context)?;
    let mut base = request.base.clone();
    let global_arm_ids = base
        .observations
        .iter()
        .map(|observation| observation.arm_id.clone())
        .collect::<BTreeSet<_>>();
    let mut contextual_by_arm = BTreeMap::new();
    for observation in &request.observations {
        if observation.context_digest != context_digest {
            return Err(BrainError::ContextDigestMismatch);
        }
        if contextual_by_arm
            .insert(observation.arm_id.clone(), observation)
            .is_some()
        {
            return Err(BrainError::DuplicateContextObservation(
                observation.arm_id.clone(),
            ));
        }
    }
    let mut merged = base.observations.clone();
    let global_fallbacks = request
        .base
        .observations
        .iter()
        .filter(|observation| !contextual_by_arm.contains_key(&observation.arm_id))
        .count();
    for observation in contextual_by_arm.values() {
        let replacement = ModelObservation {
            arm_id: observation.arm_id.clone(),
            pulls: observation.pulls,
            reward_sum: observation.reward_sum,
            failures: observation.failures,
            disabled: observation.disabled,
        };
        if let Some(existing) = merged
            .iter_mut()
            .find(|existing| existing.arm_id == replacement.arm_id)
        {
            *existing = replacement;
        } else {
            merged.push(replacement);
        }
    }
    base.observations = merged;
    let selection = select_model(&base)?;
    let contextual_observations_used = request.observations.len();
    let selection_status = if contextual_observations_used == 0 {
        "contextual_selection_global_history_only"
    } else if global_fallbacks == global_arm_ids.len() {
        "contextual_selection_exact_history"
    } else {
        "contextual_selection_mixed_history"
    };
    Ok(ContextualModelSelectionReport {
        schema: CONTEXTUAL_MODEL_SELECTION_SCHEMA.into(),
        context: request.context.clone(),
        context_digest,
        selection,
        contextual_observations_used,
        global_observation_fallbacks: global_fallbacks,
        selection_status: selection_status.into(),
        does_not_claim: vec![
            "context labels scope observations but do not prove domain similarity".into(),
            "a contextual reward remains evaluator-supplied and does not verify a model answer"
                .into(),
            "the contextual selector does not authenticate providers or redeem credentials".into(),
        ],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptChunk {
    pub id: String,
    #[serde(default = "default_user_role")]
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub priority: i32,
}

fn default_user_role() -> String {
    "user".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAssemblyRequest {
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub developer: Option<String>,
    pub task: String,
    #[serde(default)]
    pub context: Vec<PromptChunk>,
    #[serde(default)]
    pub output_contract: Option<String>,
    pub max_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAssemblyReport {
    pub schema: String,
    pub messages: Vec<PromptMessage>,
    pub included_context_ids: Vec<String>,
    pub omitted_context_ids: Vec<String>,
    pub estimated_input_tokens: u64,
    pub complete: bool,
    pub prompt_digest: String,
    pub warnings: Vec<String>,
}

fn estimate_tokens(text: &str) -> u64 {
    ((text.chars().count() as u64).saturating_add(3) / 4).max(1)
}

fn validate_role(role: &str) -> Result<(), BrainError> {
    if matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
        Ok(())
    } else {
        Err(BrainError::Empty {
            field: "prompt role",
        })
    }
}

/// Assemble a bounded prompt while preserving the IDs of context that did not fit. Required
/// material fails closed; optional material is omitted explicitly rather than silently truncated.
pub fn assemble_prompt(
    request: &PromptAssemblyRequest,
) -> Result<PromptAssemblyReport, BrainError> {
    non_empty(&request.task, "task")?;
    if request.max_input_tokens == 0 {
        return Err(BrainError::OutOfRange {
            field: "max_input_tokens",
            min: 1.0,
            max: u64::MAX as f64,
        });
    }
    if request.context.len() > MAX_PROMPT_CHUNKS {
        return Err(BrainError::TooMany {
            field: "context",
            max: MAX_PROMPT_CHUNKS,
        });
    }
    for chunk in &request.context {
        non_empty(&chunk.id, "context.id")?;
        non_empty(&chunk.content, "context.content")?;
        validate_role(&chunk.role)?;
    }
    let mut messages = Vec::new();
    if let Some(system) = &request.system {
        if !system.is_empty() {
            messages.push(PromptMessage {
                role: "system".into(),
                content: system.clone(),
                source_id: "system".into(),
            });
        }
    }
    if let Some(developer) = &request.developer {
        if !developer.is_empty() {
            messages.push(PromptMessage {
                role: "developer".into(),
                content: developer.clone(),
                source_id: "developer".into(),
            });
        }
    }
    let mut base_tokens = messages
        .iter()
        .map(|message| estimate_tokens(&message.content))
        .sum::<u64>();
    if let Some(contract) = &request.output_contract {
        if !contract.is_empty() {
            base_tokens = base_tokens.saturating_add(estimate_tokens(contract));
        }
    }
    base_tokens = base_tokens.saturating_add(estimate_tokens(&request.task));
    if base_tokens > request.max_input_tokens {
        return Err(BrainError::RequiredPromptDoesNotFit);
    }
    let mut chunks = request.context.iter().collect::<Vec<_>>();
    chunks.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut included_context_ids = Vec::new();
    let mut omitted_context_ids = Vec::new();
    let mut tokens = base_tokens;
    for chunk in chunks {
        let cost = estimate_tokens(&chunk.content);
        if tokens.saturating_add(cost) <= request.max_input_tokens {
            tokens = tokens.saturating_add(cost);
            included_context_ids.push(chunk.id.clone());
            messages.push(PromptMessage {
                role: chunk.role.clone(),
                content: chunk.content.clone(),
                source_id: chunk.id.clone(),
            });
        } else if chunk.required {
            return Err(BrainError::RequiredPromptDoesNotFit);
        } else {
            omitted_context_ids.push(chunk.id.clone());
        }
    }
    let mut task_content = request.task.clone();
    if let Some(contract) = &request.output_contract {
        if !contract.is_empty() {
            task_content.push_str("\n\nOutput contract:\n");
            task_content.push_str(contract);
        }
    }
    messages.push(PromptMessage {
        role: "user".into(),
        content: task_content,
        source_id: "task".into(),
    });
    let complete = omitted_context_ids.is_empty();
    let mut report = PromptAssemblyReport {
        schema: PROMPT_ASSEMBLY_SCHEMA.into(),
        messages,
        included_context_ids,
        omitted_context_ids,
        estimated_input_tokens: tokens,
        complete,
        prompt_digest: String::new(),
        warnings: Vec::new(),
    };
    if !report.complete {
        report.warnings.push(
            "optional context was omitted to satisfy the input budget; omission is not zero influence".into(),
        );
    }
    let digest_input = report.clone();
    report.prompt_digest = digest(&digest_input)?;
    Ok(report)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanEffect {
    #[default]
    ReadOnly,
    ProviderCall,
    ExternalWrite,
    Irreversible,
}

impl PlanEffect {
    fn needs_approval(&self) -> bool {
        !matches!(self, Self::ReadOnly)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanStep {
    pub id: String,
    pub objective: String,
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub effect: PlanEffect,
    #[serde(default)]
    pub estimated_cost: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomousPlanRequest {
    pub objective: String,
    pub steps: Vec<PlanStep>,
    pub allowed_tools: Vec<String>,
    pub max_cost: u64,
    #[serde(default = "default_true")]
    pub require_approval_for_effects: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomousPlan {
    pub schema: String,
    pub objective: String,
    pub ordered_step_ids: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub estimated_cost: u64,
    pub requires_approval: bool,
    pub execution: String,
    pub plan_digest: String,
    pub does_not_claim: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutonomousPlanReport {
    pub ok: bool,
    pub status: String,
    pub plan: Option<AutonomousPlan>,
    pub errors: Vec<String>,
}

/// Validate and topologically order a proposed plan. This is planning, never execution: the
/// returned `execution` field is always `not_started`, and non-read-only steps remain approval
/// gated even when the dependency graph is valid.
pub fn plan_autonomous(
    request: &AutonomousPlanRequest,
) -> Result<AutonomousPlanReport, BrainError> {
    non_empty(&request.objective, "objective")?;
    if request.steps.is_empty() {
        return Ok(AutonomousPlanReport {
            ok: false,
            status: "refused_empty_plan".into(),
            plan: None,
            errors: vec!["at least one step is required".into()],
        });
    }
    if request.steps.len() > MAX_PLAN_STEPS {
        return Err(BrainError::TooMany {
            field: "steps",
            max: MAX_PLAN_STEPS,
        });
    }
    let allowed = request
        .allowed_tools
        .iter()
        .filter(|tool| !tool.is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut by_id = BTreeMap::new();
    let mut errors = Vec::new();
    for step in &request.steps {
        non_empty(&step.id, "step.id")?;
        non_empty(&step.objective, "step.objective")?;
        non_empty(&step.tool, "step.tool")?;
        if step.tool.len() > MAX_TOOL_NAME_BYTES {
            errors.push(format!("step {} tool name is too long", step.id));
        }
        if !allowed.contains(&step.tool) {
            errors.push(
                BrainError::ToolNotAllowed {
                    step: step.id.clone(),
                }
                .to_string(),
            );
        }
        if by_id.insert(step.id.clone(), step).is_some() {
            errors.push(format!("duplicate step id {:?}", step.id));
        }
    }
    for step in &request.steps {
        for dependency in &step.depends_on {
            if dependency == &step.id {
                errors.push(format!("step {:?} depends on itself", step.id));
            } else if !by_id.contains_key(dependency) {
                errors.push(
                    BrainError::UnknownDependency {
                        step: step.id.clone(),
                        dependency: dependency.clone(),
                    }
                    .to_string(),
                );
            }
        }
    }
    let estimated_cost = request
        .steps
        .iter()
        .map(|step| step.estimated_cost)
        .sum::<u64>();
    if estimated_cost > request.max_cost {
        errors.push(format!(
            "estimated cost {} exceeds max cost {}",
            estimated_cost, request.max_cost
        ));
    }
    if !errors.is_empty() {
        return Ok(AutonomousPlanReport {
            ok: false,
            status: "refused_policy_or_shape".into(),
            plan: None,
            errors,
        });
    }

    let mut indegree = request
        .steps
        .iter()
        .map(|step| (step.id.clone(), step.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for step in &request.steps {
        for dependency in &step.depends_on {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(step.id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered_step_ids = Vec::with_capacity(request.steps.len());
    while let Some(id) = ready.pop_first() {
        ordered_step_ids.push(id.clone());
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("dependent was validated against the step map");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if ordered_step_ids.len() != request.steps.len() {
        return Ok(AutonomousPlanReport {
            ok: false,
            status: "refused_dependency_cycle".into(),
            plan: None,
            errors: vec![BrainError::PlanCycle.to_string()],
        });
    }
    let steps = ordered_step_ids
        .iter()
        .map(|id| (*by_id.get(id).expect("ordered id was validated")).clone())
        .collect::<Vec<_>>();
    let requires_approval = request.require_approval_for_effects
        && steps.iter().any(|step| step.effect.needs_approval());
    let mut plan = AutonomousPlan {
        schema: PLAN_SCHEMA.into(),
        objective: request.objective.clone(),
        ordered_step_ids,
        steps,
        estimated_cost,
        requires_approval,
        execution: "not_started".into(),
        plan_digest: String::new(),
        does_not_claim: vec![
            "a valid DAG is not evidence that a tool call will succeed".into(),
            "provider calls and external effects remain outside this planning kernel".into(),
            "approval is required before non-read-only execution".into(),
        ],
    };
    let digest_input = plan.clone();
    plan.plan_digest = digest(&digest_input)?;
    Ok(AutonomousPlanReport {
        ok: true,
        status: if requires_approval {
            "planned_approval_required"
        } else {
            "planned_ready_for_caller_execution"
        }
        .into(),
        plan: Some(plan),
        errors: Vec::new(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditPolicy {
    #[serde(default = "default_bandit_strategy")]
    pub strategy: String,
    #[serde(default = "default_bandit_exploration")]
    pub exploration: f64,
    #[serde(default = "default_bandit_epsilon")]
    pub epsilon: f64,
    #[serde(default = "default_bandit_min_reward")]
    pub min_reward: f64,
    #[serde(default = "default_bandit_max_reward")]
    pub max_reward: f64,
    #[serde(default = "default_bandit_failure_penalty")]
    pub failure_penalty: f64,
    /// A caller-selected seed makes exploration reproducible and replayable. It is not secret.
    #[serde(default)]
    pub seed: u64,
}

fn default_bandit_strategy() -> String {
    "ucb1".into()
}
fn default_bandit_exploration() -> f64 {
    0.50
}
fn default_bandit_epsilon() -> f64 {
    0.10
}
fn default_bandit_min_reward() -> f64 {
    -1.0
}
fn default_bandit_max_reward() -> f64 {
    1.0
}
fn default_bandit_failure_penalty() -> f64 {
    0.25
}

impl Default for BanditPolicy {
    fn default() -> Self {
        Self {
            strategy: default_bandit_strategy(),
            exploration: default_bandit_exploration(),
            epsilon: default_bandit_epsilon(),
            min_reward: default_bandit_min_reward(),
            max_reward: default_bandit_max_reward(),
            failure_penalty: default_bandit_failure_penalty(),
            seed: 0,
        }
    }
}

impl BanditPolicy {
    fn validate(&self) -> Result<(), BrainError> {
        if !matches!(
            self.strategy.as_str(),
            "ucb1" | "epsilon_greedy" | "thompson_sampling"
        ) {
            return Err(BrainError::InvalidBanditStrategy(self.strategy.clone()));
        }
        finite_range(self.exploration, "bandit.exploration", 0.0, 100.0)?;
        finite_range(self.epsilon, "bandit.epsilon", 0.0, 1.0)?;
        finite_range(self.min_reward, "bandit.min_reward", -100.0, 100.0)?;
        finite_range(self.max_reward, "bandit.max_reward", -100.0, 100.0)?;
        finite_range(self.failure_penalty, "bandit.failure_penalty", 0.0, 100.0)?;
        if self.min_reward >= self.max_reward {
            return Err(BrainError::InvalidReward);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditArm {
    pub arm_id: String,
    #[serde(default)]
    pub pulls: u64,
    #[serde(default)]
    pub reward_sum: f64,
    #[serde(default)]
    pub failures: u64,
    #[serde(default)]
    pub disabled: bool,
}

/// Caller-persisted bandit observations scoped to one stable model-selection context.
///
/// The context labels are bounded routing identity, not task text. Keeping the arms nested under
/// the digest prevents evaluator feedback from one domain or risk posture from silently changing
/// another domain's model policy while preserving the legacy global arm ledger for cold starts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextualBanditState {
    pub context_digest: String,
    pub context: ModelSelectionContext,
    #[serde(default)]
    pub generation: u64,
    pub arms: Vec<BanditArm>,
    #[serde(default)]
    pub observed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreditedOutcome {
    pub outcome_digest: String,
    pub arm_id: String,
    pub reward: f64,
    #[serde(default)]
    pub failed: bool,
    /// Digest of the evaluator contract that produced this credit. Optional for legacy direct
    /// bandit updates that do not carry an evaluator boundary.
    #[serde(default)]
    pub contract_digest: Option<String>,
    /// Context identity that received this evaluator credit. Absent for legacy global updates.
    #[serde(default)]
    pub context_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditState {
    pub schema: String,
    #[serde(default)]
    pub generation: u64,
    #[serde(default)]
    pub policy: BanditPolicy,
    pub arms: Vec<BanditArm>,
    /// Lowercase outcome digests already credited to this caller-owned state. Keeping this
    /// bounded ledger in the state makes evaluator settlement replay-safe across retries and
    /// process restarts without retaining prompts, responses, or credentials.
    #[serde(default)]
    pub credited_outcomes: Vec<CreditedOutcome>,
    /// Bounded contextual ledgers. The top-level `arms` remain the legacy global cold-start
    /// history, while each contextual row is independently replay-safe and evaluator-scoped.
    #[serde(default)]
    pub contextual_states: Vec<ContextualBanditState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditCandidateScore {
    pub arm_id: String,
    pub pulls: u64,
    pub mean_reward: f64,
    pub exploration_bonus: f64,
    pub failure_rate: f64,
    pub score: f64,
    pub eligible: bool,
    /// Beta posterior parameters are emitted only for Thompson-sampling selections. Keeping
    /// them optional preserves the legacy wire shape for UCB1 and epsilon-greedy callers while
    /// making Bayesian exploration auditable and replayable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posterior_alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posterior_beta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posterior_sample: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditSelectionReport {
    pub schema: String,
    pub selected_arm_id: Option<String>,
    pub ranking: Vec<BanditCandidateScore>,
    pub selection_status: String,
    pub state_generation: u64,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub exploration_draw: Option<f64>,
    #[serde(default)]
    pub exploration_taken: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BanditUpdate {
    pub arm_id: String,
    pub reward: f64,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub outcome_digest: Option<String>,
    #[serde(default)]
    pub contract_digest: Option<String>,
    /// Optional contextual identity. Both fields must be supplied together and the digest must
    /// equal the canonical digest of `context`.
    #[serde(default)]
    pub context_digest: Option<String>,
    #[serde(default)]
    pub context: Option<ModelSelectionContext>,
}

/// Value-only identity for one provider-backed brain run.
///
/// The identity binds the evaluator's later reward to the exact selection, prompt, plan, and
/// provider outcome without retaining prompt text, provider response text, or credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainRunIdentity {
    pub run_id: String,
    pub selection_digest: String,
    pub prompt_digest: String,
    pub plan_digest: String,
    pub provider: String,
    pub model: String,
    pub outcome_digest: String,
    #[serde(default)]
    pub request_id: Option<String>,
}

/// An explicit evaluator judgment. The brain never derives this from a provider response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainEvaluatorAssessment {
    pub evaluator_id: String,
    pub evaluator_version: String,
    pub reward: f64,
    pub passed: bool,
    #[serde(default)]
    pub failed: bool,
    /// Digest of evaluator-side notes. Raw notes and provider response text never cross this API.
    #[serde(default)]
    pub feedback_digest: Option<String>,
    #[serde(default)]
    pub failure_class: Option<String>,
    #[serde(default)]
    pub evidence_digest: Option<String>,
}

/// Input to the append-only learning boundary. State is caller-owned and returned by value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainOutcomeRecordRequest {
    pub run: BrainRunIdentity,
    pub assessment: BrainEvaluatorAssessment,
    pub bandit_state: BanditState,
    pub arm_id: String,
    /// Optional contextual identity for evaluator credit. Legacy callers may omit both fields.
    #[serde(default)]
    pub context_digest: Option<String>,
    #[serde(default)]
    pub context: Option<ModelSelectionContext>,
    /// Optional caller-owned idempotency identity for transports that retain a replay cache.
    /// The outcome digest in `run` remains the durable learning identity.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainLearningEvidence {
    pub schema: String,
    pub run: BrainRunIdentity,
    pub assessment: BrainEvaluatorAssessment,
    pub arm_id: String,
    #[serde(default)]
    pub context_digest: Option<String>,
    pub bandit_update: BanditUpdate,
    pub previous_generation: u64,
    pub next_generation: u64,
    pub next_state_digest: String,
    pub evidence_digest: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    pub does_not_claim: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrainOutcomeRecordReport {
    pub ok: bool,
    pub status: String,
    pub next_state: BanditState,
    pub learning_evidence: BrainLearningEvidence,
}

fn validate_brain_run_identity(run: &BrainRunIdentity) -> Result<(), BrainError> {
    non_empty(&run.run_id, "run.run_id")?;
    non_empty(&run.provider, "run.provider")?;
    non_empty(&run.model, "run.model")?;
    validate_digest_value(&run.selection_digest, "run.selection_digest")?;
    validate_digest_value(&run.prompt_digest, "run.prompt_digest")?;
    validate_digest_value(&run.plan_digest, "run.plan_digest")?;
    validate_digest_value(&run.outcome_digest, "run.outcome_digest")?;
    if let Some(request_id) = &run.request_id {
        non_empty(request_id, "run.request_id")?;
    }
    Ok(())
}

fn validate_brain_assessment(assessment: &BrainEvaluatorAssessment) -> Result<(), BrainError> {
    non_empty(&assessment.evaluator_id, "assessment.evaluator_id")?;
    non_empty(
        &assessment.evaluator_version,
        "assessment.evaluator_version",
    )?;
    if assessment.evaluator_id.len() > MAX_EVALUATOR_ID_BYTES
        || assessment.evaluator_version.len() > MAX_EVALUATOR_ID_BYTES
    {
        return Err(BrainError::TooMany {
            field: "assessment evaluator metadata",
            max: MAX_EVALUATOR_ID_BYTES,
        });
    }
    if assessment.passed && assessment.failed {
        return Err(BrainError::ContradictoryAssessment);
    }
    if let Some(feedback_digest) = &assessment.feedback_digest {
        validate_digest_value(feedback_digest, "assessment.feedback_digest")?;
    }
    if let Some(failure_class) = &assessment.failure_class {
        non_empty(failure_class, "assessment.failure_class")?;
        if failure_class.len() > MAX_EVALUATOR_ID_BYTES {
            return Err(BrainError::TooMany {
                field: "assessment.failure_class",
                max: MAX_EVALUATOR_ID_BYTES,
            });
        }
    }
    if let Some(evidence_digest) = &assessment.evidence_digest {
        validate_digest_value(evidence_digest, "assessment.evidence_digest")?;
    }
    Ok(())
}

/// Bind one explicit evaluator judgment to a run and advance caller-owned bandit state.
///
/// This is the durable-learning contract's value layer: applications persist the returned
/// `learning_evidence` and `next_state` in their own store. No provider text, secret, or hidden
/// server memory participates in the update.
pub fn record_brain_outcome(
    request: &BrainOutcomeRecordRequest,
) -> Result<BrainOutcomeRecordReport, BrainError> {
    validate_brain_run_identity(&request.run)?;
    validate_brain_assessment(&request.assessment)?;
    non_empty(&request.arm_id, "arm_id")?;
    if let Some(idempotency_key) = &request.idempotency_key {
        non_empty(idempotency_key, "idempotency_key")?;
        if idempotency_key.len() > MAX_CONTEXT_LABEL_BYTES {
            return Err(BrainError::TooMany {
                field: "idempotency_key",
                max: MAX_CONTEXT_LABEL_BYTES,
            });
        }
    }
    validate_bandit_state(&request.bandit_state)?;
    validate_context_binding(request.context_digest.as_ref(), request.context.as_ref())?;
    let credited_outcome_digest = digest(&json!({
        "run_id": request.run.run_id.clone(),
        "outcome_digest": request.run.outcome_digest.clone(),
    }))?;
    let contract_digest = digest(&json!({
        "run_id": request.run.run_id.clone(),
        "outcome_digest": request.run.outcome_digest.clone(),
        "arm_id": request.arm_id.clone(),
        "context_digest": request.context_digest.clone(),
        "assessment": request.assessment.clone(),
    }))?;
    let bandit_update = BanditUpdate {
        arm_id: request.arm_id.clone(),
        reward: request.assessment.reward,
        failed: request.assessment.failed,
        outcome_digest: Some(credited_outcome_digest.clone()),
        contract_digest: Some(contract_digest),
        context_digest: request.context_digest.clone(),
        context: request.context.clone(),
    };
    // Selection may be allowed to explore an unseen candidate, so an empty caller-owned state
    // is a valid first-run input to this evaluator boundary. Direct `update_bandit` remains
    // strict for callers that want to validate an already-materialized arm ledger; only this
    // higher-level outcome contract hydrates the selected arm before applying credit. Preserve an
    // exact replay, including its state shape, rather than adding an arm to a replay response.
    let state_for_update = if request
        .bandit_state
        .credited_outcomes
        .iter()
        .any(|known| known.outcome_digest == credited_outcome_digest)
    {
        request.bandit_state.clone()
    } else {
        hydrate_outcome_arm(
            &request.bandit_state,
            &request.arm_id,
            request.context_digest.as_ref(),
            request.context.as_ref(),
        )?
    };
    let next_state = update_bandit(&state_for_update, &bandit_update)?;
    let next_state_digest = digest(&next_state)?;
    let mut learning_evidence = BrainLearningEvidence {
        schema: LEARNING_EVIDENCE_SCHEMA.into(),
        run: request.run.clone(),
        assessment: request.assessment.clone(),
        arm_id: request.arm_id.clone(),
        context_digest: request.context_digest.clone(),
        bandit_update,
        previous_generation: request.bandit_state.generation,
        next_generation: next_state.generation,
        next_state_digest,
        evidence_digest: String::new(),
        idempotency_key: request.idempotency_key.clone(),
        does_not_claim: vec![
            "an evaluator reward is not proof that the provider answer is true".into(),
            "online adaptation is not a claim of general intelligence or biological learning".into(),
            "the ledger contains value-free digests and judgments, not credentials or response text".into(),
            "a passed evaluator does not grant tool permission, clinical validity, or release readiness".into(),
        ],
    };
    let digest_input = learning_evidence.clone();
    learning_evidence.evidence_digest = digest(&digest_input)?;
    Ok(BrainOutcomeRecordReport {
        ok: true,
        status: "recorded_evaluator_reward".into(),
        next_state,
        learning_evidence,
    })
}

/// Add the selected arm to the caller-owned ledger for the first evaluator settlement.
///
/// Model selection intentionally supports candidates that have no historical arm yet. The
/// outcome-recording boundary is the first place where that candidate must become persistent.
/// Contextual arms are hydrated inside their matching context row so the first credit cannot
/// accidentally leak into the global prior. This helper is deliberately not part of
/// `update_bandit`: direct low-level updates continue to reject unknown arms and therefore catch
/// malformed state transitions early.
fn hydrate_outcome_arm(
    state: &BanditState,
    arm_id: &str,
    context_digest: Option<&String>,
    context: Option<&ModelSelectionContext>,
) -> Result<BanditState, BrainError> {
    let mut hydrated = state.clone();
    if let Some(context_digest) = context_digest {
        let context = context.ok_or(BrainError::ContextRequired)?;
        if let Some(contextual) = hydrated
            .contextual_states
            .iter_mut()
            .find(|contextual| contextual.context_digest == *context_digest)
        {
            if contextual.arms.iter().all(|arm| arm.arm_id != arm_id) {
                contextual.arms.push(BanditArm {
                    arm_id: arm_id.into(),
                    pulls: 0,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                });
            }
        } else {
            if hydrated.contextual_states.len() >= MAX_CONTEXTUAL_STATES {
                return Err(BrainError::TooMany {
                    field: "bandit contextual states",
                    max: MAX_CONTEXTUAL_STATES,
                });
            }
            hydrated.contextual_states.push(ContextualBanditState {
                context_digest: context_digest.clone(),
                context: context.clone(),
                generation: 0,
                arms: vec![BanditArm {
                    arm_id: arm_id.into(),
                    pulls: 0,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                }],
                observed: false,
            });
        }
    } else if hydrated.arms.iter().all(|arm| arm.arm_id != arm_id) {
        hydrated.arms.push(BanditArm {
            arm_id: arm_id.into(),
            pulls: 0,
            reward_sum: 0.0,
            failures: 0,
            disabled: false,
        });
    }
    Ok(hydrated)
}

fn validate_bandit_arms(arms: &[BanditArm], policy: &BanditPolicy) -> Result<(), BrainError> {
    let mut seen = BTreeSet::new();
    for arm in arms {
        non_empty(&arm.arm_id, "arm.arm_id")?;
        finite_range(
            arm.reward_sum,
            "arm.reward_sum",
            policy.min_reward * arm.pulls as f64,
            policy.max_reward * arm.pulls as f64,
        )?;
        if arm.failures > arm.pulls {
            return Err(BrainError::OutOfRange {
                field: "arm.failures",
                min: 0.0,
                max: arm.pulls as f64,
            });
        }
        if !seen.insert(arm.arm_id.clone()) {
            return Err(BrainError::DuplicateArm(arm.arm_id.clone()));
        }
    }
    Ok(())
}

fn validate_bandit_state(state: &BanditState) -> Result<(), BrainError> {
    state.policy.validate()?;
    validate_bandit_arms(&state.arms, &state.policy)?;
    if state.contextual_states.len() > MAX_CONTEXTUAL_STATES {
        return Err(BrainError::TooMany {
            field: "bandit contextual states",
            max: MAX_CONTEXTUAL_STATES,
        });
    }
    let mut contextual_digests = BTreeSet::new();
    for contextual in &state.contextual_states {
        validate_context_binding(Some(&contextual.context_digest), Some(&contextual.context))?;
        if !contextual_digests.insert(contextual.context_digest.clone()) {
            return Err(BrainError::DuplicateContextState(
                contextual.context_digest.clone(),
            ));
        }
        validate_bandit_arms(&contextual.arms, &state.policy)?;
    }
    if state.credited_outcomes.len() > MAX_CREDITED_OUTCOMES {
        return Err(BrainError::TooMany {
            field: "bandit credited outcomes",
            max: MAX_CREDITED_OUTCOMES,
        });
    }
    let mut credited = BTreeSet::new();
    for outcome in &state.credited_outcomes {
        validate_digest_value(
            &outcome.outcome_digest,
            "bandit.credited_outcomes.outcome_digest",
        )?;
        non_empty(&outcome.arm_id, "bandit.credited_outcomes.arm_id")?;
        finite_range(
            outcome.reward,
            "bandit.credited_outcomes.reward",
            state.policy.min_reward,
            state.policy.max_reward,
        )?;
        if let Some(contract_digest) = &outcome.contract_digest {
            validate_digest_value(contract_digest, "bandit.credited_outcomes.contract_digest")?;
        }
        if let Some(context_digest) = &outcome.context_digest {
            validate_digest_value(context_digest, "bandit.credited_outcomes.context_digest")?;
        }
        if !credited.insert(&outcome.outcome_digest) {
            return Err(BrainError::DuplicateCreditedOutcome(
                outcome.outcome_digest.clone(),
            ));
        }
    }
    Ok(())
}

fn deterministic_bandit_draw_with_counter(
    seed: u64,
    generation: u64,
    label: &str,
    counter: u64,
) -> f64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_be_bytes());
    hasher.update(generation.to_be_bytes());
    hasher.update(label.as_bytes());
    hasher.update(counter.to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    // Keep the draw strictly inside (0, 1) so Box-Muller and inverse-power transforms never
    // receive log(0), even when a digest happens to begin or end with all zero bits.
    (u64::from_be_bytes(bytes) as f64 + 0.5) / (u64::MAX as f64 + 1.0)
}

fn deterministic_bandit_draw(seed: u64, generation: u64, label: &str) -> f64 {
    deterministic_bandit_draw_with_counter(seed, generation, label, 0)
}

fn standard_normal_from_uniforms(first: f64, second: f64) -> f64 {
    (-2.0 * first.ln()).sqrt() * (2.0 * std::f64::consts::PI * second).cos()
}

/// Draw a deterministic Gamma variate using Marsaglia--Tsang and a hash-backed uniform stream.
///
/// The kernel intentionally does not depend on an ambient RNG. The seed, generation, and arm
/// identity form the complete replay key, while the bounded retry loop prevents malformed or
/// adversarial state from turning selection into an unbounded computation.
fn deterministic_gamma_sample(shape: f64, seed: u64, generation: u64, label: &str) -> f64 {
    let shape = shape.max(1.0e-9);
    if shape < 1.0 {
        let shifted = deterministic_gamma_sample(shape + 1.0, seed, generation, label);
        let uniform = deterministic_bandit_draw_with_counter(seed, generation, label, 255);
        return shifted * uniform.powf(1.0 / shape);
    }
    let d = shape - (1.0 / 3.0);
    let c = (1.0 / (9.0 * d)).sqrt();
    for attempt in 0..32_u64 {
        let first = deterministic_bandit_draw_with_counter(seed, generation, label, attempt * 3);
        let second =
            deterministic_bandit_draw_with_counter(seed, generation, label, attempt * 3 + 1);
        let z = standard_normal_from_uniforms(first, second);
        let transformed = 1.0 + c * z;
        if transformed <= 0.0 {
            continue;
        }
        let v = transformed * transformed * transformed;
        let acceptance =
            deterministic_bandit_draw_with_counter(seed, generation, label, attempt * 3 + 2);
        if acceptance < 1.0 - 0.0331 * z.powi(4)
            || acceptance.ln() < 0.5 * z * z + d * (1.0 - v + v.ln())
        {
            return d * v;
        }
    }
    // The mean is a deterministic, finite fallback for the extremely unlikely bounded-retry
    // miss. It preserves selection availability without silently introducing a new RNG.
    shape
}

fn deterministic_beta_sample(
    alpha: f64,
    beta: f64,
    seed: u64,
    generation: u64,
    label: &str,
) -> f64 {
    let left = deterministic_gamma_sample(alpha, seed, generation, &format!("{label}/alpha"));
    let right = deterministic_gamma_sample(beta, seed, generation, &format!("{label}/beta"));
    let total = left + right;
    if total.is_finite() && total > 0.0 {
        (left / total).clamp(0.0, 1.0)
    } else {
        (alpha / (alpha + beta)).clamp(0.0, 1.0)
    }
}

fn thompson_posterior(
    arm: &BanditArm,
    policy: &BanditPolicy,
    seed: u64,
    generation: u64,
) -> (f64, f64, f64, f64) {
    let span = policy.max_reward - policy.min_reward;
    let pulls = arm.pulls as f64;
    // Continuous evaluator rewards become fractional Bernoulli evidence. This preserves the
    // evaluator's bounded score while giving failures an additional explicit safety penalty.
    let normalized_success_mass = if pulls == 0.0 {
        0.0
    } else {
        ((arm.reward_sum - policy.min_reward * pulls) / span).clamp(0.0, pulls)
    };
    let normalized_failure_mass =
        (pulls - normalized_success_mass + policy.failure_penalty * arm.failures as f64).max(0.0);
    let alpha = 1.0 + normalized_success_mass;
    let beta = 1.0 + normalized_failure_mass;
    let sample = deterministic_beta_sample(alpha, beta, seed, generation, &arm.arm_id);
    let sampled_reward = policy.min_reward + sample * span;
    (alpha, beta, sample, sampled_reward)
}

/// Select an arm using the configured bounded policy. UCB1 keeps the historical behaviour;
/// epsilon-greedy adds deterministic seeded exploration so a selection can be replayed exactly.
/// Thompson sampling draws a deterministic sample from each arm's evaluator-reward posterior;
/// it is Bayesian exploration, not provider-success learning, and remains caller-state driven.
/// Arms with no observations receive the full UCB exploration coefficient, so a good prior cannot
/// permanently starve an untested model.
pub fn select_bandit_arm(state: &BanditState) -> Result<BanditSelectionReport, BrainError> {
    validate_bandit_state(state)?;
    let total_pulls = state.arms.iter().map(|arm| arm.pulls).sum::<u64>();
    let log_total = ((total_pulls + 1) as f64).ln();
    let use_ucb = state.policy.strategy == "ucb1";
    let use_thompson = state.policy.strategy == "thompson_sampling";
    let mut ranking = state
        .arms
        .iter()
        .map(|arm| {
            let mean_reward = if arm.pulls == 0 {
                0.0
            } else {
                arm.reward_sum / arm.pulls as f64
            };
            let failure_rate = if arm.pulls == 0 {
                0.0
            } else {
                arm.failures as f64 / arm.pulls as f64
            };
            let (posterior_alpha, posterior_beta, posterior_sample, sampled_reward) =
                if use_thompson {
                    let (alpha, beta, sample, reward) =
                        thompson_posterior(arm, &state.policy, state.policy.seed, state.generation);
                    (Some(alpha), Some(beta), Some(sample), Some(reward))
                } else {
                    (None, None, None, None)
                };
            let exploration_bonus = if let Some(sampled_reward) = sampled_reward {
                sampled_reward - mean_reward
            } else if use_ucb && arm.pulls == 0 {
                state.policy.exploration
            } else if use_ucb {
                state.policy.exploration * (log_total / arm.pulls as f64).sqrt()
            } else {
                0.0
            };
            let score = if let Some(sampled_reward) = sampled_reward {
                sampled_reward - state.policy.failure_penalty * failure_rate
            } else {
                mean_reward + exploration_bonus - state.policy.failure_penalty * failure_rate
            };
            BanditCandidateScore {
                arm_id: arm.arm_id.clone(),
                pulls: arm.pulls,
                mean_reward,
                exploration_bonus,
                failure_rate,
                score,
                eligible: !arm.disabled,
                posterior_alpha,
                posterior_beta,
                posterior_sample,
            }
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.arm_id.cmp(&right.arm_id))
    });
    let exploitation_arm_id = ranking
        .iter()
        .find(|candidate| candidate.eligible)
        .map(|candidate| candidate.arm_id.clone());
    let exploration_draw = if state.policy.strategy == "epsilon_greedy" {
        Some(deterministic_bandit_draw(
            state.policy.seed,
            state.generation,
            "epsilon",
        ))
    } else {
        None
    };
    let exploration_taken = exploration_draw
        .map(|draw| draw < state.policy.epsilon)
        .unwrap_or(false);
    let selected_arm_id = if exploration_taken {
        let eligible = ranking
            .iter()
            .filter(|candidate| candidate.eligible)
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            None
        } else {
            let draw =
                deterministic_bandit_draw(state.policy.seed, state.generation, "epsilon-arm");
            let index = ((draw * eligible.len() as f64).floor() as usize)
                .min(eligible.len().saturating_sub(1));
            Some(eligible[index].arm_id.clone())
        }
    } else {
        exploitation_arm_id.clone()
    };
    Ok(BanditSelectionReport {
        schema: BANDIT_SCHEMA.into(),
        selected_arm_id: selected_arm_id.clone(),
        ranking,
        selection_status: if selected_arm_id.is_none() {
            "refused_no_eligible_arm".into()
        } else if use_thompson {
            "selected_thompson_sample".into()
        } else if exploration_taken {
            "selected_exploration".into()
        } else {
            "selected".into()
        },
        state_generation: state.generation,
        strategy: state.policy.strategy.clone(),
        exploration_draw,
        exploration_taken: exploration_taken || use_thompson,
    })
}

/// Select from a context-scoped arm ledger with global history as a cold-start fallback.
///
/// The returned report keeps the existing value-only selection shape so old callers remain
/// compatible. The caller supplies the context identity explicitly; no server-side learner state
/// is consulted or mutated.
pub fn select_bandit_arm_contextual(
    state: &BanditState,
    context_digest: &str,
    context: &ModelSelectionContext,
) -> Result<BanditSelectionReport, BrainError> {
    let context_digest_owned = context_digest.to_string();
    validate_context_binding(Some(&context_digest_owned), Some(context))?;
    validate_bandit_state(state)?;
    let mut effective = state.clone();
    if let Some(contextual) = state
        .contextual_states
        .iter()
        .find(|contextual| contextual.context_digest == context_digest)
    {
        let mut by_arm = effective
            .arms
            .into_iter()
            .map(|arm| (arm.arm_id.clone(), arm))
            .collect::<BTreeMap<_, _>>();
        for arm in &contextual.arms {
            by_arm.insert(arm.arm_id.clone(), arm.clone());
        }
        effective.arms = by_arm.into_values().collect();
    }
    select_bandit_arm(&effective)
}

/// Apply one explicit evaluator reward and return the new value-bearing state. The caller owns
/// persistence and must supply a digest of the evaluated outcome when it wants a replay link.
pub fn update_bandit(
    state: &BanditState,
    update: &BanditUpdate,
) -> Result<BanditState, BrainError> {
    validate_bandit_state(state)?;
    state.policy.validate()?;
    validate_context_binding(update.context_digest.as_ref(), update.context.as_ref())?;
    finite_range(
        update.reward,
        "update.reward",
        state.policy.min_reward,
        state.policy.max_reward,
    )?;
    if let Some(outcome_digest) = &update.outcome_digest {
        validate_digest_value(outcome_digest, "update.outcome_digest")?;
        if let Some(contract_digest) = &update.contract_digest {
            validate_digest_value(contract_digest, "update.contract_digest")?;
        }
        if let Some(prior) = state
            .credited_outcomes
            .iter()
            .find(|known| known.outcome_digest == *outcome_digest)
        {
            if prior.arm_id != update.arm_id
                || prior.reward != update.reward
                || prior.failed != update.failed
                || prior.contract_digest != update.contract_digest
                || prior.context_digest != update.context_digest
            {
                return Err(BrainError::ConflictingCreditedOutcome(
                    outcome_digest.clone(),
                ));
            }
            return Ok(state.clone());
        }
    }
    let mut next = state.clone();
    let contextual_index = update.context_digest.as_ref().map(|context_digest| {
        next.contextual_states
            .iter()
            .position(|contextual| contextual.context_digest == *context_digest)
    });
    let arm_collection = if let Some(Some(index)) = contextual_index {
        &mut next.contextual_states[index].arms
    } else if update.context_digest.is_some() {
        let context = update.context.clone().ok_or(BrainError::ContextRequired)?;
        let context_digest = update
            .context_digest
            .clone()
            .ok_or(BrainError::ContextRequired)?;
        if next.contextual_states.len() >= MAX_CONTEXTUAL_STATES {
            return Err(BrainError::TooMany {
                field: "bandit contextual states",
                max: MAX_CONTEXTUAL_STATES,
            });
        }
        next.contextual_states.push(ContextualBanditState {
            context_digest,
            context,
            generation: 0,
            arms: Vec::new(),
            observed: false,
        });
        &mut next
            .contextual_states
            .last_mut()
            .expect("contextual state was pushed")
            .arms
    } else {
        &mut next.arms
    };
    let arm = arm_collection
        .iter_mut()
        .find(|arm| arm.arm_id == update.arm_id)
        .ok_or_else(|| BrainError::UnknownArm(update.arm_id.clone()))?;
    if arm.disabled {
        return Err(BrainError::UnknownArm(update.arm_id.clone()));
    }
    arm.pulls = arm.pulls.saturating_add(1);
    arm.reward_sum += update.reward;
    if update.failed {
        arm.failures = arm.failures.saturating_add(1);
    }
    if let Some(outcome_digest) = &update.outcome_digest {
        if next.credited_outcomes.len() >= MAX_CREDITED_OUTCOMES {
            return Err(BrainError::TooMany {
                field: "bandit credited outcomes",
                max: MAX_CREDITED_OUTCOMES,
            });
        }
        next.credited_outcomes.push(CreditedOutcome {
            outcome_digest: outcome_digest.clone(),
            arm_id: update.arm_id.clone(),
            reward: update.reward,
            failed: update.failed,
            contract_digest: update.contract_digest.clone(),
            context_digest: update.context_digest.clone(),
        });
    }
    if let Some(context_digest) = &update.context_digest {
        let contextual = next
            .contextual_states
            .iter_mut()
            .find(|contextual| contextual.context_digest == *context_digest)
            .ok_or(BrainError::ContextIdentityMismatch)?;
        contextual.generation = contextual.generation.saturating_add(1);
        contextual.observed = true;
    }
    next.generation = next.generation.saturating_add(1);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(provider: &str, name: &str, quality: f64, cost: u64) -> ModelDescriptor {
        ModelDescriptor {
            provider: provider.into(),
            model: name.into(),
            capabilities: vec!["reasoning".into()],
            context_window_tokens: 16_000,
            max_output_tokens: 2_000,
            quality,
            latency_ms: 100,
            cost_per_million_tokens: cost,
            reliability: 0.9,
            requires_credential: true,
            enabled: true,
        }
    }

    fn outcome_request(
        state: BanditState,
        arm_id: &str,
        context: Option<ModelSelectionContext>,
    ) -> BrainOutcomeRecordRequest {
        let context_digest = context
            .as_ref()
            .map(|value| digest(value).expect("test context digest"));
        BrainOutcomeRecordRequest {
            run: BrainRunIdentity {
                run_id: "first-run".into(),
                selection_digest: "a".repeat(64),
                prompt_digest: "b".repeat(64),
                plan_digest: "c".repeat(64),
                provider: "provider".into(),
                model: "model".into(),
                outcome_digest: "d".repeat(64),
                request_id: None,
            },
            assessment: BrainEvaluatorAssessment {
                evaluator_id: "test-evaluator".into(),
                evaluator_version: "1".into(),
                reward: 0.7,
                passed: true,
                failed: false,
                feedback_digest: None,
                failure_class: None,
                evidence_digest: None,
            },
            bandit_state: state,
            arm_id: arm_id.into(),
            context_digest,
            context,
            idempotency_key: Some("episode:first-run".into()),
        }
    }

    #[test]
    fn model_selection_applies_hard_gates_before_deterministic_ranking() {
        let report = select_model(&ModelSelectionRequest {
            task: "summarize".into(),
            required_capabilities: vec!["reasoning".into(), "structured_output".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: Some(100),
            max_latency_ms: None,
            min_quality: None,
            min_selection_confidence: None,
            models: vec![
                model("a", "cheap", 0.7, 1),
                model("b", "expensive", 0.99, 1000),
            ],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
            model_health: BTreeMap::new(),
        })
        .unwrap();
        assert!(report.selected_model_id.is_none());
        assert!(report.ranking.iter().all(|candidate| !candidate.eligible));
        assert_eq!(report.selection_status, "refused_no_eligible_model");
    }

    #[test]
    fn model_selection_abstains_on_low_rank_separation_when_requested() {
        let report = select_model(&ModelSelectionRequest {
            task: "ambiguous model choice".into(),
            required_capabilities: vec!["reasoning".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            min_selection_confidence: Some(0.1),
            models: vec![model("a", "first", 0.8, 10), model("b", "second", 0.8, 10)],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
            model_health: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(report.eligible_model_count, 2);
        assert_eq!(report.selection_confidence, 0.0);
        assert_eq!(report.min_selection_confidence, Some(0.1));
        assert!(report.selected_model_id.is_none());
        assert_eq!(
            report.selection_status,
            "abstained_low_selection_confidence"
        );

        let unique = select_model(&ModelSelectionRequest {
            min_selection_confidence: Some(0.1),
            models: vec![model("a", "only", 0.8, 10)],
            ..ModelSelectionRequest {
                task: "unique model choice".into(),
                required_capabilities: vec!["reasoning".into()],
                input_tokens: 100,
                requested_output_tokens: 100,
                max_cost_per_million_tokens: None,
                max_latency_ms: None,
                min_quality: None,
                min_selection_confidence: None,
                models: Vec::new(),
                observations: Vec::new(),
                weights: SelectionWeights::default(),
                provider_health: BTreeMap::new(),
                model_health: BTreeMap::new(),
            }
        })
        .unwrap();
        assert_eq!(unique.selection_confidence, 1.0);
        assert_eq!(unique.min_selection_confidence, Some(0.1));
        assert_eq!(unique.selected_model_id.as_deref(), Some("a/only"));
    }

    #[test]
    fn contextual_model_selection_overrides_global_history_without_hidden_state() {
        let context = ModelSelectionContext {
            domain: "oncology".into(),
            capability: "assay_fidelity".into(),
            risk_class: "high_review".into(),
            task_family: Some("evidence_summary".into()),
        };
        let context_digest = digest(&context).unwrap();
        let base = ModelSelectionRequest {
            task: "summarize assay evidence".into(),
            required_capabilities: vec!["reasoning".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            min_selection_confidence: None,
            models: vec![
                model("a", "global", 0.8, 10),
                model("b", "context", 0.8, 10),
            ],
            observations: vec![
                ModelObservation {
                    arm_id: "a/global".into(),
                    pulls: 10,
                    reward_sum: 8.0,
                    failures: 0,
                    disabled: false,
                },
                ModelObservation {
                    arm_id: "b/context".into(),
                    pulls: 10,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                },
            ],
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
            model_health: BTreeMap::new(),
        };
        let report = select_model_contextual(&ContextualModelSelectionRequest {
            context,
            base,
            observations: vec![ContextualModelObservation {
                context_digest,
                arm_id: "b/context".into(),
                pulls: 10,
                reward_sum: 10.0,
                failures: 0,
                disabled: false,
            }],
        })
        .unwrap();
        assert_eq!(
            report.selection.selected_model_id.as_deref(),
            Some("b/context")
        );
        assert_eq!(report.contextual_observations_used, 1);
        assert_eq!(report.global_observation_fallbacks, 1);
        assert_eq!(
            report.selection_status,
            "contextual_selection_mixed_history"
        );
    }

    #[test]
    fn provider_health_is_a_kernel_gate_and_remains_visible_in_candidate_reasons() {
        let mut provider_health = BTreeMap::new();
        provider_health.insert(
            "a".into(),
            ProviderHealth {
                registered: true,
                circuit: "open".into(),
                consecutive_failures: 3,
                credential_ready: true,
                eligible: false,
            },
        );
        provider_health.insert(
            "b".into(),
            ProviderHealth {
                registered: true,
                circuit: "closed".into(),
                consecutive_failures: 0,
                credential_ready: true,
                eligible: true,
            },
        );
        let report = select_model(&ModelSelectionRequest {
            task: "provider health test".into(),
            required_capabilities: vec!["reasoning".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            min_selection_confidence: None,
            models: vec![model("a", "open", 0.99, 1), model("b", "ready", 0.7, 2)],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health,
            model_health: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(report.selected_model_id.as_deref(), Some("b/ready"));
        let refused = report
            .ranking
            .iter()
            .find(|candidate| candidate.model_id == "a/open")
            .unwrap();
        assert!(!refused.eligible);
        assert!(refused
            .reasons
            .iter()
            .any(|reason| reason == "provider_circuit_open"));
        assert!(refused
            .reasons
            .iter()
            .any(|reason| reason == "provider_health_ineligible"));
    }

    #[test]
    fn model_health_adapts_ranking_without_becoming_a_hidden_hard_gate() {
        let mut model_health = BTreeMap::new();
        model_health.insert(
            "a/degraded".into(),
            ModelHealthEvidence {
                attempts: 12,
                successes: 0,
                failures: 12,
                success_rate: Some(0.0),
                mean_latency_ms: Some(900.0),
                last_latency_ms: Some(900.0),
                prior_adjustment_applied: false,
                historical: None,
            },
        );
        let report = select_model(&ModelSelectionRequest {
            task: "model health test".into(),
            required_capabilities: vec!["reasoning".into()],
            input_tokens: 100,
            requested_output_tokens: 100,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            min_selection_confidence: None,
            models: vec![
                model("a", "degraded", 0.9, 1),
                model("b", "healthy", 0.9, 1),
            ],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
            model_health,
        })
        .unwrap();
        assert_eq!(report.selected_model_id.as_deref(), Some("b/healthy"));
        let degraded = report
            .ranking
            .iter()
            .find(|candidate| candidate.model_id == "a/degraded")
            .unwrap();
        assert!(degraded.eligible);
        assert!(degraded.score < report.ranking[0].score);
        assert!(degraded.reasons.is_empty());
    }

    #[test]
    fn invalid_model_health_is_rejected_before_selection() {
        let mut model_health = BTreeMap::new();
        model_health.insert(
            "a/model".into(),
            ModelHealthEvidence {
                attempts: 1,
                successes: 2,
                failures: 0,
                success_rate: None,
                mean_latency_ms: None,
                last_latency_ms: Some(10.0),
                prior_adjustment_applied: false,
                historical: None,
            },
        );
        let error = select_model(&ModelSelectionRequest {
            task: "invalid health test".into(),
            required_capabilities: vec![],
            input_tokens: 1,
            requested_output_tokens: 1,
            max_cost_per_million_tokens: None,
            max_latency_ms: None,
            min_quality: None,
            min_selection_confidence: None,
            models: vec![model("a", "model", 0.9, 1)],
            observations: Vec::new(),
            weights: SelectionWeights::default(),
            provider_health: BTreeMap::new(),
            model_health,
        })
        .unwrap_err();
        assert!(matches!(error, BrainError::InvalidModelHealth(_)));
    }

    #[test]
    fn prompt_assembly_reports_optional_omission_and_digest() {
        let report = assemble_prompt(&PromptAssemblyRequest {
            system: Some("be precise".into()),
            developer: None,
            task: "answer".into(),
            context: vec![
                PromptChunk {
                    id: "required".into(),
                    role: "user".into(),
                    content: "must include".into(),
                    required: true,
                    priority: 0,
                },
                PromptChunk {
                    id: "optional".into(),
                    role: "user".into(),
                    content: "this is deliberately large enough to omit".into(),
                    required: false,
                    priority: 0,
                },
            ],
            output_contract: Some("JSON".into()),
            max_input_tokens: 10,
        })
        .unwrap();
        assert_eq!(report.omitted_context_ids, vec!["optional"]);
        assert!(!report.complete);
        assert_eq!(report.prompt_digest.len(), 64);
    }

    #[test]
    fn planner_orders_dependencies_and_requires_approval_for_effects() {
        let report = plan_autonomous(&AutonomousPlanRequest {
            objective: "inspect then call model".into(),
            allowed_tools: vec!["inspect".into(), "invoke".into()],
            max_cost: 10,
            require_approval_for_effects: true,
            steps: vec![
                PlanStep {
                    id: "invoke".into(),
                    objective: "call model".into(),
                    tool: "invoke".into(),
                    arguments: json!({}),
                    depends_on: vec!["inspect".into()],
                    effect: PlanEffect::ProviderCall,
                    estimated_cost: 5,
                },
                PlanStep {
                    id: "inspect".into(),
                    objective: "inspect context".into(),
                    tool: "inspect".into(),
                    arguments: json!({}),
                    depends_on: vec![],
                    effect: PlanEffect::ReadOnly,
                    estimated_cost: 1,
                },
            ],
        })
        .unwrap();
        let plan = report.plan.unwrap();
        assert_eq!(plan.ordered_step_ids, vec!["inspect", "invoke"]);
        assert!(plan.requires_approval);
        assert_eq!(plan.execution, "not_started");
    }

    #[test]
    fn bandit_updates_are_explicit_and_unexplored_arms_are_selected() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: vec![
                BanditArm {
                    arm_id: "known".into(),
                    pulls: 10,
                    reward_sum: 1.0,
                    failures: 0,
                    disabled: false,
                },
                BanditArm {
                    arm_id: "new".into(),
                    pulls: 0,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                },
            ],
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        let selected = select_bandit_arm(&state).unwrap();
        assert_eq!(selected.selected_arm_id.as_deref(), Some("new"));
        let next = update_bandit(
            &state,
            &BanditUpdate {
                arm_id: "new".into(),
                reward: 0.8,
                failed: false,
                outcome_digest: Some("a".repeat(64)),
                contract_digest: None,
                context_digest: None,
                context: None,
            },
        )
        .unwrap();
        assert_eq!(next.generation, 1);
        assert_eq!(next.arms[1].pulls, 1);
        assert_eq!(next.arms[1].reward_sum, 0.8);
    }

    #[test]
    fn bandit_policy_is_backward_compatible_and_seeded_exploration_replays() {
        let legacy: BanditState = serde_json::from_value(json!({
            "schema": BANDIT_SCHEMA,
            "generation": 3,
            "arms": [{
                "arm_id": "legacy",
                "pulls": 1,
                "reward_sum": 0.2,
                "failures": 0,
                "disabled": false
            }]
        }))
        .unwrap();
        assert_eq!(legacy.policy.strategy, "ucb1");
        assert_eq!(legacy.policy.epsilon, 0.10);

        let mut state = legacy;
        state.policy = BanditPolicy {
            strategy: "epsilon_greedy".into(),
            exploration: 0.5,
            epsilon: 1.0,
            min_reward: -1.0,
            max_reward: 1.0,
            failure_penalty: 0.25,
            seed: 42,
        };
        state.arms.push(BanditArm {
            arm_id: "second".into(),
            pulls: 10,
            reward_sum: 8.0,
            failures: 0,
            disabled: false,
        });
        let first = select_bandit_arm(&state).unwrap();
        let replay = select_bandit_arm(&state).unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.strategy, "epsilon_greedy");
        assert!(first.exploration_taken);
        assert_eq!(first.selection_status, "selected_exploration");
        assert!(first.exploration_draw.is_some());
    }

    #[test]
    fn thompson_sampling_is_deterministic_and_emits_auditable_posteriors() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 11,
            policy: BanditPolicy {
                strategy: "thompson_sampling".into(),
                exploration: 0.5,
                epsilon: 0.1,
                min_reward: -1.0,
                max_reward: 1.0,
                failure_penalty: 0.25,
                seed: 99,
            },
            arms: vec![
                BanditArm {
                    arm_id: "openai/quality".into(),
                    pulls: 8,
                    reward_sum: 6.4,
                    failures: 1,
                    disabled: false,
                },
                BanditArm {
                    arm_id: "anthropic/exploration".into(),
                    pulls: 1,
                    reward_sum: 0.0,
                    failures: 1,
                    disabled: false,
                },
            ],
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        let first = select_bandit_arm(&state).unwrap();
        let replay = select_bandit_arm(&state).unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.strategy, "thompson_sampling");
        assert_eq!(first.selection_status, "selected_thompson_sample");
        assert!(first.exploration_taken);
        assert!(first.exploration_draw.is_none());
        assert!(first.ranking.iter().all(|row| {
            row.posterior_alpha.is_some()
                && row.posterior_beta.is_some()
                && row.posterior_sample.is_some()
                && row.posterior_alpha.unwrap() >= 1.0
                && row.posterior_beta.unwrap() >= 1.0
                && (0.0..=1.0).contains(&row.posterior_sample.unwrap())
        }));
    }

    #[test]
    fn bandit_update_is_idempotent_for_a_credited_outcome_digest() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 4,
            policy: BanditPolicy::default(),
            arms: vec![BanditArm {
                arm_id: "openai/test-model".into(),
                pulls: 1,
                reward_sum: 0.2,
                failures: 0,
                disabled: false,
            }],
            credited_outcomes: vec![CreditedOutcome {
                outcome_digest: "d".repeat(64),
                arm_id: "openai/test-model".into(),
                reward: 0.9,
                failed: false,
                contract_digest: None,
                context_digest: None,
            }],
            contextual_states: Vec::new(),
        };
        let next = update_bandit(
            &state,
            &BanditUpdate {
                arm_id: "openai/test-model".into(),
                reward: 0.9,
                failed: false,
                outcome_digest: Some("d".repeat(64)),
                contract_digest: None,
                context_digest: None,
                context: None,
            },
        )
        .unwrap();
        assert_eq!(next, state);
    }

    #[test]
    fn contextual_bandit_updates_are_isolated_and_replay_safe() {
        let coding = ModelSelectionContext {
            domain: "coding".into(),
            capability: "implementation".into(),
            risk_class: "engineering_change".into(),
            task_family: Some("coding_delivery".into()),
        };
        let biomedical = ModelSelectionContext {
            domain: "biomedical".into(),
            capability: "biomedical_review".into(),
            risk_class: "biomedical_safety".into(),
            task_family: Some("biomedical_review".into()),
        };
        let arm = |arm_id: &str| BanditArm {
            arm_id: arm_id.into(),
            pulls: 1,
            reward_sum: 0.2,
            failures: 0,
            disabled: false,
        };
        let coding_digest = digest(&coding).unwrap();
        let biomedical_digest = digest(&biomedical).unwrap();
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: vec![arm("a/one"), arm("b/two")],
            credited_outcomes: Vec::new(),
            contextual_states: vec![
                ContextualBanditState {
                    context_digest: coding_digest.clone(),
                    context: coding.clone(),
                    generation: 0,
                    arms: vec![arm("a/one"), arm("b/two")],
                    observed: false,
                },
                ContextualBanditState {
                    context_digest: biomedical_digest.clone(),
                    context: biomedical.clone(),
                    generation: 0,
                    arms: vec![arm("a/one"), arm("b/two")],
                    observed: false,
                },
            ],
        };
        let update = BanditUpdate {
            arm_id: "a/one".into(),
            reward: 1.0,
            failed: false,
            outcome_digest: Some("c".repeat(64)),
            contract_digest: None,
            context_digest: Some(coding_digest.clone()),
            context: Some(coding.clone()),
        };
        let next = update_bandit(&state, &update).unwrap();
        assert_eq!(next.arms[0].pulls, 1);
        assert_eq!(next.contextual_states[0].arms[0].pulls, 2);
        assert_eq!(next.contextual_states[1].arms[0].pulls, 1);
        assert_eq!(next.contextual_states[0].generation, 1);
        assert!(next.contextual_states[0].observed);
        let selected = select_bandit_arm_contextual(&next, &coding_digest, &coding).unwrap();
        assert_eq!(selected.selected_arm_id.as_deref(), Some("a/one"));
        assert_eq!(update_bandit(&next, &update).unwrap(), next);
        let contradictory = BanditUpdate {
            context_digest: Some(biomedical_digest),
            context: Some(biomedical),
            reward: 0.1,
            ..update
        };
        assert!(matches!(
            update_bandit(&next, &contradictory),
            Err(BrainError::ConflictingCreditedOutcome(_))
        ));
    }

    #[test]
    fn bandit_rejects_unknown_policy_strategy() {
        let mut state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: vec![BanditArm {
                arm_id: "arm".into(),
                pulls: 0,
                reward_sum: 0.0,
                failures: 0,
                disabled: false,
            }],
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        state.policy.strategy = "unbounded_random".into();
        let error = select_bandit_arm(&state).unwrap_err();
        assert!(matches!(error, BrainError::InvalidBanditStrategy(_)));
    }

    #[test]
    fn outcome_record_binds_evaluator_reward_without_response_text() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 4,
            policy: BanditPolicy::default(),
            arms: vec![BanditArm {
                arm_id: "openai/test-model".into(),
                pulls: 1,
                reward_sum: 0.2,
                failures: 0,
                disabled: false,
            }],
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        let report = record_brain_outcome(&BrainOutcomeRecordRequest {
            run: BrainRunIdentity {
                run_id: "run-1".into(),
                selection_digest: "a".repeat(64),
                prompt_digest: "b".repeat(64),
                plan_digest: "c".repeat(64),
                provider: "openai".into(),
                model: "test-model".into(),
                outcome_digest: "d".repeat(64),
                request_id: Some("request-1".into()),
            },
            assessment: BrainEvaluatorAssessment {
                evaluator_id: "json_contract".into(),
                evaluator_version: "1".into(),
                reward: 0.9,
                passed: true,
                failed: false,
                feedback_digest: Some("f".repeat(64)),
                failure_class: None,
                evidence_digest: Some("e".repeat(64)),
            },
            bandit_state: state,
            arm_id: "openai/test-model".into(),
            context_digest: None,
            context: None,
            idempotency_key: Some("episode:run-1".into()),
        })
        .unwrap();
        assert!(report.ok);
        assert_eq!(report.status, "recorded_evaluator_reward");
        assert_eq!(report.next_state.generation, 5);
        assert_eq!(report.learning_evidence.previous_generation, 4);
        assert_eq!(report.learning_evidence.next_generation, 5);
        assert_eq!(report.learning_evidence.evidence_digest.len(), 64);
        let encoded = serde_json::to_string(&report.learning_evidence).unwrap();
        assert!(!encoded.contains("provider response"));
        assert!(!encoded.contains("api_key"));
    }

    #[test]
    fn outcome_record_hydrates_an_unseen_global_arm_on_first_run() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: Vec::new(),
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        let report = record_brain_outcome(&outcome_request(state, "provider/model", None)).unwrap();
        assert_eq!(report.next_state.generation, 1);
        assert_eq!(report.next_state.arms.len(), 1);
        assert_eq!(report.next_state.arms[0].arm_id, "provider/model");
        assert_eq!(report.next_state.arms[0].pulls, 1);
        assert_eq!(report.next_state.arms[0].reward_sum, 0.7);
        let replay = record_brain_outcome(&outcome_request(
            report.next_state.clone(),
            "provider/model",
            None,
        ))
        .unwrap();
        assert_eq!(replay.next_state, report.next_state);
    }

    #[test]
    fn outcome_record_hydrates_an_unseen_contextual_arm_without_global_leakage() {
        let context = ModelSelectionContext {
            domain: "coding".into(),
            capability: "implementation".into(),
            risk_class: "engineering_change".into(),
            task_family: Some("coding_delivery".into()),
        };
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: Vec::new(),
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        let report = record_brain_outcome(&outcome_request(
            state,
            "provider/model",
            Some(context.clone()),
        ))
        .unwrap();
        assert!(report.next_state.arms.is_empty());
        assert_eq!(report.next_state.contextual_states.len(), 1);
        assert_eq!(report.next_state.contextual_states[0].context, context);
        assert_eq!(report.next_state.contextual_states[0].arms[0].pulls, 1);
        assert!(report.next_state.contextual_states[0].observed);
    }

    #[test]
    fn outcome_record_does_not_reenable_a_disabled_arm_during_hydration() {
        let state = BanditState {
            schema: BANDIT_SCHEMA.into(),
            generation: 0,
            policy: BanditPolicy::default(),
            arms: vec![BanditArm {
                arm_id: "provider/model".into(),
                pulls: 0,
                reward_sum: 0.0,
                failures: 0,
                disabled: true,
            }],
            credited_outcomes: Vec::new(),
            contextual_states: Vec::new(),
        };
        assert!(matches!(
            record_brain_outcome(&outcome_request(state, "provider/model", None)),
            Err(BrainError::UnknownArm(_))
        ));
    }

    #[test]
    fn outcome_record_rejects_contradictory_assessments() {
        let error = record_brain_outcome(&BrainOutcomeRecordRequest {
            run: BrainRunIdentity {
                run_id: "run-1".into(),
                selection_digest: "a".repeat(64),
                prompt_digest: "b".repeat(64),
                plan_digest: "c".repeat(64),
                provider: "openai".into(),
                model: "test-model".into(),
                outcome_digest: "d".repeat(64),
                request_id: None,
            },
            assessment: BrainEvaluatorAssessment {
                evaluator_id: "evaluator".into(),
                evaluator_version: "1".into(),
                reward: 0.0,
                passed: true,
                failed: true,
                feedback_digest: None,
                failure_class: None,
                evidence_digest: None,
            },
            bandit_state: BanditState {
                schema: BANDIT_SCHEMA.into(),
                generation: 0,
                policy: BanditPolicy::default(),
                arms: vec![BanditArm {
                    arm_id: "openai/test-model".into(),
                    pulls: 0,
                    reward_sum: 0.0,
                    failures: 0,
                    disabled: false,
                }],
                credited_outcomes: Vec::new(),
                contextual_states: Vec::new(),
            },
            arm_id: "openai/test-model".into(),
            context_digest: None,
            context: None,
            idempotency_key: None,
        })
        .unwrap_err();
        assert!(matches!(error, BrainError::ContradictoryAssessment));
    }

    #[test]
    fn plan_rejects_cycles_without_partial_execution() {
        let report = plan_autonomous(&AutonomousPlanRequest {
            objective: "cycle".into(),
            allowed_tools: vec!["x".into()],
            max_cost: 10,
            require_approval_for_effects: true,
            steps: vec![
                PlanStep {
                    id: "a".into(),
                    objective: "a".into(),
                    tool: "x".into(),
                    arguments: Value::Null,
                    depends_on: vec!["b".into()],
                    effect: PlanEffect::ReadOnly,
                    estimated_cost: 1,
                },
                PlanStep {
                    id: "b".into(),
                    objective: "b".into(),
                    tool: "x".into(),
                    arguments: Value::Null,
                    depends_on: vec!["a".into()],
                    effect: PlanEffect::ReadOnly,
                    estimated_cost: 1,
                },
            ],
        })
        .unwrap();
        assert!(!report.ok);
        assert_eq!(report.status, "refused_dependency_cycle");
        assert!(report.plan.is_none());
    }
}
