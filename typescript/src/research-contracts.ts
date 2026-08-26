/** Shared v1 research contracts for TypeScript adapters and workbench clients.
 *
 * This module intentionally validates transport and safety metadata only. Scientific conclusions
 * remain evidence-receipt values produced by the Rust kernel; a client cannot upgrade `unknown`
 * or bypass a protected omission by editing a JSON object.
 */
import { digestJsonSync } from "./tooling.js";

export const RESEARCH_CONTRACT_SCHEMA_VERSION = "aurora-research-contract/1.0" as const;
export const PRECLINICAL_BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions" as const;
export const RESEARCH_FEATURE_ID = "AFA-bioir-P02-F01" as const;
export const RELEASE_REVIEW_FEATURE_ID = "AFA-evalengine-P13-F01" as const;
export const RESEARCH_INGESTION_FEATURE_ID = "AFA-adapter-P06-F01" as const;
export const EXPERIMENT_DESIGN_FEATURE_ID = "AFA-lab-P09-F01" as const;
export const PROTOCOL_SIMULATION_FEATURE_ID = "AFA-lab-P10-F01" as const;
export const REPLICATION_FEATURE_ID = "AFA-evalengine-P15-F01" as const;
export const QUALITY_CONTROL_FEATURE_ID = "AFA-adapter-P07-F01" as const;
export const RESEARCH_CONTEXT_FEATURE_ID = "AFA-fiber-P03-F01" as const;
export const REPLAY_AUDIT_FEATURE_ID = "AFA-runtime-P23-F01" as const;
export const WORKFLOW_EXECUTION_FEATURE_ID = "AFA-runtime-P12-F10" as const;
export const EVALUATION_OBSERVABILITY_FEATURE_ID = "AFA-evalengine-P23-F01" as const;
export const RESEARCH_RELEASE_FEATURE_ID = "AFA-services-P16-F02" as const;
export const INSTRUMENT_PREFLIGHT_FEATURE_ID = "AFA-lab-P11-F01" as const;
export const MULTIMODAL_HARMONIZATION_FEATURE_ID = "AFA-adapter-P06-F02" as const;
export const ANALYSIS_QUALIFICATION_FEATURE_ID = "AFA-evalengine-P13-F01" as const;
export const PROTOCOL_MATRIX_FEATURE_ID = "AFA-lab-P10-F02" as const;
export const MULTIMODAL_REPLICATION_FEATURE_ID = "AFA-evalengine-P15-F02" as const;
export const QUALITY_DRIFT_FEATURE_ID = "AFA-adapter-P07-F02" as const;
export const DESIGN_FRONTIER_FEATURE_ID = "AFA-lab-P09-F02" as const;
export const AUTONOMY_BATCH_FEATURE_ID = "AFA-policy-P19-F02" as const;
export const WORKFLOW_BATCH_FEATURE_ID = "AFA-runtime-P12-F11" as const;
export const RESEARCH_RELEASE_BATCH_FEATURE_ID = "AFA-services-P16-F03" as const;
export const FEDERATED_EVALUATION_FEATURE_ID = "AFA-evalengine-P23-F02" as const;
export const RESOURCE_WORKBENCH_FEATURE_ID = "AFA-fiber-P05-F20" as const;
export const RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID = "AFA-mcp-P05-F08" as const;
export const RESOURCE_DISCOVERY_CONTRACT_VERSION = "aurora-mcp-resource-discovery/2.0" as const;
export const GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID = "AFA-governance-P16-F08" as const;
export const GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION = "signed-research-object/2.0" as const;
export const RELEASE_HARNESS_FEATURE_ID = "AFA-obligation-P16-F27" as const;
export const RELEASE_HARNESS_CONTRACT_VERSION = "release-assurance-harness/1.0" as const;
export const PROTOCOL_ASSURANCE_FEATURE_ID = "AFA-policy-P10-F27" as const;
export const PROTOCOL_ASSURANCE_CONTRACT_VERSION = "protocol-assurance-harness/1.0" as const;
export const FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID = "AFA-routing-P06-F28" as const;
export const FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION = "federated-multimodal-assurance/1.0" as const;
export const FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID = "AFA-store-P04-F24" as const;
export const FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION = "federated-knowledge-gateway/1.0" as const;
export const FEDERATED_LENS_ASSURANCE_FEATURE_ID = "AFA-lens-P04-F28" as const;
export const FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION = "federated-lens-assurance/1.0" as const;
export const SEMANTIC_PARITY_FEATURE_ID = "AFA-lab-P28-F12" as const;
export const SEMANTIC_PARITY_CONTRACT_VERSION = "lab-semantic-parity/1.0" as const;
export const FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-fiber-P02-F28" as const;
export const FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "federated-retrieval-assurance/1.0" as const;
export const FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID = "AFA-atlashub-P02-F12" as const;
export const FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION = "federated-continual-retrieval-copilot/1.0" as const;
export const CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-devplat-P03-F28" as const;
export const CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "federated-context-compilation-assurance/1.0" as const;
export const KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID = "AFA-ops-P04-F28" as const;
export const KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION = "federated-knowledge-representation-assurance/1.0" as const;
export const RESOURCE_CONTROL_PLANE_FEATURE_ID = "AFA-weave-P05-F32" as const;
export const RESOURCE_CONTROL_PLANE_CONTRACT_VERSION = "federated-resource-control-plane/1.0" as const;
export const WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID = "AFA-weavelang-P16-F27" as const;
export const WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION = "weavelang-release-assurance/1.0" as const;
export const MECHANISM_CONTROL_PLANE_FEATURE_ID = "AFA-adapter-P08-F31" as const;
export const MECHANISM_CONTROL_PLANE_CONTRACT_VERSION = "federated-mechanism-control-plane/1.0" as const;
export const MECHANISM_GATEWAY_FEATURE_ID = "AFA-fiber-P08-F24" as const;
export const MECHANISM_GATEWAY_CONTRACT_VERSION = "federated-mechanism-interoperability-gateway/1.0" as const;
export const EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-adapter-P01-F09" as const;
export const EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "evidence-surveillance-copilot/1.0" as const;
export const RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-adapter-P02-F06" as const;
export const RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "multimodal-retrieval-synthesis/1.0" as const;
export const ADAPTER_CONTEXT_COMPILATION_FEATURE_ID = "AFA-adapter-P03-F27" as const;
export const ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION = "prospective-context-compilation-assurance/1.0" as const;
export const KNOWLEDGE_WORKFLOW_FEATURE_ID = "AFA-adapter-P04-F14" as const;
export const KNOWLEDGE_WORKFLOW_CONTRACT_VERSION = "multimodal-knowledge-workflow-fabric/1.0" as const;
export const RESOURCE_WORKBENCH_FEATURE_ID = "AFA-adapter-P05-F18" as const;
export const RESOURCE_WORKBENCH_CONTRACT_VERSION = "multimodal-resource-workbench/1.0" as const;
export const INGESTION_GATEWAY_FEATURE_ID = "AFA-adapter-P06-F23" as const;
export const INGESTION_GATEWAY_CONTRACT_VERSION = "1.0" as const;
export const QUALITY_ENVELOPE_FEATURE_ID = "AFA-adapter-P07-F06" as const;
export const QUALITY_ENVELOPE_CONTRACT_VERSION = "multi-study-quality-envelope/1.0" as const;
export const EXPERIMENT_DESIGN_CONTROL_FEATURE_ID = "AFA-adapter-P09-F30" as const;
export const EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION = "federated-experiment-design-control-plane/1.0" as const;
export const PROTOCOL_SIMULATION_FEATURE_ID = "AFA-adapter-P10-F03" as const;
export const PROTOCOL_SIMULATION_CONTRACT_VERSION = "prospective-protocol-simulation/1.0" as const;
export const INSTRUMENT_MESH_FEATURE_ID = "AFA-adapter-P11-F04" as const;
export const INSTRUMENT_MESH_CONTRACT_VERSION = "federated-laboratory-integration/1.0" as const;
export const EXECUTION_CONTROL_FEATURE_ID = "AFA-adapter-P12-F31" as const;
export const EXECUTION_CONTROL_CONTRACT_VERSION = "computational-execution-control-plane/1.0" as const;
export const ANALYSIS_PORTFOLIO_FEATURE_ID = "AFA-adapter-P13-F01" as const;
export const ANALYSIS_PORTFOLIO_CONTRACT_VERSION = "local-analysis-model-portfolio/1.0" as const;
export const INTERPRETATION_ASSURANCE_FEATURE_ID = "AFA-adapter-P14-F27" as const;
export const INTERPRETATION_ASSURANCE_CONTRACT_VERSION = "interpretation-assurance/1.0" as const;
export const REPLICATION_ASSURANCE_FEATURE_ID = "AFA-adapter-P15-F28" as const;
export const REPLICATION_ASSURANCE_CONTRACT_VERSION = "federated-replication-assurance/1.0" as const;
export const RELEASE_ASSURANCE_FEATURE_ID = "AFA-adapter-P16-F26" as const;
export const RELEASE_ASSURANCE_CONTRACT_VERSION = "multimodal-research-release-assurance/1.0" as const;
export const DETERMINISM_GATEWAY_FEATURE_ID = "AFA-adapter-P17-F24" as const;
export const DETERMINISM_GATEWAY_CONTRACT_VERSION = "typed-determinism-gateway/1.0" as const;
export const PROVENANCE_ASSURANCE_FEATURE_ID = "AFA-adapter-P18-F26" as const;
export const PROVENANCE_ASSURANCE_CONTRACT_VERSION = "multimodal-provenance-signing-assurance/1.0" as const;
export const POLICY_GATEWAY_FEATURE_ID = "AFA-adapter-P19-F24" as const;
export const POLICY_GATEWAY_CONTRACT_VERSION = "federated-policy-autonomy-gateway/1.0" as const;
export const FEDERATION_WORKFLOW_FEATURE_ID = "AFA-adapter-P20-F15" as const;
export const FEDERATION_WORKFLOW_CONTRACT_VERSION = "prospective-federation-workflow-fabric/1.0" as const;
export const RELIABILITY_COPILOT_FEATURE_ID = "AFA-adapter-P21-F12" as const;
export const RELIABILITY_COPILOT_CONTRACT_VERSION = "federated-reliability-copilot/1.0" as const;
export const INTEROPERABILITY_GATEWAY_FEATURE_ID = "AFA-adapter-P22-F24" as const;
export const INTEROPERABILITY_GATEWAY_CONTRACT_VERSION = "federated-interoperability-gateway/1.0" as const;
export const EVALUATION_ASSURANCE_FEATURE_ID = "AFA-adapter-P23-F25" as const;
export const EVALUATION_ASSURANCE_CONTRACT_VERSION = "evaluation-assurance-harness/1.0" as const;
export const RESEARCH_WORKBENCH_FEATURE_ID = "AFA-adapter-P24-F18" as const;
export const RESEARCH_WORKBENCH_CONTRACT_VERSION = "multimodal-research-workbench/1.0" as const;
export const CONTRACT_FRONTIER_FEATURE_ID = "AFA-adapter-P25-F22" as const;
export const CONTRACT_FRONTIER_CONTRACT_VERSION = "adapter-contract-frontier/1.0" as const;
export const LIMITATION_CLOSURE_FEATURE_ID = "AFA-adapter-P26-F24" as const;
export const LIMITATION_CLOSURE_CONTRACT_VERSION = "adapter-limitation-closure/1.0" as const;
export const DEPENDENCY_COMPOSITION_FEATURE_ID = "AFA-adapter-P27-F18" as const;
export const DEPENDENCY_COMPOSITION_CONTRACT_VERSION = "adapter-dependency-composition/1.0" as const;
export const ADAPTER_SEMANTIC_PARITY_FEATURE_ID = "AFA-adapter-P28-F06" as const;
export const ADAPTER_SEMANTIC_PARITY_CONTRACT_VERSION = "adapter-semantic-parity/1.0" as const;
export const ADAPTER_SCALE_FRONTIER_FEATURE_ID = "AFA-adapter-P29-F15" as const;
export const ADAPTER_SCALE_FRONTIER_CONTRACT_VERSION = "adapter-scale-frontier/1.0" as const;
export const ADVERSARIAL_RECOVERY_FEATURE_ID = "AFA-adapter-P30-F24" as const;
export const ADVERSARIAL_RECOVERY_CONTRACT_VERSION = "adapter-adversarial-recovery/1.0" as const;
export const FEDERATED_COMMONS_FEATURE_ID = "AFA-adapter-P31-F22" as const;
export const FEDERATED_COMMONS_CONTRACT_VERSION = "adapter-federated-commons/1.0" as const;
export const BOUNDED_EVOLUTION_FEATURE_ID = "AFA-adapter-P32-F23" as const;
export const BOUNDED_EVOLUTION_CONTRACT_VERSION = "adapter-bounded-evolution/1.0" as const;
export const EVOLUTION_IDENTITY_FEATURE_ID = "AFA-ids-P32-F31" as const;
export const EVOLUTION_IDENTITY_CONTRACT_VERSION = "ids-bounded-evolution/1.0" as const;
export const EVOLUTION_ASSURANCE_FEATURE_ID = "AFA-mcp-P32-F27" as const;
export const EVOLUTION_ASSURANCE_CONTRACT_VERSION = "mcp-bounded-evolution-assurance/1.0" as const;
export const EVOLUTION_ASSURANCE_REQUIRED_CHECKS = ["adversarial-containment", "canonical-order", "locality", "negative-evidence", "policy-authority", "protected-closure", "release-boundary", "replay-integrity", "signed-approval", "source-receipt"] as const;
export const INTERPRETATION_PLANE_FEATURE_ID = "AFA-ids-P14-F31" as const;
export const INTERPRETATION_PLANE_CONTRACT_VERSION = "ids-interpretation-federation/1.0" as const;
export const KNOWLEDGE_GATEWAY_FEATURE_ID = "AFA-docgraph-P04-F24" as const;
export const KNOWLEDGE_GATEWAY_CONTRACT_VERSION = "docgraph-knowledge-gateway/1.0" as const;
export const ORACLE_ASSURANCE_FEATURE_ID = "AFA-oracle-P25-F27" as const;
export const ORACLE_ASSURANCE_CONTRACT_VERSION = "oracle-contract-frontier-assurance/1.0" as const;
export const FEDERATED_INGESTION_FEATURE_ID = "AFA-bioworlds-P06-F08" as const;
export const FEDERATED_INGESTION_CONTRACT_VERSION = "bioworlds-federated-multimodal-ingestion/1.0" as const;
export const QUALITY_ASSURANCE_FEATURE_ID = "AFA-bioevalx-P07-F26" as const;
export const QUALITY_ASSURANCE_CONTRACT_VERSION = "bioevalx-multimodal-quality-assurance/1.0" as const;
export const MECHANISM_CONTROL_FEATURE_ID = "AFA-benchcompiler-P08-F30" as const;
export const MECHANISM_CONTROL_CONTRACT_VERSION = "benchcompiler-federated-mechanism-control/1.0" as const;
export const EVIDENCE_WORKBENCH_FEATURE_ID = "AFA-bioworlds-P01-F17" as const;
export const EVIDENCE_WORKBENCH_CONTRACT_VERSION = "bioworlds-local-evidence-workbench/1.0" as const;
export const ANALYSIS_CONTROL_FEATURE_ID = "AFA-devx-P13-F31" as const;
export const ANALYSIS_CONTROL_CONTRACT_VERSION = "devx-federated-analysis-control-plane/1.0" as const;
export const CONTEXT_ASSURANCE_FEATURE_ID = "AFA-registry-P03-F28" as const;
export const CONTEXT_ASSURANCE_CONTRACT_VERSION = "registry-federated-context-compilation-assurance/1.0" as const;
export const EVALUATION_ASSURANCE_BIOWORLDS_FEATURE_ID = "AFA-bioworlds-P23-F28" as const;
export const EVALUATION_ASSURANCE_BIOWORLDS_CONTRACT_VERSION = "bioworlds-federated-evaluation-observability-assurance/1.0" as const;
export const QUALITY_WORKBENCH_BIOLANG_FEATURE_ID = "AFA-biolang-P07-F19" as const;
export const QUALITY_WORKBENCH_BIOLANG_CONTRACT_VERSION = "biolang-prospective-quality-workbench/1.0" as const;
export const RETRIEVAL_ASSURANCE_BIOLANG_FEATURE_ID = "AFA-biolang-P02-F26" as const;
export const RETRIEVAL_ASSURANCE_BIOLANG_CONTRACT_VERSION = "biolang-multimodal-retrieval-synthesis-assurance/1.0" as const;
export const CLI_KNOWLEDGE_INTEROPERABILITY_FEATURE_ID = "AFA-cli-P04-F23" as const;
export const CLI_KNOWLEDGE_INTEROPERABILITY_CONTRACT_VERSION = "cli-prospective-knowledge-representation-interoperability/1.0" as const;
export const LAB_EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-lab-P01-F11" as const;
export const LAB_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "evidence-surveillance-copilot/1.0" as const;
export const FIBER_MECHANISM_ASSURANCE_FEATURE_ID = "AFA-fiber-P08-F26" as const;
export const FIBER_MECHANISM_ASSURANCE_CONTRACT_VERSION = "fiber-mechanism-assurance/1.0" as const;
export const HUBAPI_QUALITY_ASSURANCE_FEATURE_ID = "AFA-hubapi-P07-F27" as const;
export const HUBAPI_QUALITY_ASSURANCE_CONTRACT_VERSION = "hubapi-quality-assurance/1.0" as const;
export const REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_FEATURE_ID = "AFA-registry-P05-F28" as const;
export const REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_CONTRACT_VERSION = "registry-federated-resource-discovery-assurance/1.0" as const;
export const SERVICES_MECHANISM_WORKBENCH_FEATURE_ID = "AFA-services-P08-F19" as const;
export const SERVICES_MECHANISM_WORKBENCH_CONTRACT_VERSION = "services-prospective-mechanism-workbench/1.0" as const;
export const GOVERNANCE_INTERPRETATION_ASSURANCE_FEATURE_ID = "AFA-governance-P14-F27" as const;
export const GOVERNANCE_INTERPRETATION_ASSURANCE_CONTRACT_VERSION = "governance-interpretation-assurance/1.0" as const;
export const ORACLE_INGESTION_CONTROL_FEATURE_ID = "AFA-oracle-P06-F30" as const;
export const ORACLE_INGESTION_CONTROL_CONTRACT_VERSION = "oracle-federated-multimodal-ingestion-control/1.0" as const;
export const STEWARDSHIP_RELEASE_WORKBENCH_FEATURE_ID = "AFA-stewardship-P16-F20" as const;
export const STEWARDSHIP_RELEASE_WORKBENCH_CONTRACT_VERSION = "stewardship-federated-release-workbench/1.0" as const;
export const API_ANALYSIS_ASSURANCE_FEATURE_ID = "AFA-api-P13-F28" as const;
export const API_ANALYSIS_ASSURANCE_CONTRACT_VERSION = "api-federated-analysis-assurance/1.0" as const;
export const STORE_EVIDENCE_OPERATIONS_FEATURE_ID = "AFA-store-P01-F31" as const;
export const STORE_EVIDENCE_OPERATIONS_CONTRACT_VERSION = "store-prospective-evidence-federated-control-plane/1.0" as const;
export const POLICY_INTEROPERABILITY_CONTROL_FEATURE_ID = "AFA-policy-P22-F32" as const;
export const POLICY_INTEROPERABILITY_CONTROL_CONTRACT_VERSION = "policy-federated-interoperability-control-plane/1.0" as const;
export const SAFETY_MECHANISM_WORKFLOW_FEATURE_ID = "AFA-safety-P08-F16" as const;
export const SAFETY_MECHANISM_WORKFLOW_CONTRACT_VERSION = "safety-federated-mechanism-workflow/1.0" as const;
export const HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID = "AFA-hubapi-P14-F26" as const;
export const HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION = "hubapi-multimodal-interpretation-assurance/1.0" as const;
export const BIOLANG_PUBLICATION_COPILOT_FEATURE_ID = "AFA-biolang-P16-F11" as const;
export const BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION = "biolang-publication-copilot/1.0" as const;
export const API_RELEASE_ASSURANCE_FEATURE_ID = "AFA-api-P16-F27" as const;
export const API_RELEASE_ASSURANCE_CONTRACT_VERSION = "api-publication-release-assurance/1.0" as const;
export const BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID = "AFA-bioevalx-P16-F24" as const;
export const BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION = "bioevalx-federated-release-gateway/1.0" as const;
export const SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID = "AFA-section-P14-F28" as const;
export const SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION = "section-federated-interpretation-assurance/1.0" as const;
export const OPS_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-ops-P02-F25" as const;
export const OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "ops-local-retrieval-assurance/1.0" as const;
export const CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_FEATURE_ID = "AFA-conformance-P04-F26" as const;
export const CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_CONTRACT_VERSION = "conformance-knowledge-world-assurance/1.0" as const;
export const BRAIN_EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-brain-P01-F01" as const;
export const BRAIN_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "brain-evidence-surveillance/1.0" as const;
export const BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-brain-P01-F02" as const;
export const BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "brain-evidence-surveillance-multimodal/1.0" as const;
export const HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-brain-P01-F03" as const;
export const HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "brain-evidence-surveillance-throughput/1.0" as const;
export const FEDERATED_EVIDENCE_SURVEILLANCE_FEATURE_ID = "AFA-brain-P01-F04" as const;
export const FEDERATED_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION = "brain-evidence-surveillance-federated/1.0" as const;
export const EVIDENCE_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P01-F05" as const;
export const EVIDENCE_CONTRACT_MODEL_CONTRACT_VERSION = "brain-evidence-contract-model/1.0" as const;
export const MULTIMODAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P01-F06" as const;
export const MULTIMODAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-multimodal-evidence-contract/1.0" as const;
export const THROUGHPUT_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P01-F07" as const;
export const THROUGHPUT_CONTRACT_MODEL_CONTRACT_VERSION = "brain-throughput-evidence-contract/1.0" as const;
export const FEDERATED_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P01-F08" as const;
export const FEDERATED_CONTRACT_MODEL_CONTRACT_VERSION = "brain-federated-evidence-contract/1.0" as const;
export const EVIDENCE_RESEARCH_COPILOT_FEATURE_ID = "AFA-brain-P01-F09" as const;
export const EVIDENCE_RESEARCH_COPILOT_CONTRACT_VERSION = "brain-evidence-research-copilot/1.0" as const;
export const MULTIMODAL_EVIDENCE_COPILOT_FEATURE_ID = "AFA-brain-P01-F10" as const;
export const MULTIMODAL_EVIDENCE_COPILOT_CONTRACT_VERSION = "brain-multimodal-evidence-research-copilot/1.0" as const;
export const HIGH_THROUGHPUT_EVIDENCE_COPILOT_FEATURE_ID = "AFA-brain-P01-F11" as const;
export const HIGH_THROUGHPUT_EVIDENCE_COPILOT_CONTRACT_VERSION = "brain-high-throughput-evidence-research-copilot/1.0" as const;
export const FEDERATED_EVIDENCE_COPILOT_FEATURE_ID = "AFA-brain-P01-F12" as const;
export const FEDERATED_EVIDENCE_COPILOT_CONTRACT_VERSION = "brain-federated-evidence-research-copilot/1.0" as const;
export const EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P01-F13" as const;
export const EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-evidence-surveillance-workflow-fabric/1.0" as const;
export const MULTIMODAL_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P01-F14" as const;
export const MULTIMODAL_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-multimodal-evidence-workflow-fabric/1.0" as const;
export const HIGH_THROUGHPUT_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P01-F15" as const;
export const HIGH_THROUGHPUT_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-high-throughput-evidence-workflow-fabric/1.0" as const;
export const FEDERATED_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P01-F16" as const;
export const FEDERATED_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-federated-evidence-workflow-fabric/1.0" as const;
export const EVIDENCE_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P01-F17" as const;
export const EVIDENCE_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-evidence-research-workbench/1.0" as const;
export const MULTIMODAL_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P01-F18" as const;
export const MULTIMODAL_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-multimodal-research-workbench/1.0" as const;
export const THROUGHPUT_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P01-F19" as const;
export const THROUGHPUT_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-throughput-research-workbench/1.0" as const;
export const FEDERATED_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P01-F20" as const;
export const FEDERATED_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-federated-research-workbench/1.0" as const;
export const EVIDENCE_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P01-F21" as const;
export const EVIDENCE_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-evidence-protocol-adapter/1.0" as const;
export const MULTIMODAL_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P01-F22" as const;
export const MULTIMODAL_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-multimodal-protocol-adapter/1.0" as const;
export const THROUGHPUT_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P01-F23" as const;
export const THROUGHPUT_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-throughput-protocol-adapter/1.0" as const;
export interface BrainThroughputProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"qualified"|"partial"|"unknown"|"blocked"; batch_id:string; partition:string; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; queue_digest:string; evidence_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputProtocolReceipt(r:BrainThroughputProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==THROUGHPUT_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("throughput protocol schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||r.protocol_version!=="aurora-research-throughput/1.0"||r.method!=="POST"||r.route!=="/v1/research/evidence/throughput/admit"||r.content_type!=="application/json"||!r.request_id.trim()||!r.idempotency_key.trim()||r.response_schema!=="ThroughputEvidenceProtocolResponse1@1"||!r.batch_id.trim()||!r.partition.trim()||!r.candidate_order.length||!r.effect_receipts.length) throw new Error("throughput protocol identity incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput protocol state is not covered"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput protocol ordering invalid"); if(![200,202,206,403,422].includes(r.status_code)) throw new Error("throughput protocol status invalid"); for(const v of [r.queue_digest,r.evidence_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput protocol digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:local-throughput-response:")&&e!=="block:unsafe-release")) throw new Error("throughput protocol effect invalid"); }
export function brainThroughputProtocolReceiptDigest(r:BrainThroughputProtocolReceipt):string { validateBrainThroughputProtocolReceipt(r); return digestJsonSync(r); }
export const FEDERATED_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P01-F24" as const;
export const FEDERATED_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-federated-protocol-adapter/1.0" as const;
export interface BrainFederatedProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"qualified"|"partial"|"unknown"|"blocked"; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; envelope_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedProtocolReceipt(r:BrainFederatedProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==FEDERATED_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("federated protocol schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||r.protocol_version!=="aurora-research-federated/1.0"||r.method!=="POST"||r.route!=="/v1/research/evidence/federated/admit"||r.content_type!=="application/json"||!r.request_id.trim()||!r.idempotency_key.trim()||r.response_schema!=="FederatedEvidenceProtocolResponse1@1"||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||!r.candidate_order.length||!r.effect_receipts.length) throw new Error("federated protocol identity incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated protocol state is not covered"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts,r.aggregate_order]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated protocol ordering invalid"); if(![200,202,206,403,422].includes(r.status_code)) throw new Error("federated protocol status invalid"); for(const v of [r.envelope_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated protocol digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:federated-response:")&&e!=="block:unsafe-release")) throw new Error("federated protocol effect invalid"); }
export function brainFederatedProtocolReceiptDigest(r:BrainFederatedProtocolReceipt):string { validateBrainFederatedProtocolReceipt(r); return digestJsonSync(r); }
export const EVIDENCE_SAFETY_ASSURANCE_FEATURE_ID = "AFA-brain-P01-F25" as const;
export const EVIDENCE_SAFETY_ASSURANCE_CONTRACT_VERSION = "brain-evidence-assurance/1.0" as const;
export interface BrainEvidenceAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_id:string; scope:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; evidence_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainEvidenceAssuranceReceipt(r:BrainEvidenceAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==EVIDENCE_SAFETY_ASSURANCE_FEATURE_ID||r.contract_version!==EVIDENCE_SAFETY_ASSURANCE_CONTRACT_VERSION) throw new Error("evidence assurance schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.study_id.trim()||!r.scope.trim()||!["qualified","unresolved","blocked"].includes(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("evidence assurance identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("evidence assurance state is not covered"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("evidence assurance ordering invalid"); for(const v of [r.evidence_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("evidence assurance digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-evidence:")&&e!=="block:unsafe-release")) throw new Error("evidence assurance effect invalid"); }
export function brainEvidenceAssuranceReceiptDigest(r:BrainEvidenceAssuranceReceipt):string { validateBrainEvidenceAssuranceReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_SAFETY_ASSURANCE_FEATURE_ID = "AFA-brain-P01-F26" as const;
export const MULTIMODAL_SAFETY_ASSURANCE_CONTRACT_VERSION = "brain-multimodal-evidence-assurance/1.0" as const;
export interface BrainMultimodalAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_order:string[]; modality_order:string[]; scope:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; evidence_digest:string; comparability_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalAssuranceReceipt(r:BrainMultimodalAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_SAFETY_ASSURANCE_FEATURE_ID||r.contract_version!==MULTIMODAL_SAFETY_ASSURANCE_CONTRACT_VERSION) throw new Error("multimodal assurance schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||r.study_order.length<2||r.modality_order.length<2||!r.scope.trim()||!["qualified","unresolved","blocked"].includes(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("multimodal assurance identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal assurance state is not covered"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal assurance ordering invalid"); for(const v of [r.evidence_digest,r.comparability_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal assurance digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-multimodal:")&&e!=="block:unsafe-release")) throw new Error("multimodal assurance effect invalid"); }
export function brainMultimodalAssuranceReceiptDigest(r:BrainMultimodalAssuranceReceipt):string { validateBrainMultimodalAssuranceReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_SAFETY_ASSURANCE_FEATURE_ID = "AFA-brain-P01-F27" as const;
export const THROUGHPUT_SAFETY_ASSURANCE_CONTRACT_VERSION = "brain-throughput-evidence-assurance/1.0" as const;
export interface BrainThroughputAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; partition:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; checkpoint_seq:number; queue_digest:string; evidence_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputAssuranceReceipt(r:BrainThroughputAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_SAFETY_ASSURANCE_FEATURE_ID||r.contract_version!==THROUGHPUT_SAFETY_ASSURANCE_CONTRACT_VERSION) throw new Error("throughput assurance schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!["qualified","unresolved","blocked"].includes(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("throughput assurance identity incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput assurance state is not covered"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput assurance ordering invalid"); for(const v of [r.queue_digest,r.evidence_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput assurance digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:throughput:")&&e!=="block:unsafe-release")) throw new Error("throughput assurance effect invalid"); }
export function brainThroughputAssuranceReceiptDigest(r:BrainThroughputAssuranceReceipt):string { validateBrainThroughputAssuranceReceipt(r); return digestJsonSync(r); }
export const FEDERATED_SAFETY_ASSURANCE_FEATURE_ID = "AFA-brain-P01-F28" as const;
export const FEDERATED_SAFETY_ASSURANCE_CONTRACT_VERSION = "brain-federated-evidence-assurance/1.0" as const;
export interface BrainFederatedAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; witness_order:string[]; counterexample_order:string[]; envelope_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedAssuranceReceipt(r:BrainFederatedAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_SAFETY_ASSURANCE_FEATURE_ID||r.contract_version!==FEDERATED_SAFETY_ASSURANCE_CONTRACT_VERSION) throw new Error("federated assurance schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||!["qualified","unresolved","blocked"].includes(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("federated assurance identity incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated assurance state is not covered"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts,r.aggregate_order]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated assurance ordering invalid"); for(const v of [r.envelope_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated assurance digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:federated:")&&e!=="block:unsafe-release")) throw new Error("federated assurance effect invalid"); }
export function brainFederatedAssuranceReceiptDigest(r:BrainFederatedAssuranceReceipt):string { validateBrainFederatedAssuranceReceipt(r); return digestJsonSync(r); }
export const EVIDENCE_OPERATIONS_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P01-F29" as const;
export const EVIDENCE_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION = "brain-evidence-operations-control-plane/1.0" as const;
export interface BrainEvidenceOperationsReceipt { schema_version:string; contract_version:string; feature_id:string; operation_id:string; actor_id:string; request_id:string; disposition:"completed"|"degraded"|"unresolved"|"denied"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; attempts:number; recovered:boolean; telemetry_digest:string; evidence_digest:string; operations_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainEvidenceOperationsReceipt(r:BrainEvidenceOperationsReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==EVIDENCE_OPERATIONS_CONTROL_PLANE_FEATURE_ID||r.contract_version!==EVIDENCE_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("evidence operations schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.operation_id.trim()||!r.actor_id.trim()||!r.request_id.trim()||!["completed","degraded","unresolved","denied"].includes(r.disposition)||!r.candidate_order.length||r.attempts<1||!r.effect_receipts.length) throw new Error("evidence operations identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("evidence operations state is not covered"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("evidence operations ordering invalid"); for(const v of [r.telemetry_digest,r.evidence_digest,r.operations_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("evidence operations digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("ops:local-evidence:")&&e!=="block:unsafe-release")) throw new Error("evidence operations effect invalid"); }
export function brainEvidenceOperationsReceiptDigest(r:BrainEvidenceOperationsReceipt):string { validateBrainEvidenceOperationsReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_OPERATIONS_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P01-F30" as const;
export const MULTIMODAL_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION = "brain-multimodal-operations-control-plane/1.0" as const;
export interface BrainMultimodalOperationsReceipt { schema_version:string; contract_version:string; feature_id:string; operation_id:string; actor_id:string; request_id:string; study_order:string[]; modality_order:string[]; disposition:"completed"|"degraded"|"unresolved"|"denied"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; attempts:number; recovered:boolean; comparability_digest:string; evidence_digest:string; operations_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalOperationsReceipt(r:BrainMultimodalOperationsReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_OPERATIONS_CONTROL_PLANE_FEATURE_ID||r.contract_version!==MULTIMODAL_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("multimodal operations schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.operation_id.trim()||!r.actor_id.trim()||!r.request_id.trim()||r.study_order.length<2||r.modality_order.length<2||!["completed","degraded","unresolved","denied"].includes(r.disposition)||!r.candidate_order.length||r.attempts<1||!r.effect_receipts.length) throw new Error("multimodal operations identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal operations state is not covered"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal operations ordering invalid"); for(const v of [r.comparability_digest,r.evidence_digest,r.operations_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal operations digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("ops:local-multimodal:")&&e!=="block:unsafe-release")) throw new Error("multimodal operations effect invalid"); }
export function brainMultimodalOperationsReceiptDigest(r:BrainMultimodalOperationsReceipt):string { validateBrainMultimodalOperationsReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_OPERATIONS_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P01-F31" as const;
export const THROUGHPUT_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION = "brain-throughput-operations-control-plane/1.0" as const;
export interface BrainThroughputOperationsReceipt { schema_version:string; contract_version:string; feature_id:string; operation_id:string; actor_id:string; request_id:string; batch_id:string; partition:string; disposition:"completed"|"degraded"|"unresolved"|"denied"; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; attempts:number; recovered:boolean; queue_digest:string; evidence_digest:string; operations_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputOperationsReceipt(r:BrainThroughputOperationsReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_OPERATIONS_CONTROL_PLANE_FEATURE_ID||r.contract_version!==THROUGHPUT_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("throughput operations schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.operation_id.trim()||!r.actor_id.trim()||!r.request_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!["completed","degraded","unresolved","denied"].includes(r.disposition)||!r.candidate_order.length||r.attempts<1||!r.effect_receipts.length) throw new Error("throughput operations identity incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput operations state is not covered"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput operations ordering invalid"); for(const v of [r.queue_digest,r.evidence_digest,r.operations_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput operations digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("ops:throughput:")&&e!=="block:unsafe-release")) throw new Error("throughput operations effect invalid"); }
export function brainThroughputOperationsReceiptDigest(r:BrainThroughputOperationsReceipt):string { validateBrainThroughputOperationsReceipt(r); return digestJsonSync(r); }
export const FEDERATED_OPERATIONS_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P01-F32" as const;
export const FEDERATED_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION = "brain-federated-operations-control-plane/1.0" as const;
export interface BrainFederatedOperationsReceipt { schema_version:string; contract_version:string; feature_id:string; operation_id:string; actor_id:string; request_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; disposition:"completed"|"degraded"|"unresolved"|"denied"; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; checkpoint_seq:number; attempts:number; recovered:boolean; envelope_digest:string; operations_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedOperationsReceipt(r:BrainFederatedOperationsReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_OPERATIONS_CONTROL_PLANE_FEATURE_ID||r.contract_version!==FEDERATED_OPERATIONS_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("federated operations schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.operation_id.trim()||!r.actor_id.trim()||!r.request_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||!["completed","degraded","unresolved","denied"].includes(r.disposition)||!r.candidate_order.length||r.attempts<1||!r.effect_receipts.length) throw new Error("federated operations identity incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated operations state is not covered"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts,r.aggregate_order]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated operations ordering invalid"); for(const v of [r.envelope_digest,r.operations_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated operations digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("ops:federated:")&&e!=="block:unsafe-release")) throw new Error("federated operations effect invalid"); }
export function brainFederatedOperationsReceiptDigest(r:BrainFederatedOperationsReceipt):string { validateBrainFederatedOperationsReceipt(r); return digestJsonSync(r); }
export const RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F01" as const;
export const RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-retrieval-synthesis/1.0" as const;
export interface BrainEvidenceSynthesis { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_id:string; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; support_order:number[]; source_order:string[]; modality_order:string[]; semantic_order:string[]; artifact_order:string[]; provenance_order:string[]; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; replay_identity:string; synthesis_digest:string; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainEvidenceSynthesis(r:BrainEvidenceSynthesis):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_SYNTHESIS_FEATURE_ID||r.contract_version!==RETRIEVAL_SYNTHESIS_CONTRACT_VERSION) throw new Error("retrieval synthesis schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.study_id.trim()||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.candidate_order.length||!r.ranked_order.length||r.ranked_order.length!==r.support_order.length||!r.effect_receipts.length) throw new Error("retrieval synthesis identity incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("retrieval synthesis state is not covered"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.source_order,r.modality_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("retrieval synthesis ordering invalid"); for(const v of [r.replay_identity,r.synthesis_digest,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("retrieval synthesis digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-research-artifacts:")&&e!=="block:unsafe-release")) throw new Error("retrieval synthesis effect invalid"); }
export function brainEvidenceSynthesisDigest(r:BrainEvidenceSynthesis):string { validateBrainEvidenceSynthesis(r); return digestJsonSync(r); }
export const MULTIMODAL_RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F02" as const;
export const MULTIMODAL_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-multimodal-retrieval-synthesis/1.0" as const;
export interface BrainMultimodalEvidenceSynthesis { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_order:string[]; modality_order:string[]; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; support_order:number[]; comparability_digest:string; synthesis_digest:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; replay_identity:string; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalEvidenceSynthesis(r:BrainMultimodalEvidenceSynthesis):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_SYNTHESIS_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION) throw new Error("multimodal retrieval schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||r.study_order.length<2||r.modality_order.length<2||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.candidate_order.length||!r.ranked_order.length||r.ranked_order.length!==r.support_order.length||!r.effect_receipts.length) throw new Error("multimodal retrieval identity incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal retrieval state is not covered"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal retrieval ordering invalid"); for(const v of [r.comparability_digest,r.synthesis_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal retrieval digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-multimodal-artifacts:")&&e!=="block:unsafe-release")) throw new Error("multimodal retrieval effect invalid"); }
export function brainMultimodalEvidenceSynthesisDigest(r:BrainMultimodalEvidenceSynthesis):string { validateBrainMultimodalEvidenceSynthesis(r); return digestJsonSync(r); }
export const THROUGHPUT_RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F03" as const;
export const THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-throughput-retrieval-synthesis/1.0" as const;
export interface BrainThroughputEvidenceSynthesis { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; partition:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; support_order:number[]; checkpoint_seq:number; queue_digest:string; synthesis_digest:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; replay_identity:string; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputEvidenceSynthesis(r:BrainThroughputEvidenceSynthesis):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_SYNTHESIS_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION) throw new Error("throughput retrieval schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.candidate_order.length||!r.ranked_order.length||r.ranked_order.length!==r.support_order.length||!r.effect_receipts.length||r.checkpoint_seq<0) throw new Error("throughput retrieval identity incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput retrieval state is not covered"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput retrieval ordering invalid"); for(const v of [r.queue_digest,r.synthesis_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput retrieval digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-throughput-artifacts:")&&e!=="block:unsafe-release")) throw new Error("throughput retrieval effect invalid"); }
export function brainThroughputEvidenceSynthesisDigest(r:BrainThroughputEvidenceSynthesis):string { validateBrainThroughputEvidenceSynthesis(r); return digestJsonSync(r); }
export const FEDERATED_RETRIEVAL_SYNTHESIS_FEATURE_ID = "AFA-brain-P02-F04" as const;
export const FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION = "brain-federated-retrieval-synthesis/1.0" as const;
export interface BrainFederatedEvidenceSynthesis { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; study_order:string[]; modality_order:string[]; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; support_order:number[]; comparability_digest:string; envelope_digest:string; synthesis_digest:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; replay_identity:string; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedEvidenceSynthesis(r:BrainFederatedEvidenceSynthesis):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_SYNTHESIS_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_VERSION) throw new Error("federated retrieval schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.candidate_order.length||!r.ranked_order.length||r.ranked_order.length!==r.support_order.length||!r.effect_receipts.length) throw new Error("federated retrieval identity incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated retrieval state is not covered"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated retrieval ordering invalid"); for(const v of [r.comparability_digest,r.envelope_digest,r.synthesis_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated retrieval digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("exchange:permitted-artifacts:")&&e!=="block:unsafe-release")) throw new Error("federated retrieval effect invalid"); }
export function brainFederatedEvidenceSynthesisDigest(r:BrainFederatedEvidenceSynthesis):string { validateBrainFederatedEvidenceSynthesis(r); return digestJsonSync(r); }
export const RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F05" as const;
export const RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-retrieval-contract-model/1.0" as const;
export interface BrainRetrievalContractModelReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_id:string; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; compatibility:"additive"|"migration_required"|"breaking"|"unknown"; input_schema:string; output_schema:string; required_order:string[]; provided_order:string[]; missing_order:string[]; semantic_loss_order:string[]; semantic_digest:string; artifact_digest:string; provenance_digest:string; contract_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalContractModelReceipt(r:BrainRetrievalContractModelReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_CONTRACT_MODEL_FEATURE_ID||r.contract_version!==RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION||r.input_schema!=="ScopedRetrievalQuery1@1"||r.output_schema!=="EvidenceSynthesis2@1") throw new Error("retrieval contract schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.study_id.trim()||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!["additive","migration_required","breaking","unknown"].includes(r.compatibility)||!r.required_order.length||!r.provided_order.length||!r.effect_receipts.length) throw new Error("retrieval contract identity incomplete"); if(r.missing_order.some(v=>!r.required_order.includes(v))||r.semantic_loss_order.some(v=>!r.provided_order.includes(v))) throw new Error("retrieval contract loss state is not covered"); for(const v of [r.required_order,r.provided_order,r.missing_order,r.semantic_loss_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("retrieval contract ordering invalid"); for(const v of [r.semantic_digest,r.artifact_digest,r.provenance_digest,r.contract_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("retrieval contract digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-research-artifacts:")&&e!=="block:unsafe-release")) throw new Error("retrieval contract effect invalid"); }
export function brainRetrievalContractModelReceiptDigest(r:BrainRetrievalContractModelReceipt):string { validateBrainRetrievalContractModelReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F06" as const;
export const MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-multimodal-retrieval-contract-model/1.0" as const;
export interface BrainMultimodalRetrievalContractModelReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_order:string[]; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; compatibility:"additive"|"migration_required"|"breaking"|"unknown"; input_schema:string; output_schema:string; modality_required_order:string[]; modality_provided_order:string[]; modality_missing_order:string[]; semantic_loss_order:string[]; semantic_digest:string; comparability_digest:string; artifact_digest:string; provenance_digest:string; contract_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalContractModelReceipt(r:BrainMultimodalRetrievalContractModelReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION||r.input_schema!=="ScopedRetrievalQuery2@1"||r.output_schema!=="EvidenceSynthesis2@1") throw new Error("multimodal retrieval contract schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||r.study_order.length<2||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!["additive","migration_required","breaking","unknown"].includes(r.compatibility)||r.modality_required_order.length<2||!r.modality_provided_order.length||!r.effect_receipts.length) throw new Error("multimodal retrieval contract identity incomplete"); if(r.modality_missing_order.some(v=>!r.modality_required_order.includes(v))||r.semantic_loss_order.some(v=>!r.modality_provided_order.includes(v))) throw new Error("multimodal retrieval contract loss state is not covered"); for(const v of [r.study_order,r.modality_required_order,r.modality_provided_order,r.modality_missing_order,r.semantic_loss_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal retrieval contract ordering invalid"); for(const v of [r.semantic_digest,r.comparability_digest,r.artifact_digest,r.provenance_digest,r.contract_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal retrieval contract digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-multimodal-artifacts:")&&e!=="block:unsafe-release")) throw new Error("multimodal retrieval contract effect invalid"); }
export function brainMultimodalRetrievalContractModelReceiptDigest(r:BrainMultimodalRetrievalContractModelReceipt):string { validateBrainMultimodalRetrievalContractModelReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F07" as const;
export const THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-throughput-retrieval-contract-model/1.0" as const;
export interface BrainThroughputRetrievalContractModelReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; partition:string; max_items:number; checkpoint_seq:number; disposition:"qualified"|"partial"|"unknown"|"blocked"; compatibility:"additive"|"migration_required"|"breaking"|"unknown"; input_schema:string; output_schema:string; required_order:string[]; provided_order:string[]; missing_order:string[]; semantic_loss_order:string[]; queue_digest:string; semantic_digest:string; artifact_digest:string; provenance_digest:string; contract_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalContractModelReceipt(r:BrainThroughputRetrievalContractModelReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION||r.input_schema!=="ScopedRetrievalQuery3@1"||r.output_schema!=="EvidenceSynthesis2@1") throw new Error("throughput retrieval contract schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.partition.trim()||r.max_items<=0||r.checkpoint_seq<=0||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!["additive","migration_required","breaking","unknown"].includes(r.compatibility)||!r.required_order.length||!r.provided_order.length||!r.effect_receipts.length) throw new Error("throughput retrieval contract identity incomplete"); if(r.missing_order.some(v=>!r.required_order.includes(v))||r.semantic_loss_order.some(v=>!r.provided_order.includes(v))) throw new Error("throughput retrieval contract loss state is not covered"); for(const v of [r.required_order,r.provided_order,r.missing_order,r.semantic_loss_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput retrieval contract ordering invalid"); for(const v of [r.queue_digest,r.semantic_digest,r.artifact_digest,r.provenance_digest,r.contract_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput retrieval contract digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-throughput-artifacts:")&&e!=="block:unsafe-release")) throw new Error("throughput retrieval contract effect invalid"); }
export function brainThroughputRetrievalContractModelReceiptDigest(r:BrainThroughputRetrievalContractModelReceipt):string { validateBrainThroughputRetrievalContractModelReceipt(r); return digestJsonSync(r); }
export const FEDERATED_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID = "AFA-brain-P02-F08" as const;
export const FEDERATED_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION = "brain-federated-retrieval-contract-model/1.0" as const;
export interface BrainFederatedRetrievalContractModelReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; compatibility:"additive"|"migration_required"|"breaking"|"unknown"; input_schema:string; output_schema:string; permitted_artifact:string; comparability_digest:string; envelope_digest:string; semantic_digest:string; artifact_digest:string; provenance_digest:string; contract_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalContractModelReceipt(r:BrainFederatedRetrievalContractModelReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_CONTRACT_MODEL_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_CONTRACT_MODEL_CONTRACT_VERSION||r.input_schema!=="FederatedRetrievalQuery1@1"||r.output_schema!=="FederatedEvidenceSynthesis1@1") throw new Error("federated retrieval contract schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!["additive","migration_required","breaking","unknown"].includes(r.compatibility)||r.permitted_artifact!=="qualified-evidence-summary"||!r.effect_receipts.length) throw new Error("federated retrieval contract identity incomplete"); for(const v of [r.study_order,r.modality_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated retrieval contract ordering invalid"); for(const v of [r.comparability_digest,r.envelope_digest,r.semantic_digest,r.artifact_digest,r.provenance_digest,r.contract_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated retrieval contract digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("exchange:permitted-artifacts:")&&e!=="block:unsafe-release")) throw new Error("federated retrieval contract effect invalid"); }
export function brainFederatedRetrievalContractModelReceiptDigest(r:BrainFederatedRetrievalContractModelReceipt):string { validateBrainFederatedRetrievalContractModelReceipt(r); return digestJsonSync(r); }
export const RETRIEVAL_RESEARCH_COPILOT_FEATURE_ID = "AFA-brain-P02-F09" as const;
export const RETRIEVAL_RESEARCH_COPILOT_CONTRACT_VERSION = "brain-retrieval-research-copilot/1.0" as const;
export interface BrainRetrievalCopilotReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; operator_id:string; study_id:string; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; plan_order:string[]; action_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; synthesis_digest:string; plan_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalCopilotReceipt(r:BrainRetrievalCopilotReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_RESEARCH_COPILOT_FEATURE_ID||r.contract_version!==RETRIEVAL_RESEARCH_COPILOT_CONTRACT_VERSION) throw new Error("retrieval copilot schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.operator_id.trim()||!r.study_id.trim()||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.plan_order.length||!r.action_order.length||r.plan_order.length!==r.action_order.length||r.budget_units<=0||!r.effect_receipts.length) throw new Error("retrieval copilot identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("retrieval copilot state is not covered"); for(const v of [r.plan_order,r.action_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("retrieval copilot ordering invalid"); for(const v of [r.synthesis_digest,r.plan_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("retrieval copilot digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-research-artifacts:")&&e!=="block:unsafe-release")) throw new Error("retrieval copilot effect invalid"); }
export function brainRetrievalCopilotReceiptDigest(r:BrainRetrievalCopilotReceipt):string { validateBrainRetrievalCopilotReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_RETRIEVAL_COPILOT_FEATURE_ID = "AFA-brain-P02-F10" as const;
export const MULTIMODAL_RETRIEVAL_COPILOT_CONTRACT_VERSION = "brain-multimodal-retrieval-research-copilot/1.0" as const;
export interface BrainMultimodalRetrievalCopilotReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; operator_id:string; study_order:string[]; modality_order:string[]; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; plan_order:string[]; action_order:string[]; tool_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; comparability_digest:string; synthesis_digest:string; plan_digest:string; approval_reference:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalCopilotReceipt(r:BrainMultimodalRetrievalCopilotReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_COPILOT_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_COPILOT_CONTRACT_VERSION) throw new Error("multimodal retrieval copilot schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.operator_id.trim()||r.study_order.length<2||r.modality_order.length<2||!r.scope.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.plan_order.length||r.plan_order.length!==r.action_order.length||!r.tool_order.length||r.budget_units<=0||!r.effect_receipts.length) throw new Error("multimodal retrieval copilot identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal retrieval copilot state is not covered"); for(const v of [r.study_order,r.modality_order,r.plan_order,r.action_order,r.tool_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal retrieval copilot ordering invalid"); for(const v of [r.comparability_digest,r.synthesis_digest,r.plan_digest,r.approval_reference,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal retrieval copilot digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("invoke:declared-tool:")&&e!=="block:unsafe-release")) throw new Error("multimodal retrieval copilot effect invalid"); }
export function brainMultimodalRetrievalCopilotReceiptDigest(r:BrainMultimodalRetrievalCopilotReceipt):string { validateBrainMultimodalRetrievalCopilotReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_RETRIEVAL_COPILOT_FEATURE_ID = "AFA-brain-P02-F11" as const;
export const THROUGHPUT_RETRIEVAL_COPILOT_CONTRACT_VERSION = "brain-throughput-retrieval-research-copilot/1.0" as const;
export interface BrainThroughputRetrievalCopilotReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; operator_id:string; batch_id:string; partition:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; plan_order:string[]; action_order:string[]; tool_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; queue_digest:string; synthesis_digest:string; plan_digest:string; approval_reference:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalCopilotReceipt(r:BrainThroughputRetrievalCopilotReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_COPILOT_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_COPILOT_CONTRACT_VERSION) throw new Error("throughput retrieval copilot schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.operator_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.plan_order.length||r.plan_order.length!==r.action_order.length||!r.tool_order.length||r.checkpoint_seq<=0||r.budget_units<=0||!r.effect_receipts.length) throw new Error("throughput retrieval copilot identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput retrieval copilot state is not covered"); for(const v of [r.plan_order,r.action_order,r.tool_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput retrieval copilot ordering invalid"); for(const v of [r.queue_digest,r.synthesis_digest,r.plan_digest,r.approval_reference,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput retrieval copilot digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("invoke:declared-tool:")&&e!=="block:unsafe-release")) throw new Error("throughput retrieval copilot effect invalid"); }
export function brainThroughputRetrievalCopilotReceiptDigest(r:BrainThroughputRetrievalCopilotReceipt):string { validateBrainThroughputRetrievalCopilotReceipt(r); return digestJsonSync(r); }
export const FEDERATED_RETRIEVAL_COPILOT_FEATURE_ID = "AFA-brain-P02-F12" as const;
export const FEDERATED_RETRIEVAL_COPILOT_CONTRACT_VERSION = "brain-federated-retrieval-research-copilot/1.0" as const;
export interface BrainFederatedRetrievalCopilotReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; operator_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; plan_order:string[]; action_order:string[]; tool_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; comparability_digest:string; envelope_digest:string; synthesis_digest:string; plan_digest:string; approval_reference:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalCopilotReceipt(r:BrainFederatedRetrievalCopilotReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_COPILOT_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_COPILOT_CONTRACT_VERSION) throw new Error("federated retrieval copilot schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.operator_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!["qualified","partial","unknown","blocked"].includes(r.disposition)||!r.plan_order.length||r.plan_order.length!==r.action_order.length||!r.tool_order.length||r.budget_units<=0||!r.effect_receipts.length) throw new Error("federated retrieval copilot identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated retrieval copilot state is not covered"); for(const v of [r.study_order,r.modality_order,r.plan_order,r.action_order,r.tool_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated retrieval copilot ordering invalid"); for(const v of [r.comparability_digest,r.envelope_digest,r.synthesis_digest,r.plan_digest,r.approval_reference,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated retrieval copilot digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("exchange:permitted-artifacts:")&&e!=="block:unsafe-release")) throw new Error("federated retrieval copilot effect invalid"); }
export function brainFederatedRetrievalCopilotReceiptDigest(r:BrainFederatedRetrievalCopilotReceipt):string { validateBrainFederatedRetrievalCopilotReceipt(r); return digestJsonSync(r); }
export const RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P02-F13" as const;
export const RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-retrieval-workflow-fabric/1.0" as const;
export interface BrainRetrievalWorkflowFabricReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workflow_id:string; study_id:string; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; stage_order:string[]; plan_order:string[]; completed_order:string[]; blocked_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; unknown_order:string[]; synthesis_digest:string; checkpoint_digest:string; workflow_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalWorkflowFabricReceipt(r:BrainRetrievalWorkflowFabricReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("retrieval workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workflow_id.trim()||!r.study_id.trim()||!r.scope.trim()||!r.stage_order.length||!r.plan_order.length||!r.completed_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("retrieval workflow identity, stages, plan, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("retrieval workflow state is not covered"); for(const v of [r.stage_order,r.plan_order,r.completed_order,r.blocked_order,r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("retrieval workflow ordering is invalid"); for(const v of [r.synthesis_digest,r.checkpoint_digest,r.workflow_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("retrieval workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:retrieval-work:")&&!e.startsWith("compensate:retrieval-work:")&&e!=="block:unsafe-release")) throw new Error("retrieval workflow effect is outside schedule/compensation gate"); if(r.disposition==="qualified"&&!r.effect_receipts.some(e=>e.startsWith("schedule:retrieval-work:"))) throw new Error("qualified retrieval workflow requires schedule receipt"); }
export function brainRetrievalWorkflowFabricReceiptDigest(r:BrainRetrievalWorkflowFabricReceipt):string { validateBrainRetrievalWorkflowFabricReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P02-F14" as const;
export const MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-multimodal-retrieval-workflow-fabric/1.0" as const;
export interface BrainMultimodalRetrievalWorkflowFabricReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workflow_id:string; scope:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; stage_order:string[]; plan_order:string[]; completed_order:string[]; blocked_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; unknown_order:string[]; comparability_digest:string; synthesis_digest:string; checkpoint_digest:string; workflow_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalWorkflowFabricReceipt(r:BrainMultimodalRetrievalWorkflowFabricReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("multimodal retrieval workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workflow_id.trim()||!r.scope.trim()||r.study_order.length<2||r.modality_order.length<2||!r.stage_order.length||!r.plan_order.length||!r.completed_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("multimodal workflow identity, coverage, stages, plan, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal workflow state is not covered"); for(const v of [r.study_order,r.modality_order,r.stage_order,r.plan_order,r.completed_order,r.blocked_order,r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal workflow ordering is invalid"); for(const v of [r.comparability_digest,r.synthesis_digest,r.checkpoint_digest,r.workflow_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:multimodal-retrieval-work:")&&!e.startsWith("compensate:multimodal-retrieval-work:")&&e!=="block:unsafe-release")) throw new Error("multimodal workflow effect is outside schedule/compensation gate"); if(r.disposition==="qualified"&&!r.effect_receipts.some(e=>e.startsWith("schedule:multimodal-retrieval-work:"))) throw new Error("qualified multimodal workflow requires schedule receipt"); }
export function brainMultimodalRetrievalWorkflowFabricReceiptDigest(r:BrainMultimodalRetrievalWorkflowFabricReceipt):string { validateBrainMultimodalRetrievalWorkflowFabricReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P02-F15" as const;
export const THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-throughput-retrieval-workflow-fabric/1.0" as const;
export interface BrainThroughputRetrievalWorkflowFabricReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workflow_id:string; batch_id:string; partition:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; stage_order:string[]; plan_order:string[]; completed_order:string[]; blocked_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; unknown_order:string[]; checkpoint_seq:number; queue_digest:string; synthesis_digest:string; checkpoint_digest:string; workflow_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalWorkflowFabricReceipt(r:BrainThroughputRetrievalWorkflowFabricReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("throughput retrieval workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workflow_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!r.stage_order.length||!r.plan_order.length||!r.completed_order.length||!r.effect_receipts.length||!Number.isInteger(r.checkpoint_seq)||r.checkpoint_seq<=0||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("throughput workflow identity, queue, checkpoint, stages, plan, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput workflow state is not covered"); for(const v of [r.stage_order,r.plan_order,r.completed_order,r.blocked_order,r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput workflow ordering is invalid"); for(const v of [r.queue_digest,r.synthesis_digest,r.checkpoint_digest,r.workflow_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:throughput-retrieval-work:")&&!e.startsWith("compensate:throughput-retrieval-work:")&&e!=="block:unsafe-release")) throw new Error("throughput workflow effect is outside schedule/compensation gate"); if(r.disposition==="qualified"&&!r.effect_receipts.some(e=>e.startsWith("schedule:throughput-retrieval-work:"))) throw new Error("qualified throughput workflow requires schedule receipt"); }
export function brainThroughputRetrievalWorkflowFabricReceiptDigest(r:BrainThroughputRetrievalWorkflowFabricReceipt):string { validateBrainThroughputRetrievalWorkflowFabricReceipt(r); return digestJsonSync(r); }
export const FEDERATED_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P02-F16" as const;
export const FEDERATED_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-federated-retrieval-workflow-fabric/1.0" as const;
export interface BrainFederatedRetrievalWorkflowFabricReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workflow_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; stage_order:string[]; plan_order:string[]; completed_order:string[]; blocked_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; unknown_order:string[]; aggregate_order:string[]; comparability_digest:string; envelope_digest:string; synthesis_digest:string; checkpoint_digest:string; workflow_digest:string; approval_reference:string; replay_identity:string; checkpoint_seq:number; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalWorkflowFabricReceipt(r:BrainFederatedRetrievalWorkflowFabricReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("federated workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workflow_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!r.stage_order.length||!r.plan_order.length||!r.completed_order.length||!r.effect_receipts.length||!Number.isInteger(r.checkpoint_seq)||r.checkpoint_seq<=0||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("federated workflow identity, coverage, stages, plan, checkpoint, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated workflow state is not covered"); for(const v of [r.study_order,r.modality_order,r.stage_order,r.plan_order,r.completed_order,r.blocked_order,r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated workflow ordering is invalid"); if(JSON.stringify([...new Set(r.aggregate_order)].sort())!==JSON.stringify(r.aggregate_order)) throw new Error("federated aggregate ordering is invalid"); for(const v of [r.comparability_digest,r.envelope_digest,r.synthesis_digest,r.checkpoint_digest,r.workflow_digest,r.approval_reference,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:federated-retrieval-work:")&&!e.startsWith("compensate:federated-retrieval-work:")&&e!=="block:unsafe-release")) throw new Error("federated workflow effect is outside schedule/compensation gate"); if(r.disposition==="qualified"&&!r.effect_receipts.some(e=>e.startsWith("schedule:federated-retrieval-work:"))) throw new Error("qualified federated workflow requires schedule receipt"); }
export function brainFederatedRetrievalWorkflowFabricReceiptDigest(r:BrainFederatedRetrievalWorkflowFabricReceipt):string { validateBrainFederatedRetrievalWorkflowFabricReceipt(r); return digestJsonSync(r); }
export const RETRIEVAL_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P02-F17" as const;
export const RETRIEVAL_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-retrieval-research-workbench/1.0" as const;
export interface BrainRetrievalResearchWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workspace_id:string; study_id:string; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; view_order:string[]; panel_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; synthesis_digest:string; workbench_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalResearchWorkbenchReceipt(r:BrainRetrievalResearchWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_RESEARCH_WORKBENCH_FEATURE_ID||r.contract_version!==RETRIEVAL_RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("retrieval workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workspace_id.trim()||!r.study_id.trim()||!r.scope.trim()||!r.view_order.length||!r.panel_order.length||!r.action_receipts.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("workbench identity, views, panels, retrieval, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("workbench retrieval state is not covered"); for(const v of [r.view_order,r.panel_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("workbench ordering is not canonical"); for(const v of [r.synthesis_digest,r.workbench_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-retrieval-artifacts:")&&e!=="block:unsafe-release")) throw new Error("workbench effect is not read-only"); }
export function brainRetrievalResearchWorkbenchReceiptDigest(r:BrainRetrievalResearchWorkbenchReceipt):string { validateBrainRetrievalResearchWorkbenchReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_RETRIEVAL_WORKBENCH_FEATURE_ID = "AFA-brain-P02-F18" as const;
export const MULTIMODAL_RETRIEVAL_WORKBENCH_CONTRACT_VERSION = "brain-multimodal-retrieval-research-workbench/1.0" as const;
export interface BrainMultimodalRetrievalWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workspace_id:string; scope:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; view_order:string[]; panel_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; comparability_digest:string; synthesis_digest:string; workbench_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalWorkbenchReceipt(r:BrainMultimodalRetrievalWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_WORKBENCH_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_WORKBENCH_CONTRACT_VERSION) throw new Error("multimodal workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workspace_id.trim()||!r.scope.trim()||r.study_order.length<2||r.modality_order.length<2||!r.view_order.length||!r.panel_order.length||!r.action_receipts.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("multimodal workbench identity, coverage, views, panels, retrieval, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal workbench state is not covered by candidates"); for(const v of [r.study_order,r.modality_order,r.view_order,r.panel_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal workbench ordering is not canonical"); for(const v of [r.comparability_digest,r.synthesis_digest,r.workbench_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-multimodal-retrieval-artifacts:")&&e!=="block:unsafe-release")) throw new Error("multimodal workbench effect is not read-only"); }
export function brainMultimodalRetrievalWorkbenchReceiptDigest(r:BrainMultimodalRetrievalWorkbenchReceipt):string { validateBrainMultimodalRetrievalWorkbenchReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_RETRIEVAL_WORKBENCH_FEATURE_ID = "AFA-brain-P02-F19" as const;
export const THROUGHPUT_RETRIEVAL_WORKBENCH_CONTRACT_VERSION = "brain-throughput-retrieval-research-workbench/1.0" as const;
export interface BrainThroughputRetrievalWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workspace_id:string; batch_id:string; partition:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; view_order:string[]; panel_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; queue_digest:string; synthesis_digest:string; workbench_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalWorkbenchReceipt(r:BrainThroughputRetrievalWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_WORKBENCH_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_WORKBENCH_CONTRACT_VERSION) throw new Error("throughput workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workspace_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!r.view_order.length||!r.panel_order.length||!r.action_receipts.length||!r.candidate_order.length||!Number.isInteger(r.checkpoint_seq)||r.checkpoint_seq<=0||!Number.isInteger(r.budget_units)||r.budget_units<=0||!r.effect_receipts.length) throw new Error("throughput workbench identity, queue, checkpoint, views, panels, budget, locality, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput workbench state is not covered by candidates"); for(const v of [r.view_order,r.panel_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput workbench ordering is not canonical"); for(const v of [r.queue_digest,r.synthesis_digest,r.workbench_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-throughput-retrieval-artifacts:")&&e!=="block:unsafe-release")) throw new Error("throughput workbench effect is not read-only"); }
export function brainThroughputRetrievalWorkbenchReceiptDigest(r:BrainThroughputRetrievalWorkbenchReceipt):string { validateBrainThroughputRetrievalWorkbenchReceipt(r); return digestJsonSync(r); }

export const RETRIEVAL_PROTOCOL_FEATURE_ID = "AFA-brain-P02-F21" as const;
export const RETRIEVAL_PROTOCOL_CONTRACT_VERSION = "brain-retrieval-protocol-gateway/1.0" as const;
export const RETRIEVAL_PROTOCOL_STAGE_ORDER = ["protocol:open", "protocol:authorize", "protocol:retrieve", "protocol:synthesize", "protocol:close"] as const;
export interface BrainRetrievalProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_id:string; session_id:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; offered_capability_order:string[]; required_capability_order:string[]; negotiated_capability_order:string[]; stage_order:string[]; completed_stage_order:string[]; blocked_stage_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; negotiation_digest:string; transcript_digest:string; synthesis_digest:string; protocol_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalProtocolReceipt(r:BrainRetrievalProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_PROTOCOL_FEATURE_ID||r.contract_version!==RETRIEVAL_PROTOCOL_CONTRACT_VERSION) throw new Error("retrieval protocol schema, feature, or version mismatch"); const stages=["protocol:open","protocol:authorize","protocol:retrieve","protocol:synthesize","protocol:close"]; if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.protocol_id.trim()||!r.session_id.trim()||!r.offered_capability_order.length||!r.required_capability_order.length||JSON.stringify(r.stage_order)!==JSON.stringify(stages)||!r.completed_stage_order.length||!r.action_receipts.length||!r.candidate_order.length||!Number.isInteger(r.budget_units)||r.budget_units<stages.length||!r.effect_receipts.length) throw new Error("retrieval protocol identity, negotiation, stages, budget, locality, or effects are incomplete"); for(const v of [r.offered_capability_order,r.required_capability_order,r.negotiated_capability_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("retrieval protocol vectors are not canonical"); if((r.disposition!=="blocked"&&r.required_capability_order.some(v=>!r.offered_capability_order.includes(v)))||r.negotiated_capability_order.some(v=>!r.required_capability_order.includes(v))) throw new Error("retrieval protocol capability negotiation is invalid"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("retrieval protocol evidence state is not covered by candidates"); if([...r.completed_stage_order,...r.blocked_stage_order].some(v=>!stages.includes(v))||r.completed_stage_order.some(v=>r.blocked_stage_order.includes(v))) throw new Error("retrieval protocol stage transcript is invalid"); for(const v of [r.negotiation_digest,r.transcript_digest,r.synthesis_digest,r.protocol_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("retrieval protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-retrieval-protocol:")&&e!=="block:unsafe-release")) throw new Error("retrieval protocol effect is not read-only"); }
export function brainRetrievalProtocolReceiptDigest(r:BrainRetrievalProtocolReceipt):string { validateBrainRetrievalProtocolReceipt(r); return digestJsonSync(r); }

export const MULTIMODAL_RETRIEVAL_PROTOCOL_FEATURE_ID = "AFA-brain-P02-F22" as const;
export const MULTIMODAL_RETRIEVAL_PROTOCOL_CONTRACT_VERSION = "brain-multimodal-retrieval-protocol-gateway/1.0" as const;
export interface BrainMultimodalRetrievalProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_id:string; session_id:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; offered_capability_order:string[]; required_capability_order:string[]; negotiated_capability_order:string[]; stage_order:string[]; completed_stage_order:string[]; blocked_stage_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; comparability_digest:string; negotiation_digest:string; transcript_digest:string; synthesis_digest:string; protocol_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalProtocolReceipt(r:BrainMultimodalRetrievalProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_PROTOCOL_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_PROTOCOL_CONTRACT_VERSION) throw new Error("multimodal retrieval protocol schema, feature, or version mismatch"); const stages=["protocol:open","protocol:authorize","protocol:retrieve","protocol:synthesize","protocol:close"]; if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.protocol_id.trim()||!r.session_id.trim()||r.study_order.length<2||r.modality_order.length<2||!r.offered_capability_order.length||!r.required_capability_order.length||JSON.stringify(r.stage_order)!==JSON.stringify(stages)||!r.completed_stage_order.length||!r.action_receipts.length||!r.candidate_order.length||!Number.isInteger(r.budget_units)||r.budget_units<stages.length||!r.effect_receipts.length) throw new Error("multimodal protocol identity, coverage, negotiation, stages, budget, locality, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.offered_capability_order,r.required_capability_order,r.negotiated_capability_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal protocol vectors are not canonical"); if((r.disposition!=="blocked"&&r.required_capability_order.some(v=>!r.offered_capability_order.includes(v)))||r.negotiated_capability_order.some(v=>!r.required_capability_order.includes(v))||[...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal protocol state is not covered by its declaration"); if([...r.completed_stage_order,...r.blocked_stage_order].some(v=>!stages.includes(v))||r.completed_stage_order.some(v=>r.blocked_stage_order.includes(v))) throw new Error("multimodal protocol stage transcript is invalid"); for(const v of [r.comparability_digest,r.negotiation_digest,r.transcript_digest,r.synthesis_digest,r.protocol_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-multimodal-protocol:")&&e!=="block:unsafe-release")) throw new Error("multimodal protocol effect is not read-only"); }
export function brainMultimodalRetrievalProtocolReceiptDigest(r:BrainMultimodalRetrievalProtocolReceipt):string { validateBrainMultimodalRetrievalProtocolReceipt(r); return digestJsonSync(r); }

export const THROUGHPUT_RETRIEVAL_PROTOCOL_FEATURE_ID = "AFA-brain-P02-F23" as const;
export const THROUGHPUT_RETRIEVAL_PROTOCOL_CONTRACT_VERSION = "brain-throughput-retrieval-protocol-gateway/1.0" as const;
export interface BrainThroughputRetrievalProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_id:string; session_id:string; batch_id:string; partition:string; checkpoint_seq:number; queue_digest:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; offered_capability_order:string[]; required_capability_order:string[]; negotiated_capability_order:string[]; stage_order:string[]; completed_stage_order:string[]; blocked_stage_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; negotiation_digest:string; transcript_digest:string; synthesis_digest:string; protocol_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalProtocolReceipt(r:BrainThroughputRetrievalProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_PROTOCOL_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_PROTOCOL_CONTRACT_VERSION) throw new Error("throughput retrieval protocol schema, feature, or version mismatch"); const stages=["protocol:open","protocol:authorize","protocol:retrieve","protocol:synthesize","protocol:close"]; if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.protocol_id.trim()||!r.session_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!Number.isInteger(r.checkpoint_seq)||r.checkpoint_seq<=0||!r.offered_capability_order.length||!r.required_capability_order.length||JSON.stringify(r.stage_order)!==JSON.stringify(stages)||!r.completed_stage_order.length||!r.action_receipts.length||!r.candidate_order.length||!Number.isInteger(r.budget_units)||r.budget_units<stages.length||!r.effect_receipts.length) throw new Error("throughput protocol identity, queue, negotiation, stages, budget, locality, or effects are incomplete"); for(const v of [r.offered_capability_order,r.required_capability_order,r.negotiated_capability_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput protocol vectors are not canonical"); if((r.disposition!=="blocked"&&r.required_capability_order.some(v=>!r.offered_capability_order.includes(v)))||r.negotiated_capability_order.some(v=>!r.required_capability_order.includes(v))||[...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput protocol state is not covered by its declaration"); if([...r.completed_stage_order,...r.blocked_stage_order].some(v=>!stages.includes(v))||r.completed_stage_order.some(v=>r.blocked_stage_order.includes(v))) throw new Error("throughput protocol stage transcript is invalid"); for(const v of [r.queue_digest,r.negotiation_digest,r.transcript_digest,r.synthesis_digest,r.protocol_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-throughput-protocol:")&&e!=="block:unsafe-release")) throw new Error("throughput protocol effect is not read-only"); }
export function brainThroughputRetrievalProtocolReceiptDigest(r:BrainThroughputRetrievalProtocolReceipt):string { validateBrainThroughputRetrievalProtocolReceipt(r); return digestJsonSync(r); }

export const FEDERATED_RETRIEVAL_PROTOCOL_FEATURE_ID = "AFA-brain-P02-F24" as const;
export const FEDERATED_RETRIEVAL_PROTOCOL_CONTRACT_VERSION = "brain-federated-retrieval-protocol-gateway/1.0" as const;
export interface BrainFederatedRetrievalProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_id:string; session_id:string; federation_id:string; institution_id:string; purpose:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; offered_capability_order:string[]; required_capability_order:string[]; negotiated_capability_order:string[]; stage_order:string[]; completed_stage_order:string[]; blocked_stage_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; comparability_digest:string; envelope_digest:string; negotiation_digest:string; transcript_digest:string; synthesis_digest:string; protocol_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalProtocolReceipt(r:BrainFederatedRetrievalProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_PROTOCOL_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_PROTOCOL_CONTRACT_VERSION) throw new Error("federated retrieval protocol schema, feature, or version mismatch"); const stages=["protocol:open","protocol:authorize","protocol:retrieve","protocol:synthesize","protocol:close"]; if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.protocol_id.trim()||!r.session_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!r.offered_capability_order.length||!r.required_capability_order.length||JSON.stringify(r.stage_order)!==JSON.stringify(stages)||!r.completed_stage_order.length||!r.action_receipts.length||!r.candidate_order.length||!Number.isInteger(r.budget_units)||r.budget_units<stages.length||!r.effect_receipts.length) throw new Error("federated protocol identity, coverage, negotiation, stages, budget, locality, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.offered_capability_order,r.required_capability_order,r.negotiated_capability_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated protocol vectors are not canonical"); if(JSON.stringify([...r.aggregate_order].sort())!==JSON.stringify(r.aggregate_order)) throw new Error("federated aggregate order is not canonical"); if((r.disposition!=="blocked"&&r.required_capability_order.some(v=>!r.offered_capability_order.includes(v)))||r.negotiated_capability_order.some(v=>!r.required_capability_order.includes(v))||[...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated protocol state is not covered by its declaration"); if([...r.completed_stage_order,...r.blocked_stage_order].some(v=>!stages.includes(v))||r.completed_stage_order.some(v=>r.blocked_stage_order.includes(v))) throw new Error("federated protocol stage transcript is invalid"); for(const v of [r.comparability_digest,r.envelope_digest,r.negotiation_digest,r.transcript_digest,r.synthesis_digest,r.protocol_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("read:local-federated-protocol:")&&e!=="block:unsafe-release")) throw new Error("federated protocol effect is not read-only"); }
export function brainFederatedRetrievalProtocolReceiptDigest(r:BrainFederatedRetrievalProtocolReceipt):string { validateBrainFederatedRetrievalProtocolReceipt(r); return digestJsonSync(r); }

export const RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-brain-P02-F25" as const;
export const RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "brain-retrieval-assurance-harness/1.0" as const;
export interface BrainRetrievalAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_id:string; scope:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; synthesis_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalAssuranceReceipt(r:BrainRetrievalAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_ASSURANCE_FEATURE_ID||r.contract_version!==RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("retrieval assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.study_id.trim()||!r.scope.trim()||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("retrieval assurance identity, verdict, witnesses, locality, or effects are incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("retrieval assurance state is not covered by candidates"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("retrieval assurance ordering is not canonical"); for(const v of [r.synthesis_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("retrieval assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-retrieval:")&&e!=="block:unsafe-release")) throw new Error("retrieval assurance effect is outside the local release gate"); }
export function brainRetrievalAssuranceReceiptDigest(r:BrainRetrievalAssuranceReceipt):string { validateBrainRetrievalAssuranceReceipt(r); return digestJsonSync(r); }

export const MULTIMODAL_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-brain-P02-F26" as const;
export const MULTIMODAL_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "brain-multimodal-retrieval-assurance-harness/1.0" as const;
export interface BrainMultimodalRetrievalAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_order:string[]; modality_order:string[]; scope:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; comparability_digest:string; synthesis_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalAssuranceReceipt(r:BrainMultimodalRetrievalAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_ASSURANCE_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("multimodal retrieval assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||r.study_order.length<2||r.modality_order.length<2||!r.scope.trim()||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("multimodal retrieval assurance identity, closure, verdict, witnesses, locality, or effects are incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal retrieval assurance state is not covered by candidates"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal retrieval assurance ordering is not canonical"); for(const v of [r.comparability_digest,r.synthesis_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal retrieval assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-multimodal-retrieval:")&&e!=="block:unsafe-release")) throw new Error("multimodal retrieval assurance effect is outside the local gate"); }
export function brainMultimodalRetrievalAssuranceReceiptDigest(r:BrainMultimodalRetrievalAssuranceReceipt):string { validateBrainMultimodalRetrievalAssuranceReceipt(r); return digestJsonSync(r); }

export const THROUGHPUT_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-brain-P02-F27" as const;
export const THROUGHPUT_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "brain-throughput-retrieval-assurance-harness/1.0" as const;
export interface BrainThroughputRetrievalAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; partition:string; checkpoint_seq:number; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; queue_digest:string; synthesis_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalAssuranceReceipt(r:BrainThroughputRetrievalAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_ASSURANCE_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("throughput retrieval assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!Number.isInteger(r.checkpoint_seq)||r.checkpoint_seq<=0||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("throughput assurance identity, queue, checkpoint, verdict, witnesses, locality, or effects are incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput assurance state is not covered by candidates"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput assurance ordering is not canonical"); for(const v of [r.queue_digest,r.synthesis_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-throughput-retrieval:")&&e!=="block:unsafe-release")) throw new Error("throughput assurance effect is outside the local gate"); }
export function brainThroughputRetrievalAssuranceReceiptDigest(r:BrainThroughputRetrievalAssuranceReceipt):string { validateBrainThroughputRetrievalAssuranceReceipt(r); return digestJsonSync(r); }

export const FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID = "AFA-brain-P02-F28" as const;
export const FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION = "brain-federated-retrieval-assurance-harness/1.0" as const;
export interface BrainFederatedRetrievalAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; institution_id:string; purpose:string; endpoint:string; study_order:string[]; modality_order:string[]; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; witness_order:string[]; counterexample_order:string[]; comparability_digest:string; envelope_digest:string; synthesis_digest:string; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalAssuranceReceipt(r:BrainFederatedRetrievalAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("federated retrieval assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length) throw new Error("federated assurance identity, closure, verdict, witnesses, locality, or effects are incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated assurance state is not covered by candidates"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated assurance ordering is not canonical"); if(JSON.stringify([...r.aggregate_order].sort())!==JSON.stringify(r.aggregate_order)) throw new Error("federated aggregate order is not canonical"); for(const v of [r.comparability_digest,r.envelope_digest,r.synthesis_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-federated-retrieval:")&&e!=="block:unsafe-release")) throw new Error("federated assurance effect is outside the local gate"); }
export function brainFederatedRetrievalAssuranceReceiptDigest(r:BrainFederatedRetrievalAssuranceReceipt):string { validateBrainFederatedRetrievalAssuranceReceipt(r); return digestJsonSync(r); }

export const RETRIEVAL_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P02-F29" as const;
export const RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION = "brain-retrieval-federated-control-plane/1.0" as const;
export const RETRIEVAL_CONTROL_ACTION_ORDER = ["control:observe", "control:reconcile", "control:authorize", "control:publish"] as const;
export interface BrainRetrievalFederatedControlPlaneReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; plane_id:string; session_id:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; action_order:string[]; completed_action_order:string[]; blocked_action_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; synthesis_digest:string; control_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainRetrievalFederatedControlPlaneReceipt(r:BrainRetrievalFederatedControlPlaneReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==RETRIEVAL_CONTROL_PLANE_FEATURE_ID||r.contract_version!==RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("retrieval control plane schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.plane_id.trim()||!r.session_id.trim()||JSON.stringify(r.action_order)!==JSON.stringify(RETRIEVAL_CONTROL_ACTION_ORDER)||!r.completed_action_order.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("control-plane identity, actions, retrieval, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("control-plane evidence state is not covered by candidates"); const actionPositions = new Map(RETRIEVAL_CONTROL_ACTION_ORDER.map((action,index)=>[action,index])); for(const values of [r.completed_action_order,r.blocked_action_order]) { if(values.some(v=>!actionPositions.has(v))||values.some((v,i)=>i>0 && (actionPositions.get(values[i-1])??-1)>=(actionPositions.get(v)??-1))||r.completed_action_order.some(v=>r.blocked_action_order.includes(v))) throw new Error("control-plane action transcript is not canonical"); } for(const v of [r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("control-plane ordering is not canonical"); for(const v of [r.synthesis_digest,r.control_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("control-plane digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("manage:local-retrieval-control:")&&e!=="block:unsafe-release")) throw new Error("control-plane effect is outside local management gate"); }
export function brainRetrievalFederatedControlPlaneReceiptDigest(r:BrainRetrievalFederatedControlPlaneReceipt):string { validateBrainRetrievalFederatedControlPlaneReceipt(r); return digestJsonSync(r); }

export const MULTIMODAL_RETRIEVAL_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P02-F30" as const;
export const MULTIMODAL_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION = "brain-multimodal-retrieval-control-plane/1.0" as const;
export const MULTIMODAL_RETRIEVAL_CONTROL_ACTION_ORDER = ["control:observe", "control:reconcile", "control:authorize", "control:publish"] as const;
export interface BrainMultimodalRetrievalControlPlaneReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; plane_id:string; session_id:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; action_order:string[]; completed_action_order:string[]; blocked_action_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; comparability_digest:string; synthesis_digest:string; control_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalRetrievalControlPlaneReceipt(r:BrainMultimodalRetrievalControlPlaneReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_RETRIEVAL_CONTROL_PLANE_FEATURE_ID||r.contract_version!==MULTIMODAL_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("multimodal control plane schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.plane_id.trim()||!r.session_id.trim()||r.study_order.length<2||r.modality_order.length<2||JSON.stringify(r.action_order)!==JSON.stringify(MULTIMODAL_RETRIEVAL_CONTROL_ACTION_ORDER)||!r.completed_action_order.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("multimodal control-plane identity, closure, actions, retrieval, locality, budget, or effects are incomplete"); const actionPositions = new Map(MULTIMODAL_RETRIEVAL_CONTROL_ACTION_ORDER.map((action,index)=>[action,index])); for(const values of [r.completed_action_order,r.blocked_action_order]) { if(values.some(v=>!actionPositions.has(v))||values.some((v,i)=>i>0 && (actionPositions.get(values[i-1])??-1)>=(actionPositions.get(v)??-1))||r.completed_action_order.some(v=>r.blocked_action_order.includes(v))) throw new Error("multimodal control-plane action transcript is not canonical"); } if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal control-plane evidence state is not covered by candidates"); for(const v of [r.study_order,r.modality_order,r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal control-plane ordering is not canonical"); for(const v of [r.comparability_digest,r.synthesis_digest,r.control_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal control-plane digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("manage:local-multimodal-retrieval-control:")&&e!=="block:unsafe-release")) throw new Error("multimodal control-plane effect is outside local management gate"); }
export function brainMultimodalRetrievalControlPlaneReceiptDigest(r:BrainMultimodalRetrievalControlPlaneReceipt):string { validateBrainMultimodalRetrievalControlPlaneReceipt(r); return digestJsonSync(r); }

export const THROUGHPUT_RETRIEVAL_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P02-F31" as const;
export const THROUGHPUT_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION = "brain-throughput-retrieval-control-plane/1.0" as const;
export const THROUGHPUT_RETRIEVAL_CONTROL_ACTION_ORDER = ["control:observe", "control:reconcile", "control:authorize", "control:publish"] as const;
export interface BrainThroughputRetrievalControlPlaneReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; plane_id:string; session_id:string; batch_id:string; partition:string; checkpoint_seq:number; action_order:string[]; completed_action_order:string[]; blocked_action_order:string[]; compensation_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; queue_digest:string; synthesis_digest:string; control_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputRetrievalControlPlaneReceipt(r:BrainThroughputRetrievalControlPlaneReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_RETRIEVAL_CONTROL_PLANE_FEATURE_ID||r.contract_version!==THROUGHPUT_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("throughput control plane schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.plane_id.trim()||!r.session_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!Number.isInteger(r.checkpoint_seq)||r.checkpoint_seq<=0||JSON.stringify(r.action_order)!==JSON.stringify(THROUGHPUT_RETRIEVAL_CONTROL_ACTION_ORDER)||!r.completed_action_order.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("throughput control-plane identity, queue, checkpoint, actions, retrieval, locality, budget, or effects are incomplete"); const actionPositions = new Map(THROUGHPUT_RETRIEVAL_CONTROL_ACTION_ORDER.map((action,index)=>[action,index])); for(const values of [r.completed_action_order,r.blocked_action_order]) { if(values.some(v=>!actionPositions.has(v))||values.some((v,i)=>i>0 && (actionPositions.get(values[i-1])??-1)>=(actionPositions.get(v)??-1))||r.completed_action_order.some(v=>r.blocked_action_order.includes(v))) throw new Error("throughput control-plane action transcript is not canonical"); } if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput control-plane evidence state is not covered by candidates"); for(const v of [r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput control-plane ordering is not canonical"); for(const v of [r.queue_digest,r.synthesis_digest,r.control_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput control-plane digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("manage:local-throughput-retrieval-control:")&&e!=="block:unsafe-release")) throw new Error("throughput control-plane effect is outside local management gate"); }
export function brainThroughputRetrievalControlPlaneReceiptDigest(r:BrainThroughputRetrievalControlPlaneReceipt):string { validateBrainThroughputRetrievalControlPlaneReceipt(r); return digestJsonSync(r); }

export const FEDERATED_RETRIEVAL_CONTROL_PLANE_FEATURE_ID = "AFA-brain-P02-F32" as const;
export const FEDERATED_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION = "brain-federated-retrieval-control-plane/1.0" as const;
export const FEDERATED_RETRIEVAL_CONTROL_ACTION_ORDER = ["control:observe", "control:reconcile", "control:authorize", "control:publish"] as const;
export interface BrainFederatedRetrievalControlPlaneReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; plane_id:string; session_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; action_order:string[]; completed_action_order:string[]; blocked_action_order:string[]; compensation_order:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; comparability_digest:string; envelope_digest:string; synthesis_digest:string; control_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalControlPlaneReceipt(r:BrainFederatedRetrievalControlPlaneReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_CONTROL_PLANE_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("federated control plane schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.plane_id.trim()||!r.session_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||JSON.stringify(r.action_order)!==JSON.stringify(FEDERATED_RETRIEVAL_CONTROL_ACTION_ORDER)||!r.completed_action_order.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("federated control-plane identity, closure, actions, retrieval, locality, budget, or effects are incomplete"); const actionPositions = new Map(FEDERATED_RETRIEVAL_CONTROL_ACTION_ORDER.map((action,index)=>[action,index])); for(const values of [r.completed_action_order,r.blocked_action_order]) { if(values.some(v=>!actionPositions.has(v))||values.some((v,i)=>i>0 && (actionPositions.get(values[i-1])??-1)>=(actionPositions.get(v)??-1))||r.completed_action_order.some(v=>r.blocked_action_order.includes(v))) throw new Error("federated control-plane action transcript is not canonical"); } if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated control-plane evidence state is not covered by candidates"); for(const v of [r.study_order,r.modality_order,r.compensation_order,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated control-plane ordering is not canonical"); for(const v of [r.comparability_digest,r.envelope_digest,r.synthesis_digest,r.control_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated control-plane digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("manage:local-federated-retrieval-control:")&&e!=="block:unsafe-release")) throw new Error("federated control-plane effect is outside local management gate"); }
export function brainFederatedRetrievalControlPlaneReceiptDigest(r:BrainFederatedRetrievalControlPlaneReceipt):string { validateBrainFederatedRetrievalControlPlaneReceipt(r); return digestJsonSync(r); }

export const CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F01" as const;
export const CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-research-context-compilation/1.0" as const;
export interface BrainResearchContextCompilationReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; scope:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; required_fact_order:string[]; resolved_fact_order:string[]; missing_fact_order:string[]; blocked_fact_order:string[]; unknown_fact_order:string[]; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainResearchContextCompilationReceipt(r:BrainResearchContextCompilationReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_COMPILATION_FEATURE_ID||r.contract_version!==CONTEXT_COMPILATION_CONTRACT_VERSION) throw new Error("context compilation schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.scope.trim()||!r.required_fact_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("context identity, boundary, disposition, required facts, locality, or effects are incomplete"); for(const v of [r.required_fact_order,r.resolved_fact_order,r.missing_fact_order,r.blocked_fact_order,r.unknown_fact_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("context vectors are not canonical"); const required=new Set(r.required_fact_order), classified=new Set([...r.resolved_fact_order,...r.missing_fact_order,...r.blocked_fact_order,...r.unknown_fact_order]); if(classified.size!==required.size||[...classified].some(v=>!required.has(v))) throw new Error("context fact states do not partition required facts"); for(const v of [r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("context digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-research-context:")&&e!=="block:unsafe-release")) throw new Error("context effect is outside local compilation gate"); }
export function brainResearchContextCompilationReceiptDigest(r:BrainResearchContextCompilationReceipt):string { validateBrainResearchContextCompilationReceipt(r); return digestJsonSync(r); }

export const MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F02" as const;
export const MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-multimodal-context-compilation/1.0" as const;
export interface BrainMultimodalContextCompilationReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; scope:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; required_fact_order:string[]; resolved_fact_order:string[]; missing_fact_order:string[]; blocked_fact_order:string[]; unknown_fact_order:string[]; comparability_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalContextCompilationReceipt(r:BrainMultimodalContextCompilationReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID||r.contract_version!==MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION) throw new Error("multimodal context compilation schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.scope.trim()||r.study_order.length<2||r.modality_order.length<2||!r.required_fact_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("multimodal context identity, closure, disposition, locality, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.required_fact_order,r.resolved_fact_order,r.missing_fact_order,r.blocked_fact_order,r.unknown_fact_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal context vectors are not canonical"); const required=new Set(r.required_fact_order), classified=[...r.resolved_fact_order,...r.missing_fact_order,...r.blocked_fact_order,...r.unknown_fact_order]; if(new Set(classified).size!==required.size||classified.some(v=>!required.has(v))) throw new Error("multimodal context fact states do not partition required facts"); for(const v of [r.comparability_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal context digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-multimodal-research-context:")&&e!=="block:unsafe-release")) throw new Error("multimodal context effect is outside local compilation gate"); }
export function brainMultimodalContextCompilationReceiptDigest(r:BrainMultimodalContextCompilationReceipt):string { validateBrainMultimodalContextCompilationReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F03" as const;
export const THROUGHPUT_CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-throughput-context-compilation/1.0" as const;
export interface BrainThroughputContextCompilationReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; objective:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; batch_order:string[]; accepted_order:string[]; deferred_order:string[]; blocked_order:string[]; unknown_order:string[]; queue_digest:string; throughput_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputContextCompilationReceipt(r:BrainThroughputContextCompilationReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_CONTEXT_COMPILATION_FEATURE_ID||r.contract_version!==THROUGHPUT_CONTEXT_COMPILATION_CONTRACT_VERSION) throw new Error("throughput context compilation schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.objective.trim()||!r.batch_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("throughput context identity, batch, locality, disposition, or effects are incomplete"); for(const v of [r.batch_order,r.accepted_order,r.deferred_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput context vectors are not canonical"); const batch=new Set(r.batch_order), classified=[...r.accepted_order,...r.deferred_order,...r.blocked_order,...r.unknown_order]; if(new Set(classified).size!==batch.size||classified.some(v=>!batch.has(v))) throw new Error("throughput context queue states do not partition the batch"); for(const v of [r.queue_digest,r.throughput_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput context digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-throughput-context:")&&e!=="block:unsafe-release")) throw new Error("throughput context effect is outside local compilation gate"); }
export function brainThroughputContextCompilationReceiptDigest(r:BrainThroughputContextCompilationReceipt):string { validateBrainThroughputContextCompilationReceipt(r); return digestJsonSync(r); }
export const FEDERATED_CONTEXT_COMPILATION_FEATURE_ID = "AFA-brain-P03-F04" as const;
export const FEDERATED_CONTEXT_COMPILATION_CONTRACT_VERSION = "brain-federated-context-compilation/1.0" as const;
export interface BrainFederatedContextCompilationReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; institution_id:string; purpose:string; semantic_profile:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; comparability_digest:string; envelope_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export function validateBrainFederatedContextCompilationReceipt(r:BrainFederatedContextCompilationReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_CONTEXT_COMPILATION_FEATURE_ID||r.contract_version!==FEDERATED_CONTEXT_COMPILATION_CONTRACT_VERSION) throw new Error("federated context compilation schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.aggregate_only||!r.request_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.semantic_profile.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!r.candidate_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("federated context identity, closure, aggregate-only locality, disposition, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated context vectors are not canonical"); const candidates=new Set(r.candidate_order), classified=[...r.qualified_order,...r.blocked_order,...r.unknown_order]; if(new Set(classified).size!==candidates.size||classified.some(v=>!candidates.has(v))||r.aggregate_order.some(v=>typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v))) throw new Error("federated context candidate states or aggregate order are invalid"); for(const v of [r.comparability_digest,r.envelope_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated context digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("manage:local-federated-context:")&&e!=="block:unsafe-release")) throw new Error("federated context effect is outside local management gate"); }
export function brainFederatedContextCompilationReceiptDigest(r:BrainFederatedContextCompilationReceipt):string { validateBrainFederatedContextCompilationReceipt(r); return digestJsonSync(r); }
export const CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID = "AFA-brain-P03-F05" as const;
export const CONTEXT_OMISSION_ADJUDICATION_CONTRACT_VERSION = "brain-context-omission-adjudication/1.0" as const;
export interface BrainContextOmissionAdjudicationReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; required_evidence_order:string[]; admitted_order:string[]; contested_order:string[]; missing_order:string[]; blocked_order:string[]; unknown_order:string[]; omission_certificate_order:string[]; adjudication_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextOmissionAdjudicationReceipt(r:BrainContextOmissionAdjudicationReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_OMISSION_ADJUDICATION_FEATURE_ID||r.contract_version!==CONTEXT_OMISSION_ADJUDICATION_CONTRACT_VERSION) throw new Error("omission adjudication schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.required_evidence_order.length||!r.omission_certificate_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("omission adjudication identity, evidence, certificates, locality, disposition, or effects are incomplete"); for(const v of [r.required_evidence_order,r.admitted_order,r.contested_order,r.missing_order,r.blocked_order,r.unknown_order,r.omission_certificate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("omission adjudication vectors are not canonical"); const required=new Set(r.required_evidence_order), classified=[...r.admitted_order,...r.contested_order,...r.missing_order,...r.blocked_order,...r.unknown_order]; if(new Set(classified).size!==required.size||classified.some(v=>!required.has(v))) throw new Error("omission adjudication states do not partition required evidence"); for(const v of [r.adjudication_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("omission adjudication digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-omission-adjudication:")&&e!=="block:unsafe-release")) throw new Error("omission adjudication effect is outside local compilation gate"); }
export function brainContextOmissionAdjudicationReceiptDigest(r:BrainContextOmissionAdjudicationReceipt):string { validateBrainContextOmissionAdjudicationReceipt(r); return digestJsonSync(r); }
export const CONTEXT_RELEASE_ADMISSION_FEATURE_ID = "AFA-brain-P03-F06" as const;
export const CONTEXT_RELEASE_ADMISSION_CONTRACT_VERSION = "brain-context-release-admission/1.0" as const;
export const CONTEXT_RELEASE_ACTION = "release:local-context" as const;
export interface BrainContextReleaseAdmissionReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; disposition:"admitted"|"blocked"|"approval_required"|"unresolved"; actor:string; action:string; context_digest:string; omission_certificate_digest:string; replay_identity:string; policy_decision:"allow"|"deny"|"redact"|"local_only"|"approval_required"|"unresolved"; policy_reasons:string[]; grant_scope:string; grant_expiry:string; remaining_units:number; release_digest:string; effect_receipts:string[]; artifact:Record<string,unknown>; boundary:string; }
export function validateBrainContextReleaseAdmissionReceipt(r:BrainContextReleaseAdmissionReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_RELEASE_ADMISSION_FEATURE_ID||r.contract_version!==CONTEXT_RELEASE_ADMISSION_CONTRACT_VERSION) throw new Error("context release admission schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.request_id.trim()||!r.actor.trim()||r.action!==CONTEXT_RELEASE_ACTION||!r.grant_scope.trim()||!r.grant_expiry.trim()||r.remaining_units<0||!r.policy_reasons.length||!r.effect_receipts.length||!new Set(["admitted","blocked","approval_required","unresolved"]).has(r.disposition)) throw new Error("context release identity, policy, grant, budget, disposition, or effects are incomplete"); for(const v of [r.context_digest,r.omission_certificate_digest,r.replay_identity,r.release_digest,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("context release digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("release:local-context:")&&e!=="block:unsafe-release")) throw new Error("context release effect is outside admission gate"); }
export function brainContextReleaseAdmissionReceiptDigest(r:BrainContextReleaseAdmissionReceipt):string { validateBrainContextReleaseAdmissionReceipt(r); return digestJsonSync(r); }
export const CONTEXT_FRESHNESS_DRIFT_FEATURE_ID = "AFA-brain-P03-F07" as const;
export const CONTEXT_FRESHNESS_DRIFT_CONTRACT_VERSION = "brain-context-freshness-drift/1.0" as const;
export interface BrainContextFreshnessDriftReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; disposition:"fresh"|"drifted"|"stale"|"unknown"|"blocked"; changed_dimension_order:string[]; freshness_age_seconds:number; baseline_digest:string; candidate_digest:string; drift_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextFreshnessDriftReceipt(r:BrainContextFreshnessDriftReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_FRESHNESS_DRIFT_FEATURE_ID||r.contract_version!==CONTEXT_FRESHNESS_DRIFT_CONTRACT_VERSION) throw new Error("freshness/drift schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.effect_receipts.length||!new Set(["fresh","drifted","stale","unknown","blocked"]).has(r.disposition)) throw new Error("freshness/drift identity, locality, disposition, or effects are incomplete"); for(const v of [r.changed_dimension_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("freshness/drift vectors are not canonical"); for(const v of [r.baseline_digest,r.candidate_digest,r.drift_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("freshness/drift digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("evaluate:local-context-freshness:")&&e!=="block:unsafe-release")) throw new Error("freshness/drift effect is outside local evaluation gate"); }
export function brainContextFreshnessDriftReceiptDigest(r:BrainContextFreshnessDriftReceipt):string { validateBrainContextFreshnessDriftReceipt(r); return digestJsonSync(r); }
export const CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID = "AFA-brain-P03-F08" as const;
export const CONTEXT_UNCERTAINTY_ENVELOPE_CONTRACT_VERSION = "brain-context-uncertainty-envelope/1.0" as const;
export interface BrainContextUncertaintyEnvelopeReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; required_evidence_order:string[]; qualified_order:string[]; uncertain_order:string[]; missing_order:string[]; blocked_order:string[]; interval_width_order:string[]; uncertainty_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextUncertaintyEnvelopeReceipt(r:BrainContextUncertaintyEnvelopeReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_UNCERTAINTY_ENVELOPE_FEATURE_ID||r.contract_version!==CONTEXT_UNCERTAINTY_ENVELOPE_CONTRACT_VERSION) throw new Error("uncertainty envelope schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.required_evidence_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("uncertainty envelope identity, evidence, locality, disposition, or effects are incomplete"); for(const v of [r.required_evidence_order,r.qualified_order,r.uncertain_order,r.missing_order,r.blocked_order,r.interval_width_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("uncertainty envelope vectors are not canonical"); const required=new Set(r.required_evidence_order), classified=new Set([...r.qualified_order,...r.uncertain_order,...r.missing_order,...r.blocked_order]); if(classified.size!==required.size||[...classified].some(v=>!required.has(v))) throw new Error("uncertainty envelope states do not partition required evidence"); for(const v of [r.uncertainty_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("uncertainty envelope digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-uncertainty-envelope:")&&e!=="block:unsafe-release")) throw new Error("uncertainty envelope effect is outside local compilation gate"); }
export function brainContextUncertaintyEnvelopeReceiptDigest(r:BrainContextUncertaintyEnvelopeReceipt):string { validateBrainContextUncertaintyEnvelopeReceipt(r); return digestJsonSync(r); }
export const CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID = "AFA-brain-P03-F09" as const;
export const CONTEXT_CONTRADICTION_RESOLUTION_CONTRACT_VERSION = "brain-context-contradiction-resolution/1.0" as const;
export interface BrainContextContradictionResolutionReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; group_order:string[]; resolved_group_order:string[]; contested_group_order:string[]; missing_group_order:string[]; blocked_group_order:string[]; unknown_group_order:string[]; resolution_plan_order:string[]; conflict_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextContradictionResolutionReceipt(r:BrainContextContradictionResolutionReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID||r.contract_version!==CONTEXT_CONTRADICTION_RESOLUTION_CONTRACT_VERSION) throw new Error("contradiction-resolution schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.group_order.length||!r.resolution_plan_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("contradiction-resolution identity, groups, plan, locality, disposition, or effects are incomplete"); for(const v of [r.group_order,r.resolved_group_order,r.contested_group_order,r.missing_group_order,r.blocked_group_order,r.unknown_group_order,r.resolution_plan_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("contradiction-resolution vectors are not canonical"); const groups=new Set(r.group_order), classified=new Set([...r.resolved_group_order,...r.contested_group_order,...r.missing_group_order,...r.blocked_group_order,...r.unknown_group_order]); if(classified.size!==groups.size||[...classified].some(v=>!groups.has(v))) throw new Error("contradiction-resolution group states do not partition groups"); for(const v of [r.conflict_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("contradiction-resolution digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-contradiction-resolution:")&&e!=="block:unsafe-release")) throw new Error("contradiction-resolution effect is outside local compilation gate"); }
export function brainContextContradictionResolutionReceiptDigest(r:BrainContextContradictionResolutionReceipt):string { validateBrainContextContradictionResolutionReceipt(r); return digestJsonSync(r); }
export const CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID = "AFA-brain-P03-F10" as const;
export const CONTEXT_DEPENDENCY_CLOSURE_CONTRACT_VERSION = "brain-context-dependency-closure/1.0" as const;
export interface BrainContextDependencyClosureReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; objective:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; context_order:string[]; resolved_order:string[]; missing_dependency_order:string[]; cycle_order:string[]; blocked_order:string[]; dependency_order:string[]; closure_digest:string; context_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextDependencyClosureReceipt(r:BrainContextDependencyClosureReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID||r.contract_version!==CONTEXT_DEPENDENCY_CLOSURE_CONTRACT_VERSION) throw new Error("dependency closure schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.objective.trim()||!r.context_order.length||!r.effect_receipts.length||!new Set(["qualified","partial","unknown","blocked"]).has(r.disposition)) throw new Error("dependency closure identity, graph, locality, disposition, or effects are incomplete"); for(const v of [r.context_order,r.resolved_order,r.missing_dependency_order,r.cycle_order,r.blocked_order,r.dependency_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("dependency closure vectors are not canonical"); const contexts=new Set(r.context_order), classified=new Set([...r.resolved_order,...r.missing_dependency_order,...r.cycle_order,...r.blocked_order]); if(classified.size!==contexts.size||[...classified].some(v=>!contexts.has(v))) throw new Error("dependency closure context states do not partition contexts"); for(const v of [r.closure_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("dependency closure digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("compile:local-dependency-closure:")&&e!=="block:unsafe-release")) throw new Error("dependency closure effect is outside local compilation gate"); }
export function brainContextDependencyClosureReceiptDigest(r:BrainContextDependencyClosureReceipt):string { validateBrainContextDependencyClosureReceipt(r); return digestJsonSync(r); }
export const CONTEXT_DECISION_PROJECTION_FEATURE_ID = "AFA-brain-P03-F11" as const;
export const CONTEXT_DECISION_PROJECTION_CONTRACT_VERSION = "brain-context-decision-projection/1.0" as const;
export interface BrainContextDecisionProjectionReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; query_id:string; goal:string; disposition:"admitted"|"refinement_required"|"blocked"; selected_order:string[]; dependency_order:string[]; unresolved_obligation_order:string[]; refinement_frontier_order:string[]; context_digest:string; section_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextDecisionProjectionReceipt(r:BrainContextDecisionProjectionReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_DECISION_PROJECTION_FEATURE_ID||r.contract_version!==CONTEXT_DECISION_PROJECTION_CONTRACT_VERSION) throw new Error("decision projection schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.selected_order.length||!r.refinement_frontier_order.length||!r.effect_receipts.length||!new Set(["admitted","refinement_required","blocked"]).has(r.disposition)) throw new Error("decision projection identity, obligations, frontier, locality, disposition, or effects are incomplete"); if(r.disposition!=="admitted"&&!r.unresolved_obligation_order.length) throw new Error("non-admitted projection must retain an unresolved obligation"); for(const v of [r.selected_order,r.dependency_order,r.unresolved_obligation_order,r.refinement_frontier_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("decision projection vectors are not canonical"); for(const v of [r.context_digest,r.section_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("decision projection digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("project:local-decision-section:")&&e!=="block:unsafe-release")) throw new Error("decision projection effect is outside local projection gate"); }
export function brainContextDecisionProjectionReceiptDigest(r:BrainContextDecisionProjectionReceipt):string { validateBrainContextDecisionProjectionReceipt(r); return digestJsonSync(r); }
export const FEDERATED_DECISION_PROJECTION_FEATURE_ID = "AFA-brain-P03-F12" as const;
export const FEDERATED_DECISION_PROJECTION_CONTRACT_VERSION = "brain-federated-decision-projection/1.0" as const;
export interface PeerDecisionAttestation { institution_id:string; epoch:number; context_digest:string; section_digest:string; evidence_state:"proven"|"supported"|"speculative"|"contradicted"|"unknown"; replay_identity:string; policy_allow:boolean; protected_closure:boolean; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export interface BrainFederatedDecisionProjectionReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; query_id:string; goal:string; semantic_profile:string; disposition:"admitted"|"refinement_required"|"blocked"; institution_order:string[]; qualified_institution_order:string[]; stale_institution_order:string[]; blocked_institution_order:string[]; unknown_institution_order:string[]; aggregate_order:string[]; quorum:number; minimum_quorum:number; current_epoch:number; context_digest:string; section_digest:string; federation_envelope_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export function validateBrainFederatedDecisionProjectionReceipt(r:BrainFederatedDecisionProjectionReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_DECISION_PROJECTION_FEATURE_ID||r.contract_version!==FEDERATED_DECISION_PROJECTION_CONTRACT_VERSION) throw new Error("federated projection schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.aggregate_only||!r.request_id.trim()||!r.federation_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.semantic_profile.trim()||r.institution_order.length<2||!r.aggregate_order.length||!r.effect_receipts.length||!r.disposition) throw new Error("federated projection identity, quorum, locality, aggregate-only, or effects are incomplete"); if(!Number.isInteger(r.minimum_quorum)||r.minimum_quorum<1||r.quorum!==r.qualified_institution_order.length||r.quorum>r.institution_order.length) throw new Error("federated quorum is invalid"); for(const v of [r.institution_order,r.qualified_institution_order,r.stale_institution_order,r.blocked_institution_order,r.unknown_institution_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated projection vectors are not canonical"); const classified=new Set([...r.qualified_institution_order,...r.stale_institution_order,...r.blocked_institution_order,...r.unknown_institution_order]); if(classified.size!==r.institution_order.length||[...classified].some(v=>!r.institution_order.includes(v))) throw new Error("federated peer states do not partition institutions"); for(const v of [r.context_digest,r.section_digest,r.federation_envelope_digest,r.replay_identity,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated projection digest is invalid"); if(typeof r.artifact.content_hash!=="string"||!/^[0-9a-f]{64}$/.test(r.artifact.content_hash)) throw new Error("federated projection artifact digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("project:federated-decision-section:")&&e!=="block:unsafe-release")) throw new Error("federated projection effect is outside release gate"); }
export function brainFederatedDecisionProjectionReceiptDigest(r:BrainFederatedDecisionProjectionReceipt):string { validateBrainFederatedDecisionProjectionReceipt(r); return digestJsonSync(r); }
export const CONTEXT_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P03-F13" as const;
export const CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-context-workflow-fabric/1.0" as const;
export interface BrainContextWorkflowReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workflow_id:string; query_id:string; goal:string; disposition:"admitted"|"refinement_required"|"blocked"; stage_order:string[]; plan_order:string[]; completed_order:string[]; blocked_order:string[]; compensation_order:string[]; checkpoint_digest:string; workflow_digest:string; context_digest:string; replay_identity:string; budget_units:number; consumed_budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextWorkflowReceipt(r:BrainContextWorkflowReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workflow_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.stage_order.length||!r.plan_order.length||r.budget_units<1||r.consumed_budget_units>r.budget_units||!r.effect_receipts.length) throw new Error("workflow identity, stage plan, budget, locality, or effects are incomplete"); if(new Set(r.stage_order).size!==r.stage_order.length||[...r.completed_order,...r.blocked_order].some(v=>!r.stage_order.includes(v))||r.completed_order.some(v=>r.blocked_order.includes(v))) throw new Error("workflow stage coverage is invalid"); for(const v of [r.plan_order,r.blocked_order,r.compensation_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("workflow vectors are not canonical"); if(new Set(r.completed_order).size!==r.completed_order.length) throw new Error("workflow completed order contains duplicates"); for(const v of [r.checkpoint_digest,r.workflow_digest,r.context_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:context-workflow:")&&!e.startsWith("compensate:context-workflow:")&&e!=="block:unsafe-release")) throw new Error("workflow effect is outside schedule/compensation gate"); }
export function brainContextWorkflowReceiptDigest(r:BrainContextWorkflowReceipt):string { validateBrainContextWorkflowReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P03-F14" as const;
export const MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-multimodal-context-workflow-fabric/1.0" as const;
export interface ModalContextInput { study_id:string; modality:string; artifact_digest:string; semantic_digest:string; replay_identity:string; state:"proven"|"supported"|"speculative"|"contradicted"|"unknown"; comparable:boolean; raw_data_local:boolean; boundary:string; }
export interface BrainMultimodalContextWorkflowReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workflow_id:string; query_id:string; goal:string; disposition:"admitted"|"refinement_required"|"blocked"; study_order:string[]; modality_order:string[]; cell_order:string[]; accepted_order:string[]; missing_order:string[]; incompatible_order:string[]; unknown_order:string[]; plan_order:string[]; checkpoint_digest:string; workflow_digest:string; replay_identity:string; budget_units:number; consumed_budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalContextWorkflowReceipt(r:BrainMultimodalContextWorkflowReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==MULTIMODAL_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("multimodal workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workflow_id.trim()||!r.query_id.trim()||!r.goal.trim()||r.study_order.length<2||r.modality_order.length<2||!r.cell_order.length||!r.plan_order.length||r.budget_units<1||r.consumed_budget_units>r.budget_units||!r.effect_receipts.length) throw new Error("multimodal workflow identity, closure, budget, locality, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.cell_order,r.accepted_order,r.missing_order,r.incompatible_order,r.unknown_order,r.plan_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal workflow vectors are not canonical"); const classified=new Set([...r.accepted_order,...r.missing_order,...r.incompatible_order,...r.unknown_order]); if(classified.size!==r.cell_order.length||[...classified].some(v=>!r.cell_order.includes(v))) throw new Error("multimodal cells do not partition outcomes"); for(const v of [r.checkpoint_digest,r.workflow_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:multimodal-context-workflow:")&&!e.startsWith("compensate:multimodal-context-workflow:")&&e!=="block:unsafe-release")) throw new Error("multimodal workflow effect is outside schedule/compensation gate"); }
export function brainMultimodalContextWorkflowReceiptDigest(r:BrainMultimodalContextWorkflowReceipt):string { validateBrainMultimodalContextWorkflowReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P03-F15" as const;
export const THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-throughput-context-workflow-fabric/1.0" as const;
export interface BrainThroughputContextWorkflowReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; query_id:string; goal:string; disposition:"admitted"|"refinement_required"|"blocked"; queue_order:string[]; scheduled_order:string[]; blocked_order:string[]; unknown_order:string[]; concurrency:number; budget_units:number; consumed_budget_units:number; batch_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputContextWorkflowReceipt(r:BrainThroughputContextWorkflowReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("throughput workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.queue_order.length||r.concurrency<1||r.budget_units<1||r.consumed_budget_units>r.budget_units||!r.effect_receipts.length) throw new Error("throughput workflow identity, queue, concurrency, budget, locality, or effects are incomplete"); for(const v of [r.queue_order,r.scheduled_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput workflow vectors are not canonical"); const classified=new Set([...r.scheduled_order,...r.blocked_order,...r.unknown_order]); if(classified.size!==r.queue_order.length||[...classified].some(v=>!r.queue_order.includes(v))) throw new Error("throughput jobs do not partition outcomes"); for(const v of [r.batch_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:throughput-context-workflow:")&&e!=="block:unsafe-release")) throw new Error("throughput workflow effect is outside batch gate"); }
export function brainThroughputContextWorkflowReceiptDigest(r:BrainThroughputContextWorkflowReceipt):string { validateBrainThroughputContextWorkflowReceipt(r); return digestJsonSync(r); }
export const FEDERATED_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID = "AFA-brain-P03-F16" as const;
export const FEDERATED_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION = "brain-federated-context-workflow-fabric/1.0" as const;
export interface BrainFederatedContextWorkflowReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; workflow_id:string; query_id:string; goal:string; semantic_profile:string; disposition:"admitted"|"refinement_required"|"blocked"; institution_order:string[]; qualified_institution_order:string[]; stale_institution_order:string[]; blocked_institution_order:string[]; unknown_institution_order:string[]; required_stage_order:string[]; scheduled_stage_order:string[]; aggregate_order:string[]; quorum:number; minimum_quorum:number; current_epoch:number; budget_units:number; consumed_budget_units:number; checkpoint_digest:string; workflow_digest:string; federation_envelope_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export function validateBrainFederatedContextWorkflowReceipt(r:BrainFederatedContextWorkflowReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID||r.contract_version!==FEDERATED_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("federated workflow schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.aggregate_only||!r.request_id.trim()||!r.federation_id.trim()||!r.workflow_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.semantic_profile.trim()||r.institution_order.length<2||!r.required_stage_order.length||!r.scheduled_stage_order.length||!r.aggregate_order.length||r.minimum_quorum<1||r.quorum!==r.qualified_institution_order.length||r.quorum>r.institution_order.length||r.budget_units<1||r.consumed_budget_units>r.budget_units||!r.effect_receipts.length) throw new Error("federated workflow identity, stage closure, quorum, budget, locality, or effects are incomplete"); for(const v of [r.institution_order,r.qualified_institution_order,r.stale_institution_order,r.blocked_institution_order,r.unknown_institution_order,r.required_stage_order,r.scheduled_stage_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated workflow vectors are not canonical"); if(r.scheduled_stage_order.some(v=>!r.required_stage_order.includes(v))) throw new Error("scheduled stages must be required stages"); const classified=new Set([...r.qualified_institution_order,...r.stale_institution_order,...r.blocked_institution_order,...r.unknown_institution_order]); if(classified.size!==r.institution_order.length||[...classified].some(v=>!r.institution_order.includes(v))) throw new Error("federated peer states do not partition institutions"); for(const v of [...r.aggregate_order,r.checkpoint_digest,r.workflow_digest,r.federation_envelope_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated workflow digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("schedule:federated-context-workflow:")&&e!=="block:unsafe-release")) throw new Error("federated workflow effect is outside schedule gate"); }
export function brainFederatedContextWorkflowReceiptDigest(r:BrainFederatedContextWorkflowReceipt):string { validateBrainFederatedContextWorkflowReceipt(r); return digestJsonSync(r); }
export const CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID = "AFA-brain-P03-F17" as const;
export const CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION = "brain-context-research-workbench/1.0" as const;
export interface BrainContextWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; session_id:string; query_id:string; goal:string; disposition:"ready"|"needs_refinement"|"blocked"; view_order:string[]; action_order:string[]; blocked_action_order:string[]; selected_context_order:string[]; unresolved_obligation_order:string[]; refinement_frontier_order:string[]; context_digest:string; section_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextWorkbenchReceipt(r:BrainContextWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID||r.contract_version!==CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.session_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.view_order.length||!r.action_order.length||!r.effect_receipts.length||!new Set(["ready","needs_refinement","blocked"]).has(r.disposition)) throw new Error("workbench identity, view, action, locality, disposition, or effects are incomplete"); for(const v of [r.view_order,r.action_order,r.blocked_action_order,r.selected_context_order,r.unresolved_obligation_order,r.refinement_frontier_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("workbench vectors are not canonical"); for(const v of [r.context_digest,r.section_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-context-workbench:")&&e!=="block:unsafe-release")) throw new Error("workbench effect is outside read-only view gate"); }
export function brainContextWorkbenchReceiptDigest(r:BrainContextWorkbenchReceipt):string { validateBrainContextWorkbenchReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_CONTEXT_WORKBENCH_FEATURE_ID = "AFA-brain-P03-F18" as const;
export const MULTIMODAL_CONTEXT_WORKBENCH_CONTRACT_VERSION = "brain-multimodal-context-workbench/1.0" as const;
export interface BrainMultimodalContextWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; session_id:string; query_id:string; goal:string; disposition:"ready"|"needs_refinement"|"blocked"; study_order:string[]; modality_order:string[]; cell_order:string[]; qualified_cell_order:string[]; missing_cell_order:string[]; incompatible_cell_order:string[]; unknown_cell_order:string[]; view_order:string[]; action_order:string[]; blocked_action_order:string[]; context_digest:string; section_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalContextWorkbenchReceipt(r:BrainMultimodalContextWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_CONTEXT_WORKBENCH_FEATURE_ID||r.contract_version!==MULTIMODAL_CONTEXT_WORKBENCH_CONTRACT_VERSION) throw new Error("multimodal workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.session_id.trim()||!r.query_id.trim()||!r.goal.trim()||r.study_order.length<2||r.modality_order.length<2||!r.cell_order.length||!r.view_order.length||!r.action_order.length||!r.effect_receipts.length||!new Set(["ready","needs_refinement","blocked"]).has(r.disposition)) throw new Error("multimodal workbench identity, cell closure, view, action, locality, disposition, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.cell_order,r.qualified_cell_order,r.missing_cell_order,r.incompatible_cell_order,r.unknown_cell_order,r.view_order,r.action_order,r.blocked_action_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal workbench vectors are not canonical"); const classified=new Set([...r.qualified_cell_order,...r.missing_cell_order,...r.incompatible_cell_order,...r.unknown_cell_order]); if(classified.size!==r.cell_order.length||[...classified].some(v=>!r.cell_order.includes(v))) throw new Error("multimodal cells do not partition outcomes"); for(const v of [r.context_digest,r.section_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-multimodal-workbench:")&&e!=="block:unsafe-release")) throw new Error("multimodal workbench effect is outside read-only view gate"); }
export function brainMultimodalContextWorkbenchReceiptDigest(r:BrainMultimodalContextWorkbenchReceipt):string { validateBrainMultimodalContextWorkbenchReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_CONTEXT_WORKBENCH_FEATURE_ID = "AFA-brain-P03-F19" as const;
export const THROUGHPUT_CONTEXT_WORKBENCH_CONTRACT_VERSION = "brain-throughput-context-workbench/1.0" as const;
export interface BrainThroughputContextWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; session_id:string; query_id:string; goal:string; disposition:"ready"|"needs_refinement"|"blocked"; queue_order:string[]; admitted_job_order:string[]; blocked_job_order:string[]; unknown_job_order:string[]; view_order:string[]; action_order:string[]; blocked_action_order:string[]; concurrency:number; budget_units:number; consumed_budget_units:number; batch_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputContextWorkbenchReceipt(r:BrainThroughputContextWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_CONTEXT_WORKBENCH_FEATURE_ID||r.contract_version!==THROUGHPUT_CONTEXT_WORKBENCH_CONTRACT_VERSION) throw new Error("throughput workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.session_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.queue_order.length||!r.view_order.length||!r.action_order.length||!r.effect_receipts.length||r.concurrency<=0||r.budget_units<=0||r.consumed_budget_units>r.budget_units||!new Set(["ready","needs_refinement","blocked"]).has(r.disposition)) throw new Error("throughput workbench identity, queue, budget, concurrency, view, action, locality, or disposition is incomplete"); for(const v of [r.queue_order,r.admitted_job_order,r.blocked_job_order,r.unknown_job_order,r.view_order,r.action_order,r.blocked_action_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput workbench vectors are not canonical"); const classified=new Set([...r.admitted_job_order,...r.blocked_job_order,...r.unknown_job_order]); if(classified.size!==r.queue_order.length||[...classified].some(v=>!r.queue_order.includes(v))) throw new Error("throughput jobs do not partition outcomes"); for(const v of [r.batch_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-throughput-workbench:")&&e!=="block:unsafe-release")) throw new Error("throughput workbench effect is outside read-only view gate"); }
export function brainThroughputContextWorkbenchReceiptDigest(r:BrainThroughputContextWorkbenchReceipt):string { validateBrainThroughputContextWorkbenchReceipt(r); return digestJsonSync(r); }
export const FEDERATED_CONTEXT_WORKBENCH_FEATURE_ID = "AFA-brain-P03-F20" as const;
export const FEDERATED_CONTEXT_WORKBENCH_CONTRACT_VERSION = "brain-federated-context-research-workbench/1.0" as const;
export interface BrainFederatedContextWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; session_id:string; federation_id:string; query_id:string; goal:string; semantic_profile:string; disposition:"ready"|"needs_refinement"|"blocked"; institution_order:string[]; qualified_institution_order:string[]; stale_institution_order:string[]; blocked_institution_order:string[]; unknown_institution_order:string[]; view_order:string[]; action_order:string[]; blocked_action_order:string[]; aggregate_order:string[]; quorum:number; minimum_quorum:number; current_epoch:number; budget_units:number; consumed_budget_units:number; checkpoint_digest:string; federation_envelope_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export function validateBrainFederatedContextWorkbenchReceipt(r:BrainFederatedContextWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_CONTEXT_WORKBENCH_FEATURE_ID||r.contract_version!==FEDERATED_CONTEXT_WORKBENCH_CONTRACT_VERSION) throw new Error("federated workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.aggregate_only||!r.session_id.trim()||!r.federation_id.trim()||!r.query_id.trim()||!r.goal.trim()||!r.semantic_profile.trim()||r.institution_order.length<2||!r.view_order.length||!r.action_order.length||r.minimum_quorum<1||r.quorum!==r.qualified_institution_order.length||r.quorum>r.institution_order.length||r.budget_units<1||r.consumed_budget_units>r.budget_units||!r.effect_receipts.length||!new Set(["ready","needs_refinement","blocked"]).has(r.disposition)) throw new Error("federated workbench identity, quorum, budget, locality, view, action, or disposition is incomplete"); for(const v of [r.institution_order,r.qualified_institution_order,r.stale_institution_order,r.blocked_institution_order,r.unknown_institution_order,r.view_order,r.action_order,r.blocked_action_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated workbench vectors are not canonical"); const classified=new Set([...r.qualified_institution_order,...r.stale_institution_order,...r.blocked_institution_order,...r.unknown_institution_order]); if(classified.size!==r.institution_order.length||[...classified].some(v=>!r.institution_order.includes(v))) throw new Error("federated peer states do not partition institutions"); for(const v of [...r.aggregate_order,r.checkpoint_digest,r.federation_envelope_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-federated-context-workbench:")&&e!=="block:unsafe-release")) throw new Error("federated workbench effect is outside read-only view gate"); }
export function brainFederatedContextWorkbenchReceiptDigest(r:BrainFederatedContextWorkbenchReceipt):string { validateBrainFederatedContextWorkbenchReceipt(r); return digestJsonSync(r); }
export const CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P03-F21" as const;
export const CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-context-protocol-adapter/1.0" as const;
export const CONTEXT_PROTOCOL_VERSION = "aurora-research-context/1.0" as const;
export const CONTEXT_PROTOCOL_ROUTE = "/v1/research/context/compile" as const;
export const CONTEXT_PROTOCOL_METHOD = "POST" as const;
export const CONTEXT_PROTOCOL_RESPONSE_SCHEMA = "ContextProtocolResponse1@1" as const;
export interface BrainContextProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; query_id:string; study_id:string; scope:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"ready"|"partial"|"unknown"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; context_digest:string; section_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextProtocolReceipt(r:BrainContextProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("context protocol schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.query_id.trim()||!r.study_id.trim()||!r.scope.trim()||r.protocol_version!==CONTEXT_PROTOCOL_VERSION||r.method!==CONTEXT_PROTOCOL_METHOD||r.route!==CONTEXT_PROTOCOL_ROUTE||r.content_type!=="application/json"||!r.idempotency_key.trim()||r.response_schema!==CONTEXT_PROTOCOL_RESPONSE_SCHEMA||!r.candidate_order.length||!r.effect_receipts.length||![200,202,206,403,422].includes(r.status_code)||!new Set(["ready","partial","unknown","blocked"]).has(r.disposition)) throw new Error("context protocol identity, route, idempotency, candidates, locality, or effects are incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("context protocol state is not covered by candidates"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("context protocol ordering is invalid"); for(const v of [r.context_digest,r.section_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("context protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:local-context-response:")&&e!=="block:unsafe-release")) throw new Error("context protocol effect is outside local response gate"); }
export function brainContextProtocolReceiptDigest(r:BrainContextProtocolReceipt):string { validateBrainContextProtocolReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P03-F22" as const;
export const MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-multimodal-context-protocol-adapter/1.0" as const;
export const MULTIMODAL_CONTEXT_PROTOCOL_VERSION = "aurora-research-context-multimodal/1.0" as const;
export const MULTIMODAL_CONTEXT_PROTOCOL_ROUTE = "/v1/research/context/multimodal/compile" as const;
export const MULTIMODAL_CONTEXT_PROTOCOL_METHOD = "POST" as const;
export const MULTIMODAL_CONTEXT_PROTOCOL_RESPONSE_SCHEMA = "MultimodalContextProtocolResponse1@1" as const;
export interface BrainMultimodalContextProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; query_id:string; scope:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"ready"|"partial"|"unknown"|"blocked"; study_order:string[]; modality_order:string[]; cell_order:string[]; qualified_order:string[]; missing_order:string[]; incompatible_order:string[]; unknown_order:string[]; context_digest:string; section_digest:string; comparability_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalContextProtocolReceipt(r:BrainMultimodalContextProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==MULTIMODAL_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("multimodal context protocol schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.query_id.trim()||!r.scope.trim()||r.protocol_version!==MULTIMODAL_CONTEXT_PROTOCOL_VERSION||r.method!==MULTIMODAL_CONTEXT_PROTOCOL_METHOD||r.route!==MULTIMODAL_CONTEXT_PROTOCOL_ROUTE||r.content_type!=="application/json"||!r.idempotency_key.trim()||r.response_schema!==MULTIMODAL_CONTEXT_PROTOCOL_RESPONSE_SCHEMA||r.study_order.length<2||r.modality_order.length<2||!r.cell_order.length||!r.effect_receipts.length||![200,202,206,403,422].includes(r.status_code)||!new Set(["ready","partial","unknown","blocked"]).has(r.disposition)) throw new Error("multimodal context protocol identity, route, coverage, idempotency, locality, or effects are incomplete"); if([...r.qualified_order,...r.missing_order,...r.incompatible_order,...r.unknown_order].some(v=>!r.cell_order.includes(v))) throw new Error("multimodal protocol state is not covered by cells"); for(const v of [r.study_order,r.modality_order,r.cell_order,r.qualified_order,r.missing_order,r.incompatible_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal protocol ordering is not canonical"); for(const v of [r.context_digest,r.section_digest,r.comparability_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:local-multimodal-context-response:")&&e!=="block:unsafe-release")) throw new Error("multimodal protocol effect is outside local response gate"); }
export function brainMultimodalContextProtocolReceiptDigest(r:BrainMultimodalContextProtocolReceipt):string { validateBrainMultimodalContextProtocolReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P03-F23" as const;
export const THROUGHPUT_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-throughput-context-protocol-adapter/1.0" as const;
export const THROUGHPUT_CONTEXT_PROTOCOL_VERSION = "aurora-research-context-throughput/1.0" as const;
export const THROUGHPUT_CONTEXT_PROTOCOL_ROUTE = "/v1/research/context/throughput/compile" as const;
export const THROUGHPUT_CONTEXT_PROTOCOL_METHOD = "POST" as const;
export const THROUGHPUT_CONTEXT_PROTOCOL_RESPONSE_SCHEMA = "ThroughputContextProtocolResponse1@1" as const;
export interface BrainThroughputContextProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"ready"|"partial"|"unknown"|"blocked"; batch_id:string; partition:string; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; queue_digest:string; context_digest:string; section_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputContextProtocolReceipt(r:BrainThroughputContextProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==THROUGHPUT_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("throughput context protocol schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||r.protocol_version!==THROUGHPUT_CONTEXT_PROTOCOL_VERSION||r.method!==THROUGHPUT_CONTEXT_PROTOCOL_METHOD||r.route!==THROUGHPUT_CONTEXT_PROTOCOL_ROUTE||r.content_type!=="application/json"||!r.idempotency_key.trim()||r.response_schema!==THROUGHPUT_CONTEXT_PROTOCOL_RESPONSE_SCHEMA||!r.batch_id.trim()||!r.partition.trim()||!r.candidate_order.length||!r.effect_receipts.length||![200,202,206,403,422].includes(r.status_code)||!new Set(["ready","partial","unknown","blocked"]).has(r.disposition)) throw new Error("throughput context protocol identity, route, queue, idempotency, locality, or effects are incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput context protocol state is not covered by candidates"); for(const v of [r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput context protocol ordering is not canonical"); for(const v of [r.queue_digest,r.context_digest,r.section_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput context protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:local-throughput-context-response:")&&e!=="block:unsafe-release")) throw new Error("throughput context protocol effect is outside local response gate"); }
export function brainThroughputContextProtocolReceiptDigest(r:BrainThroughputContextProtocolReceipt):string { validateBrainThroughputContextProtocolReceipt(r); return digestJsonSync(r); }
export const FEDERATED_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID = "AFA-brain-P03-F24" as const;
export const FEDERATED_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION = "brain-federated-context-protocol-adapter/1.0" as const;
export const FEDERATED_CONTEXT_PROTOCOL_VERSION = "aurora-research-context-federated/1.0" as const;
export const FEDERATED_CONTEXT_PROTOCOL_ROUTE = "/v1/research/context/federated/compile" as const;
export const FEDERATED_CONTEXT_PROTOCOL_METHOD = "POST" as const;
export const FEDERATED_CONTEXT_PROTOCOL_RESPONSE_SCHEMA = "FederatedContextProtocolResponse1@1" as const;
export interface BrainFederatedContextProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; purpose:string; scope:string; goal:string; semantic_profile:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"ready"|"partial"|"unknown"|"blocked"; institution_order:string[]; endpoint_order:string[]; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; minimum_quorum:number; quorum:number; checkpoint_seq:number; envelope_digest:string; context_digest:string; section_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export function validateBrainFederatedContextProtocolReceipt(r:BrainFederatedContextProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_CONTEXT_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==FEDERATED_CONTEXT_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("federated context protocol schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.request_id.trim()||!r.federation_id.trim()||!r.purpose.trim()||!r.scope.trim()||!r.goal.trim()||!r.semantic_profile.trim()||r.protocol_version!==FEDERATED_CONTEXT_PROTOCOL_VERSION||r.method!==FEDERATED_CONTEXT_PROTOCOL_METHOD||r.route!==FEDERATED_CONTEXT_PROTOCOL_ROUTE||r.content_type!=="application/json"||!r.idempotency_key.trim()||r.response_schema!==FEDERATED_CONTEXT_PROTOCOL_RESPONSE_SCHEMA||r.institution_order.length<2||!r.candidate_order.length||r.minimum_quorum<1||r.quorum!==r.admitted_order.length||r.quorum>r.candidate_order.length||!r.effect_receipts.length) throw new Error("federated context protocol identity, quorum, locality, or effects are incomplete"); for(const v of [r.institution_order,r.endpoint_order,r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated context protocol ordering is not canonical"); const classified=new Set([...r.admitted_order,...r.blocked_order,...r.unknown_order]); if(classified.size!==r.candidate_order.length||[...classified].some(v=>!r.candidate_order.includes(v))) throw new Error("federated context protocol states do not partition candidates"); if(![200,202,206,403,422].includes(r.status_code)||!new Set(["ready","partial","unknown","blocked"]).has(r.disposition)) throw new Error("federated context protocol status is invalid"); for(const v of [r.envelope_digest,r.context_digest,r.section_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated context protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:federated-context-response:")&&e!=="block:unsafe-release")) throw new Error("federated context protocol effect is outside the governed response gate"); if(!r.raw_data_local&&(r.disposition!=="blocked"||!r.omissions.includes("protocol:raw-data-locality-failed"))) throw new Error("raw-data locality failure must remain blocked and explicit"); }
export function brainFederatedContextProtocolReceiptDigest(r:BrainFederatedContextProtocolReceipt):string { validateBrainFederatedContextProtocolReceipt(r); return digestJsonSync(r); }
export const CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-brain-P03-F25" as const;
export const CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "brain-context-compilation-assurance/1.0" as const;
export interface BrainContextCompilationAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; study_id:string; scope:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; witness_order:string[]; counterexample_order:string[]; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainContextCompilationAssuranceReceipt(r:BrainContextCompilationAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID||r.contract_version!==CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION) throw new Error("context assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.study_id.trim()||!r.scope.trim()||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)) throw new Error("context assurance identity, witnesses, locality, or effects are incomplete"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("context assurance ordering is not canonical"); const classified=new Set([...r.qualified_order,...r.blocked_order,...r.unknown_order]); if(classified.size!==r.candidate_order.length||[...classified].some(v=>!r.candidate_order.includes(v))) throw new Error("context assurance outcomes do not partition candidates"); for(const v of [r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("context assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-context-compilation:")&&e!=="block:unsafe-release")) throw new Error("context assurance effect is outside the local release gate"); }
export function brainContextCompilationAssuranceReceiptDigest(r:BrainContextCompilationAssuranceReceipt):string { validateBrainContextCompilationAssuranceReceipt(r); return digestJsonSync(r); }
export const MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-brain-P03-F26" as const;
export const MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "brain-multimodal-context-compilation-assurance/1.0" as const;
export interface BrainMultimodalContextCompilationAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; scope:string; verdict:"qualified"|"unresolved"|"blocked"; study_order:string[]; modality_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; missing_order:string[]; incomparable_order:string[]; witness_order:string[]; counterexample_order:string[]; verification_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalContextCompilationAssuranceReceipt(r:BrainMultimodalContextCompilationAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID||r.contract_version!==MULTIMODAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION) throw new Error("multimodal assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.scope.trim()||r.study_order.length<2||r.modality_order.length<2||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)) throw new Error("multimodal assurance identity, closure, witnesses, locality, or effects are incomplete"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.missing_order,r.incomparable_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal assurance ordering is not canonical"); const classified=new Set([...r.qualified_order,...r.blocked_order,...r.unknown_order]); if(classified.size!==r.candidate_order.length||[...classified].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal assurance outcomes do not partition candidates"); for(const v of [r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-multimodal-context:")&&e!=="block:unsafe-release")) throw new Error("multimodal assurance effect is outside the local release gate"); }
export function brainMultimodalContextCompilationAssuranceReceiptDigest(r:BrainMultimodalContextCompilationAssuranceReceipt):string { validateBrainMultimodalContextCompilationAssuranceReceipt(r); return digestJsonSync(r); }
export const THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-brain-P03-F27" as const;
export const THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "brain-throughput-context-compilation-assurance/1.0" as const;
export interface BrainThroughputContextCompilationAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; batch_id:string; partition:string; verdict:"qualified"|"unresolved"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; checkpoint_seq:number; queue_digest:string; verification_digest:string; replay_identity:string; witness_order:string[]; counterexample_order:string[]; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainThroughputContextCompilationAssuranceReceipt(r:BrainThroughputContextCompilationAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID||r.contract_version!==THROUGHPUT_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION) throw new Error("throughput assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.batch_id.trim()||!r.partition.trim()||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)) throw new Error("throughput assurance identity, queue, witnesses, locality, or effects are incomplete"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("throughput assurance ordering is not canonical"); const classified=new Set([...r.qualified_order,...r.blocked_order,...r.unknown_order]); if(classified.size!==r.candidate_order.length||[...classified].some(v=>!r.candidate_order.includes(v))) throw new Error("throughput assurance outcomes do not partition candidates"); for(const v of [r.queue_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("throughput assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:local-throughput-context:")&&e!=="block:unsafe-release")) throw new Error("throughput assurance effect is outside the local release gate"); }
export function brainThroughputContextCompilationAssuranceReceiptDigest(r:BrainThroughputContextCompilationAssuranceReceipt):string { validateBrainThroughputContextCompilationAssuranceReceipt(r); return digestJsonSync(r); }
export const FEDERATED_CONTINUAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID = "AFA-brain-P03-F28" as const;
export const FEDERATED_CONTINUAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION = "brain-federated-continual-context-compilation-assurance/1.0" as const;
export interface BrainFederatedContinualContextCompilationAssuranceReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; federation_id:string; purpose:string; scope:string; goal:string; semantic_profile:string; verdict:"qualified"|"unresolved"|"blocked"; institution_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; stale_order:string[]; aggregate_order:string[]; quorum:number; minimum_quorum:number; envelope_digest:string; verification_digest:string; replay_identity:string; witness_order:string[]; counterexample_order:string[]; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; aggregate_only:boolean; boundary:string; }
export function validateBrainFederatedContinualContextCompilationAssuranceReceipt(r:BrainFederatedContinualContextCompilationAssuranceReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_CONTINUAL_CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID||r.contract_version!==FEDERATED_CONTINUAL_CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION) throw new Error("federated assurance schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.aggregate_only||!r.request_id.trim()||!r.federation_id.trim()||!r.purpose.trim()||!r.scope.trim()||!r.goal.trim()||!r.semantic_profile.trim()||r.institution_order.length<2||!r.candidate_order.length||!r.witness_order.length||!r.effect_receipts.length||r.minimum_quorum<1||r.quorum!==r.qualified_order.length||r.quorum>r.candidate_order.length||!new Set(["qualified","unresolved","blocked"]).has(r.verdict)) throw new Error("federated assurance identity, quorum, locality, aggregate-only, or effects are incomplete"); for(const v of [r.institution_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.stale_order,r.aggregate_order,r.witness_order,r.counterexample_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated assurance ordering is not canonical"); const classified=new Set([...r.qualified_order,...r.blocked_order,...r.unknown_order]); if(classified.size!==r.candidate_order.length||[...classified].some(v=>!r.candidate_order.includes(v))) throw new Error("federated assurance outcomes do not partition candidates"); if(r.aggregate_order.length!==r.qualified_order.length) throw new Error("federated aggregate order does not match qualified peers"); for(const v of [...r.aggregate_order,r.envelope_digest,r.verification_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated assurance digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("assurance:federated-context:")&&e!=="block:unsafe-release")) throw new Error("federated assurance effect is outside the governed release gate"); }
export function brainFederatedContinualContextCompilationAssuranceReceiptDigest(r:BrainFederatedContinualContextCompilationAssuranceReceipt):string { validateBrainFederatedContinualContextCompilationAssuranceReceipt(r); return digestJsonSync(r); }
export const FEDERATED_RETRIEVAL_WORKBENCH_FEATURE_ID = "AFA-brain-P02-F20" as const;
export const FEDERATED_RETRIEVAL_WORKBENCH_CONTRACT_VERSION = "brain-federated-retrieval-research-workbench/1.0" as const;
export interface BrainFederatedRetrievalWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workspace_id:string; federation_id:string; institution_id:string; purpose:string; endpoint:string; study_order:string[]; modality_order:string[]; disposition:"qualified"|"partial"|"unknown"|"blocked"; view_order:string[]; panel_order:string[]; action_receipts:string[]; candidate_order:string[]; ranked_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; comparability_digest:string; envelope_digest:string; synthesis_digest:string; workbench_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedRetrievalWorkbenchReceipt(r:BrainFederatedRetrievalWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RETRIEVAL_WORKBENCH_FEATURE_ID||r.contract_version!==FEDERATED_RETRIEVAL_WORKBENCH_CONTRACT_VERSION) throw new Error("federated workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workspace_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.endpoint.trim()||r.study_order.length<2||r.modality_order.length<2||!r.view_order.length||!r.panel_order.length||!r.action_receipts.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("federated workbench identity, coverage, views, panels, retrieval, locality, budget, or effects are incomplete"); if([...r.ranked_order,...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated workbench state is not covered by candidates"); for(const v of [r.study_order,r.modality_order,r.view_order,r.panel_order,r.action_receipts,r.candidate_order,r.ranked_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated workbench ordering is not canonical"); if(JSON.stringify([...new Set(r.aggregate_order)].sort())!==JSON.stringify(r.aggregate_order)) throw new Error("federated aggregate ordering is not canonical"); for(const v of [r.comparability_digest,r.envelope_digest,r.synthesis_digest,r.workbench_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-federated-retrieval-artifacts:")&&e!=="block:unsafe-release")) throw new Error("federated workbench effect is not read-only"); }
export function brainFederatedRetrievalWorkbenchReceiptDigest(r:BrainFederatedRetrievalWorkbenchReceipt):string { validateBrainFederatedRetrievalWorkbenchReceipt(r); return digestJsonSync(r); }
export interface BrainMultimodalProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"qualified"|"partial"|"unknown"|"blocked"; study_order:string[]; modality_order:string[]; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; evidence_digest:string; comparability_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainMultimodalProtocolReceipt(r:BrainMultimodalProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==MULTIMODAL_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==MULTIMODAL_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("multimodal protocol schema mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||r.protocol_version!=="aurora-research-multimodal/1.0"||r.method!=="POST"||r.route!=="/v1/research/evidence/multimodal/surveil"||r.content_type!=="application/json"||!r.request_id.trim()||!r.idempotency_key.trim()||r.response_schema!=="MultimodalEvidenceProtocolResponse1@1"||r.study_order.length<2||r.modality_order.length<2||!r.candidate_order.length||!r.effect_receipts.length) throw new Error("multimodal protocol identity incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("multimodal protocol state is not covered"); for(const v of [r.study_order,r.modality_order,r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("multimodal protocol ordering invalid"); if(![200,202,206,403,422].includes(r.status_code)) throw new Error("multimodal protocol status invalid"); for(const v of [r.evidence_digest,r.comparability_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("multimodal protocol digest invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:local-multimodal-response:")&&e!=="block:unsafe-release")) throw new Error("multimodal protocol effect invalid"); }
export function brainMultimodalProtocolReceiptDigest(r:BrainMultimodalProtocolReceipt):string { validateBrainMultimodalProtocolReceipt(r); return digestJsonSync(r); }
export interface BrainEvidenceProtocolReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; protocol_version:string; method:string; route:string; content_type:string; idempotency_key:string; response_schema:string; status_code:number; disposition:"qualified"|"partial"|"unknown"|"blocked"; candidate_order:string[]; qualified_order:string[]; blocked_order:string[]; unknown_order:string[]; evidence_digest:string; request_digest:string; response_digest:string; replay_identity:string; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainEvidenceProtocolReceipt(r:BrainEvidenceProtocolReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==EVIDENCE_PROTOCOL_ADAPTER_FEATURE_ID||r.contract_version!==EVIDENCE_PROTOCOL_ADAPTER_CONTRACT_VERSION) throw new Error("protocol schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||r.protocol_version!=="aurora-research/1.0"||r.method!=="POST"||r.route!=="/v1/research/evidence/surveil"||r.content_type!=="application/json"||!r.request_id.trim()||!r.idempotency_key.trim()||r.response_schema!=="EvidenceProtocolResponse1@1"||!r.candidate_order.length||!r.effect_receipts.length) throw new Error("protocol identity, route, idempotency, evidence, locality, or effects are incomplete"); if([...r.qualified_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("protocol state is not covered by candidates"); for(const v of [r.candidate_order,r.qualified_order,r.blocked_order,r.unknown_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("protocol ordering is invalid"); if(![200,202,206,403,422].includes(r.status_code)) throw new Error("protocol status code is invalid"); for(const v of [r.evidence_digest,r.request_digest,r.response_digest,r.replay_identity,r.artifact.content_hash]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("protocol digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("protocol:local-response:")&&e!=="block:unsafe-release")) throw new Error("protocol effect is invalid"); }
export function brainEvidenceProtocolReceiptDigest(r:BrainEvidenceProtocolReceipt):string { validateBrainEvidenceProtocolReceipt(r); return digestJsonSync(r); }
export interface BrainFederatedResearchWorkbenchReceipt { schema_version:string; contract_version:string; feature_id:string; request_id:string; workspace_id:string; federation_id:string; institution_id:string; purpose:string; endpoint:string; disposition:"qualified"|"partial"|"unknown"|"blocked"; view_order:string[]; panel_order:string[]; action_receipts:string[]; candidate_order:string[]; admitted_order:string[]; blocked_order:string[]; unknown_order:string[]; aggregate_order:string[]; evidence_digest:string; envelope_digest:string; workbench_digest:string; replay_identity:string; budget_units:number; omissions:string[]; uncertainty:string[]; negative_evidence:string[]; effect_receipts:string[]; artifact:Record<string,unknown>; raw_data_local:boolean; boundary:string; }
export function validateBrainFederatedResearchWorkbenchReceipt(r:BrainFederatedResearchWorkbenchReceipt):void { if(r.schema_version!==RESEARCH_CONTRACT_SCHEMA_VERSION||r.feature_id!==FEDERATED_RESEARCH_WORKBENCH_FEATURE_ID||r.contract_version!==FEDERATED_RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("federated workbench schema, feature, or version mismatch"); if(r.boundary!==PRECLINICAL_BOUNDARY||!r.raw_data_local||!r.request_id.trim()||!r.workspace_id.trim()||!r.federation_id.trim()||!r.institution_id.trim()||!r.purpose.trim()||!r.endpoint.trim()||!r.view_order.length||!r.panel_order.length||!r.action_receipts.length||!r.candidate_order.length||!r.effect_receipts.length||!Number.isInteger(r.budget_units)||r.budget_units<=0) throw new Error("federated workbench identity, views, evidence, locality, budget, or effects are incomplete"); if([...r.admitted_order,...r.blocked_order,...r.unknown_order].some(v=>!r.candidate_order.includes(v))) throw new Error("federated workbench state is not covered by candidates"); for(const v of [r.view_order,r.panel_order,r.action_receipts,r.candidate_order,r.admitted_order,r.blocked_order,r.unknown_order,r.aggregate_order,r.omissions,r.uncertainty,r.negative_evidence,r.effect_receipts]) if(JSON.stringify([...new Set(v)].sort())!==JSON.stringify(v)) throw new Error("federated workbench ordering is invalid"); for(const v of [r.evidence_digest,r.envelope_digest,r.workbench_digest,r.replay_identity,r.artifact.content_hash,...r.aggregate_order]) if(typeof v!=="string"||!/^[0-9a-f]{64}$/.test(v)) throw new Error("federated workbench digest is invalid"); if(r.effect_receipts.some(e=>!e.startsWith("view:local-federated-artifacts:")&&e!=="block:unsafe-release")) throw new Error("federated workbench effect is not read-only"); }
export function brainFederatedResearchWorkbenchReceiptDigest(r:BrainFederatedResearchWorkbenchReceipt):string { validateBrainFederatedResearchWorkbenchReceipt(r); return digestJsonSync(r); }

export type PolicyDecision = "allow" | "deny" | "redact" | "local_only" | "approval_required" | "unresolved";
export type EvidenceState = "proven" | "supported" | "speculative" | "contradicted" | "unknown";

export interface PolicyReceipt {
  schema_version: string;
  receipt_id: string;
  decision: PolicyDecision;
  reasons: string[];
  evaluated_artifacts: string[];
  authority_reference?: string | null;
  boundary: string;
}

export interface EvidenceOmission {
  item: string;
  reason: string;
  could_change_decision: "no_known_impact" | "potentially_material" | "unknown";
}

export interface EvidenceReceipt {
  schema_version: string;
  receipt_id: string;
  intent: string;
  sources: readonly { source_id: string; source_type: string; locator: string; digest?: string | null; availability: string }[];
  derivation: string[];
  uncertainty: readonly { kind: string; statement: string }[];
  omissions: readonly EvidenceOmission[];
  competing_explanations: readonly unknown[];
  negative_evidence: readonly unknown[];
  conclusion_state: EvidenceState;
  boundary: string;
}

export interface ReleaseReview {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  card_digest: string;
  verdict: "pass" | "conditional" | "blocked" | "not_evaluated";
  reasons: string[];
  replications: readonly Record<string, unknown>[];
  checks: readonly Record<string, unknown>[];
  provenance_complete: boolean;
  boundary: string;
}

export function validateReleaseReview(review: ReleaseReview): void {
  if (review.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (review.feature_id !== RELEASE_REVIEW_FEATURE_ID || !review.capability_id.trim()) throw new Error("release review feature or capability is missing");
  if (review.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!/^[0-9a-f]{64}$/.test(review.card_digest)) throw new Error("release review card digest is not a canonical sha256");
  if (!review.reasons.length) throw new Error("release review requires reasons");
  if (review.verdict === "pass" && !review.provenance_complete) throw new Error("a passing release review requires complete provenance");
}

export function releaseReviewDigest(review: ReleaseReview): string {
  validateReleaseReview(review);
  return digestJsonSync(review);
}

export interface ResearchIngestionBundle {
  schema_version: string;
  feature_id: string;
  source_id: string;
  adapter: string;
  adapter_version: string;
  source_digest: string;
  ingestion_digest: string;
  artifact: Record<string, unknown>;
  conformance: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateResearchIngestionBundle(bundle: ResearchIngestionBundle): void {
  if (bundle.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (bundle.feature_id !== RESEARCH_INGESTION_FEATURE_ID || !bundle.source_id.trim()) throw new Error("research ingestion feature or source is missing");
  if (bundle.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  for (const digest of [bundle.source_digest, bundle.ingestion_digest, bundle.artifact.content_hash]) {
    if (typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)) throw new Error("research ingestion digest is not a canonical sha256");
  }
  if (!bundle.raw_data_local) throw new Error("raw research data must remain local");
  if (bundle.conformance.verified !== true) throw new Error("research ingestion is not conformance verified");
}

export function researchIngestionBundleDigest(bundle: ResearchIngestionBundle): string {
  validateResearchIngestionBundle(bundle);
  return digestJsonSync(bundle);
}

export interface ExperimentDesignPlan {
  payload: Record<string, unknown> & { allocations: readonly { arm_id: string; units: number }[]; total_units: number };
  artifact: Record<string, unknown>;
}

export function validateExperimentDesignPlan(plan: ExperimentDesignPlan): void {
  if (plan.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (plan.payload.feature_id !== EXPERIMENT_DESIGN_FEATURE_ID) throw new Error("experiment design feature mismatch");
  if (plan.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!plan.payload.allocations.length || plan.payload.allocations.reduce((sum, allocation) => sum + allocation.units, 0) !== plan.payload.total_units) throw new Error("experiment design allocation total is inconsistent");
  if (typeof plan.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(plan.artifact.content_hash)) throw new Error("experiment design artifact digest is invalid");
}

export function experimentDesignPlanDigest(plan: ExperimentDesignPlan): string {
  validateExperimentDesignPlan(plan);
  return digestJsonSync(plan);
}

export interface ProtocolSimulationReport {
  payload: Record<string, unknown> & { results: readonly { status: "passed" | "failed_closed" | "requires_approval" }[] };
  artifact: Record<string, unknown>;
}

export function validateProtocolSimulationReport(report: ProtocolSimulationReport): void {
  if (report.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (report.payload.feature_id !== PROTOCOL_SIMULATION_FEATURE_ID) throw new Error("protocol simulation feature mismatch");
  if (report.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!report.payload.results.length) throw new Error("protocol simulation results are incomplete");
  if (typeof report.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(report.artifact.content_hash)) throw new Error("protocol simulation artifact digest is invalid");
}

export function protocolSimulationReportDigest(report: ProtocolSimulationReport): string {
  validateProtocolSimulationReport(report);
  return digestJsonSync(report);
}

export interface ReplicationReport {
  payload: Record<string, unknown> & {
    summary: {
      disposition: "replicated" | "partially_replicated" | "contradicted" | "null_result" | "insufficient_evidence";
      total_observations: number;
      reasons: readonly string[];
    };
  };
  artifact: Record<string, unknown>;
}

export function validateReplicationReport(report: ReplicationReport): void {
  if (report.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (report.payload.feature_id !== REPLICATION_FEATURE_ID) throw new Error("replication feature mismatch");
  if (report.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (report.payload.summary.total_observations <= 0 || report.payload.summary.reasons.length === 0) throw new Error("replication summary is incomplete");
  if (!["replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence"].includes(report.payload.summary.disposition)) throw new Error("replication disposition is unknown");
  if (typeof report.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(report.artifact.content_hash)) throw new Error("replication artifact digest is invalid");
}

export function replicationReportDigest(report: ReplicationReport): string {
  validateReplicationReport(report);
  return digestJsonSync(report);
}

export interface QualityControlReceipt {
  payload: Record<string, unknown> & {
    summary: {
      disposition: "pass" | "pass_with_warnings" | "blocked" | "unknown";
      reasons: readonly string[];
    };
    raw_data_local: boolean;
  };
  artifact: Record<string, unknown>;
}

export function validateQualityControlReceipt(receipt: QualityControlReceipt): void {
  if (receipt.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.payload.feature_id !== QUALITY_CONTROL_FEATURE_ID) throw new Error("quality-control feature mismatch");
  if (receipt.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.payload.summary.reasons.length) throw new Error("quality-control summary is incomplete");
  if (!["pass", "pass_with_warnings", "blocked", "unknown"].includes(receipt.payload.summary.disposition)) throw new Error("quality-control disposition is unknown");
  if (!receipt.payload.raw_data_local) throw new Error("raw research data must remain local");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("quality-control artifact digest is invalid");
}

export function qualityControlReceiptDigest(receipt: QualityControlReceipt): string {
  validateQualityControlReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResearchContextReceipt {
  payload: Record<string, unknown> & {
    protected_closure_satisfied: boolean;
    supports_sufficiency_claim: boolean;
    unresolved_obligations: number;
    section_digest: string;
    certificate_digest: string;
  };
  artifact: Record<string, unknown>;
}

export function validateResearchContextReceipt(receipt: ResearchContextReceipt): void {
  if (receipt.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.payload.feature_id !== RESEARCH_CONTEXT_FEATURE_ID) throw new Error("research-context feature mismatch");
  if (receipt.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.payload.protected_closure_satisfied) throw new Error("protected closure is not satisfied");
  if (!Number.isInteger(receipt.payload.unresolved_obligations) || receipt.payload.unresolved_obligations < 0) throw new Error("unresolved-obligation count is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.payload.section_digest) || !/^[0-9a-f]{64}$/.test(receipt.payload.certificate_digest)) throw new Error("research-context source digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("research-context artifact digest is invalid");
}

export function researchContextReceiptDigest(receipt: ResearchContextReceipt): string {
  validateResearchContextReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ReplayAuditReceipt {
  payload: Record<string, unknown> & {
    status: "equivalent" | "diverged" | "invalid";
    baseline_digest: string;
    candidate_digest: string;
    reasons: readonly string[];
  };
  artifact: Record<string, unknown>;
}

export function validateReplayAuditReceipt(receipt: ReplayAuditReceipt): void {
  if (receipt.payload.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.payload.feature_id !== REPLAY_AUDIT_FEATURE_ID) throw new Error("replay-audit feature mismatch");
  if (receipt.payload.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!["equivalent", "diverged", "invalid"].includes(receipt.payload.status)) throw new Error("replay-audit status is unknown");
  if (!receipt.payload.reasons.length) throw new Error("replay-audit reasons are required");
  if (!/^[0-9a-f]{64}$/.test(receipt.payload.baseline_digest) || !/^[0-9a-f]{64}$/.test(receipt.payload.candidate_digest)) throw new Error("replay-audit source digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("replay-audit artifact digest is invalid");
}

export function replayAuditReceiptDigest(receipt: ReplayAuditReceipt): string {
  validateReplayAuditReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface WorkflowExecutionReceipt {
  schema_version: string;
  feature_id: string;
  workflow_id: string;
  mode: "dry_run" | "execute";
  status: "dry_run" | "succeeded";
  ordered_nodes: readonly string[];
  completed_nodes: readonly string[];
  run: Record<string, unknown>;
  run_digest: string;
  remaining_budget: Record<string, number>;
  artifact: Record<string, unknown>;
  reasons: readonly string[];
  boundary: string;
}

export function validateWorkflowExecutionReceipt(receipt: WorkflowExecutionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== WORKFLOW_EXECUTION_FEATURE_ID || !receipt.workflow_id.trim()) throw new Error("workflow-execution feature or workflow is missing");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.ordered_nodes.length || receipt.completed_nodes.some((node) => !receipt.ordered_nodes.includes(node))) throw new Error("workflow execution order is incomplete");
  if (!receipt.reasons.length) throw new Error("workflow execution reasons are required");
  if (receipt.run.workflow_id !== receipt.workflow_id) throw new Error("workflow run identity does not match receipt");
  const expectedRunStatus = receipt.status === "dry_run" ? "planned" : "succeeded";
  if (receipt.run.status !== expectedRunStatus) throw new Error("workflow run status does not match receipt status");
  if (!/^[0-9a-f]{64}$/.test(receipt.run_digest)) throw new Error("workflow run digest is not a canonical sha256");
  if (Object.values(receipt.remaining_budget).some((amount) => !Number.isFinite(amount) || amount < 0)) throw new Error("workflow remaining budget is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("workflow execution artifact digest is invalid");
}

export function workflowExecutionReceiptDigest(receipt: WorkflowExecutionReceipt): string {
  validateWorkflowExecutionReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface EvaluationCardReceipt {
  schema_version: string;
  feature_id: string;
  card: Record<string, unknown> & {
    schema_version: string;
    capability_id: string;
    benchmark_world: string;
    baselines: readonly string[];
    metrics: readonly Record<string, unknown>[];
    uncertainty: readonly Record<string, unknown>[];
    release_verdict: "pass" | "conditional" | "blocked" | "not_evaluated";
  };
  card_digest: string;
  observations_digest: string;
  baseline_counts: Record<string, number>;
  omissions: readonly string[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateEvaluationCardReceipt(receipt: EvaluationCardReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== EVALUATION_OBSERVABILITY_FEATURE_ID) throw new Error("evaluation-observability feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.card.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || !receipt.card.capability_id.trim() || !receipt.card.benchmark_world.trim()) throw new Error("evaluation card identity is incomplete");
  if (!receipt.card.baselines.length || !receipt.card.metrics.length || !receipt.card.uncertainty.length) throw new Error("evaluation card evidence fields are incomplete");
  if (!receipt.reasons.length || !Object.keys(receipt.baseline_counts).length) throw new Error("evaluation receipt needs baseline counts and reasons");
  if (receipt.card.release_verdict === "pass" && receipt.omissions.length) throw new Error("a passing evaluation card cannot hide baseline omissions");
  if (Object.values(receipt.baseline_counts).some((count) => !Number.isInteger(count) || count < 0)) throw new Error("evaluation baseline count is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.card_digest) || !/^[0-9a-f]{64}$/.test(receipt.observations_digest)) throw new Error("evaluation receipt source digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("evaluation receipt artifact digest is invalid");
}

export function evaluationCardReceiptDigest(receipt: EvaluationCardReceipt): string {
  validateEvaluationCardReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResearchReleaseReceipt {
  schema_version: string;
  feature_id: string;
  release_id: string;
  research_object: {
    release_id: string;
    artifact_ids: readonly string[];
    evidence_receipt_ids: readonly string[];
    boundary: string;
    federation: {
      envelope: {
        raw_data_local: boolean;
        signature?: string | null;
        localization_statement: string;
        export: Record<string, unknown> & { content_hash: string; provenance: readonly Record<string, unknown>[] };
      };
    };
  };
  release_digest: string;
  omissions: readonly string[];
  reasons: readonly string[];
  boundary: string;
}

export function validateResearchReleaseReceipt(receipt: ResearchReleaseReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== RESEARCH_RELEASE_FEATURE_ID || !receipt.release_id.trim()) throw new Error("research-release feature or identity is missing");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || receipt.research_object.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.research_object.release_id !== receipt.release_id) throw new Error("research object release identity does not match receipt");
  if (!receipt.research_object.artifact_ids.length || new Set(receipt.research_object.artifact_ids).size !== receipt.research_object.artifact_ids.length) throw new Error("research object artifact ids are incomplete or duplicated");
  if (!receipt.research_object.evidence_receipt_ids.length || new Set(receipt.research_object.evidence_receipt_ids).size !== receipt.research_object.evidence_receipt_ids.length) throw new Error("research object evidence ids are incomplete or duplicated");
  const envelope = receipt.research_object.federation.envelope;
  if (!envelope.raw_data_local || !envelope.signature || !envelope.localization_statement.trim()) throw new Error("research release signature and localization are required");
  if (!envelope.export.provenance.length) throw new Error("research release provenance is incomplete");
  if (!receipt.reasons.length) throw new Error("research release reasons are required");
  if (!/^[0-9a-f]{64}$/.test(receipt.release_digest) || !/^[0-9a-f]{64}$/.test(envelope.export.content_hash)) throw new Error("research release digest is not a canonical sha256");
}

export function researchReleaseReceiptDigest(receipt: ResearchReleaseReceipt): string {
  validateResearchReleaseReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface InstrumentPreflightReceipt {
  schema_version: string;
  feature_id: string;
  run_id: string;
  study_id: string;
  decision: "ready" | "blocked" | "requires_approval" | "emergency_stop";
  ordered_actions: readonly string[];
  action_digests: Record<string, string>;
  remaining_budget: Record<string, number>;
  omissions: readonly string[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateInstrumentPreflightReceipt(receipt: InstrumentPreflightReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== INSTRUMENT_PREFLIGHT_FEATURE_ID || !receipt.run_id.trim() || !receipt.study_id.trim()) throw new Error("instrument-preflight identity is missing");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!receipt.ordered_actions.length || !Object.keys(receipt.action_digests).length || !receipt.reasons.length) throw new Error("instrument preflight evidence is incomplete");
  if (new Set(receipt.ordered_actions).size !== receipt.ordered_actions.length || receipt.ordered_actions.some((action) => !(action in receipt.action_digests))) throw new Error("instrument action ordering or digest coverage is invalid");
  if (Object.values(receipt.action_digests).some((digest) => !/^[0-9a-f]{64}$/.test(digest))) throw new Error("instrument action digest is invalid");
  if (Object.values(receipt.remaining_budget).some((amount) => !Number.isFinite(amount) || amount < 0)) throw new Error("instrument remaining budget is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("instrument preflight artifact digest is invalid");
}

export function instrumentPreflightReceiptDigest(receipt: InstrumentPreflightReceipt): string {
  validateInstrumentPreflightReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface HarmonizedResearchObject {
  schema_version: string;
  feature_id: string;
  study_id: string;
  reference_schema: string;
  decision: "comparable" | "partial" | "blocked";
  modality_order: readonly string[];
  alignment: Record<string, readonly string[]>;
  omitted_modalities: readonly string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateHarmonizedResearchObject(object: HarmonizedResearchObject): void {
  if (object.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (object.feature_id !== MULTIMODAL_HARMONIZATION_FEATURE_ID || !object.study_id.trim() || !object.reference_schema.trim()) throw new Error("multimodal research object identity is incomplete");
  if (object.boundary !== PRECLINICAL_BOUNDARY || !object.raw_data_local) throw new Error("multimodal raw data must remain local");
  if (!object.modality_order.length || !Object.keys(object.alignment).length || !object.reasons.length) throw new Error("multimodal alignment and reasons are incomplete");
  if (object.modality_order.some((modality) => !(modality in object.alignment))) throw new Error("multimodal alignment omits a modality projection");
  if (typeof object.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(object.artifact.content_hash)) throw new Error("multimodal artifact digest is invalid");
}

export function harmonizedResearchObjectDigest(object: HarmonizedResearchObject): string {
  validateHarmonizedResearchObject(object);
  return digestJsonSync(object);
}

export interface QualifiedAnalysisResult {
  schema_version: string;
  feature_id: string;
  question_id: string;
  estimand: string;
  verdict: "qualified" | "conditional" | "blocked";
  selected_candidate: string | null;
  candidate_order: readonly string[];
  uncertainty: readonly string[];
  omissions: readonly string[];
  negative_evidence: readonly string[];
  reasons: readonly string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualifiedAnalysisResult(result: QualifiedAnalysisResult): void {
  if (result.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (result.feature_id !== ANALYSIS_QUALIFICATION_FEATURE_ID || !result.question_id.trim() || !result.estimand.trim()) throw new Error("qualified analysis identity is incomplete");
  if (result.boundary !== PRECLINICAL_BOUNDARY || !result.raw_data_local) throw new Error("qualified analysis must retain raw data locally");
  if (!result.candidate_order.length || !result.reasons.length || !result.uncertainty.length) throw new Error("qualified analysis evidence is incomplete");
  if (result.verdict === "qualified" && result.selected_candidate === null) throw new Error("qualified analysis needs a selected candidate");
  if (typeof result.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(result.artifact.content_hash)) throw new Error("qualified analysis artifact digest is invalid");
}

export function qualifiedAnalysisResultDigest(result: QualifiedAnalysisResult): string {
  validateQualifiedAnalysisResult(result);
  return digestJsonSync(result);
}

export interface ProtocolMatrixReceipt {
  schema_version: string;
  feature_id: string;
  protocol_id: string;
  total_cells: number;
  passed_cells: number;
  failed_closed_cells: number;
  approval_cells: number;
  cells: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateProtocolMatrixReceipt(receipt: ProtocolMatrixReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (receipt.feature_id !== PROTOCOL_MATRIX_FEATURE_ID || !receipt.protocol_id.trim()) throw new Error("protocol matrix identity is incomplete");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!Number.isInteger(receipt.total_cells) || receipt.total_cells <= 0 || receipt.total_cells !== receipt.cells.length) throw new Error("protocol matrix cell count is invalid");
  if ([receipt.passed_cells, receipt.failed_closed_cells, receipt.approval_cells].some((value) => !Number.isInteger(value) || value < 0)) throw new Error("protocol matrix status count is invalid");
  if (receipt.passed_cells + receipt.failed_closed_cells + receipt.approval_cells !== receipt.total_cells) throw new Error("protocol matrix status counts do not partition cells");
  if (!receipt.cells.length || receipt.cells.some((cell) => typeof cell.cell_id !== "string" || !cell.cell_id.trim() || !Array.isArray(cell.reasons) || (cell.reasons as unknown[]).length === 0)) throw new Error("protocol matrix cells need ids and reasons");
  const statuses = new Set(["passed", "failed_closed", "requires_approval"]);
  if (receipt.cells.some((cell) => typeof cell.status !== "string" || !statuses.has(cell.status))) throw new Error("protocol matrix cell status is unknown");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("protocol matrix artifact digest is invalid");
}

export function protocolMatrixReceiptDigest(receipt: ProtocolMatrixReceipt): string {
  validateProtocolMatrixReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface MultimodalReplicationReport {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  claim: string;
  request_digest: string;
  required_modalities: readonly string[];
  summary: Record<string, unknown>;
  studies: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateMultimodalReplicationReport(report: MultimodalReplicationReport): void {
  if (report.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (report.feature_id !== MULTIMODAL_REPLICATION_FEATURE_ID || !report.capability_id.trim() || !report.claim.trim()) throw new Error("multimodal replication identity is incomplete");
  if (report.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (!report.required_modalities.length || !report.studies.length) throw new Error("multimodal replication evidence set is incomplete");
  const disposition = report.summary.disposition;
  if (typeof disposition !== "string" || !new Set(["replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence"]).has(disposition)) throw new Error("multimodal replication disposition is unknown");
  if (report.summary.total_observations !== report.studies.length || !Array.isArray(report.summary.reasons) || report.summary.reasons.length === 0) throw new Error("multimodal replication summary is inconsistent");
  if (report.studies.some((study) => typeof study.study_id !== "string" || !study.study_id.trim() || !Array.isArray(study.reasons))) throw new Error("multimodal study comparability record is incomplete");
  if (typeof report.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(report.artifact.content_hash)) throw new Error("multimodal replication artifact digest is invalid");
}

export function multimodalReplicationReportDigest(report: MultimodalReplicationReport): string {
  validateMultimodalReplicationReport(report);
  return digestJsonSync(report);
}

export interface QualityDriftReceipt {
  schema_version: string;
  feature_id: string;
  dataset_id: string;
  modality: string;
  request_digest: string;
  summary: Record<string, unknown>;
  metrics: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualityDriftReceipt(receipt: QualityDriftReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== QUALITY_DRIFT_FEATURE_ID) throw new Error("quality drift schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.dataset_id.trim() || !receipt.modality.trim()) throw new Error("quality drift identity or locality is invalid");
  if (typeof receipt.summary.disposition !== "string" || !new Set(["stable", "drifted", "unknown", "blocked"]).has(receipt.summary.disposition)) throw new Error("quality drift disposition is unknown");
  if (!receipt.metrics.length || !Array.isArray(receipt.summary.reasons) || receipt.summary.reasons.length === 0) throw new Error("quality drift metrics and reasons are incomplete");
  if (receipt.metrics.length !== Number(receipt.summary.stable ?? 0) + Number(receipt.summary.drifted ?? 0) + Number(receipt.summary.unknown ?? 0)) throw new Error("quality drift metric counts are inconsistent");
  if (!/^[0-9a-f]{64}$/.test(receipt.request_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("quality drift digest is invalid");
}

export function qualityDriftReceiptDigest(receipt: QualityDriftReceipt): string {
  validateQualityDriftReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface DesignFrontierReceipt {
  schema_version: string;
  feature_id: string;
  study_id: string;
  feasible_scenarios: number;
  blocked_scenarios: number;
  scenarios: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateDesignFrontierReceipt(receipt: DesignFrontierReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== DESIGN_FRONTIER_FEATURE_ID) throw new Error("design frontier schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.study_id.trim() || !receipt.scenarios.length) throw new Error("design frontier identity or boundary is invalid");
  if (receipt.feasible_scenarios < 0 || receipt.blocked_scenarios < 0 || receipt.feasible_scenarios + receipt.blocked_scenarios !== receipt.scenarios.length) throw new Error("design frontier scenario counts are inconsistent");
  if (receipt.scenarios.some((scenario) => typeof scenario.scenario_id !== "string" || !scenario.scenario_id.trim() || !new Set(["feasible", "blocked"]).has(String(scenario.disposition)) || !Array.isArray(scenario.reasons) || scenario.reasons.length === 0)) throw new Error("design frontier scenario record is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("design frontier artifact digest is invalid");
}

export function designFrontierReceiptDigest(receipt: DesignFrontierReceipt): string {
  validateDesignFrontierReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface BatchAdmissionReceipt {
  schema_version: string;
  feature_id: string;
  actor: string;
  total_actions: number;
  allowed_actions: number;
  approval_actions: number;
  denied_actions: number;
  actions: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateBatchAdmissionReceipt(receipt: BatchAdmissionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== AUTONOMY_BATCH_FEATURE_ID) throw new Error("autonomy batch schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.actor.trim() || receipt.total_actions <= 0 || receipt.total_actions !== receipt.actions.length) throw new Error("autonomy batch identity or boundary is invalid");
  if ([receipt.allowed_actions, receipt.approval_actions, receipt.denied_actions].some((value) => !Number.isInteger(value) || value < 0) || receipt.allowed_actions + receipt.approval_actions + receipt.denied_actions !== receipt.total_actions) throw new Error("autonomy batch counts are inconsistent");
  if (receipt.actions.some((action) => typeof action.action_id !== "string" || !action.action_id.trim() || !new Set(["allowed", "approval_required", "denied"]).has(String(action.decision)) || !Array.isArray(action.reasons) || action.reasons.length === 0)) throw new Error("autonomy batch action record is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("autonomy batch artifact digest is invalid");
}

export function batchAdmissionReceiptDigest(receipt: BatchAdmissionReceipt): string {
  validateBatchAdmissionReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface WorkflowBatchReceipt {
  schema_version: string;
  feature_id: string;
  total_workflows: number;
  succeeded_workflows: number;
  dry_run_workflows: number;
  blocked_workflows: number;
  entries: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateWorkflowBatchReceipt(receipt: WorkflowBatchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== WORKFLOW_BATCH_FEATURE_ID) throw new Error("workflow batch schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || receipt.total_workflows <= 0 || receipt.total_workflows !== receipt.entries.length) throw new Error("workflow batch identity or boundary is invalid");
  if ([receipt.succeeded_workflows, receipt.dry_run_workflows, receipt.blocked_workflows].some((value) => !Number.isInteger(value) || value < 0) || receipt.succeeded_workflows + receipt.dry_run_workflows + receipt.blocked_workflows !== receipt.total_workflows) throw new Error("workflow batch counts are inconsistent");
  if (receipt.entries.some((entry) => typeof entry.workflow_id !== "string" || !entry.workflow_id.trim() || !new Set(["succeeded", "dry_run", "blocked"]).has(String(entry.disposition)) || !Array.isArray(entry.reasons) || entry.reasons.length === 0)) throw new Error("workflow batch entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("workflow batch artifact digest is invalid");
}

export function workflowBatchReceiptDigest(receipt: WorkflowBatchReceipt): string {
  validateWorkflowBatchReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResearchReleaseBatchReceipt {
  schema_version: string;
  feature_id: string;
  total_releases: number;
  published_releases: number;
  blocked_releases: number;
  entries: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateResearchReleaseBatchReceipt(receipt: ResearchReleaseBatchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESEARCH_RELEASE_BATCH_FEATURE_ID) throw new Error("research-release batch schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || receipt.total_releases <= 0 || receipt.total_releases !== receipt.entries.length) throw new Error("research-release batch identity or boundary is invalid");
  if (![receipt.published_releases, receipt.blocked_releases].every((value) => Number.isInteger(value) && value >= 0) || receipt.published_releases + receipt.blocked_releases !== receipt.total_releases) throw new Error("research-release batch counts are inconsistent");
  if (receipt.entries.some((entry) => typeof entry.release_id !== "string" || !entry.release_id.trim() || !new Set(["published", "blocked"]).has(String(entry.disposition)) || !Array.isArray(entry.reasons) || entry.reasons.length === 0 || (entry.disposition === "published" && typeof entry.release_digest !== "string"))) throw new Error("research-release batch entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("research-release batch artifact digest is invalid");
}

export function researchReleaseBatchReceiptDigest(receipt: ResearchReleaseBatchReceipt): string {
  validateResearchReleaseBatchReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedEvaluationReceipt {
  schema_version: string;
  feature_id: string;
  capability_id: string;
  benchmark_world: string;
  minimum_sites: number;
  total_sites: number;
  agreeing_sites: number;
  contradictory_sites: number;
  blocked_sites: number;
  disposition: string;
  entries: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedEvaluationReceipt(receipt: FederatedEvaluationReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_EVALUATION_FEATURE_ID) throw new Error("federated evaluation schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.capability_id.trim() || !receipt.benchmark_world.trim() || !Number.isInteger(receipt.minimum_sites) || receipt.minimum_sites <= 0 || !Number.isInteger(receipt.total_sites) || receipt.total_sites <= 0 || receipt.total_sites !== receipt.entries.length) throw new Error("federated evaluation identity or boundary is invalid");
  if ([receipt.agreeing_sites, receipt.contradictory_sites, receipt.blocked_sites].some((value) => !Number.isInteger(value) || value < 0) || receipt.agreeing_sites + receipt.contradictory_sites + receipt.blocked_sites !== receipt.total_sites) throw new Error("federated evaluation counts are inconsistent");
  if (!new Set(["consensus", "partial", "contradicted", "blocked"]).has(receipt.disposition)) throw new Error("federated evaluation disposition is unknown");
  if (receipt.entries.some((entry) => typeof entry.site_id !== "string" || !entry.site_id.trim() || !new Set(["accepted", "contradictory", "blocked"]).has(String(entry.disposition)) || !Array.isArray(entry.reasons) || entry.reasons.length === 0 || (entry.disposition === "accepted" && typeof entry.card_digest !== "string"))) throw new Error("federated evaluation site entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated evaluation artifact digest is invalid");
}

export function federatedEvaluationReceiptDigest(receipt: FederatedEvaluationReceipt): string {
  validateFederatedEvaluationReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface QualifiedResourceSet {
  schema_version: string;
  feature_id: string;
  need_id: string;
  requester: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  considered_candidates: number;
  qualified_count: number;
  resources: readonly Record<string, unknown>[];
  omissions: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateQualifiedResourceSet(receipt: QualifiedResourceSet): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_WORKBENCH_FEATURE_ID) throw new Error("resource workbench schema or feature mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.need_id.trim() || !receipt.requester.trim()) throw new Error("resource workbench identity or boundary is invalid");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("resource discovery disposition is unknown");
  if (!Number.isInteger(receipt.considered_candidates) || receipt.considered_candidates <= 0 || !Number.isInteger(receipt.qualified_count) || receipt.qualified_count < 0 || receipt.qualified_count !== receipt.resources.length || receipt.reasons.length === 0) throw new Error("resource discovery counts or reasons are incomplete");
  if (receipt.resources.some((resource) => typeof resource.resource_id !== "string" || !resource.resource_id.trim() || typeof resource.origin !== "string" || !resource.origin.trim() || !Number.isInteger(resource.rank) || Number(resource.rank) <= 0 || !Array.isArray(resource.reasons) || resource.reasons.length === 0)) throw new Error("qualified resource entry is incomplete");
  if (receipt.omissions.some((omission) => typeof omission.resource_id !== "string" || !omission.resource_id.trim() || typeof omission.reason !== "string" || !omission.reason.trim())) throw new Error("resource omission entry is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource workbench artifact digest is invalid");
}

export function qualifiedResourceSetDigest(receipt: QualifiedResourceSet): string {
  validateQualifiedResourceSet(receipt);
  return digestJsonSync(receipt);
}

export interface ResourceDiscoveryContractReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  requested_by: string;
  compatibility_profile: string;
  result: Record<string, unknown>;
  migration_notes: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateResourceDiscoveryContractReceipt(receipt: ResourceDiscoveryContractReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID || receipt.contract_version !== RESOURCE_DISCOVERY_CONTRACT_VERSION) throw new Error("resource discovery contract schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.requested_by.trim() || !receipt.compatibility_profile.trim() || new TextEncoder().encode(receipt.compatibility_profile).length > 256 || receipt.migration_notes.length === 0) throw new Error("resource discovery contract identity, compatibility, migration, or boundary is invalid");
  if (receipt.result.feature_id !== RESOURCE_WORKBENCH_FEATURE_ID || receipt.result.boundary !== PRECLINICAL_BOUNDARY) throw new Error("resource discovery contract result is not the qualified-resource contract");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource discovery contract artifact digest is invalid");
}

export function resourceDiscoveryContractReceiptDigest(receipt: ResourceDiscoveryContractReceipt): string {
  validateResourceDiscoveryContractReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface SignedResearchObjectReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  run_id: string;
  release_id: string;
  origin: string;
  purpose: string;
  artifact_ids: string[];
  evidence_receipt_ids: string[];
  release_digest: string;
  signer_public_key_hex: string;
  signer_signature_hex: string;
  migration_notes: string[];
  omissions: string[];
  raw_data_local: boolean;
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateSignedResearchObjectReceipt(receipt: SignedResearchObjectReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== GOVERNANCE_RESEARCH_RELEASE_FEATURE_ID || receipt.contract_version !== GOVERNANCE_RESEARCH_RELEASE_CONTRACT_VERSION) throw new Error("governance research-release schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || [receipt.run_id, receipt.release_id, receipt.origin, receipt.purpose].some((value) => !value.trim())) throw new Error("signed research object identity or locality is invalid");
  if (receipt.artifact_ids.length === 0 || new Set(receipt.artifact_ids).size !== receipt.artifact_ids.length || receipt.evidence_receipt_ids.length === 0 || new Set(receipt.evidence_receipt_ids).size !== receipt.evidence_receipt_ids.length || receipt.migration_notes.length === 0) throw new Error("signed research object provenance or migration is incomplete");
  if (!/^[0-9a-f]{64}$/.test(receipt.release_digest) || !/^[0-9a-f]{64}$/.test(receipt.signer_public_key_hex) || !/^[0-9a-f]{128}$/.test(receipt.signer_signature_hex)) throw new Error("signed research object signature material is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("signed research object artifact digest is invalid");
}

export function signedResearchObjectReceiptDigest(receipt: SignedResearchObjectReceipt): string {
  validateSignedResearchObjectReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ReleaseHarnessReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  object_digest: string;
  disposition: "passed" | "blocked" | "unknown";
  checks: readonly Record<string, unknown>[];
  omissions: string[];
  reasons: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateReleaseHarnessReceipt(receipt: ReleaseHarnessReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RELEASE_HARNESS_FEATURE_ID || receipt.contract_version !== RELEASE_HARNESS_CONTRACT_VERSION) throw new Error("release harness schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0 || receipt.reasons.length === 0) throw new Error("release harness identity, disposition, checks, or boundary is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.object_digest)) throw new Error("release harness object digest is invalid");
  if (receipt.checks.some((check) => typeof check.check_id !== "string" || !check.check_id.trim() || !new Set(["passed", "blocked", "unknown"]).has(String(check.disposition)) || typeof check.reason !== "string" || !check.reason.trim())) throw new Error("release harness check is incomplete");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("release harness artifact digest is invalid");
}

export function releaseHarnessReceiptDigest(receipt: ReleaseHarnessReceipt): string {
  validateReleaseHarnessReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ProtocolAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  protocol_id: string;
  disposition: "passed" | "blocked" | "unknown";
  total_cells: number;
  passed_cells: number;
  blocked_cells: number;
  unknown_cells: number;
  checks: string[];
  omissions: string[];
  simulation_digest: string;
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateProtocolAssuranceReceipt(receipt: ProtocolAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== PROTOCOL_ASSURANCE_FEATURE_ID || receipt.contract_version !== PROTOCOL_ASSURANCE_CONTRACT_VERSION) throw new Error("protocol assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.protocol_id.trim()) throw new Error("protocol assurance identity or boundary is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("protocol assurance disposition or checks is incomplete");
  if (!Number.isInteger(receipt.total_cells) || receipt.total_cells <= 0 || [receipt.passed_cells, receipt.blocked_cells, receipt.unknown_cells].some((value) => !Number.isInteger(value) || value < 0) || receipt.total_cells !== receipt.passed_cells + receipt.blocked_cells + receipt.unknown_cells) throw new Error("protocol assurance cell counts do not partition");
  if (!/^[0-9a-f]{64}$/.test(receipt.simulation_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("protocol assurance digest is not a canonical sha256");
}

export function protocolAssuranceReceiptDigest(receipt: ProtocolAssuranceReceipt): string {
  validateProtocolAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedMultimodalAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  benchmark_id: string;
  institution_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  harmonized_digest: string;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederatedMultimodalAssuranceReceipt(receipt: FederatedMultimodalAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_MULTIMODAL_ASSURANCE_CONTRACT_VERSION) throw new Error("federated multimodal assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.benchmark_id.trim()) throw new Error("federated multimodal assurance identity or locality is invalid");
  if (receipt.institution_ids.length < 2 || receipt.institution_ids.some((institution) => !institution.trim()) || new Set(receipt.institution_ids).size !== receipt.institution_ids.length) throw new Error("federated multimodal institution set is incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("federated multimodal disposition or checks is incomplete");
  if (!/^[0-9a-f]{64}$/.test(receipt.harmonized_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated multimodal digest is not a canonical sha256");
}

export function federatedMultimodalAssuranceReceiptDigest(receipt: FederatedMultimodalAssuranceReceipt): string {
  validateFederatedMultimodalAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedKnowledgeGatewayReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  interoperability_profile: string;
  institution_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  manifest_digest: string;
  permitted_tags: string[];
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederatedKnowledgeGatewayReceipt(receipt: FederatedKnowledgeGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID || receipt.contract_version !== FEDERATED_KNOWLEDGE_GATEWAY_CONTRACT_VERSION) throw new Error("federated knowledge gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.interoperability_profile.trim()) throw new Error("federated knowledge gateway identity or locality is invalid");
  if (receipt.institution_ids.length < 2 || receipt.institution_ids.some((institution) => !institution.trim()) || new Set(receipt.institution_ids).size !== receipt.institution_ids.length) throw new Error("federated knowledge institution set is incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("federated knowledge disposition or checks is incomplete");
  if (!/^[0-9a-f]{64}$/.test(receipt.manifest_digest) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated knowledge digest is not a canonical sha256");
}

export function federatedKnowledgeGatewayReceiptDigest(receipt: FederatedKnowledgeGatewayReceipt): string {
  validateFederatedKnowledgeGatewayReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedLensAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  institution_ids: string[];
  required_lens_ids: string[];
  report_digests: string[];
  absent_lens_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedLensAssuranceReceipt(receipt: FederatedLensAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_LENS_ASSURANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION) throw new Error("federated lens assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim()) throw new Error("federated lens assurance identity or boundary is invalid");
  if (receipt.institution_ids.length < 2 || receipt.institution_ids.some((institution) => !institution.trim()) || JSON.stringify([...receipt.institution_ids].sort()) !== JSON.stringify(receipt.institution_ids)) throw new Error("federated lens institution ordering is invalid");
  if (receipt.required_lens_ids.length === 0 || !new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("federated lens required set, disposition, or checks is incomplete");
  if (receipt.report_digests.some((digest) => !/^[0-9a-f]{64}$/.test(digest)) || typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated lens digest is invalid");
}

export function federatedLensAssuranceReceiptDigest(receipt: FederatedLensAssuranceReceipt): string {
  validateFederatedLensAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface LabSemanticParityReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  protocol_id: string;
  benchmark_id: string;
  institution_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  semantic_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateLabSemanticParityReceipt(receipt: LabSemanticParityReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== SEMANTIC_PARITY_FEATURE_ID || receipt.contract_version !== SEMANTIC_PARITY_CONTRACT_VERSION) throw new Error("lab semantic parity schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.protocol_id.trim() || !receipt.benchmark_id.trim()) throw new Error("lab semantic parity identity or boundary is invalid");
  if (receipt.institution_ids.length < 2 || JSON.stringify([...new Set(receipt.institution_ids)].sort()) !== JSON.stringify(receipt.institution_ids)) throw new Error("lab semantic parity institution ordering is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || receipt.checks.length === 0) throw new Error("lab semantic parity disposition or checks is incomplete");
  if (receipt.semantic_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.semantic_digest)) throw new Error("lab semantic parity semantic digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("lab semantic parity artifact digest is invalid");
}

export function labSemanticParityReceiptDigest(receipt: LabSemanticParityReceipt): string {
  validateLabSemanticParityReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface FederatedRetrievalAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  returned_source_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  evidence_receipt_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedRetrievalAssuranceReceipt(receipt: FederatedRetrievalAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("federated retrieval assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("federated retrieval identity, boundary, or checks are incomplete");
  if (JSON.stringify([...new Set(receipt.returned_source_ids)].sort()) !== JSON.stringify(receipt.returned_source_ids)) throw new Error("federated retrieval source ordering is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("federated retrieval disposition is unknown");
  if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("federated retrieval evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated retrieval artifact digest is invalid");
}

export function federatedRetrievalAssuranceReceiptDigest(receipt: FederatedRetrievalAssuranceReceipt): string {
  validateFederatedRetrievalAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface RetrievalSourceUpdate {
  source_id: string;
  version: string;
  digest: string;
  evidence_state: string;
  stale: boolean;
}

export interface FederatedContinualRetrievalReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  selected_source_ids: string[];
  stale_source_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  prior_synthesis_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateFederatedContinualRetrievalReceipt(receipt: FederatedContinualRetrievalReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID || receipt.contract_version !== FEDERATED_CONTINUAL_RETRIEVAL_CONTRACT_VERSION) throw new Error("federated continual retrieval schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("federated continual retrieval identity, boundary, or checks are incomplete");
  if (!receipt.selected_source_ids.length || JSON.stringify([...new Set(receipt.selected_source_ids)].sort()) !== JSON.stringify(receipt.selected_source_ids)) throw new Error("federated continual retrieval source ordering is invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("federated continual retrieval disposition is unknown");
  if (receipt.prior_synthesis_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.prior_synthesis_digest)) throw new Error("federated continual retrieval prior digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated continual retrieval artifact digest is invalid");
}

export function federatedContinualRetrievalReceiptDigest(receipt: FederatedContinualRetrievalReceipt): string {
  validateFederatedContinualRetrievalReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ContextCompilationAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  resolved_context_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  evidence_receipt_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateContextCompilationAssuranceReceipt(receipt: ContextCompilationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION) throw new Error("context compilation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("context compilation assurance identity, boundary, or checks are incomplete");
  if (!receipt.resolved_context_ids.length || JSON.stringify([...new Set(receipt.resolved_context_ids)].sort()) !== JSON.stringify(receipt.resolved_context_ids)) throw new Error("context compilation resolved identities are invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("context compilation disposition is unknown");
  if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("context compilation evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("context compilation artifact digest is invalid");
}

export function contextCompilationAssuranceReceiptDigest(receipt: ContextCompilationAssuranceReceipt): string {
  validateContextCompilationAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface KnowledgeRepresentationAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  federation_id: string;
  query_id: string;
  resolved_fact_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  evidence_receipt_digest: string | null;
  checks: string[];
  omissions: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateKnowledgeRepresentationAssuranceReceipt(receipt: KnowledgeRepresentationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION) throw new Error("knowledge representation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.query_id.trim() || receipt.checks.length === 0) throw new Error("knowledge representation assurance identity, boundary, or checks are incomplete");
  if (!receipt.resolved_fact_ids.length || JSON.stringify([...new Set(receipt.resolved_fact_ids)].sort()) !== JSON.stringify(receipt.resolved_fact_ids)) throw new Error("knowledge representation fact identities are invalid");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("knowledge representation disposition is unknown");
  if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("knowledge representation evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("knowledge representation artifact digest is invalid");
}

export function knowledgeRepresentationAssuranceReceiptDigest(receipt: KnowledgeRepresentationAssuranceReceipt): string {
  validateKnowledgeRepresentationAssuranceReceipt(receipt);
  return digestJsonSync(receipt);
}

export interface ResourceControlPlaneReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; federation_id: string; institution_ids: string[]; qualified_resource_ids: string[]; disposition: "passed" | "blocked" | "unknown"; qualification_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateResourceControlPlaneReceipt(receipt: ResourceControlPlaneReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_CONTROL_PLANE_FEATURE_ID || receipt.contract_version !== RESOURCE_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("resource control-plane schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || receipt.institution_ids.length < 2 || JSON.stringify([...new Set(receipt.institution_ids)].sort()) !== JSON.stringify(receipt.institution_ids)) throw new Error("resource control-plane identity is invalid"); if (!receipt.qualified_resource_ids.length || !new Set(["passed", "blocked", "unknown"]).has(receipt.disposition) || !receipt.checks.length) throw new Error("resource control-plane qualification is incomplete"); if (receipt.qualification_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.qualification_digest)) throw new Error("resource control-plane digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource control-plane artifact digest is invalid"); }
export function resourceControlPlaneReceiptDigest(receipt: ResourceControlPlaneReceipt): string { validateResourceControlPlaneReceipt(receipt); return digestJsonSync(receipt); }

export interface WeaveLangReleaseAssuranceReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; run_id: string; release_id: string; disposition: "passed" | "blocked" | "unknown"; artifact_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateWeaveLangReleaseAssuranceReceipt(receipt: WeaveLangReleaseAssuranceReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID || receipt.contract_version !== WEAVELANG_RELEASE_ASSURANCE_CONTRACT_VERSION) throw new Error("WeaveLang release assurance schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.run_id.trim() || !receipt.release_id.trim() || !receipt.checks.length) throw new Error("WeaveLang release assurance identity or checks are incomplete"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("WeaveLang release assurance disposition is unknown"); if (receipt.artifact_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.artifact_digest)) throw new Error("WeaveLang release artifact digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("WeaveLang release receipt digest is invalid"); }
export function weaveLangReleaseAssuranceReceiptDigest(receipt: WeaveLangReleaseAssuranceReceipt): string { validateWeaveLangReleaseAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface MechanismControlPlaneReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; federation_id: string; question_id: string; admitted_candidate_ids: string[]; disposition: "passed" | "blocked" | "unknown"; evidence_receipt_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateMechanismControlPlaneReceipt(receipt: MechanismControlPlaneReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MECHANISM_CONTROL_PLANE_FEATURE_ID || receipt.contract_version !== MECHANISM_CONTROL_PLANE_CONTRACT_VERSION) throw new Error("mechanism control-plane schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.question_id.trim() || !receipt.admitted_candidate_ids.length || !receipt.checks.length) throw new Error("mechanism control-plane identity or checks are incomplete"); if (JSON.stringify([...new Set(receipt.admitted_candidate_ids)].sort()) !== JSON.stringify(receipt.admitted_candidate_ids)) throw new Error("mechanism candidate ordering is invalid"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("mechanism control-plane disposition is unknown"); if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("mechanism evidence digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("mechanism receipt digest is invalid"); }
export function mechanismControlPlaneReceiptDigest(receipt: MechanismControlPlaneReceipt): string { validateMechanismControlPlaneReceipt(receipt); return digestJsonSync(receipt); }

export interface MechanismGatewayReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; federation_id: string; source_profile: string; target_profile: string; projected_candidate_ids: string[]; interoperability_profile: string; disposition: "passed" | "blocked" | "unknown"; projection_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateMechanismGatewayReceipt(receipt: MechanismGatewayReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MECHANISM_GATEWAY_FEATURE_ID || receipt.contract_version !== MECHANISM_GATEWAY_CONTRACT_VERSION) throw new Error("mechanism gateway schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.source_profile.trim() || !receipt.target_profile.trim() || !receipt.interoperability_profile.trim() || !receipt.projected_candidate_ids.length || !receipt.checks.length) throw new Error("mechanism gateway identity or checks are incomplete"); if (JSON.stringify([...new Set(receipt.projected_candidate_ids)].sort()) !== JSON.stringify(receipt.projected_candidate_ids)) throw new Error("mechanism gateway candidate ordering is invalid"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("mechanism gateway disposition is unknown"); if (receipt.projection_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.projection_digest)) throw new Error("mechanism gateway projection digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("mechanism gateway receipt digest is invalid"); }
export function mechanismGatewayReceiptDigest(receipt: MechanismGatewayReceipt): string { validateMechanismGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface EvidenceSurveillanceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  study_id: string;
  intent: string;
  selected_source_ids: string[];
  disposition: "passed" | "blocked" | "unknown";
  qualified_set: Record<string, unknown>;
  effect_receipts: readonly Record<string, unknown>[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateEvidenceSurveillanceReceipt(receipt: EvidenceSurveillanceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("evidence surveillance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.intent.trim() || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("evidence surveillance identity or checks are incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("evidence surveillance disposition is unknown");
  if (JSON.stringify(receipt.qualified_set.selected_source_ids) !== JSON.stringify(receipt.selected_source_ids) || receipt.qualified_set.study_id !== receipt.study_id || receipt.qualified_set.intent !== receipt.intent) throw new Error("qualified evidence set is not linked to its receipt");
  if (receipt.qualified_set.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.qualified_set.boundary !== PRECLINICAL_BOUNDARY || receipt.qualified_set.ordering_rule !== "relevance_score descending, source_id ascending") throw new Error("qualified evidence set schema, boundary, or ordering is invalid");
  if (new Set(receipt.selected_source_ids).size !== receipt.selected_source_ids.length) throw new Error("qualified evidence source identities are not unique");
  if (receipt.qualified_set.evidence_state === "proven" && (receipt.omissions.length || receipt.uncertainty.length)) throw new Error("proven evidence cannot contain unresolved omissions");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("evidence surveillance artifact digest is invalid");
  for (const effect of receipt.effect_receipts) if (effect.effect !== "read_local_data" || typeof effect.authorized !== "boolean" || typeof effect.reason !== "string" || typeof effect.receipt_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.receipt_digest)) throw new Error("evidence surveillance effect receipt is invalid");
}

export function evidenceSurveillanceReceiptDigest(receipt: EvidenceSurveillanceReceipt): string { validateEvidenceSurveillanceReceipt(receipt); return digestJsonSync(receipt); }

export interface RetrievalSynthesisReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  request_id: string;
  query_id: string;
  disposition: "passed" | "blocked" | "unknown";
  synthesis: Record<string, unknown>;
  effect_receipts: readonly Record<string, unknown>[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  artifact: Record<string, unknown>;
  boundary: string;
}

export function validateRetrievalSynthesisReceipt(receipt: RetrievalSynthesisReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RETRIEVAL_SYNTHESIS_FEATURE_ID || receipt.contract_version !== RETRIEVAL_SYNTHESIS_CONTRACT_VERSION) throw new Error("retrieval synthesis schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.query_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("retrieval synthesis identity or checks are incomplete");
  if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("retrieval synthesis disposition is unknown");
  if (receipt.synthesis.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.synthesis.query_id !== receipt.query_id || receipt.synthesis.boundary !== PRECLINICAL_BOUNDARY || typeof receipt.synthesis.comparability_profile !== "string" || !receipt.synthesis.comparability_profile.trim()) throw new Error("retrieval synthesis linkage or boundary is invalid");
  if (JSON.stringify(receipt.synthesis.omissions) !== JSON.stringify(receipt.omissions) || JSON.stringify(receipt.synthesis.uncertainty) !== JSON.stringify(receipt.uncertainty)) throw new Error("retrieval synthesis omission linkage is invalid");
  if (!Array.isArray(receipt.synthesis.selected_evidence_ids) || new Set(receipt.synthesis.selected_evidence_ids).size !== receipt.synthesis.selected_evidence_ids.length || receipt.synthesis.selected_evidence_ids.length !== receipt.synthesis.selected_digests.length || receipt.synthesis.selected_evidence_ids.length !== receipt.synthesis.selected_modalities.length) throw new Error("retrieval synthesis selected evidence alignment is invalid");
  if (receipt.synthesis.evidence_state === "proven" && (receipt.omissions.length || receipt.uncertainty.length)) throw new Error("proven synthesis cannot contain unresolved omissions");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("retrieval synthesis artifact digest is invalid");
  for (const effect of receipt.effect_receipts) if (effect.effect !== "read_local_data" || typeof effect.authorized !== "boolean" || typeof effect.reason !== "string" || typeof effect.receipt_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.receipt_digest)) throw new Error("retrieval synthesis effect receipt is invalid");
}

export function retrievalSynthesisReceiptDigest(receipt: RetrievalSynthesisReceipt): string { validateRetrievalSynthesisReceipt(receipt); return digestJsonSync(receipt); }

export interface AdapterContextCompilationReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; query_id: string; resolved_fact_ids: string[]; disposition: "passed" | "blocked" | "unknown"; evidence_receipt_digest: string | null; checks: string[]; omissions: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateAdapterContextCompilationReceipt(receipt: AdapterContextCompilationReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ADAPTER_CONTEXT_COMPILATION_FEATURE_ID || receipt.contract_version !== ADAPTER_CONTEXT_COMPILATION_CONTRACT_VERSION) throw new Error("adapter context compilation schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.query_id.trim() || !receipt.checks.length) throw new Error("adapter context compilation identity or checks are incomplete"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("adapter context compilation disposition is unknown"); if (!receipt.resolved_fact_ids.length || new Set(receipt.resolved_fact_ids).size !== receipt.resolved_fact_ids.length) throw new Error("resolved decision fact identities are invalid"); if (receipt.evidence_receipt_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.evidence_receipt_digest)) throw new Error("adapter context evidence digest is invalid"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adapter context artifact digest is invalid"); }
export function adapterContextCompilationReceiptDigest(receipt: AdapterContextCompilationReceipt): string { validateAdapterContextCompilationReceipt(receipt); return digestJsonSync(receipt); }

export interface KnowledgeWorkflowReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; workflow_id: string; disposition: "passed" | "blocked" | "unknown"; world: Record<string, unknown>; checks: string[]; omissions: string[]; uncertainty: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateKnowledgeWorkflowReceipt(receipt: KnowledgeWorkflowReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== KNOWLEDGE_WORKFLOW_FEATURE_ID || receipt.contract_version !== KNOWLEDGE_WORKFLOW_CONTRACT_VERSION) throw new Error("knowledge workflow schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.checks.length) throw new Error("knowledge workflow identity or checks are incomplete"); if (!new Set(["passed", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("knowledge workflow disposition is unknown"); if (receipt.world.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.world.workflow_id !== receipt.workflow_id || receipt.world.boundary !== PRECLINICAL_BOUNDARY || !Array.isArray(receipt.world.study_ids) || !receipt.world.study_ids.length || !Array.isArray(receipt.world.stages) || !receipt.world.stages.length) throw new Error("typed knowledge world linkage is invalid"); if (JSON.stringify(receipt.world.omissions) !== JSON.stringify(receipt.omissions) || JSON.stringify(receipt.world.uncertainty) !== JSON.stringify(receipt.uncertainty)) throw new Error("knowledge workflow omission linkage is invalid"); if (!Array.isArray(receipt.world.resolved_claim_ids) || new Set(receipt.world.resolved_claim_ids).size !== receipt.world.resolved_claim_ids.length) throw new Error("typed knowledge claim identities are not unique"); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("knowledge workflow artifact digest is invalid"); }
export function knowledgeWorkflowReceiptDigest(receipt: KnowledgeWorkflowReceipt): string { validateKnowledgeWorkflowReceipt(receipt); return digestJsonSync(receipt); }

export interface ResourceWorkbenchReceipt { schema_version: string; feature_id: string; contract_version: string; request_id: string; need_id: string; disposition: "qualified" | "partial" | "blocked" | "unknown"; qualified_resources: readonly Record<string, unknown>[]; omissions: readonly Record<string, unknown>[]; checks: string[]; artifact: Record<string, unknown>; boundary: string; }
export function validateResourceWorkbenchReceipt(receipt: ResourceWorkbenchReceipt): void { if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESOURCE_WORKBENCH_FEATURE_ID || receipt.contract_version !== RESOURCE_WORKBENCH_CONTRACT_VERSION) throw new Error("resource workbench schema, feature, or version mismatch"); if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.request_id.trim() || !receipt.need_id.trim() || !receipt.checks.length) throw new Error("resource workbench identity or checks are incomplete"); if (!new Set(["qualified", "partial", "blocked", "unknown"]).has(receipt.disposition)) throw new Error("resource workbench disposition is unknown"); receipt.qualified_resources.forEach((item, index) => { if (item.rank !== index + 1 || typeof item.resource_id !== "string" || !item.resource_id.trim() || typeof item.origin !== "string" || !item.origin.trim() || !Array.isArray(item.reasons) || !item.reasons.length || typeof item.artifact_digest !== "string" || !/^[0-9a-f]{64}$/.test(item.artifact_digest)) throw new Error("qualified resource ranking, reasons, or digest is invalid"); }); receipt.omissions.forEach((item) => { if (typeof item.resource_id !== "string" || !item.resource_id.trim() || typeof item.reason !== "string" || !item.reason.trim()) throw new Error("resource omission is incomplete"); }); if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("resource workbench artifact digest is invalid"); }
export function resourceWorkbenchReceiptDigest(receipt: ResourceWorkbenchReceipt): string { validateResourceWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface IngestionGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  study_id: string;
  disposition: "admitted" | "partial" | "blocked";
  harmonized: Record<string, unknown>;
  admitted_bundles: string[];
  omitted_bundles: string[];
  effect_receipts: readonly Record<string, unknown>[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateIngestionGatewayReceipt(receipt: IngestionGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INGESTION_GATEWAY_FEATURE_ID || receipt.contract_version !== INGESTION_GATEWAY_CONTRACT_VERSION) throw new Error("ingestion gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.reasons.length) throw new Error("ingestion gateway identity, locality, or reasons are incomplete");
  if (!new Set(["admitted", "partial", "blocked"]).has(receipt.disposition)) throw new Error("ingestion gateway disposition is unknown");
  if (receipt.harmonized.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.harmonized.study_id !== receipt.study_id || receipt.harmonized.boundary !== PRECLINICAL_BOUNDARY) throw new Error("harmonized research object linkage is invalid");
  if (new Set(receipt.admitted_bundles).size !== receipt.admitted_bundles.length || new Set(receipt.omitted_bundles).size !== receipt.omitted_bundles.length) throw new Error("ingestion gateway bundle identities are not unique");
  if (receipt.disposition === "blocked" && receipt.effect_receipts.length) throw new Error("blocked gateway receipts cannot contain effects");
  if (receipt.effect_receipts.length !== receipt.admitted_bundles.length) throw new Error("each admitted bundle needs one effect receipt");
  for (const effect of receipt.effect_receipts) if (effect.action !== "admit-local-harmonization" || effect.authorized !== true || !receipt.admitted_bundles.includes(String(effect.bundle_id)) || typeof effect.source_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.source_digest)) throw new Error("ingestion gateway effect receipt is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("ingestion gateway artifact digest is invalid");
}

export function ingestionGatewayReceiptDigest(receipt: IngestionGatewayReceipt): string { validateIngestionGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface QualityEnvelopeReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  envelope_id: string;
  reference_schema: string;
  comparability_profile: string;
  decision: "qualified" | "partial" | "blocked" | "unknown";
  study_order: string[];
  modality_coverage: Record<string, number>;
  verdicts: readonly Record<string, unknown>[];
  omitted_modalities: string[];
  comparability_conflicts: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualityEnvelopeReceipt(receipt: QualityEnvelopeReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== QUALITY_ENVELOPE_FEATURE_ID || receipt.contract_version !== QUALITY_ENVELOPE_CONTRACT_VERSION) throw new Error("quality envelope schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.envelope_id.trim() || !receipt.reference_schema.trim() || !receipt.comparability_profile.trim() || !receipt.reasons.length) throw new Error("quality envelope identity, locality, profile, or reasons are incomplete");
  if (!new Set(["qualified", "partial", "blocked", "unknown"]).has(receipt.decision)) throw new Error("quality envelope decision is unknown");
  if (!receipt.study_order.length || JSON.stringify([...new Set(receipt.study_order)].sort()) !== JSON.stringify(receipt.study_order) || receipt.verdicts.length !== receipt.study_order.length) throw new Error("quality envelope study ordering is invalid");
  receipt.verdicts.forEach((verdict, index) => { if (verdict.study_id !== receipt.study_order[index] || typeof verdict.modality !== "string" || !verdict.modality.trim() || !new Set(["pass", "pass_with_warnings", "blocked", "unknown"]).has(verdict.quality_disposition) || typeof verdict.comparable !== "boolean" || !Array.isArray(verdict.reasons) || !verdict.reasons.length) throw new Error("quality envelope study verdict linkage is invalid"); });
  for (const [modality, count] of Object.entries(receipt.modality_coverage)) if (!modality.trim() || !Number.isInteger(count) || count < 0) throw new Error("quality envelope modality coverage is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("quality envelope artifact digest is invalid");
}

export function qualityEnvelopeReceiptDigest(receipt: QualityEnvelopeReceipt): string { validateQualityEnvelopeReceipt(receipt); return digestJsonSync(receipt); }

export interface ExperimentDesignReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  objective_id: string;
  decision: "admitted" | "partial" | "blocked";
  site_order: string[];
  assignments: readonly Record<string, unknown>[];
  modality_coverage: Record<string, number>;
  omitted_modalities: string[];
  comparability_conflicts: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateExperimentDesignReceipt(receipt: ExperimentDesignReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EXPERIMENT_DESIGN_CONTROL_FEATURE_ID || receipt.contract_version !== EXPERIMENT_DESIGN_CONTROL_CONTRACT_VERSION) throw new Error("experiment design schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.objective_id.trim() || !receipt.reasons.length) throw new Error("experiment design identity, locality, or reasons are incomplete");
  if (!new Set(["admitted", "partial", "blocked"]).has(receipt.decision)) throw new Error("experiment design decision is unknown");
  if (!receipt.site_order.length || JSON.stringify([...new Set(receipt.site_order)].sort()) !== JSON.stringify(receipt.site_order)) throw new Error("experiment design site ordering is invalid");
  if (receipt.decision === "blocked" && receipt.assignments.length) throw new Error("blocked experiment design cannot contain assignments");
  for (const assignment of receipt.assignments) if (typeof assignment.site_id !== "string" || !assignment.site_id.trim() || typeof assignment.modality !== "string" || !assignment.modality.trim() || typeof assignment.instrument_profile !== "string" || !assignment.instrument_profile.trim() || assignment.authorized !== true || typeof assignment.budget !== "number" || !Number.isFinite(assignment.budget)) throw new Error("experiment design assignment is invalid");
  for (const [modality, count] of Object.entries(receipt.modality_coverage)) if (!modality.trim() || !Number.isInteger(count) || count < 0) throw new Error("experiment design modality coverage is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("experiment design artifact digest is invalid");
}

export function experimentDesignReceiptDigest(receipt: ExperimentDesignReceipt): string { validateExperimentDesignReceipt(receipt); return digestJsonSync(receipt); }

export interface ProtocolSimulationReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  protocol_id: string;
  design_digest: string;
  results: readonly Record<string, unknown>[];
  passed: number;
  failed_closed: number;
  approval_required: number;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateProtocolSimulationReceipt(receipt: ProtocolSimulationReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== PROTOCOL_SIMULATION_FEATURE_ID || receipt.contract_version !== PROTOCOL_SIMULATION_CONTRACT_VERSION) throw new Error("protocol simulation schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.protocol_id.trim() || !/^[0-9a-f]{64}$/.test(receipt.design_digest) || !receipt.results.length) throw new Error("protocol simulation identity, digest, locality, or results are incomplete");
  if (receipt.passed + receipt.failed_closed + receipt.approval_required !== receipt.results.length) throw new Error("protocol simulation state counts do not match results");
  const ids = receipt.results.map((result) => String(result.scenario_id ?? ""));
  if (ids.some((id) => !id.trim()) || JSON.stringify(ids) !== JSON.stringify([...new Set(ids)].sort())) throw new Error("protocol simulation scenario ordering is invalid");
  for (const result of receipt.results) if (!new Set(["passed", "failed_closed", "approval_required"]).has(result.state) || !Array.isArray(result.reasons) || !result.reasons.length) throw new Error("protocol simulation scenario result is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("protocol simulation artifact digest is invalid");
}

export function protocolSimulationReceiptDigest(receipt: ProtocolSimulationReceipt): string { validateProtocolSimulationReceipt(receipt); return digestJsonSync(receipt); }

export interface InstrumentMeshReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  federation_id: string;
  action_id: string;
  decision: "admitted" | "approval_required" | "blocked" | "unknown";
  candidate_order: string[];
  selected_instrument_id: string | null;
  selected_site_id: string | null;
  selected_protocol_profile: string | null;
  satisfied_capabilities: string[];
  missing_capabilities: string[];
  missing_interlocks: string[];
  effect: Record<string, unknown> | null;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateInstrumentMeshReceipt(receipt: InstrumentMeshReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INSTRUMENT_MESH_FEATURE_ID || receipt.contract_version !== INSTRUMENT_MESH_CONTRACT_VERSION) throw new Error("instrument mesh schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.action_id.trim() || !receipt.reasons.length) throw new Error("instrument mesh identity, locality, boundary, or reasons are incomplete");
  if (!new Set(["admitted", "approval_required", "blocked", "unknown"]).has(receipt.decision)) throw new Error("instrument mesh decision is unknown");
  if (JSON.stringify([...new Set(receipt.candidate_order)].sort()) !== JSON.stringify(receipt.candidate_order)) throw new Error("instrument mesh candidate ordering is invalid");
  if (receipt.missing_capabilities.some((item) => !item.trim()) || receipt.missing_interlocks.some((item) => !item.trim())) throw new Error("instrument mesh missing capability or interlock is empty");
  if (receipt.decision === "admitted") {
    if (!receipt.selected_instrument_id || !receipt.selected_site_id || !receipt.effect) throw new Error("admitted instrument mesh receipt needs selection and effect receipt");
    if (receipt.effect.authorized !== true || receipt.effect.executed !== false || receipt.effect.raw_data_local !== true) throw new Error("instrument mesh effect must be authorized, not executed, and local");
  } else if (receipt.effect !== null) throw new Error("non-admitted instrument mesh receipt cannot contain an effect");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("instrument mesh artifact digest is invalid");
}

export function instrumentMeshReceiptDigest(receipt: InstrumentMeshReceipt): string { validateInstrumentMeshReceipt(receipt); return digestJsonSync(receipt); }

export interface ComputationalExecutionReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  run_id: string;
  decision: "dry_run" | "admitted" | "approval_required" | "blocked";
  ordered_nodes: string[];
  admitted_nodes: string[];
  run: Record<string, unknown>;
  run_digest: string;
  authorized_effects: readonly Record<string, unknown>[];
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  effects_executed: boolean;
  raw_data_local: boolean;
  boundary: string;
}

export function validateComputationalExecutionReceipt(receipt: ComputationalExecutionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EXECUTION_CONTROL_FEATURE_ID || receipt.contract_version !== EXECUTION_CONTROL_CONTRACT_VERSION) throw new Error("computational execution schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || receipt.effects_executed || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.run_id.trim() || !receipt.ordered_nodes.length || !receipt.reasons.length) throw new Error("computational execution identity, locality, non-execution, graph, or reasons are incomplete");
  if (!new Set(["dry_run", "admitted", "approval_required", "blocked"]).has(receipt.decision)) throw new Error("computational execution decision is unknown");
  if (new Set(receipt.ordered_nodes).size !== receipt.ordered_nodes.length || new Set(receipt.admitted_nodes).size !== receipt.admitted_nodes.length || receipt.admitted_nodes.some((node) => !receipt.ordered_nodes.includes(node))) throw new Error("computational execution node identities are invalid");
  if (receipt.run.workflow_id !== receipt.workflow_id || receipt.run.status !== "planned") throw new Error("execution run linkage or planned status is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.run_digest)) throw new Error("computational execution run digest is invalid");
  if (receipt.decision === "admitted" && receipt.authorized_effects.length !== receipt.admitted_nodes.length) throw new Error("every admitted node needs an authorized effect");
  if (receipt.decision !== "admitted" && receipt.authorized_effects.length) throw new Error("non-admitted execution cannot contain effects");
  for (const effect of receipt.authorized_effects) if (effect.effect !== "execute_local_computation" || effect.authorized !== true || effect.executed !== false || typeof effect.payload_digest !== "string" || !/^[0-9a-f]{64}$/.test(effect.payload_digest)) throw new Error("computational execution effect receipt is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("computational execution artifact digest is invalid");
}

export function computationalExecutionReceiptDigest(receipt: ComputationalExecutionReceipt): string { validateComputationalExecutionReceipt(receipt); return digestJsonSync(receipt); }

export interface AnalysisPortfolioReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  question_id: string;
  estimand: string;
  verdict: "qualified" | "conditional" | "blocked";
  selected_candidate: string | null;
  candidate_order: string[];
  uncertainty: string[];
  omissions: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateAnalysisPortfolioReceipt(receipt: AnalysisPortfolioReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ANALYSIS_PORTFOLIO_FEATURE_ID || receipt.contract_version !== ANALYSIS_PORTFOLIO_CONTRACT_VERSION) throw new Error("analysis portfolio schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.question_id.trim() || !receipt.estimand.trim() || !receipt.candidate_order.length || !receipt.reasons.length) throw new Error("analysis portfolio identity, candidates, locality, boundary, or reasons are incomplete");
  if (!new Set(["qualified", "conditional", "blocked"]).has(receipt.verdict)) throw new Error("analysis portfolio verdict is unknown");
  if (JSON.stringify([...new Set(receipt.candidate_order)].sort()) !== JSON.stringify(receipt.candidate_order)) throw new Error("analysis portfolio candidate ordering is invalid");
  if (receipt.verdict === "qualified" && !receipt.selected_candidate) throw new Error("qualified analysis portfolio needs a selected candidate");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("analysis portfolio artifact digest is invalid");
}

export function analysisPortfolioReceiptDigest(receipt: AnalysisPortfolioReceipt): string { validateAnalysisPortfolioReceipt(receipt); return digestJsonSync(receipt); }

export interface InterpretationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  result_id: string;
  verdict: "qualified" | "conditional" | "blocked";
  claim_order: string[];
  covered_modalities: string[];
  omitted_modalities: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateInterpretationAssuranceReceipt(receipt: InterpretationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INTERPRETATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== INTERPRETATION_ASSURANCE_CONTRACT_VERSION) throw new Error("interpretation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.result_id.trim() || !receipt.claim_order.length || !receipt.reasons.length) throw new Error("interpretation assurance identity, claims, locality, boundary, or reasons are incomplete");
  if (!new Set(["qualified", "conditional", "blocked"]).has(receipt.verdict)) throw new Error("interpretation assurance verdict is unknown");
  if (JSON.stringify([...new Set(receipt.claim_order)].sort()) !== JSON.stringify(receipt.claim_order)) throw new Error("interpretation assurance claim ordering is invalid");
  if (receipt.verdict === "qualified" && receipt.omitted_modalities.length) throw new Error("qualified interpretation cannot omit required modalities");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("interpretation assurance artifact digest is invalid");
}

export function interpretationAssuranceReceiptDigest(receipt: InterpretationAssuranceReceipt): string { validateInterpretationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ReplicationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  claim_id: string;
  protocol_digest: string;
  verdict: "replicated" | "partially_replicated" | "contradicted" | "null_result" | "insufficient_evidence" | "blocked";
  observation_order: string[];
  independent_site_order: string[];
  positive_count: number;
  null_count: number;
  negative_count: number;
  inconclusive_count: number;
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateReplicationAssuranceReceipt(receipt: ReplicationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== REPLICATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== REPLICATION_ASSURANCE_CONTRACT_VERSION) throw new Error("replication assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.claim_id.trim() || !receipt.observation_order.length || !receipt.reasons.length) throw new Error("replication assurance identity, observations, locality, boundary, or reasons are incomplete");
  if (!new Set(["replicated", "partially_replicated", "contradicted", "null_result", "insufficient_evidence", "blocked"]).has(receipt.verdict)) throw new Error("replication assurance verdict is unknown");
  if (JSON.stringify([...new Set(receipt.observation_order)].sort()) !== JSON.stringify(receipt.observation_order) || JSON.stringify([...new Set(receipt.independent_site_order)].sort()) !== JSON.stringify(receipt.independent_site_order)) throw new Error("replication assurance ordering is invalid");
  if (![receipt.positive_count, receipt.null_count, receipt.negative_count, receipt.inconclusive_count].every((value) => Number.isInteger(value) && value >= 0) || receipt.positive_count + receipt.null_count + receipt.negative_count + receipt.inconclusive_count !== receipt.observation_order.length) throw new Error("replication assurance counts do not match observations");
  if (!/^[0-9a-f]{64}$/.test(receipt.protocol_digest)) throw new Error("replication assurance protocol digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("replication assurance artifact digest is invalid");
}

export function replicationAssuranceReceiptDigest(receipt: ReplicationAssuranceReceipt): string { validateReplicationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ReleaseAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  run_id: string;
  release_id: string;
  verdict: "released" | "conditional" | "incomplete" | "incomparable" | "blocked";
  study_order: string[];
  modality_order: string[];
  artifact_order: string[];
  evidence_receipt_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  policy_decision: "allow" | "deny" | "redact" | "local_only" | "approval_required" | "unresolved";
  effect_receipt: string;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateReleaseAssuranceReceipt(receipt: ReleaseAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RELEASE_ASSURANCE_FEATURE_ID || receipt.contract_version !== RELEASE_ASSURANCE_CONTRACT_VERSION) throw new Error("release assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.run_id.trim() || !receipt.release_id.trim() || !receipt.study_order.length || !receipt.evidence_receipt_order.length || !receipt.reasons.length || !receipt.effect_receipt.trim()) throw new Error("release assurance identity, studies, evidence, locality, boundary, or effects are incomplete");
  if (!new Set(["released", "conditional", "incomplete", "incomparable", "blocked"]).has(receipt.verdict)) throw new Error("release assurance verdict is unknown");
  for (const values of [receipt.study_order, receipt.modality_order, receipt.artifact_order, receipt.evidence_receipt_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("release assurance ordering is invalid");
  if (!new Set(["allow", "deny", "redact", "local_only", "approval_required", "unresolved"]).has(receipt.policy_decision)) throw new Error("release assurance policy decision is unknown");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("release assurance artifact digest is invalid");
}

export function releaseAssuranceReceiptDigest(receipt: ReleaseAssuranceReceipt): string { validateReleaseAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface DeterminismGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  capability_id: string;
  endpoint_id: string;
  negotiated_version: string;
  verdict: "accepted" | "migrated" | "approval_required" | "incompatible" | "blocked";
  canonical_field_order: string[];
  canonical_input_digest: string;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  effect_receipt: string;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateDeterminismGatewayReceipt(receipt: DeterminismGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== DETERMINISM_GATEWAY_FEATURE_ID || receipt.contract_version !== DETERMINISM_GATEWAY_CONTRACT_VERSION) throw new Error("typed determinism schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.capability_id.trim() || !receipt.endpoint_id.trim() || !receipt.canonical_field_order.length || !receipt.reasons.length || !receipt.effect_receipt.trim()) throw new Error("typed determinism identity, fields, locality, boundary, reasons, or effects are incomplete");
  if (!new Set(["accepted", "migrated", "approval_required", "incompatible", "blocked"]).has(receipt.verdict)) throw new Error("typed determinism verdict is unknown");
  if (JSON.stringify([...new Set(receipt.canonical_field_order)].sort()) !== JSON.stringify(receipt.canonical_field_order)) throw new Error("typed determinism field order is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.canonical_input_digest)) throw new Error("typed determinism input digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("typed determinism artifact digest is invalid");
}

export function determinismGatewayReceiptDigest(receipt: DeterminismGatewayReceipt): string { validateDeterminismGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface ProvenanceAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  envelope_id: string;
  root_artifact_id: string;
  root_digest: string;
  verdict: "signed" | "conditional" | "unresolved" | "contradicted" | "blocked";
  lineage_order: string[];
  derivation_order: string[];
  study_order: string[];
  modality_order: string[];
  tool_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  signer_public_key_hex: string;
  signer_signature_hex: string;
  effect_receipt: string;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateProvenanceAssuranceReceipt(receipt: ProvenanceAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== PROVENANCE_ASSURANCE_FEATURE_ID || receipt.contract_version !== PROVENANCE_ASSURANCE_CONTRACT_VERSION) throw new Error("provenance assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.envelope_id.trim() || !receipt.root_artifact_id.trim() || !receipt.lineage_order.length || !receipt.derivation_order.length || !receipt.reasons.length || !receipt.effect_receipt.trim()) throw new Error("provenance identity, lineage, derivations, locality, boundary, reasons, or effects are incomplete");
  if (!new Set(["signed", "conditional", "unresolved", "contradicted", "blocked"]).has(receipt.verdict)) throw new Error("provenance assurance verdict is unknown");
  for (const values of [receipt.lineage_order, receipt.derivation_order, receipt.study_order, receipt.modality_order, receipt.tool_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("provenance assurance ordering is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.root_digest) || receipt.tool_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("provenance digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("provenance artifact digest is invalid");
}

export function provenanceAssuranceReceiptDigest(receipt: ProvenanceAssuranceReceipt): string { validateProvenanceAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface PolicyGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  action_id: string;
  decision: "allowed" | "approval_required" | "local_only" | "denied" | "unresolved";
  required_tier: "a0" | "a1" | "a2" | "a3" | "a4";
  permitted_actions: string[];
  budget_order: string[];
  reasons: string[];
  uncertainty: string[];
  effect_receipt: string;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validatePolicyGatewayReceipt(receipt: PolicyGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== POLICY_GATEWAY_FEATURE_ID || receipt.contract_version !== POLICY_GATEWAY_CONTRACT_VERSION) throw new Error("policy gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.action_id.trim() || !receipt.permitted_actions.length || !receipt.budget_order.length || !receipt.reasons.length || !receipt.effect_receipt.trim()) throw new Error("policy gateway identity, action, grant, budget, locality, boundary, reasons, or effects are incomplete");
  if (!new Set(["allowed", "approval_required", "local_only", "denied", "unresolved"]).has(receipt.decision)) throw new Error("policy gateway decision is unknown");
  if (JSON.stringify([...new Set(receipt.permitted_actions)].sort()) !== JSON.stringify(receipt.permitted_actions) || JSON.stringify([...new Set(receipt.budget_order)].sort()) !== JSON.stringify(receipt.budget_order)) throw new Error("policy gateway ordering is invalid");
  if (!new Set(["a0", "a1", "a2", "a3", "a4"]).has(receipt.required_tier)) throw new Error("policy gateway autonomy tier is unknown");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("policy gateway artifact digest is invalid");
}

export function policyGatewayReceiptDigest(receipt: PolicyGatewayReceipt): string { validatePolicyGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface FederationWorkflowReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  workflow_id: string;
  decision: "scheduled" | "approval_required" | "local_only" | "partial" | "blocked";
  task_order: string[];
  checkpoint_order: string[];
  compensation_order: string[];
  total_budget_units: number;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  reasons: string[];
  effect_receipt: string;
  envelope: Record<string, unknown>;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederationWorkflowReceipt(receipt: FederationWorkflowReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATION_WORKFLOW_FEATURE_ID || receipt.contract_version !== FEDERATION_WORKFLOW_CONTRACT_VERSION) throw new Error("federation workflow schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.workflow_id.trim() || !receipt.task_order.length || !receipt.checkpoint_order.length || !receipt.compensation_order.length || !receipt.reasons.length || !receipt.effect_receipt.trim()) throw new Error("federation workflow identity, tasks, checkpoints, compensation, locality, boundary, reasons, or effects are incomplete");
  if (!new Set(["scheduled", "approval_required", "local_only", "partial", "blocked"]).has(receipt.decision)) throw new Error("federation workflow decision is unknown");
  for (const values of [receipt.task_order, receipt.checkpoint_order, receipt.compensation_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federation workflow ordering is invalid");
  if (!Number.isInteger(receipt.total_budget_units) || receipt.total_budget_units <= 0) throw new Error("federation workflow budget is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federation workflow artifact digest is invalid");
  if (!receipt.envelope || typeof receipt.envelope !== "object") throw new Error("federation workflow envelope is invalid");
}

export function federationWorkflowReceiptDigest(receipt: FederationWorkflowReceipt): string { validateFederationWorkflowReceipt(receipt); return digestJsonSync(receipt); }

export interface ReliabilityCopilotReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  workload_id: string;
  decision: "completed" | "dry_run" | "partial" | "degraded" | "blocked";
  invocation_order: string[];
  retry_order: string[];
  tool_order: string[];
  budget_used_units: number;
  timeout_order: string[];
  omissions: string[];
  uncertainty: string[];
  failure_reasons: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateReliabilityCopilotReceipt(receipt: ReliabilityCopilotReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RELIABILITY_COPILOT_FEATURE_ID || receipt.contract_version !== RELIABILITY_COPILOT_CONTRACT_VERSION) throw new Error("reliability copilot schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.workload_id.trim() || !receipt.invocation_order.length || !receipt.tool_order.length || !receipt.effect_receipts.length) throw new Error("reliability copilot identity, invocations, tools, effects, locality, or boundary are incomplete");
  if (!new Set(["completed", "dry_run", "partial", "degraded", "blocked"]).has(receipt.decision)) throw new Error("reliability copilot decision is unknown");
  for (const values of [receipt.invocation_order, receipt.retry_order, receipt.tool_order, receipt.timeout_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("reliability copilot ordering is invalid");
  if (!Number.isInteger(receipt.budget_used_units) || receipt.budget_used_units < 0) throw new Error("reliability copilot budget is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("reliability copilot artifact digest is invalid");
}

export function reliabilityCopilotReceiptDigest(receipt: ReliabilityCopilotReceipt): string { validateReliabilityCopilotReceipt(receipt); return digestJsonSync(receipt); }

export interface InteroperabilityGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  endpoint_id: string;
  negotiated_version: string;
  disposition: "accepted" | "migrated" | "approval_required" | "blocked" | "incompatible" | "unknown";
  capability_order: string[];
  artifact_digest_order: string[];
  replay_token: string;
  omissions: string[];
  uncertainty: string[];
  semantic_loss: readonly Record<string, unknown>[];
  checks: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateInteroperabilityGatewayReceipt(receipt: InteroperabilityGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INTEROPERABILITY_GATEWAY_FEATURE_ID || receipt.contract_version !== INTEROPERABILITY_GATEWAY_CONTRACT_VERSION) throw new Error("interoperability gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.endpoint_id.trim() || !receipt.negotiated_version.trim() || !receipt.capability_order.length || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("interoperability gateway identity, capabilities, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["accepted", "migrated", "approval_required", "blocked", "incompatible", "unknown"]).has(receipt.disposition)) throw new Error("interoperability gateway disposition is unknown");
  for (const values of [receipt.capability_order, receipt.artifact_digest_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("interoperability gateway ordering is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.replay_token) || receipt.artifact_digest_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("interoperability gateway digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("interoperability gateway artifact digest is invalid");
}

export function interoperabilityGatewayReceiptDigest(receipt: InteroperabilityGatewayReceipt): string { validateInteroperabilityGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface EvaluationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  run_id: string;
  capability_id: string;
  benchmark_id: string;
  baseline_id: string;
  verdict: "passed" | "conditional" | "unknown" | "blocked";
  metric_order: string[];
  gate_order: string[];
  witness_order: string[];
  counterexample_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  reasons: string[];
  effect_receipts: string[];
  replay_identity: string;
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateEvaluationAssuranceReceipt(receipt: EvaluationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVALUATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== EVALUATION_ASSURANCE_CONTRACT_VERSION) throw new Error("evaluation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.run_id.trim() || !receipt.capability_id.trim() || !receipt.benchmark_id.trim() || !receipt.baseline_id.trim() || !receipt.metric_order.length || !receipt.gate_order.length || !receipt.reasons.length || !receipt.effect_receipts.length) throw new Error("evaluation assurance identity, metrics, gates, reasons, effects, locality, or boundary are incomplete");
  if (!new Set(["passed", "conditional", "unknown", "blocked"]).has(receipt.verdict)) throw new Error("evaluation assurance verdict is unknown");
  for (const values of [receipt.metric_order, receipt.gate_order, receipt.witness_order, receipt.counterexample_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("evaluation assurance ordering is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.replay_identity)) throw new Error("evaluation assurance replay identity is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("evaluation assurance artifact digest is invalid");
}

export function evaluationAssuranceReceiptDigest(receipt: EvaluationAssuranceReceipt): string { validateEvaluationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ResearchWorkbenchReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  workspace_id: string;
  disposition: "ready" | "partial" | "blocked" | "local_only";
  study_order: string[];
  modality_order: string[];
  view_order: string[];
  panel_order: string[];
  artifact_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  action_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateResearchWorkbenchReceipt(receipt: ResearchWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RESEARCH_WORKBENCH_FEATURE_ID || receipt.contract_version !== RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("research workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.workspace_id.trim() || !receipt.study_order.length || !receipt.view_order.length || !receipt.panel_order.length || !receipt.action_receipts.length) throw new Error("research workbench identity, studies, views, panels, actions, locality, or boundary are incomplete");
  if (!new Set(["ready", "partial", "blocked", "local_only"]).has(receipt.disposition)) throw new Error("research workbench disposition is unknown");
  for (const values of [receipt.study_order, receipt.modality_order, receipt.view_order, receipt.panel_order, receipt.artifact_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("research workbench ordering is invalid");
  if (receipt.artifact_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("research workbench artifact digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("research workbench receipt digest is invalid");
}

export function researchWorkbenchReceiptDigest(receipt: ResearchWorkbenchReceipt): string { validateResearchWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface ContractFrontierReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  adapter_id: string;
  capability_id: string;
  negotiated_version: string;
  disposition: "accepted" | "migrated" | "approval_required" | "blocked" | "incompatible" | "unknown";
  input_schema: string;
  output_schema: string;
  modality_order: string[];
  effect_order: string[];
  permission_order: string[];
  artifact_digest_order: string[];
  omissions: string[];
  uncertainty: string[];
  semantic_loss: string[];
  checks: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateContractFrontierReceipt(receipt: ContractFrontierReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== CONTRACT_FRONTIER_FEATURE_ID || receipt.contract_version !== CONTRACT_FRONTIER_CONTRACT_VERSION) throw new Error("adapter contract frontier schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.adapter_id.trim() || !receipt.capability_id.trim() || !receipt.negotiated_version.trim() || !receipt.input_schema.trim() || !receipt.output_schema.trim() || !receipt.modality_order.length || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("adapter contract frontier identity, schemas, modalities, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["accepted", "migrated", "approval_required", "blocked", "incompatible", "unknown"]).has(receipt.disposition)) throw new Error("adapter contract frontier disposition is unknown");
  for (const values of [receipt.modality_order, receipt.effect_order, receipt.permission_order, receipt.artifact_digest_order, receipt.semantic_loss, receipt.checks, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("adapter contract frontier ordering is invalid");
  if (receipt.artifact_digest_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("adapter contract frontier artifact digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adapter contract frontier receipt digest is invalid");
}

export function contractFrontierReceiptDigest(receipt: ContractFrontierReceipt): string { validateContractFrontierReceipt(receipt); return digestJsonSync(receipt); }

export interface LimitationClosureReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  disposition: "closed" | "partial" | "unknown" | "blocked";
  case_order: string[];
  resolved_order: string[];
  unresolved_order: string[];
  evidence_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  reasons: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateLimitationClosureReceipt(receipt: LimitationClosureReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== LIMITATION_CLOSURE_FEATURE_ID || receipt.contract_version !== LIMITATION_CLOSURE_CONTRACT_VERSION) throw new Error("limitation closure schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.case_order.length || !receipt.reasons.length || !receipt.effect_receipts.length) throw new Error("limitation closure identity, cases, reasons, effects, locality, or boundary are incomplete");
  if (!new Set(["closed", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("limitation closure disposition is unknown");
  for (const values of [receipt.case_order, receipt.resolved_order, receipt.unresolved_order, receipt.evidence_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.reasons, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("limitation closure ordering is invalid");
  if (receipt.evidence_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("limitation closure evidence digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("limitation closure receipt digest is invalid");
}

export function limitationClosureReceiptDigest(receipt: LimitationClosureReceipt): string { validateLimitationClosureReceipt(receipt); return digestJsonSync(receipt); }

export interface AdapterCompositionReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  objective_id: string;
  disposition: "composed" | "partial" | "unknown" | "blocked";
  component_order: string[];
  selected_order: string[];
  missing_capability_order: string[];
  dependency_order: string[];
  modality_order: string[];
  artifact_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  reasons: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateAdapterCompositionReceipt(receipt: AdapterCompositionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== DEPENDENCY_COMPOSITION_FEATURE_ID || receipt.contract_version !== DEPENDENCY_COMPOSITION_CONTRACT_VERSION) throw new Error("adapter dependency composition schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.objective_id.trim() || !receipt.component_order.length || !receipt.reasons.length || !receipt.effect_receipts.length) throw new Error("adapter dependency composition identity, components, reasons, effects, locality, or boundary are incomplete");
  if (!new Set(["composed", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("adapter dependency composition disposition is unknown");
  for (const values of [receipt.component_order, receipt.selected_order, receipt.missing_capability_order, receipt.dependency_order, receipt.modality_order, receipt.artifact_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.reasons, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("adapter dependency composition ordering is invalid");
  if (receipt.artifact_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("adapter dependency composition artifact digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adapter dependency composition receipt digest is invalid");
}

export function adapterCompositionReceiptDigest(receipt: AdapterCompositionReceipt): string { validateAdapterCompositionReceipt(receipt); return digestJsonSync(receipt); }

export interface AdapterSemanticParityReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  objective_id: string;
  disposition: "passed" | "unknown" | "blocked";
  adapter_order: string[];
  study_order: string[];
  schema_order: string[];
  semantic_digest: string | null;
  modality_order: string[];
  artifact_order: string[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateAdapterSemanticParityReceipt(receipt: AdapterSemanticParityReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ADAPTER_SEMANTIC_PARITY_FEATURE_ID || receipt.contract_version !== ADAPTER_SEMANTIC_PARITY_CONTRACT_VERSION) throw new Error("adapter semantic parity schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.objective_id.trim() || receipt.adapter_order.length < 2 || receipt.study_order.length < 2 || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("adapter semantic parity identity, reports, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["passed", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("adapter semantic parity disposition is unknown");
  for (const values of [receipt.adapter_order, receipt.study_order, receipt.schema_order, receipt.modality_order, receipt.artifact_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("adapter semantic parity ordering is invalid");
  if (receipt.semantic_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.semantic_digest)) throw new Error("adapter semantic parity digest is invalid");
  if ([...receipt.schema_order, ...receipt.artifact_order].some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("adapter semantic parity artifact digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adapter semantic parity receipt digest is invalid");
}

export function adapterSemanticParityReceiptDigest(receipt: AdapterSemanticParityReceipt): string { validateAdapterSemanticParityReceipt(receipt); return digestJsonSync(receipt); }

export interface ScaleFrontierReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  disposition: "ready" | "partial" | "unknown" | "blocked";
  scenario_order: string[];
  admissible_order: string[];
  blocked_order: string[];
  frontier_order: string[];
  max_admitted_concurrency: number;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateScaleFrontierReceipt(receipt: ScaleFrontierReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ADAPTER_SCALE_FRONTIER_FEATURE_ID || receipt.contract_version !== ADAPTER_SCALE_FRONTIER_CONTRACT_VERSION) throw new Error("adapter scale frontier schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.scenario_order.length || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("adapter scale frontier identity, scenarios, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["ready", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("adapter scale frontier disposition is unknown");
  for (const values of [receipt.scenario_order, receipt.admissible_order, receipt.blocked_order, receipt.frontier_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("adapter scale frontier ordering is invalid");
  if (!Number.isInteger(receipt.max_admitted_concurrency) || receipt.max_admitted_concurrency < 0) throw new Error("adapter scale frontier concurrency is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adapter scale frontier receipt digest is invalid");
}

export function scaleFrontierReceiptDigest(receipt: ScaleFrontierReceipt): string { validateScaleFrontierReceipt(receipt); return digestJsonSync(receipt); }

export interface AdversarialRecoveryReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  disposition: "recovered" | "partial" | "unknown" | "blocked";
  event_order: string[];
  recovered_order: string[];
  blocked_order: string[];
  replay_order: string[];
  checkpoint_order: string[];
  recovery_digest: string | null;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateAdversarialRecoveryReceipt(receipt: AdversarialRecoveryReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ADVERSARIAL_RECOVERY_FEATURE_ID || receipt.contract_version !== ADVERSARIAL_RECOVERY_CONTRACT_VERSION) throw new Error("adversarial recovery schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.event_order.length || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("adversarial recovery identity, events, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["recovered", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("adversarial recovery disposition is unknown");
  for (const values of [receipt.event_order, receipt.recovered_order, receipt.blocked_order, receipt.replay_order, receipt.checkpoint_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("adversarial recovery ordering is invalid");
  if (receipt.recovery_digest !== null && !/^[0-9a-f]{64}$/.test(receipt.recovery_digest)) throw new Error("adversarial recovery digest is invalid");
  if (receipt.checkpoint_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("adversarial recovery checkpoint digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("adversarial recovery receipt digest is invalid");
}

export function adversarialRecoveryReceiptDigest(receipt: AdversarialRecoveryReceipt): string { validateAdversarialRecoveryReceipt(receipt); return digestJsonSync(receipt); }

export interface FederatedCommonsReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  federation_id: string;
  objective_id: string;
  required_purpose: string;
  disposition: "shared" | "partial" | "unknown" | "blocked";
  institution_order: string[];
  admitted_order: string[];
  denied_order: string[];
  semantic_profile_order: string[];
  artifact_order: string[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederatedCommonsReceipt(receipt: FederatedCommonsReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_COMMONS_FEATURE_ID || receipt.contract_version !== FEDERATED_COMMONS_CONTRACT_VERSION) throw new Error("federated commons schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.objective_id.trim() || !receipt.required_purpose.trim() || !receipt.institution_order.length || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("federated commons identity, institutions, purpose, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["shared", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("federated commons disposition is unknown");
  for (const values of [receipt.institution_order, receipt.admitted_order, receipt.denied_order, receipt.semantic_profile_order, receipt.artifact_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federated commons ordering is invalid");
  if (receipt.artifact_order.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("federated commons artifact digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("federated commons receipt digest is invalid");
}

export function federatedCommonsReceiptDigest(receipt: FederatedCommonsReceipt): string { validateFederatedCommonsReceipt(receipt); return digestJsonSync(receipt); }

export interface BoundedEvolutionReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  objective_id: string;
  disposition: "admitted" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  evidence_order: string[];
  replay_order: string[];
  budget: number;
  budget_remaining: number;
  max_concurrency: number;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateBoundedEvolutionReceipt(receipt: BoundedEvolutionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== BOUNDED_EVOLUTION_FEATURE_ID || receipt.contract_version !== BOUNDED_EVOLUTION_CONTRACT_VERSION) throw new Error("bounded evolution schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.objective_id.trim() || !receipt.candidate_order.length || !receipt.checks.length || !receipt.effect_receipts.length || receipt.budget_remaining > receipt.budget || !Number.isInteger(receipt.max_concurrency) || receipt.max_concurrency <= 0) throw new Error("bounded evolution identity, candidates, budget, checks, effects, locality, or boundary are incomplete");
  if (!new Set(["admitted", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("bounded evolution disposition is unknown");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("bounded evolution ordering is invalid");
  for (const values of [receipt.evidence_order, receipt.replay_order]) { if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values) || values.some((value) => !/^[0-9a-f]{64}$/.test(value))) throw new Error("bounded evolution digest ordering is invalid"); }
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("bounded evolution receipt digest is invalid");
}

export function boundedEvolutionReceiptDigest(receipt: BoundedEvolutionReceipt): string { validateBoundedEvolutionReceipt(receipt); return digestJsonSync(receipt); }

export interface EvolutionIdentityReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  workflow_id: string;
  candidate_id: string;
  generation: number;
  parent_digest: string | null;
  baseline_digest: string;
  artifact_digest: string;
  replay_identity: string;
  boundary: string;
}

export function validateEvolutionIdentityReceipt(receipt: EvolutionIdentityReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVOLUTION_IDENTITY_FEATURE_ID || receipt.contract_version !== EVOLUTION_IDENTITY_CONTRACT_VERSION) throw new Error("evolution identity schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.workflow_id.trim() || !receipt.candidate_id.trim() || !Number.isInteger(receipt.generation) || receipt.generation <= 0 || (receipt.generation > 1 && receipt.parent_digest === null)) throw new Error("evolution identity, generation, parent lineage, or boundary is incomplete");
  if ([receipt.workflow_id, receipt.candidate_id].some((value) => /[\u0000-\u001f]/.test(value))) throw new Error("evolution identity contains a control character");
  if ([receipt.workflow_id, receipt.candidate_id].join(":").toLowerCase().match(/clinical|diagnosis|treatment|triage|enrollment/)) throw new Error("clinical decision surfaces are outside the research identity boundary");
  for (const value of [receipt.parent_digest, receipt.baseline_digest, receipt.artifact_digest, receipt.replay_identity]) if (value !== null && !/^[0-9a-f]{64}$/.test(value)) throw new Error("evolution identity digest is invalid");
}

export function evolutionIdentityReceiptDigest(receipt: EvolutionIdentityReceipt): string { validateEvolutionIdentityReceipt(receipt); return digestJsonSync(receipt); }

export interface EvolutionAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  source_receipt_digest: string;
  replay_identity: string;
  benchmark_digest: string;
  verdict: "pass" | "unknown" | "blocked";
  passed_checks: string[];
  failed_checks: string[];
  missing_checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateEvolutionAssuranceReceipt(receipt: EvolutionAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVOLUTION_ASSURANCE_FEATURE_ID || receipt.contract_version !== EVOLUTION_ASSURANCE_CONTRACT_VERSION) throw new Error("bounded evolution assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.effect_receipts.length) throw new Error("bounded evolution assurance identity, effects, locality, or boundary is incomplete");
  if (!new Set(["pass", "unknown", "blocked"]).has(receipt.verdict)) throw new Error("bounded evolution assurance verdict is unknown");
  for (const values of [receipt.passed_checks, receipt.failed_checks, receipt.missing_checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("bounded evolution assurance ordering is invalid");
  for (const digest of [receipt.source_receipt_digest, receipt.replay_identity, receipt.benchmark_digest]) if (!/^[0-9a-f]{64}$/.test(digest)) throw new Error("bounded evolution assurance digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash)) throw new Error("bounded evolution assurance artifact digest is invalid");
}

export function evolutionAssuranceReceiptDigest(receipt: EvolutionAssuranceReceipt): string { validateEvolutionAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface InterpretationPlaneReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  disposition: "admitted" | "partial" | "unknown" | "blocked";
  interpretation_order: string[];
  blocked_order: string[];
  replay_identity: string;
  budget: number;
  budget_remaining: number;
  max_concurrency: number;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateInterpretationPlaneReceipt(receipt: InterpretationPlaneReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== INTERPRETATION_PLANE_FEATURE_ID || receipt.contract_version !== INTERPRETATION_PLANE_CONTRACT_VERSION) throw new Error("interpretation plane schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || (!receipt.interpretation_order.length && !receipt.blocked_order.length) || !receipt.checks.length || !receipt.effect_receipts.length || receipt.budget_remaining > receipt.budget || !Number.isInteger(receipt.max_concurrency) || receipt.max_concurrency <= 0) throw new Error("interpretation plane identity, ordering, budget, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["admitted", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("interpretation plane disposition is unknown");
  for (const values of [receipt.interpretation_order, receipt.blocked_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("interpretation plane ordering is invalid");
  if (!/^[0-9a-f]{64}$/.test(receipt.replay_identity)) throw new Error("interpretation plane replay identity is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash) || typeof receipt.artifact.media_type !== "string" || !receipt.artifact.media_type.trim() || typeof receipt.artifact.scope !== "string" || !receipt.artifact.scope.trim()) throw new Error("interpretation plane artifact is invalid");
}

export function interpretationPlaneReceiptDigest(receipt: InterpretationPlaneReceipt): string { validateInterpretationPlaneReceipt(receipt); return digestJsonSync(receipt); }

export interface KnowledgeGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  federation_id: string;
  disposition: "shared" | "partial" | "unknown" | "blocked";
  world: Record<string, unknown>;
  replay_identity: string;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateKnowledgeGatewayReceipt(receipt: KnowledgeGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== KNOWLEDGE_GATEWAY_FEATURE_ID || receipt.contract_version !== KNOWLEDGE_GATEWAY_CONTRACT_VERSION) throw new Error("knowledge gateway schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.effect_receipts.length || !receipt.checks.length || receipt.world.boundary !== PRECLINICAL_BOUNDARY) throw new Error("knowledge gateway identity, world, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["shared", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("knowledge gateway disposition is unknown");
  for (const values of [receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, Array.isArray(receipt.world.claim_order) ? receipt.world.claim_order as string[] : [], Array.isArray(receipt.world.omissions) ? receipt.world.omissions as string[] : [], Array.isArray(receipt.world.uncertainty) ? receipt.world.uncertainty as string[] : [], Array.isArray(receipt.world.negative_evidence) ? receipt.world.negative_evidence as string[] : []]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("knowledge gateway ordering is invalid");
  const claimOrder = Array.isArray(receipt.world.claim_order) ? receipt.world.claim_order : [];
  if (!claimOrder.length && !(Array.isArray(receipt.world.omissions) && receipt.world.omissions.length) && !(Array.isArray(receipt.world.uncertainty) && receipt.world.uncertainty.length) && !(Array.isArray(receipt.world.negative_evidence) && receipt.world.negative_evidence.length)) throw new Error("knowledge gateway world is empty without an explicit unresolved state");
  if (typeof receipt.world.world_id !== "string" || !receipt.world.world_id.trim() || typeof receipt.world.scope !== "string" || !receipt.world.scope.trim() || typeof receipt.world.target_schema !== "string" || !receipt.world.target_schema.trim()) throw new Error("knowledge gateway world identity and schema are incomplete");
  for (const value of [...(Array.isArray(receipt.world.artifact_order) ? receipt.world.artifact_order : []), ...(Array.isArray(receipt.world.evidence_order) ? receipt.world.evidence_order : []), ...(Array.isArray(receipt.world.provenance_order) ? receipt.world.provenance_order : []), receipt.world.world_digest, receipt.replay_identity]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("knowledge gateway digest is invalid");
}

export function knowledgeGatewayReceiptDigest(receipt: KnowledgeGatewayReceipt): string { validateKnowledgeGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface OracleCapabilityManifestReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  manifest_id: string;
  request_id: string;
  workflow_id: string;
  benchmark_id: string;
  scope: string;
  disposition: "admitted" | "partial" | "unknown" | "blocked";
  admitted_order: string[];
  blocked_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  source_receipt_digest: string;
  benchmark_digest: string;
  replay_identity: string;
  budget: number;
  budget_remaining: number;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateOracleCapabilityManifestReceipt(receipt: OracleCapabilityManifestReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ORACLE_ASSURANCE_FEATURE_ID || receipt.contract_version !== ORACLE_ASSURANCE_CONTRACT_VERSION) throw new Error("oracle assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.manifest_id.trim() || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.benchmark_id.trim() || !receipt.scope.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.budget_remaining > receipt.budget) throw new Error("oracle assurance identity, checks, effects, locality, budget, or boundary is incomplete");
  if (!new Set(["admitted", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("oracle assurance disposition is unknown");
  if (!(receipt.admitted_order.length || receipt.blocked_order.length || receipt.omissions.length || receipt.uncertainty.length || receipt.negative_evidence.length)) throw new Error("oracle assurance must retain an admission or unresolved state");
  for (const values of [receipt.admitted_order, receipt.blocked_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("oracle assurance ordering is invalid");
  for (const digest of [receipt.source_receipt_digest, receipt.benchmark_digest, receipt.replay_identity, ...receipt.evidence_order, ...receipt.provenance_order]) if (!/^[0-9a-f]{64}$/.test(digest)) throw new Error("oracle assurance digest is invalid");
  if (typeof receipt.artifact.content_hash !== "string" || !/^[0-9a-f]{64}$/.test(receipt.artifact.content_hash) || typeof receipt.artifact.media_type !== "string" || !receipt.artifact.media_type.trim() || typeof receipt.artifact.scope !== "string" || !receipt.artifact.scope.trim()) throw new Error("oracle assurance artifact is invalid");
}

export function oracleCapabilityManifestReceiptDigest(receipt: OracleCapabilityManifestReceipt): string { validateOracleCapabilityManifestReceipt(receipt); return digestJsonSync(receipt); }

export interface FederatedMultimodalIngestionReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  institution_id: string;
  disposition: "harmonized" | "partial" | "unknown" | "blocked";
  object: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateFederatedMultimodalIngestionReceipt(receipt: FederatedMultimodalIngestionReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_INGESTION_FEATURE_ID || receipt.contract_version !== FEDERATED_INGESTION_CONTRACT_VERSION) throw new Error("federated ingestion schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.institution_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.object.boundary !== PRECLINICAL_BOUNDARY) throw new Error("federated ingestion identity, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["harmonized", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("federated ingestion disposition is unknown");
  const accepted = Array.isArray(receipt.object.accepted_order) ? receipt.object.accepted_order as string[] : [];
  const blocked = Array.isArray(receipt.object.blocked_order) ? receipt.object.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.object.omissions) ? receipt.object.omissions as string[] : []), ...(Array.isArray(receipt.object.uncertainty) ? receipt.object.uncertainty as string[] : []), ...(Array.isArray(receipt.object.negative_evidence) ? receipt.object.negative_evidence as string[] : [])];
  if (!accepted.length && !blocked.length && !unresolved.length) throw new Error("federated ingestion must retain an admitted or unresolved object state");
  for (const values of [Array.isArray(receipt.object.modality_order) ? receipt.object.modality_order as string[] : [], accepted, blocked, Array.isArray(receipt.object.omissions) ? receipt.object.omissions as string[] : [], Array.isArray(receipt.object.uncertainty) ? receipt.object.uncertainty as string[] : [], Array.isArray(receipt.object.negative_evidence) ? receipt.object.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federated ingestion ordering is invalid");
  for (const value of [...(Array.isArray(receipt.object.artifact_order) ? receipt.object.artifact_order : []), ...(Array.isArray(receipt.object.provenance_order) ? receipt.object.provenance_order : []), receipt.object.replay_identity, receipt.object.object_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("federated ingestion digest is invalid");
  if (typeof receipt.object.object_id !== "string" || !receipt.object.object_id.trim() || typeof receipt.object.study_id !== "string" || !receipt.object.study_id.trim() || typeof receipt.object.scope !== "string" || !receipt.object.scope.trim() || typeof receipt.object.semantic_profile !== "string") throw new Error("federated ingestion object identity is incomplete");
}

export function federatedMultimodalIngestionReceiptDigest(receipt: FederatedMultimodalIngestionReceipt): string { validateFederatedMultimodalIngestionReceipt(receipt); return digestJsonSync(receipt); }

export interface QualityAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  verdict: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateQualityAssuranceReceipt(receipt: QualityAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== QUALITY_ASSURANCE_FEATURE_ID || receipt.contract_version !== QUALITY_ASSURANCE_CONTRACT_VERSION) throw new Error("quality assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.verdict.boundary !== PRECLINICAL_BOUNDARY) throw new Error("quality assurance identity, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("quality assurance disposition is unknown");
  const qualified = Array.isArray(receipt.verdict.qualified_order) ? receipt.verdict.qualified_order as string[] : [];
  const blocked = Array.isArray(receipt.verdict.blocked_order) ? receipt.verdict.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.verdict.omissions) ? receipt.verdict.omissions as string[] : []), ...(Array.isArray(receipt.verdict.uncertainty) ? receipt.verdict.uncertainty as string[] : []), ...(Array.isArray(receipt.verdict.negative_evidence) ? receipt.verdict.negative_evidence as string[] : [])];
  if (!qualified.length && !blocked.length && !unresolved.length) throw new Error("quality assurance must retain a qualified or unresolved verdict");
  for (const values of [Array.isArray(receipt.verdict.study_order) ? receipt.verdict.study_order as string[] : [], qualified, blocked, Array.isArray(receipt.verdict.witness_order) ? receipt.verdict.witness_order as string[] : [], Array.isArray(receipt.verdict.omissions) ? receipt.verdict.omissions as string[] : [], Array.isArray(receipt.verdict.uncertainty) ? receipt.verdict.uncertainty as string[] : [], Array.isArray(receipt.verdict.negative_evidence) ? receipt.verdict.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("quality assurance ordering is invalid");
  for (const value of [...(Array.isArray(receipt.verdict.artifact_order) ? receipt.verdict.artifact_order : []), ...(Array.isArray(receipt.verdict.provenance_order) ? receipt.verdict.provenance_order : []), receipt.verdict.replay_identity, receipt.verdict.verdict_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("quality assurance digest is invalid");
  if (typeof receipt.verdict.verdict_id !== "string" || !receipt.verdict.verdict_id.trim() || !Array.isArray(receipt.verdict.study_order)) throw new Error("quality assurance verdict identity is incomplete");
}

export function qualityAssuranceReceiptDigest(receipt: QualityAssuranceReceipt): string { validateQualityAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface MechanismControlReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  objective_id: string;
  disposition: "ranked" | "partial" | "unknown" | "blocked";
  portfolio: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateMechanismControlReceipt(receipt: MechanismControlReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MECHANISM_CONTROL_FEATURE_ID || receipt.contract_version !== MECHANISM_CONTROL_CONTRACT_VERSION) throw new Error("mechanism control schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.objective_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.portfolio.boundary !== PRECLINICAL_BOUNDARY) throw new Error("mechanism control identity, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["ranked", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("mechanism control disposition is unknown");
  const ranked = Array.isArray(receipt.portfolio.ranked_order) ? receipt.portfolio.ranked_order as string[] : [];
  const blocked = Array.isArray(receipt.portfolio.blocked_order) ? receipt.portfolio.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.portfolio.omissions) ? receipt.portfolio.omissions as string[] : []), ...(Array.isArray(receipt.portfolio.uncertainty) ? receipt.portfolio.uncertainty as string[] : []), ...(Array.isArray(receipt.portfolio.negative_evidence) ? receipt.portfolio.negative_evidence as string[] : [])];
  if (!ranked.length && !blocked.length && !unresolved.length) throw new Error("mechanism control must retain a ranked or unresolved portfolio");
  for (const values of [Array.isArray(receipt.portfolio.study_order) ? receipt.portfolio.study_order as string[] : [], Array.isArray(receipt.portfolio.competing_order) ? receipt.portfolio.competing_order as string[] : [], blocked, Array.isArray(receipt.portfolio.omissions) ? receipt.portfolio.omissions as string[] : [], Array.isArray(receipt.portfolio.uncertainty) ? receipt.portfolio.uncertainty as string[] : [], Array.isArray(receipt.portfolio.negative_evidence) ? receipt.portfolio.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("mechanism control ordering is invalid");
  const scores = Array.isArray(receipt.portfolio.rank_score_order) ? receipt.portfolio.rank_score_order as number[] : [];
  if (ranked.length !== scores.length || scores.some((value, index) => index > 0 && scores[index - 1] < value) || new Set(ranked).size !== ranked.length) throw new Error("mechanism control ranking is invalid");
  for (const value of [...(Array.isArray(receipt.portfolio.evidence_order) ? receipt.portfolio.evidence_order : []), ...(Array.isArray(receipt.portfolio.provenance_order) ? receipt.portfolio.provenance_order : []), receipt.portfolio.replay_identity, receipt.portfolio.portfolio_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("mechanism control digest is invalid");
  if (typeof receipt.portfolio.portfolio_id !== "string" || !receipt.portfolio.portfolio_id.trim()) throw new Error("mechanism control portfolio identity is incomplete");
}

export function mechanismControlReceiptDigest(receipt: MechanismControlReceipt): string { validateMechanismControlReceipt(receipt); return digestJsonSync(receipt); }

export interface EvidenceWorkbenchReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  study_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  evidence: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateEvidenceWorkbenchReceipt(receipt: EvidenceWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_WORKBENCH_FEATURE_ID || receipt.contract_version !== EVIDENCE_WORKBENCH_CONTRACT_VERSION) throw new Error("evidence workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.study_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.evidence.boundary !== PRECLINICAL_BOUNDARY) throw new Error("evidence workbench identity, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("evidence workbench disposition is unknown");
  const sourceOrder = Array.isArray(receipt.evidence.source_order) ? receipt.evidence.source_order as string[] : [];
  const qualified = Array.isArray(receipt.evidence.qualified_order) ? receipt.evidence.qualified_order as string[] : [];
  const alerts = Array.isArray(receipt.evidence.alert_order) ? receipt.evidence.alert_order as string[] : [];
  const blocked = Array.isArray(receipt.evidence.blocked_order) ? receipt.evidence.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.evidence.omissions) ? receipt.evidence.omissions as string[] : []), ...(Array.isArray(receipt.evidence.uncertainty) ? receipt.evidence.uncertainty as string[] : []), ...(Array.isArray(receipt.evidence.negative_evidence) ? receipt.evidence.negative_evidence as string[] : [])];
  if (!sourceOrder.length || (!qualified.length && !alerts.length && !blocked.length && !unresolved.length)) throw new Error("evidence workbench must retain sources and a qualified or unresolved state");
  for (const values of [sourceOrder, qualified, alerts, blocked, Array.isArray(receipt.evidence.omissions) ? receipt.evidence.omissions as string[] : [], Array.isArray(receipt.evidence.uncertainty) ? receipt.evidence.uncertainty as string[] : [], Array.isArray(receipt.evidence.negative_evidence) ? receipt.evidence.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("evidence workbench ordering is invalid");
  for (const value of [...(Array.isArray(receipt.evidence.evidence_order) ? receipt.evidence.evidence_order : []), ...(Array.isArray(receipt.evidence.provenance_order) ? receipt.evidence.provenance_order : []), receipt.evidence.replay_identity, receipt.evidence.set_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("evidence workbench digest is invalid");
  if (typeof receipt.evidence.set_id !== "string" || !receipt.evidence.set_id.trim()) throw new Error("evidence workbench set identity is incomplete");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("view:"))) throw new Error("evidence workbench effect is not read-only");
}

export function evidenceWorkbenchReceiptDigest(receipt: EvidenceWorkbenchReceipt): string { validateEvidenceWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface AnalysisControlReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  objective_id: string;
  disposition: "ranked" | "partial" | "unknown" | "blocked";
  portfolio: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateAnalysisControlReceipt(receipt: AnalysisControlReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ANALYSIS_CONTROL_FEATURE_ID || receipt.contract_version !== ANALYSIS_CONTROL_CONTRACT_VERSION) throw new Error("analysis control schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.objective_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.portfolio.boundary !== PRECLINICAL_BOUNDARY) throw new Error("analysis control identity, portfolio, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["ranked", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("analysis control disposition is unknown");
  const admitted = Array.isArray(receipt.portfolio.admitted_order) ? receipt.portfolio.admitted_order as string[] : [];
  const blocked = Array.isArray(receipt.portfolio.blocked_order) ? receipt.portfolio.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.portfolio.omissions) ? receipt.portfolio.omissions as string[] : []), ...(Array.isArray(receipt.portfolio.uncertainty) ? receipt.portfolio.uncertainty as string[] : []), ...(Array.isArray(receipt.portfolio.negative_evidence) ? receipt.portfolio.negative_evidence as string[] : [])];
  if (!admitted.length && !blocked.length && !unresolved.length) throw new Error("analysis control must retain an admitted or unresolved portfolio");
  for (const values of [Array.isArray(receipt.portfolio.candidate_order) ? receipt.portfolio.candidate_order as string[] : [], admitted, blocked, Array.isArray(receipt.portfolio.class_order) ? receipt.portfolio.class_order as string[] : [], Array.isArray(receipt.portfolio.omissions) ? receipt.portfolio.omissions as string[] : [], Array.isArray(receipt.portfolio.uncertainty) ? receipt.portfolio.uncertainty as string[] : [], Array.isArray(receipt.portfolio.negative_evidence) ? receipt.portfolio.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("analysis control ordering is invalid");
  const scores = Array.isArray(receipt.portfolio.rank_score_order) ? receipt.portfolio.rank_score_order as number[] : [];
  if (admitted.length !== scores.length || scores.some((value, index) => index > 0 && scores[index - 1] < value) || new Set(admitted).size !== admitted.length) throw new Error("analysis control ranking is invalid");
  for (const value of [...(Array.isArray(receipt.portfolio.result_order) ? receipt.portfolio.result_order : []), ...(Array.isArray(receipt.portfolio.model_order) ? receipt.portfolio.model_order : []), ...(Array.isArray(receipt.portfolio.provenance_order) ? receipt.portfolio.provenance_order : []), receipt.portfolio.replay_identity, receipt.portfolio.portfolio_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("analysis control digest is invalid");
  if (typeof receipt.portfolio.portfolio_id !== "string" || !receipt.portfolio.portfolio_id.trim()) throw new Error("analysis control portfolio identity is incomplete");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:digest-only-analysis-manifest:"))) throw new Error("analysis control effect is not digest-only");
}

export function analysisControlReceiptDigest(receipt: AnalysisControlReceipt): string { validateAnalysisControlReceipt(receipt); return digestJsonSync(receipt); }

export interface ContextAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  question_id: string;
  disposition: "compiled" | "partial" | "unknown" | "blocked";
  context: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateContextAssuranceReceipt(receipt: ContextAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== CONTEXT_ASSURANCE_FEATURE_ID || receipt.contract_version !== CONTEXT_ASSURANCE_CONTRACT_VERSION) throw new Error("context assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.question_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.context.boundary !== PRECLINICAL_BOUNDARY) throw new Error("context assurance identity, context, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["compiled", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("context assurance disposition is unknown");
  const selected = Array.isArray(receipt.context.selected_order) ? receipt.context.selected_order as string[] : [];
  const blocked = Array.isArray(receipt.context.blocked_order) ? receipt.context.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.context.omissions) ? receipt.context.omissions as string[] : []), ...(Array.isArray(receipt.context.uncertainty) ? receipt.context.uncertainty as string[] : []), ...(Array.isArray(receipt.context.negative_evidence) ? receipt.context.negative_evidence as string[] : [])];
  if (!selected.length && !blocked.length && !unresolved.length) throw new Error("context assurance must retain selected or unresolved context");
  for (const values of [Array.isArray(receipt.context.fact_order) ? receipt.context.fact_order as string[] : [], selected, blocked, Array.isArray(receipt.context.class_order) ? receipt.context.class_order as string[] : [], Array.isArray(receipt.context.omissions) ? receipt.context.omissions as string[] : [], Array.isArray(receipt.context.uncertainty) ? receipt.context.uncertainty as string[] : [], Array.isArray(receipt.context.negative_evidence) ? receipt.context.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("context assurance ordering is invalid");
  for (const value of [...(Array.isArray(receipt.context.semantic_order) ? receipt.context.semantic_order : []), ...(Array.isArray(receipt.context.evidence_order) ? receipt.context.evidence_order : []), ...(Array.isArray(receipt.context.provenance_order) ? receipt.context.provenance_order : []), receipt.context.replay_identity, receipt.context.context_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("context assurance digest is invalid");
  if (typeof receipt.context.context_id !== "string" || !receipt.context.context_id.trim()) throw new Error("context assurance context identity is incomplete");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:signed-context-digest:"))) throw new Error("context assurance effect is not signed digest-only exchange");
}

export function contextAssuranceReceiptDigest(receipt: ContextAssuranceReceipt): string { validateContextAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface BioworldsEvaluationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  capability_id: string;
  benchmark_id: string;
  disposition: "passed" | "conditional" | "unknown" | "blocked";
  summary: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateBioworldsEvaluationAssuranceReceipt(receipt: BioworldsEvaluationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVALUATION_ASSURANCE_BIOWORLDS_FEATURE_ID || receipt.contract_version !== EVALUATION_ASSURANCE_BIOWORLDS_CONTRACT_VERSION) throw new Error("bioworlds evaluation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.capability_id.trim() || !receipt.benchmark_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.summary.boundary !== PRECLINICAL_BOUNDARY) throw new Error("bioworlds evaluation assurance identity, summary, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["passed", "conditional", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("bioworlds evaluation assurance disposition is unknown");
  const admitted = Array.isArray(receipt.summary.admitted_order) ? receipt.summary.admitted_order as string[] : [];
  const blocked = Array.isArray(receipt.summary.blocked_order) ? receipt.summary.blocked_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.summary.omissions) ? receipt.summary.omissions as string[] : []), ...(Array.isArray(receipt.summary.uncertainty) ? receipt.summary.uncertainty as string[] : []), ...(Array.isArray(receipt.summary.negative_evidence) ? receipt.summary.negative_evidence as string[] : [])];
  if (!admitted.length && !blocked.length && !unresolved.length) throw new Error("bioworlds evaluation assurance must retain an admitted or unresolved summary");
  for (const values of [Array.isArray(receipt.summary.observation_order) ? receipt.summary.observation_order as string[] : [], admitted, blocked, Array.isArray(receipt.summary.metric_order) ? receipt.summary.metric_order as string[] : [], Array.isArray(receipt.summary.site_order) ? receipt.summary.site_order as string[] : [], Array.isArray(receipt.summary.omissions) ? receipt.summary.omissions as string[] : [], Array.isArray(receipt.summary.uncertainty) ? receipt.summary.uncertainty as string[] : [], Array.isArray(receipt.summary.negative_evidence) ? receipt.summary.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("bioworlds evaluation assurance ordering is invalid");
  for (const value of [...(Array.isArray(receipt.summary.baseline_order) ? receipt.summary.baseline_order : []), ...(Array.isArray(receipt.summary.artifact_order) ? receipt.summary.artifact_order : []), ...(Array.isArray(receipt.summary.provenance_order) ? receipt.summary.provenance_order : []), receipt.summary.replay_identity, receipt.summary.summary_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("bioworlds evaluation assurance digest is invalid");
  if (typeof receipt.summary.summary_id !== "string" || !receipt.summary.summary_id.trim()) throw new Error("bioworlds evaluation assurance summary identity is incomplete");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:evaluation-manifest-digest-only:"))) throw new Error("bioworlds evaluation assurance effect is not digest-only");
  for (const field of ["positive_count", "null_count", "negative_count", "inconclusive_count"]) if (typeof receipt.summary[field] !== "number" || !Number.isInteger(receipt.summary[field]) || (receipt.summary[field] as number) < 0) throw new Error("bioworlds evaluation assurance outcome count is invalid");
}

export function bioworldsEvaluationAssuranceReceiptDigest(receipt: BioworldsEvaluationAssuranceReceipt): string { validateBioworldsEvaluationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface BiolangQualityWorkbenchReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  study_id: string;
  disposition: "released" | "conditional" | "quarantined" | "unknown" | "blocked";
  summary: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateBiolangQualityWorkbenchReceipt(receipt: BiolangQualityWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== QUALITY_WORKBENCH_BIOLANG_FEATURE_ID || receipt.contract_version !== QUALITY_WORKBENCH_BIOLANG_CONTRACT_VERSION) throw new Error("biolang quality workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.study_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.summary.boundary !== PRECLINICAL_BOUNDARY) throw new Error("biolang quality workbench identity, summary, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["released", "conditional", "quarantined", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("biolang quality workbench disposition is unknown");
  const qualified = Array.isArray(receipt.summary.qualified_order) ? receipt.summary.qualified_order as string[] : [];
  const warning = Array.isArray(receipt.summary.warning_order) ? receipt.summary.warning_order as string[] : [];
  const quarantined = Array.isArray(receipt.summary.quarantined_order) ? receipt.summary.quarantined_order as string[] : [];
  const unknown = Array.isArray(receipt.summary.unknown_order) ? receipt.summary.unknown_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.summary.omissions) ? receipt.summary.omissions as string[] : []), ...(Array.isArray(receipt.summary.uncertainty) ? receipt.summary.uncertainty as string[] : []), ...(Array.isArray(receipt.summary.negative_evidence) ? receipt.summary.negative_evidence as string[] : [])];
  if (!qualified.length && !warning.length && !quarantined.length && !unknown.length && !unresolved.length) throw new Error("biolang quality workbench must retain a qualified or unresolved summary");
  for (const values of [Array.isArray(receipt.summary.observation_order) ? receipt.summary.observation_order as string[] : [], qualified, warning, quarantined, unknown, Array.isArray(receipt.summary.batch_order) ? receipt.summary.batch_order as string[] : [], Array.isArray(receipt.summary.sample_order) ? receipt.summary.sample_order as string[] : [], Array.isArray(receipt.summary.metric_order) ? receipt.summary.metric_order as string[] : [], Array.isArray(receipt.summary.omissions) ? receipt.summary.omissions as string[] : [], Array.isArray(receipt.summary.uncertainty) ? receipt.summary.uncertainty as string[] : [], Array.isArray(receipt.summary.negative_evidence) ? receipt.summary.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("biolang quality workbench ordering is invalid");
  for (const value of [...(Array.isArray(receipt.summary.artifact_order) ? receipt.summary.artifact_order : []), ...(Array.isArray(receipt.summary.provenance_order) ? receipt.summary.provenance_order : []), receipt.summary.replay_identity, receipt.summary.summary_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("biolang quality workbench digest is invalid");
  if (typeof receipt.summary.summary_id !== "string" || !receipt.summary.summary_id.trim()) throw new Error("biolang quality workbench summary identity is incomplete");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("write:local-quality-manifest:"))) throw new Error("biolang quality workbench effect is not a local quality manifest");
  for (const field of ["passed_count", "warning_count", "quarantined_count", "unknown_count"]) if (typeof receipt.summary[field] !== "number" || !Number.isInteger(receipt.summary[field]) || (receipt.summary[field] as number) < 0) throw new Error("biolang quality workbench count is invalid");
}

export function biolangQualityWorkbenchReceiptDigest(receipt: BiolangQualityWorkbenchReceipt): string { validateBiolangQualityWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface BiolangRetrievalAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  query_id: string;
  disposition: "passed" | "conditional" | "unknown" | "blocked";
  summary: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateBiolangRetrievalAssuranceReceipt(receipt: BiolangRetrievalAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== RETRIEVAL_ASSURANCE_BIOLANG_FEATURE_ID || receipt.contract_version !== RETRIEVAL_ASSURANCE_BIOLANG_CONTRACT_VERSION) throw new Error("biolang retrieval assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.query_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.summary.boundary !== PRECLINICAL_BOUNDARY) throw new Error("biolang retrieval assurance identity, summary, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["passed", "conditional", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("biolang retrieval assurance disposition is unknown");
  const selected = Array.isArray(receipt.summary.selected_order) ? receipt.summary.selected_order as string[] : [];
  const blocked = Array.isArray(receipt.summary.blocked_order) ? receipt.summary.blocked_order as string[] : [];
  const unknown = Array.isArray(receipt.summary.unknown_order) ? receipt.summary.unknown_order as string[] : [];
  const unresolved = [...(Array.isArray(receipt.summary.omissions) ? receipt.summary.omissions as string[] : []), ...(Array.isArray(receipt.summary.uncertainty) ? receipt.summary.uncertainty as string[] : []), ...(Array.isArray(receipt.summary.negative_evidence) ? receipt.summary.negative_evidence as string[] : [])];
  if (!selected.length && !blocked.length && !unknown.length && !unresolved.length) throw new Error("biolang retrieval assurance must retain a selected or unresolved summary");
  const rankedOrder = Array.isArray(receipt.summary.ranked_order) ? receipt.summary.ranked_order as string[] : [];
  if (new Set(rankedOrder).size !== rankedOrder.length) throw new Error("biolang retrieval assurance ranked order contains duplicates");
  for (const values of [Array.isArray(receipt.summary.candidate_order) ? receipt.summary.candidate_order as string[] : [], selected, blocked, unknown, Array.isArray(receipt.summary.study_order) ? receipt.summary.study_order as string[] : [], Array.isArray(receipt.summary.modality_order) ? receipt.summary.modality_order as string[] : [], Array.isArray(receipt.summary.omissions) ? receipt.summary.omissions as string[] : [], Array.isArray(receipt.summary.uncertainty) ? receipt.summary.uncertainty as string[] : [], Array.isArray(receipt.summary.negative_evidence) ? receipt.summary.negative_evidence as string[] : [], receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("biolang retrieval assurance ordering is invalid");
  for (const value of [...(Array.isArray(receipt.summary.artifact_order) ? receipt.summary.artifact_order : []), ...(Array.isArray(receipt.summary.provenance_order) ? receipt.summary.provenance_order : []), receipt.summary.replay_identity, receipt.summary.summary_digest]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("biolang retrieval assurance digest is invalid");
  if (typeof receipt.summary.summary_id !== "string" || !receipt.summary.summary_id.trim()) throw new Error("biolang retrieval assurance summary identity is incomplete");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:retrieval-assurance:") && effect !== "block:unsafe-release")) throw new Error("biolang retrieval assurance effect is outside the evaluation or unsafe-release boundary");
  for (const [field, orderName] of [["selected_count", "selected_order"], ["blocked_count", "blocked_order"], ["unknown_count", "unknown_order"]]) if (typeof receipt.summary[field] !== "number" || !Number.isInteger(receipt.summary[field]) || (receipt.summary[field] as number) < 0 || receipt.summary[field] !== (Array.isArray(receipt.summary[orderName]) ? (receipt.summary[orderName] as unknown[]).length : 0)) throw new Error("biolang retrieval assurance summary count is invalid");
}

export function biolangRetrievalAssuranceReceiptDigest(receipt: BiolangRetrievalAssuranceReceipt): string { validateBiolangRetrievalAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface CliKnowledgeInteroperabilityReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  disposition: "passed" | "conditional" | "unknown" | "blocked";
  world: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateCliKnowledgeInteroperabilityReceipt(receipt: CliKnowledgeInteroperabilityReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== CLI_KNOWLEDGE_INTEROPERABILITY_FEATURE_ID || receipt.contract_version !== CLI_KNOWLEDGE_INTEROPERABILITY_CONTRACT_VERSION) throw new Error("CLI knowledge interoperability schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.checks.length || !receipt.effect_receipts.length || receipt.world.boundary !== PRECLINICAL_BOUNDARY || JSON.stringify(receipt.omissions) !== JSON.stringify(receipt.world.omissions) || JSON.stringify(receipt.uncertainty) !== JSON.stringify(receipt.world.uncertainty) || JSON.stringify(receipt.negative_evidence) !== JSON.stringify(receipt.world.negative_evidence)) throw new Error("CLI knowledge interoperability identity, world linkage, checks, effects, locality, or boundary is incomplete");
  if (!new Set(["passed", "conditional", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("CLI knowledge interoperability disposition is unknown");
  for (const field of ["schema_version", "world_id", "target_schema", "replay_identity", "world_digest", "boundary"]) if (typeof receipt.world[field] !== "string" || !(receipt.world[field] as string).trim()) throw new Error("CLI typed knowledge-world identity is incomplete");
  for (const value of [receipt.world.replay_identity, receipt.world.world_digest, ...(Array.isArray(receipt.world.evidence_order) ? receipt.world.evidence_order : []), ...(Array.isArray(receipt.world.provenance_order) ? receipt.world.provenance_order : [])]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("CLI typed knowledge-world digest is invalid");
  for (const name of ["claim_order", "admitted_order", "blocked_order", "unknown_order", "subject_order", "predicate_order", "omissions", "uncertainty", "negative_evidence"]) { const values = Array.isArray(receipt.world[name]) ? receipt.world[name] as string[] : []; if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("CLI typed knowledge-world ordering is invalid"); }
  for (const values of [receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("CLI knowledge interoperability receipt ordering is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:permitted-artifacts:") && effect !== "block:knowledge-world-release")) throw new Error("CLI knowledge interoperability effect is outside permitted-artifact exchange");
}

export function cliKnowledgeInteroperabilityReceiptDigest(receipt: CliKnowledgeInteroperabilityReceipt): string { validateCliKnowledgeInteroperabilityReceipt(receipt); return digestJsonSync(receipt); }

export interface LabEvidenceSurveillanceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  feed_id: string;
  workflow_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  qualified_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  source_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateLabEvidenceSurveillanceReceipt(receipt: LabEvidenceSurveillanceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== LAB_EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== LAB_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("lab evidence surveillance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.feed_id.trim() || !receipt.workflow_id.trim() || !receipt.effect_receipts.length) throw new Error("lab evidence surveillance identity, locality, effects, or boundary is incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("lab evidence surveillance disposition is unknown");
  if (!receipt.qualified_order.length && !receipt.blocked_order.length && !receipt.unknown_order.length) throw new Error("lab evidence surveillance must retain a qualified, blocked, or unknown item");
  for (const values of [receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("lab evidence surveillance ordering is invalid");
  for (const value of [receipt.replay_identity, ...receipt.source_order, ...receipt.provenance_order, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("lab evidence surveillance digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("invoke:declared-tools:") && effect !== "block:evidence-surveillance-release")) throw new Error("lab evidence surveillance effect is outside bounded tool invocation");
}

export function labEvidenceSurveillanceReceiptDigest(receipt: LabEvidenceSurveillanceReceipt): string { validateLabEvidenceSurveillanceReceipt(receipt); return digestJsonSync(receipt); }

export interface FiberMechanismAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  question_id: string;
  workflow_id: string;
  target_schema: string;
  disposition: "qualified" | "conditional" | "unknown" | "blocked";
  ranked_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  mechanism_order: string[];
  study_order: string[];
  modality_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateFiberMechanismAssuranceReceipt(receipt: FiberMechanismAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FIBER_MECHANISM_ASSURANCE_FEATURE_ID || receipt.contract_version !== FIBER_MECHANISM_ASSURANCE_CONTRACT_VERSION) throw new Error("fiber mechanism assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.question_id.trim() || !receipt.workflow_id.trim() || !receipt.target_schema.trim() || !receipt.ranked_order.length) throw new Error("fiber mechanism assurance identity, ranking, locality, or boundary is incomplete");
  if (!new Set(["qualified", "conditional", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("fiber mechanism assurance disposition is unknown");
  if (receipt.disposition !== "qualified" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("fiber mechanism assurance must block unsafe release");
  if (receipt.disposition === "qualified" && receipt.effect_receipts.length) throw new Error("qualified fiber mechanism assurance cannot carry a release block");
  for (const values of [receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.mechanism_order, receipt.study_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("fiber mechanism assurance ordering is invalid");
  for (const value of [...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("fiber mechanism assurance digest is invalid");
  if (receipt.effect_receipts.some((effect) => effect !== "block:unsafe-release")) throw new Error("fiber mechanism assurance effect is outside unsafe-release gate");
}

export function fiberMechanismAssuranceReceiptDigest(receipt: FiberMechanismAssuranceReceipt): string { validateFiberMechanismAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface HubapiQualityAssuranceReceipt {
  schema_version: string;
  feature_id: string;
  contract_version: string;
  object_id: string;
  study_id: string;
  scope: string;
  target_schema: string;
  disposition: "qualified" | "conditional" | "unknown" | "blocked";
  ranked_metric_order: string[];
  passed_order: string[];
  failed_order: string[];
  unknown_order: string[];
  witness_order: string[];
  modality_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateHubapiQualityAssuranceReceipt(receipt: HubapiQualityAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== HUBAPI_QUALITY_ASSURANCE_FEATURE_ID || receipt.contract_version !== HUBAPI_QUALITY_ASSURANCE_CONTRACT_VERSION) throw new Error("hubapi quality assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.object_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || !receipt.target_schema.trim() || !receipt.ranked_metric_order.length) throw new Error("hubapi quality assurance identity, ranking, locality, or boundary is incomplete");
  if (!new Set(["qualified", "conditional", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("hubapi quality assurance disposition is unknown");
  if (receipt.disposition !== "qualified" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("hubapi quality assurance must block unsafe release");
  if (receipt.disposition === "qualified" && receipt.effect_receipts.length) throw new Error("qualified hubapi quality assurance cannot carry a release block");
  if (new Set(receipt.ranked_metric_order).size !== receipt.ranked_metric_order.length) throw new Error("hubapi quality assurance ranking contains duplicates");
  for (const values of [receipt.passed_order, receipt.failed_order, receipt.unknown_order, receipt.witness_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("hubapi quality assurance ordering is invalid");
  for (const value of [...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("hubapi quality assurance digest is invalid");
  if (receipt.effect_receipts.some((effect) => effect !== "block:unsafe-release")) throw new Error("hubapi quality assurance effect is outside unsafe-release gate");
}

export function hubapiQualityAssuranceReceiptDigest(receipt: HubapiQualityAssuranceReceipt): string { validateHubapiQualityAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface RegistryResourceDiscoveryAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  federation_id: string;
  requester: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  selected_order: string[];
  omitted_order: string[];
  semantic_order: string[];
  artifact_order: string[];
  provenance_order: string[];
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  federation_manifest: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateRegistryResourceDiscoveryAssuranceReceipt(receipt: RegistryResourceDiscoveryAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_FEATURE_ID || receipt.contract_version !== REGISTRY_RESOURCE_DISCOVERY_ASSURANCE_CONTRACT_VERSION) throw new Error("registry resource discovery assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.requester.trim() || !receipt.scope.trim() || !receipt.candidate_order.length || !receipt.checks.length) throw new Error("registry resource discovery identity, locality, candidates, or checks are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("registry resource discovery disposition is unknown");
  if (new Set(receipt.candidate_order).size !== receipt.candidate_order.length || new Set(receipt.selected_order).size !== receipt.selected_order.length || new Set(receipt.omitted_order).size !== receipt.omitted_order.length) throw new Error("registry resource discovery ordering contains duplicates");
  const candidateSet = new Set(receipt.candidate_order);
  if ([...receipt.selected_order, ...receipt.omitted_order].some((value) => !candidateSet.has(value))) throw new Error("registry resource discovery selected or omitted resource is unknown");
  for (const values of [receipt.omitted_order, receipt.checks, receipt.omissions, receipt.uncertainty, receipt.negative_evidence]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("registry resource discovery canonical ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("registry resource discovery digest ordering is invalid");
  for (const value of [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.provenance_order, receipt.replay_identity, receipt.federation_manifest.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("registry resource discovery digest is invalid");
  if (receipt.selected_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:signed-resource-manifest:"))) throw new Error("selected registry resources require signed manifest exchange receipts");
  if (!receipt.selected_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:federated-resource-discovery"])) throw new Error("empty registry resource result must be explicitly blocked");
}

export function registryResourceDiscoveryAssuranceReceiptDigest(receipt: RegistryResourceDiscoveryAssuranceReceipt): string { validateRegistryResourceDiscoveryAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ServicesMechanismWorkbenchReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  objective_id: string;
  target_schema: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  ranked_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  mechanism_order: string[];
  study_order: string[];
  modality_order: string[];
  score_order: number[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateServicesMechanismWorkbenchReceipt(receipt: ServicesMechanismWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== SERVICES_MECHANISM_WORKBENCH_FEATURE_ID || receipt.contract_version !== SERVICES_MECHANISM_WORKBENCH_CONTRACT_VERSION) throw new Error("services mechanism workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.objective_id.trim() || !receipt.target_schema.trim() || !receipt.scope.trim() || !receipt.ranked_order.length) throw new Error("services mechanism workbench identity, ranking, locality, or boundary is incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("services mechanism workbench disposition is unknown");
  for (const values of [receipt.ranked_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order]) if (new Set(values).size !== values.length) throw new Error("services mechanism workbench ranking contains duplicates");
  if (receipt.score_order.length !== receipt.ranked_order.length || [...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.ranked_order.includes(value))) throw new Error("services mechanism workbench score or disposition linkage is incomplete");
  for (const values of [receipt.mechanism_order, receipt.study_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, receipt.artifact_order, receipt.evidence_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("services mechanism workbench ordering is invalid");
  for (const value of [...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("services mechanism workbench digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("write:local-mechanism-workbench:"))) throw new Error("admitted mechanism workbench requires a local artifact effect");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:mechanism-workbench-release"])) throw new Error("empty mechanism workbench result must be explicitly blocked");
}

export function servicesMechanismWorkbenchReceiptDigest(receipt: ServicesMechanismWorkbenchReceipt): string { validateServicesMechanismWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface GovernanceInterpretationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  objective_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  ranked_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  result_order: string[];
  visualization_order: string[];
  support_order: number[];
  semantic_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  baseline_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateGovernanceInterpretationAssuranceReceipt(receipt: GovernanceInterpretationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== GOVERNANCE_INTERPRETATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== GOVERNANCE_INTERPRETATION_ASSURANCE_CONTRACT_VERSION) throw new Error("governance interpretation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.objective_id.trim() || !receipt.scope.trim() || !receipt.ranked_order.length) throw new Error("governance interpretation identity, ranking, locality, or boundary is incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("governance interpretation disposition is unknown");
  if (receipt.support_order.length !== receipt.ranked_order.length || [...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.ranked_order.includes(value))) throw new Error("governance interpretation support or disposition linkage is incomplete");
  for (const values of [receipt.ranked_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.result_order, receipt.visualization_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, receipt.semantic_order, receipt.artifact_order, receipt.evidence_order, receipt.provenance_order, receipt.baseline_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("governance interpretation ordering is invalid");
  for (const value of [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, ...receipt.baseline_order, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("governance interpretation digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:interpretation-assurance:"))) throw new Error("admitted interpretations require an evaluation receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:interpretation-assurance-release"])) throw new Error("empty interpretation result must be explicitly blocked");
}

export function governanceInterpretationAssuranceReceiptDigest(receipt: GovernanceInterpretationAssuranceReceipt): string { validateGovernanceInterpretationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface OracleIngestionControlReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  federation_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  modality_order: string[];
  accepted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  study_order: string[];
  semantic_profile_order: string[];
  artifact_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  aggregate_manifest: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateOracleIngestionControlReceipt(receipt: OracleIngestionControlReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== ORACLE_INGESTION_CONTROL_FEATURE_ID || receipt.contract_version !== ORACLE_INGESTION_CONTROL_CONTRACT_VERSION) throw new Error("oracle ingestion control schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.modality_order.length || !receipt.effect_receipts.length) throw new Error("oracle ingestion identity, modalities, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("oracle ingestion disposition is unknown");
  for (const values of [receipt.modality_order, receipt.accepted_order, receipt.blocked_order, receipt.unknown_order, receipt.study_order, receipt.semantic_profile_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, receipt.artifact_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("oracle ingestion ordering is invalid");
  for (const value of [...receipt.artifact_order, ...receipt.provenance_order, receipt.replay_identity, receipt.aggregate_manifest.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("oracle ingestion digest is invalid");
  if (receipt.accepted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:aggregate-ingestion-manifest:"))) throw new Error("accepted modalities require aggregate manifest exchange");
  if (!receipt.accepted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:federated-ingestion-release"])) throw new Error("empty ingestion result must be explicitly blocked");
}

export function oracleIngestionControlReceiptDigest(receipt: OracleIngestionControlReceipt): string { validateOracleIngestionControlReceipt(receipt); return digestJsonSync(receipt); }

export interface StewardshipReleaseWorkbenchReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  federation_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  object_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  origin_order: string[];
  artifact_order: string[];
  provenance_order: string[];
  evidence_order: string[];
  release_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  federation_manifest: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateStewardshipReleaseWorkbenchReceipt(receipt: StewardshipReleaseWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== STEWARDSHIP_RELEASE_WORKBENCH_FEATURE_ID || receipt.contract_version !== STEWARDSHIP_RELEASE_WORKBENCH_CONTRACT_VERSION) throw new Error("stewardship release workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.object_order.length || !receipt.effect_receipts.length) throw new Error("stewardship release identity, objects, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("stewardship release disposition is unknown");
  for (const values of [receipt.object_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.origin_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, receipt.artifact_order, receipt.provenance_order, receipt.evidence_order, receipt.release_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("stewardship release ordering is invalid");
  for (const value of [...receipt.artifact_order, ...receipt.provenance_order, ...receipt.evidence_order, ...receipt.release_order, receipt.replay_identity, receipt.federation_manifest.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("stewardship release digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:signed-research-object-manifest:"))) throw new Error("admitted releases require signed manifest exchange");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:release-workbench-publish"])) throw new Error("empty release result must be explicitly blocked");
}

export function stewardshipReleaseWorkbenchReceiptDigest(receipt: StewardshipReleaseWorkbenchReceipt): string { validateStewardshipReleaseWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface ApiAnalysisAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  question_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  result_id: string;
  estimand: string;
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  selected_candidate: string | null;
  class_order: string[];
  result_order: string[];
  model_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  replay_identity: string;
  benchmark_digest: string;
  evidence_receipt_digest: string;
  artifact: Record<string, unknown>;
  checks: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  raw_data_local: boolean;
  boundary: string;
}

export function validateApiAnalysisAssuranceReceipt(receipt: ApiAnalysisAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== API_ANALYSIS_ASSURANCE_FEATURE_ID || receipt.contract_version !== API_ANALYSIS_ASSURANCE_CONTRACT_VERSION) throw new Error("API analysis assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.question_id.trim() || !receipt.result_id.trim() || !receipt.estimand.trim() || !receipt.candidate_order.length || !receipt.checks.length || !receipt.effect_receipts.length) throw new Error("API analysis assurance identity, candidates, checks, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("API analysis assurance disposition is unknown");
  if (receipt.disposition === "qualified" && receipt.selected_candidate === null) throw new Error("qualified analysis assurance needs a selected candidate");
  for (const values of [receipt.candidate_order, receipt.blocked_order, receipt.class_order, receipt.result_order, receipt.model_order, receipt.evidence_order, receipt.provenance_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.checks, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("API analysis assurance ordering is invalid");
  if (receipt.admitted_order.some((candidate) => !receipt.candidate_order.includes(candidate))) throw new Error("API analysis assurance admitted candidate is not covered");
  for (const value of [...receipt.result_order, ...receipt.model_order, ...receipt.evidence_order, ...receipt.provenance_order, receipt.replay_identity, receipt.benchmark_digest, receipt.evidence_receipt_digest, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("API analysis assurance digest is invalid");
  if (!receipt.effect_receipts.some((effect) => effect.startsWith("exchange:digest-only-analysis-assurance:") || effect.startsWith("block:unsafe-release:"))) throw new Error("API analysis assurance effect receipt is not an exchange or fail-closed block");
}

export function apiAnalysisAssuranceReceiptDigest(receipt: ApiAnalysisAssuranceReceipt): string { validateApiAnalysisAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface StoreEvidenceOperationsReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  feed_id: string;
  workflow_id: string;
  federation_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  alert_order: string[];
  qualified_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  source_order: string[];
  provenance_order: string[];
  evidence_order: string[];
  checkpoint_id: string;
  replay_identity: string;
  telemetry_digest: string;
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  federation_manifest: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateStoreEvidenceOperationsReceipt(receipt: StoreEvidenceOperationsReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== STORE_EVIDENCE_OPERATIONS_FEATURE_ID || receipt.contract_version !== STORE_EVIDENCE_OPERATIONS_CONTRACT_VERSION) throw new Error("store evidence operations schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.feed_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.checkpoint_id.trim() || !receipt.alert_order.length || !receipt.effect_receipts.length) throw new Error("store evidence operations identity, alerts, locality, checkpoint, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("store evidence operations disposition is unknown");
  for (const values of [receipt.alert_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, receipt.source_order, receipt.provenance_order, receipt.evidence_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("store evidence operations ordering is invalid");
  if ([...receipt.qualified_order, ...receipt.blocked_order, ...receipt.unknown_order].some((alert) => !receipt.alert_order.includes(alert))) throw new Error("store evidence operations state is not covered by alert order");
  for (const value of [...receipt.source_order, ...receipt.provenance_order, ...receipt.evidence_order, receipt.replay_identity, receipt.telemetry_digest, receipt.federation_manifest.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("store evidence operations digest is invalid");
  if (!receipt.effect_receipts.some((effect) => effect.startsWith("checkpoint:evidence-operations:") || effect.startsWith("exchange:permitted-evidence-summary:") || effect.startsWith("block:evidence-operations-release:"))) throw new Error("store evidence operations effect is not checkpoint, exchange, or fail-closed block");
}

export function storeEvidenceOperationsReceiptDigest(receipt: StoreEvidenceOperationsReceipt): string { validateStoreEvidenceOperationsReceipt(receipt); return digestJsonSync(receipt); }

export interface PolicyInteroperabilityControlReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  federation_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  offer_order: string[];
  accepted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  capability_order: string[];
  schema_order: string[];
  input_order: string[];
  output_order: string[];
  provenance_order: string[];
  evidence_order: string[];
  migration_order: string[];
  replay_identity: string;
  benchmark_digest: string;
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  integration_artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validatePolicyInteroperabilityControlReceipt(receipt: PolicyInteroperabilityControlReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== POLICY_INTEROPERABILITY_CONTROL_FEATURE_ID || receipt.contract_version !== POLICY_INTEROPERABILITY_CONTROL_CONTRACT_VERSION) throw new Error("policy interoperability control schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.offer_order.length || !receipt.effect_receipts.length) throw new Error("policy interoperability identity, offers, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("policy interoperability disposition is unknown");
  for (const values of [receipt.offer_order, receipt.accepted_order, receipt.blocked_order, receipt.unknown_order, receipt.capability_order, receipt.schema_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("policy interoperability ordering is invalid");
  if ([...receipt.accepted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((offer) => !receipt.offer_order.includes(offer))) throw new Error("policy interoperability state is not covered by offer order");
  for (const values of [receipt.input_order, receipt.output_order, receipt.provenance_order, receipt.evidence_order, receipt.migration_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("policy interoperability digest ordering is invalid");
  for (const value of [...receipt.input_order, ...receipt.output_order, ...receipt.provenance_order, ...receipt.evidence_order, ...receipt.migration_order, receipt.replay_identity, receipt.benchmark_digest, receipt.integration_artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("policy interoperability digest is invalid");
  if (!receipt.effect_receipts.some((effect) => effect.startsWith("exchange:permitted-capability-summary:") || effect.startsWith("block:policy-interoperability-release:"))) throw new Error("policy interoperability effect is not exchange or fail-closed block");
}

export function policyInteroperabilityControlReceiptDigest(receipt: PolicyInteroperabilityControlReceipt): string { validatePolicyInteroperabilityControlReceipt(receipt); return digestJsonSync(receipt); }

export interface SafetyMechanismWorkflowReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  federation_id: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  ranked_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  mechanism_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  action_order: string[];
  replay_identity: string;
  benchmark_digest: string;
  checkpoint_id: string;
  checkpoint_digest: string;
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  effect_receipts: string[];
  workflow_artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateSafetyMechanismWorkflowReceipt(receipt: SafetyMechanismWorkflowReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== SAFETY_MECHANISM_WORKFLOW_FEATURE_ID || receipt.contract_version !== SAFETY_MECHANISM_WORKFLOW_CONTRACT_VERSION) throw new Error("safety mechanism workflow schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.checkpoint_id.trim() || !receipt.candidate_order.length || !receipt.ranked_order.length || !receipt.effect_receipts.length) throw new Error("safety mechanism workflow identity, candidates, checkpoint, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("safety mechanism workflow disposition is unknown");
  for (const values of [receipt.candidate_order, receipt.ranked_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.mechanism_order, receipt.action_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts, receipt.evidence_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("safety mechanism workflow ordering is invalid");
  if ([...receipt.ranked_order, ...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((candidate) => !receipt.candidate_order.includes(candidate))) throw new Error("safety mechanism workflow state is not covered by candidate order");
  for (const value of [...receipt.evidence_order, ...receipt.provenance_order, receipt.replay_identity, receipt.benchmark_digest, receipt.checkpoint_digest, receipt.workflow_artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("safety mechanism workflow digest is invalid");
  if (!receipt.effect_receipts.some((effect) => effect.startsWith("schedule:approved-workflow:") || effect.startsWith("checkpoint:mechanism-workflow:") || effect.startsWith("block:safety-workflow:"))) throw new Error("safety mechanism workflow effect is not schedule, checkpoint, or fail-closed block");
}

export function safetyMechanismWorkflowReceiptDigest(receipt: SafetyMechanismWorkflowReceipt): string { validateSafetyMechanismWorkflowReceipt(receipt); return digestJsonSync(receipt); }

export interface HubapiMultimodalInterpretationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  objective_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  ranked_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  result_order: string[];
  visualization_order: string[];
  study_order: string[];
  modality_order: string[];
  support_order: number[];
  semantic_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  comparability_order: string[];
  baseline_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateHubapiMultimodalInterpretationAssuranceReceipt(receipt: HubapiMultimodalInterpretationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== HUBAPI_INTERPRETATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== HUBAPI_INTERPRETATION_ASSURANCE_CONTRACT_VERSION) throw new Error("hubapi interpretation assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.objective_id.trim() || !receipt.scope.trim() || !receipt.ranked_order.length || !receipt.study_order.length || !receipt.modality_order.length || !receipt.effect_receipts.length) throw new Error("hubapi interpretation identity, coverage, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("hubapi interpretation disposition is unknown");
  if (receipt.support_order.length !== receipt.ranked_order.length || [...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.ranked_order.includes(value))) throw new Error("hubapi interpretation support or disposition linkage is incomplete");
  for (const values of [receipt.ranked_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.result_order, receipt.visualization_order, receipt.study_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("hubapi interpretation ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.evidence_order, receipt.provenance_order, receipt.comparability_order, receipt.baseline_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("hubapi interpretation digest ordering is invalid");
  const digests = [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, ...receipt.comparability_order, ...receipt.baseline_order, receipt.replay_identity, receipt.artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("hubapi interpretation digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:interpretation-assurance:"))) throw new Error("admitted interpretations require an evaluation receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty interpretation result must be explicitly blocked");
}

export function hubapiMultimodalInterpretationAssuranceReceiptDigest(receipt: HubapiMultimodalInterpretationAssuranceReceipt): string { validateHubapiMultimodalInterpretationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface BiolangPublicationCopilotReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  ranked_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  release_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  tool_invocation_order: string[];
  provenance_order: string[];
  replay_order: string[];
  benchmark_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  objects: Record<string, unknown>[];
  publication_artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateBiolangPublicationCopilotReceipt(receipt: BiolangPublicationCopilotReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== BIOLANG_PUBLICATION_COPILOT_FEATURE_ID || receipt.contract_version !== BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION) throw new Error("biolang publication copilot schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.scope.trim() || !receipt.ranked_order.length || !receipt.effect_receipts.length) throw new Error("publication copilot identity, ranking, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("publication copilot disposition is unknown");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.ranked_order.includes(value))) throw new Error("publication copilot candidate state is not covered by ranking");
  for (const values of [receipt.ranked_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.release_order, receipt.artifact_order, receipt.evidence_order, receipt.tool_invocation_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("publication copilot ordering is invalid");
  for (const values of [receipt.provenance_order, receipt.replay_order, receipt.benchmark_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("publication copilot digest ordering is invalid");
  const digests = [...receipt.provenance_order, ...receipt.replay_order, ...receipt.benchmark_order, receipt.replay_identity, receipt.publication_artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("publication copilot digest is invalid");
  for (const object of receipt.objects) if (object.raw_data_local !== true || object.boundary !== PRECLINICAL_BOUNDARY || typeof object.run_id !== "string" || !object.run_id.trim() || typeof object.release_id !== "string" || !object.release_id.trim() || !Array.isArray(object.artifact_ids) || !object.artifact_ids.length || !Array.isArray(object.evidence_receipt_ids) || !object.evidence_receipt_ids.length) throw new Error("signed research object is incomplete or non-local");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("invoke:declared-tools:"))) throw new Error("admitted releases require a declared-tool invocation receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty publication result must be explicitly blocked");
}

export function biolangPublicationCopilotReceiptDigest(receipt: BiolangPublicationCopilotReceipt): string { validateBiolangPublicationCopilotReceipt(receipt); return digestJsonSync(receipt); }

export interface ApiReleaseAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  release_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  replay_order: string[];
  benchmark_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  objects: Record<string, unknown>[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateApiReleaseAssuranceReceipt(receipt: ApiReleaseAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== API_RELEASE_ASSURANCE_FEATURE_ID || receipt.contract_version !== API_RELEASE_ASSURANCE_CONTRACT_VERSION) throw new Error("API release assurance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.scope.trim() || !receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("API release identity, candidates, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("API release disposition is unknown");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("API release candidate state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.release_order, receipt.artifact_order, receipt.evidence_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("API release ordering is invalid");
  for (const values of [receipt.provenance_order, receipt.replay_order, receipt.benchmark_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("API release digest ordering is invalid");
  const digests = [...receipt.provenance_order, ...receipt.replay_order, ...receipt.benchmark_order, receipt.replay_identity, receipt.artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("API release digest is invalid");
  for (const object of receipt.objects) if (object.raw_data_local !== true || object.boundary !== PRECLINICAL_BOUNDARY || typeof object.run_id !== "string" || !object.run_id.trim() || typeof object.release_id !== "string" || !object.release_id.trim() || !Array.isArray(object.artifact_ids) || !object.artifact_ids.length || !Array.isArray(object.evidence_receipt_ids) || !object.evidence_receipt_ids.length) throw new Error("API release object is incomplete or non-local");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:release-assurance:"))) throw new Error("admitted releases require an evaluation receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty release result must be explicitly blocked");
}

export function apiReleaseAssuranceReceiptDigest(receipt: ApiReleaseAssuranceReceipt): string { validateApiReleaseAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface BioevalxFederationGatewayReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  federation_id: string;
  endpoint: string;
  protocol: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  release_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  replay_order: string[];
  benchmark_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  objects: Record<string, unknown>[];
  federation_artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateBioevalxFederationGatewayReceipt(receipt: BioevalxFederationGatewayReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== BIOEVALX_FEDERATION_GATEWAY_FEATURE_ID || receipt.contract_version !== BIOEVALX_FEDERATION_GATEWAY_CONTRACT_VERSION) throw new Error("bioevalx federation schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.endpoint.trim() || !receipt.protocol.trim() || !receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("federation identity, endpoint, protocol, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("federation disposition is unknown");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("federation candidate state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.release_order, receipt.artifact_order, receipt.evidence_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federation ordering is invalid");
  for (const values of [receipt.provenance_order, receipt.replay_order, receipt.benchmark_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federation digest ordering is invalid");
  const digests = [...receipt.provenance_order, ...receipt.replay_order, ...receipt.benchmark_order, receipt.replay_identity, receipt.federation_artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("federation digest is invalid");
  for (const object of receipt.objects) if (object.raw_data_local !== true || object.boundary !== PRECLINICAL_BOUNDARY || object.endpoint !== receipt.endpoint || object.protocol !== receipt.protocol || !Array.isArray(object.artifact_ids) || !object.artifact_ids.length || !Array.isArray(object.evidence_receipt_ids) || !object.evidence_receipt_ids.length) throw new Error("federation object is incomplete or inconsistent");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:permitted-artifacts:"))) throw new Error("admitted releases require permitted-artifact exchange");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:federation-release"])) throw new Error("empty federation result must be explicitly blocked");
}

export function bioevalxFederationGatewayReceiptDigest(receipt: BioevalxFederationGatewayReceipt): string { validateBioevalxFederationGatewayReceipt(receipt); return digestJsonSync(receipt); }

export interface SectionInterpretationAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  federation_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  result_order: string[];
  visualization_order: string[];
  study_order: string[];
  modality_order: string[];
  support_order: number[];
  semantic_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  comparability_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  interpretations: Record<string, unknown>[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateSectionInterpretationAssuranceReceipt(receipt: SectionInterpretationAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== SECTION_INTERPRETATION_ASSURANCE_FEATURE_ID || receipt.contract_version !== SECTION_INTERPRETATION_ASSURANCE_CONTRACT_VERSION) throw new Error("section interpretation schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.scope.trim() || !receipt.candidate_order.length || receipt.support_order.length !== receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("section interpretation identity, ranking, support, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("section interpretation disposition is unknown");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("section interpretation state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.result_order, receipt.visualization_order, receipt.study_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("section interpretation ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.evidence_order, receipt.provenance_order, receipt.comparability_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("section interpretation digest ordering is invalid");
  const digests = [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, ...receipt.comparability_order, receipt.replay_identity, receipt.artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("section interpretation digest is invalid");
  for (const interpretation of receipt.interpretations) if (interpretation.raw_data_local !== true || interpretation.boundary !== PRECLINICAL_BOUNDARY || !interpretation.comparability_digest) throw new Error("interactive interpretation is incomplete or non-local");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:interpretation-assurance:"))) throw new Error("admitted interpretations require an assurance effect");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty interpretation result must be explicitly blocked");
}

export function sectionInterpretationAssuranceReceiptDigest(receipt: SectionInterpretationAssuranceReceipt): string { validateSectionInterpretationAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface OpsRetrievalAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  study_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  source_order: string[];
  modality_order: string[];
  support_order: number[];
  semantic_order: string[];
  artifact_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateOpsRetrievalAssuranceReceipt(receipt: OpsRetrievalAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== OPS_RETRIEVAL_ASSURANCE_FEATURE_ID || receipt.contract_version !== OPS_RETRIEVAL_ASSURANCE_CONTRACT_VERSION) throw new Error("ops retrieval schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || !receipt.candidate_order.length || receipt.support_order.length !== receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("retrieval identity, ranking, support, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("retrieval disposition is unknown");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("retrieval candidate state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.source_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("retrieval ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("retrieval digest ordering is invalid");
  const digests = [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.provenance_order, receipt.replay_identity, receipt.artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("retrieval digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:retrieval-assurance:"))) throw new Error("admitted retrieval requires an evaluation receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty retrieval result must be explicitly blocked");
}

export function opsRetrievalAssuranceReceiptDigest(receipt: OpsRetrievalAssuranceReceipt): string { validateOpsRetrievalAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface ConformanceKnowledgeWorldAssuranceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  workflow_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  admitted_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  predicate_order: string[];
  study_order: string[];
  modality_order: string[];
  support_order: number[];
  semantic_order: string[];
  artifact_order: string[];
  evidence_order: string[];
  provenance_order: string[];
  comparability_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  benchmark_digest: string | null;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateConformanceKnowledgeWorldAssuranceReceipt(receipt: ConformanceKnowledgeWorldAssuranceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_FEATURE_ID || receipt.contract_version !== CONFORMANCE_KNOWLEDGE_WORLD_ASSURANCE_CONTRACT_VERSION) throw new Error("conformance knowledge-world schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.scope.trim() || !receipt.candidate_order.length || receipt.support_order.length !== receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("knowledge-world identity, ranking, support, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("knowledge-world disposition is unknown");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("knowledge-world state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.predicate_order, receipt.study_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("knowledge-world ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.evidence_order, receipt.provenance_order, receipt.comparability_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("knowledge-world digest ordering is invalid");
  const digests = [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.evidence_order, ...receipt.provenance_order, ...receipt.comparability_order, receipt.replay_identity, receipt.artifact.content_hash];
  if (receipt.benchmark_digest !== null) digests.push(receipt.benchmark_digest);
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("knowledge-world digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("evaluate:knowledge-world-assurance:"))) throw new Error("admitted knowledge world requires an evaluation receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty knowledge world must be explicitly blocked");
}

export function conformanceKnowledgeWorldAssuranceReceiptDigest(receipt: ConformanceKnowledgeWorldAssuranceReceipt): string { validateConformanceKnowledgeWorldAssuranceReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainEvidenceSurveillanceReceipt {
  schema_version: string;
  contract_version: string;
  feature_id: string;
  request_id: string;
  study_id: string;
  scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[];
  qualified_order: string[];
  blocked_order: string[];
  unknown_order: string[];
  source_order: string[];
  modality_order: string[];
  relevance_order: number[];
  semantic_order: string[];
  artifact_order: string[];
  provenance_order: string[];
  omissions: string[];
  uncertainty: string[];
  negative_evidence: string[];
  replay_identity: string;
  effect_receipts: string[];
  artifact: Record<string, unknown>;
  raw_data_local: boolean;
  boundary: string;
}

export function validateBrainEvidenceSurveillanceReceipt(receipt: BrainEvidenceSurveillanceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== BRAIN_EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== BRAIN_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("brain surveillance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || !receipt.candidate_order.length || receipt.relevance_order.length !== receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("evidence identity, ranking, relevance, locality, or effects are incomplete");
  if (!new Set(["qualified", "partial", "unknown", "blocked"]).has(receipt.disposition)) throw new Error("brain surveillance disposition is unknown");
  if ([...receipt.qualified_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("evidence state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.source_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("brain surveillance ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("brain surveillance digest ordering is invalid");
  const digests = [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.provenance_order, receipt.replay_identity, receipt.artifact.content_hash];
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("brain surveillance digest is invalid");
  if (receipt.qualified_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:"))) throw new Error("qualified evidence requires a local-read receipt");
  if (!receipt.qualified_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty evidence result must be explicitly blocked");
}

export function brainEvidenceSurveillanceReceiptDigest(receipt: BrainEvidenceSurveillanceReceipt): string { validateBrainEvidenceSurveillanceReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainMultimodalEvidenceSurveillanceReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; study_order: string[]; scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; candidate_order: string[]; qualified_order: string[];
  blocked_order: string[]; unknown_order: string[]; source_order: string[]; modality_order: string[]; relevance_order: number[];
  semantic_order: string[]; artifact_order: string[]; provenance_order: string[]; omissions: string[]; uncertainty: string[];
  negative_evidence: string[]; replay_identity: string; effect_receipts: string[]; artifact: Record<string, unknown>;
  raw_data_local: boolean; boundary: string;
}

export function validateBrainMultimodalEvidenceSurveillanceReceipt(receipt: BrainMultimodalEvidenceSurveillanceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== BRAIN_MULTIMODAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("multimodal brain surveillance schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.scope.trim() || receipt.study_order.length < 2 || !receipt.candidate_order.length || receipt.relevance_order.length !== receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("multimodal identity, study coverage, ranking, locality, or effects are incomplete");
  if ([...receipt.qualified_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("multimodal state is not covered by candidate order");
  for (const values of [receipt.study_order, receipt.candidate_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.source_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal digest ordering is invalid");
  const digests = [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.provenance_order, receipt.replay_identity, receipt.artifact.content_hash];
  for (const value of digests) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("multimodal brain surveillance digest is invalid");
  if (receipt.qualified_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:"))) throw new Error("qualified multimodal evidence requires a local-read receipt");
  if (!receipt.qualified_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty multimodal evidence result must be explicitly blocked");
}

export function brainMultimodalEvidenceSurveillanceReceiptDigest(receipt: BrainMultimodalEvidenceSurveillanceReceipt): string { validateBrainMultimodalEvidenceSurveillanceReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainHighThroughputEvidenceReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; batch_id: string; partition: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; candidate_order: string[]; admitted_order: string[];
  blocked_order: string[]; unknown_order: string[]; relevance_order: number[]; omissions: string[]; uncertainty: string[];
  negative_evidence: string[]; checkpoint_seq: number; queue_digest: string; replay_identity: string; effect_receipts: string[];
  artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainHighThroughputEvidenceReceipt(receipt: BrainHighThroughputEvidenceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== HIGH_THROUGHPUT_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("throughput evidence schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.batch_id.trim() || !receipt.partition.trim() || !receipt.candidate_order.length || receipt.relevance_order.length !== receipt.candidate_order.length || receipt.checkpoint_seq < 1 || !receipt.effect_receipts.length) throw new Error("throughput identity, checkpoint, ranking, locality, or effects are incomplete");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("throughput state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("throughput ordering is invalid");
  for (const value of [receipt.queue_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("throughput digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:"))) throw new Error("admitted batch requires a local-read receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty batch must be explicitly blocked");
}

export function brainHighThroughputEvidenceReceiptDigest(receipt: BrainHighThroughputEvidenceReceipt): string { validateBrainHighThroughputEvidenceReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainFederatedEvidenceReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; federation_id: string; institution_id: string;
  purpose: string; semantic_profile: string; endpoint: string; disposition: "qualified" | "partial" | "unknown" | "blocked";
  candidate_order: string[]; admitted_order: string[]; blocked_order: string[]; unknown_order: string[]; aggregate_order: string[];
  omissions: string[]; uncertainty: string[]; negative_evidence: string[]; replay_identity: string; effect_receipts: string[];
  artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainFederatedEvidenceReceipt(receipt: BrainFederatedEvidenceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_EVIDENCE_SURVEILLANCE_FEATURE_ID || receipt.contract_version !== FEDERATED_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION) throw new Error("federated evidence schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.institution_id.trim() || !receipt.purpose.trim() || !receipt.semantic_profile.trim() || !receipt.endpoint.trim() || !receipt.candidate_order.length || !receipt.effect_receipts.length) throw new Error("federated identity, envelope, locality, ranking, or effects are incomplete");
  if ([...receipt.admitted_order, ...receipt.blocked_order, ...receipt.unknown_order].some((value) => !receipt.candidate_order.includes(value))) throw new Error("federated state is not covered by candidate order");
  for (const values of [receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.aggregate_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federated ordering is invalid");
  for (const value of [...receipt.aggregate_order, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("federated digest is invalid");
  if (receipt.admitted_order.length && receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:permitted-artifacts:"))) throw new Error("admitted federation requires a permitted-artifact exchange receipt");
  if (!receipt.admitted_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("empty federation result must be explicitly blocked");
}

export function brainFederatedEvidenceReceiptDigest(receipt: BrainFederatedEvidenceReceipt): string { validateBrainFederatedEvidenceReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainEvidenceContractModelReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; study_id: string; scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; compatibility: "additive" | "migration_required" | "breaking" | "unknown";
  input_schema: string; output_schema: string; required_order: string[]; provided_order: string[]; missing_order: string[]; semantic_loss_order: string[];
  semantic_digest: string; artifact_digest: string; provenance_digest: string; contract_digest: string; replay_identity: string;
  omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>;
  raw_data_local: boolean; boundary: string;
}

export function validateBrainEvidenceContractModelReceipt(receipt: BrainEvidenceContractModelReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_CONTRACT_MODEL_FEATURE_ID || receipt.contract_version !== EVIDENCE_CONTRACT_MODEL_CONTRACT_VERSION) throw new Error("evidence contract schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || receipt.input_schema !== "EvidenceFeed1@1" || receipt.output_schema !== "QualifiedEvidenceSet2@1" || !receipt.required_order.length || !receipt.provided_order.length || !receipt.effect_receipts.length) throw new Error("contract identity, schemas, fields, locality, or effects are incomplete");
  if ([...receipt.missing_order].some((value) => !receipt.required_order.includes(value)) || [...receipt.semantic_loss_order].some((value) => !receipt.provided_order.includes(value))) throw new Error("contract loss state is outside declared fields");
  for (const values of [receipt.required_order, receipt.provided_order, receipt.missing_order, receipt.semantic_loss_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("contract ordering is invalid");
  for (const value of [receipt.semantic_digest, receipt.artifact_digest, receipt.provenance_digest, receipt.contract_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("contract digest is invalid");
  if (receipt.disposition === "qualified" && receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:"))) throw new Error("qualified contract requires a local-read receipt");
  if (receipt.disposition !== "qualified" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-qualified contract must be explicitly blocked");
}

export function brainEvidenceContractModelReceiptDigest(receipt: BrainEvidenceContractModelReceipt): string { validateBrainEvidenceContractModelReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainMultimodalContractModelReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; study_order: string[]; scope: string; comparability_profile: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; compatibility: "additive" | "migration_required" | "breaking" | "unknown";
  input_schema: string; output_schema: string; modality_order: string[]; binding_order: string[]; missing_order: string[]; semantic_disagreement_order: string[];
  schema_order: string[]; unit_order: string[]; coordinate_order: string[]; semantic_order: string[]; artifact_order: string[]; provenance_order: string[];
  contract_digest: string; replay_identity: string; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[];
  artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainMultimodalContractModelReceipt(receipt: BrainMultimodalContractModelReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MULTIMODAL_CONTRACT_MODEL_FEATURE_ID || receipt.contract_version !== MULTIMODAL_CONTRACT_MODEL_CONTRACT_VERSION) throw new Error("multimodal contract schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || receipt.study_order.length < 2 || !receipt.scope.trim() || !receipt.comparability_profile.trim() || receipt.input_schema !== "EvidenceFeed2@1" || receipt.output_schema !== "QualifiedEvidenceSet2@1" || receipt.modality_order.length < 2 || !receipt.binding_order.length || !receipt.effect_receipts.length) throw new Error("multimodal identity, schemas, study/modality closure, locality, or effects are incomplete");
  for (const values of [receipt.study_order, receipt.modality_order, receipt.binding_order, receipt.missing_order, receipt.semantic_disagreement_order, receipt.schema_order, receipt.unit_order, receipt.coordinate_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal contract ordering is invalid");
  for (const values of [receipt.semantic_order, receipt.artifact_order, receipt.provenance_order]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal contract digest ordering is invalid");
  for (const value of [...receipt.semantic_order, ...receipt.artifact_order, ...receipt.provenance_order, receipt.contract_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("multimodal contract digest is invalid");
  if (receipt.disposition === "qualified" && receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:"))) throw new Error("qualified multimodal contract requires a local-read receipt");
  if (receipt.disposition !== "qualified" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-qualified multimodal contract must be explicitly blocked");
}

export function brainMultimodalContractModelReceiptDigest(receipt: BrainMultimodalContractModelReceipt): string { validateBrainMultimodalContractModelReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainThroughputContractModelReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; batch_id: string; partition: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; compatibility: "additive" | "migration_required" | "breaking" | "unknown";
  input_schema: string; output_schema: string; required_order: string[]; provided_order: string[]; missing_order: string[]; semantic_loss_order: string[];
  max_items: number; observed_items: number; admitted_items: number; checkpoint_seq: number; queue_digest: string; contract_digest: string; replay_identity: string;
  omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainThroughputContractModelReceipt(receipt: BrainThroughputContractModelReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== THROUGHPUT_CONTRACT_MODEL_FEATURE_ID || receipt.contract_version !== THROUGHPUT_CONTRACT_MODEL_CONTRACT_VERSION) throw new Error("throughput contract schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.batch_id.trim() || !receipt.partition.trim() || receipt.input_schema !== "EvidenceFeed3@1" || receipt.output_schema !== "QualifiedEvidenceSet2@1" || !receipt.required_order.length || !receipt.provided_order.length || receipt.max_items < 1 || receipt.checkpoint_seq < 1 || !receipt.effect_receipts.length) throw new Error("throughput identity, schemas, fields, capacity, checkpoint, locality, or effects are incomplete");
  if (receipt.admitted_items > receipt.max_items || receipt.admitted_items > receipt.observed_items) throw new Error("admitted item count exceeds declared capacity or observations");
  if (receipt.missing_order.some((value) => !receipt.required_order.includes(value)) || receipt.semantic_loss_order.some((value) => !receipt.provided_order.includes(value))) throw new Error("throughput loss state is outside declared fields");
  for (const values of [receipt.required_order, receipt.provided_order, receipt.missing_order, receipt.semantic_loss_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("throughput contract ordering is invalid");
  for (const value of [receipt.queue_digest, receipt.contract_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("throughput contract digest is invalid");
  if (receipt.disposition === "qualified" && receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:"))) throw new Error("qualified throughput contract requires a local-read receipt");
  if (receipt.disposition !== "qualified" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-qualified throughput contract must be explicitly blocked");
}

export function brainThroughputContractModelReceiptDigest(receipt: BrainThroughputContractModelReceipt): string { validateBrainThroughputContractModelReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainFederatedContractModelReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; federation_id: string; institution_id: string; purpose: string; endpoint: string; semantic_profile: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; compatibility: "additive" | "migration_required" | "breaking" | "unknown"; input_schema: string; output_schema: string;
  required_order: string[]; provided_order: string[]; missing_order: string[]; semantic_loss_order: string[]; allowed_artifact_order: string[]; export_scope: string;
  semantic_digest: string; provenance_digest: string; contract_digest: string; envelope_digest: string; replay_identity: string; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainFederatedContractModelReceipt(receipt: BrainFederatedContractModelReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_CONTRACT_MODEL_FEATURE_ID || receipt.contract_version !== FEDERATED_CONTRACT_MODEL_CONTRACT_VERSION) throw new Error("federated contract schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.federation_id.trim() || !receipt.institution_id.trim() || !receipt.purpose.trim() || !receipt.endpoint.trim() || !receipt.semantic_profile.trim() || receipt.input_schema !== "EvidenceFeed4@1" || receipt.output_schema !== "QualifiedEvidenceSet2@1" || !receipt.required_order.length || !receipt.provided_order.length || !receipt.allowed_artifact_order.length || !receipt.export_scope.trim() || !receipt.effect_receipts.length) throw new Error("federated identity, schemas, fields, artifact policy, export scope, locality, or effects are incomplete");
  if (receipt.missing_order.some((value) => !receipt.required_order.includes(value)) || receipt.semantic_loss_order.some((value) => !receipt.provided_order.includes(value))) throw new Error("federated loss state is outside declared fields");
  for (const values of [receipt.required_order, receipt.provided_order, receipt.missing_order, receipt.semantic_loss_order, receipt.allowed_artifact_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federated contract ordering is invalid");
  for (const value of [receipt.semantic_digest, receipt.provenance_digest, receipt.contract_digest, receipt.envelope_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("federated contract digest is invalid");
  if (receipt.disposition === "qualified" && receipt.effect_receipts.some((effect) => !effect.startsWith("exchange:permitted-artifacts:"))) throw new Error("qualified federation requires a permitted-artifact exchange receipt");
  if (receipt.disposition !== "qualified" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-qualified federation must be explicitly blocked");
}

export function brainFederatedContractModelReceiptDigest(receipt: BrainFederatedContractModelReceipt): string { validateBrainFederatedContractModelReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainEvidenceResearchCopilotReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; operator_id: string; study_id: string; scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; plan_order: string[]; action_order: string[]; candidate_order: string[];
  qualified_order: string[]; blocked_order: string[]; unknown_order: string[]; evidence_receipt_digest: string; plan_digest: string;
  replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[];
  effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainEvidenceResearchCopilotReceipt(receipt: BrainEvidenceResearchCopilotReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_RESEARCH_COPILOT_FEATURE_ID || receipt.contract_version !== EVIDENCE_RESEARCH_COPILOT_CONTRACT_VERSION) throw new Error("evidence copilot schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.operator_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || !receipt.plan_order.length || !receipt.action_order.length || receipt.plan_order.length !== receipt.action_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("evidence copilot identity, bounded plan, locality, budget, or effects are incomplete");
  if (receipt.qualified_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("evidence copilot state is not covered by candidate order");
  for (const values of [receipt.plan_order, receipt.action_order, receipt.candidate_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("evidence copilot ordering is invalid");
  for (const value of [receipt.evidence_receipt_digest, receipt.plan_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("evidence copilot digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("read:local-research-artifacts:") && effect !== "block:unsafe-release")) throw new Error("evidence copilot effect is outside local read/compute gate");
  if (receipt.qualified_order.length && !receipt.effect_receipts.some((effect) => effect.startsWith("read:local-research-artifacts:"))) throw new Error("qualified copilot plan requires a local read receipt");
  if (!receipt.qualified_order.length && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-qualified copilot plan must be explicitly blocked");
}

export function brainEvidenceResearchCopilotReceiptDigest(receipt: BrainEvidenceResearchCopilotReceipt): string { validateBrainEvidenceResearchCopilotReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainMultimodalEvidenceResearchCopilotReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; operator_id: string; study_order: string[]; scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; plan_order: string[]; action_order: string[]; tool_order: string[]; candidate_order: string[];
  qualified_order: string[]; blocked_order: string[]; unknown_order: string[]; modality_order: string[]; evidence_receipt_digest: string; plan_digest: string;
  approval_reference: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[];
  effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainMultimodalEvidenceResearchCopilotReceipt(receipt: BrainMultimodalEvidenceResearchCopilotReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MULTIMODAL_EVIDENCE_COPILOT_FEATURE_ID || receipt.contract_version !== MULTIMODAL_EVIDENCE_COPILOT_CONTRACT_VERSION) throw new Error("multimodal copilot schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.operator_id.trim() || receipt.study_order.length < 2 || !receipt.scope.trim() || !receipt.plan_order.length || !receipt.action_order.length || receipt.plan_order.length !== receipt.action_order.length || !receipt.tool_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("multimodal copilot identity, study floor, bounded plan, tool, locality, budget, or effects are incomplete");
  if (receipt.qualified_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("multimodal copilot state is not covered by candidate order");
  for (const values of [receipt.study_order, receipt.plan_order, receipt.action_order, receipt.tool_order, receipt.candidate_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.modality_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal copilot ordering is invalid");
  for (const value of [receipt.evidence_receipt_digest, receipt.plan_digest, receipt.approval_reference, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("multimodal copilot digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("invoke:declared-tool:") && effect !== "block:unsafe-release")) throw new Error("multimodal copilot effect is outside declared-tool gate");
  if (receipt.disposition !== "blocked" && receipt.qualified_order.length && !receipt.effect_receipts.some((effect) => effect.startsWith("invoke:declared-tool:"))) throw new Error("qualified multimodal plan requires a declared-tool receipt");
  if (receipt.disposition !== "qualified" && receipt.disposition !== "partial" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-qualified multimodal plan must be explicitly blocked");
}

export function brainMultimodalEvidenceResearchCopilotReceiptDigest(receipt: BrainMultimodalEvidenceResearchCopilotReceipt): string { validateBrainMultimodalEvidenceResearchCopilotReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainHighThroughputEvidenceResearchCopilotReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; operator_id: string; batch_id: string; partition: string; checkpoint_seq: number;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; plan_order: string[]; action_order: string[]; tool_order: string[]; candidate_order: string[];
  admitted_order: string[]; blocked_order: string[]; unknown_order: string[]; queue_digest: string; evidence_receipt_digest: string; plan_digest: string;
  approval_reference: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[];
  effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainHighThroughputEvidenceResearchCopilotReceipt(receipt: BrainHighThroughputEvidenceResearchCopilotReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== HIGH_THROUGHPUT_EVIDENCE_COPILOT_FEATURE_ID || receipt.contract_version !== HIGH_THROUGHPUT_EVIDENCE_COPILOT_CONTRACT_VERSION) throw new Error("throughput copilot schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.operator_id.trim() || !receipt.batch_id.trim() || !receipt.partition.trim() || !receipt.candidate_order.length || !receipt.plan_order.length || !receipt.action_order.length || receipt.plan_order.length !== receipt.action_order.length || !receipt.tool_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("throughput copilot identity, batch, bounded plan, tool, locality, budget, or effects are incomplete");
  if (receipt.admitted_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("throughput copilot state is not covered by candidate order");
  for (const values of [receipt.plan_order, receipt.action_order, receipt.tool_order, receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("throughput copilot ordering is invalid");
  for (const value of [receipt.queue_digest, receipt.evidence_receipt_digest, receipt.plan_digest, receipt.approval_reference, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("throughput copilot digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("invoke:declared-tool:") && effect !== "block:unsafe-release")) throw new Error("throughput copilot effect is outside declared-tool gate");
  if (receipt.disposition !== "blocked" && receipt.admitted_order.length && !receipt.effect_receipts.some((effect) => effect.startsWith("invoke:declared-tool:"))) throw new Error("admitted throughput batch requires a declared-tool receipt");
  if (receipt.disposition !== "qualified" && receipt.disposition !== "partial" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-admitted throughput batch must be explicitly blocked");
}

export function brainHighThroughputEvidenceResearchCopilotReceiptDigest(receipt: BrainHighThroughputEvidenceResearchCopilotReceipt): string { validateBrainHighThroughputEvidenceResearchCopilotReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainFederatedEvidenceResearchCopilotReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; operator_id: string; federation_id: string; institution_id: string; purpose: string; semantic_profile: string; endpoint: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; plan_order: string[]; action_order: string[]; tool_order: string[]; candidate_order: string[]; admitted_order: string[]; blocked_order: string[]; unknown_order: string[]; aggregate_order: string[];
  envelope_digest: string; evidence_receipt_digest: string; plan_digest: string; approval_reference: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainFederatedEvidenceResearchCopilotReceipt(receipt: BrainFederatedEvidenceResearchCopilotReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_EVIDENCE_COPILOT_FEATURE_ID || receipt.contract_version !== FEDERATED_EVIDENCE_COPILOT_CONTRACT_VERSION) throw new Error("federated copilot schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.operator_id.trim() || !receipt.federation_id.trim() || !receipt.institution_id.trim() || !receipt.purpose.trim() || !receipt.semantic_profile.trim() || !receipt.endpoint.trim() || !receipt.candidate_order.length || !receipt.plan_order.length || !receipt.action_order.length || receipt.plan_order.length !== receipt.action_order.length || !receipt.tool_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("federated copilot identity, envelope, bounded plan, tool, locality, budget, or effects are incomplete");
  if (receipt.admitted_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("federated copilot state is not covered by candidate order");
  for (const values of [receipt.plan_order, receipt.action_order, receipt.tool_order, receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.aggregate_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federated copilot ordering is invalid");
  for (const value of [...receipt.aggregate_order, receipt.envelope_digest, receipt.evidence_receipt_digest, receipt.plan_digest, receipt.approval_reference, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("federated copilot digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("invoke:declared-tool:") && effect !== "block:unsafe-release")) throw new Error("federated copilot effect is outside declared-tool gate");
  if (receipt.disposition !== "blocked" && receipt.admitted_order.length && !receipt.effect_receipts.some((effect) => effect.startsWith("invoke:declared-tool:"))) throw new Error("admitted federation requires a declared-tool receipt");
  if (receipt.disposition !== "qualified" && receipt.disposition !== "partial" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("non-admitted federation must be explicitly blocked");
}

export function brainFederatedEvidenceResearchCopilotReceiptDigest(receipt: BrainFederatedEvidenceResearchCopilotReceipt): string { validateBrainFederatedEvidenceResearchCopilotReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainEvidenceWorkflowFabricReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workflow_id: string; study_id: string; scope: string;
  disposition: "qualified" | "partial" | "unknown" | "blocked"; stage_order: string[]; plan_order: string[]; completed_order: string[]; blocked_order: string[]; compensation_order: string[]; candidate_order: string[]; qualified_order: string[]; unknown_order: string[];
  evidence_receipt_digest: string; checkpoint_digest: string; workflow_digest: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainEvidenceWorkflowFabricReceipt(receipt: BrainEvidenceWorkflowFabricReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID || receipt.contract_version !== EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("evidence workflow schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || !receipt.stage_order.length || !receipt.plan_order.length || !receipt.completed_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("workflow identity, stages, plan, locality, budget, or effects are incomplete");
  if (receipt.qualified_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("workflow evidence state is not covered by candidates");
  for (const values of [receipt.stage_order, receipt.plan_order, receipt.completed_order, receipt.blocked_order, receipt.compensation_order, receipt.candidate_order, receipt.qualified_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("workflow ordering is invalid");
  for (const value of [receipt.evidence_receipt_digest, receipt.checkpoint_digest, receipt.workflow_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("workflow digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("schedule:research-work:") && !effect.startsWith("compensate:research-work:") && effect !== "block:unsafe-release")) throw new Error("workflow effect is outside schedule/compensation gate");
  if (receipt.disposition === "qualified" && !receipt.effect_receipts.some((effect) => effect.startsWith("schedule:research-work:"))) throw new Error("qualified workflow requires schedule receipt");
  if (receipt.disposition === "blocked" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("blocked workflow must be explicitly blocked");
}

export function brainEvidenceWorkflowFabricReceiptDigest(receipt: BrainEvidenceWorkflowFabricReceipt): string { validateBrainEvidenceWorkflowFabricReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainMultimodalEvidenceWorkflowFabricReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workflow_id: string; scope: string; study_order: string[]; modality_order: string[];
  disposition: "qualified" | "partial" | "unknown" | "blocked"; stage_order: string[]; plan_order: string[]; completed_order: string[]; blocked_order: string[]; compensation_order: string[]; candidate_order: string[]; qualified_order: string[]; unknown_order: string[];
  evidence_receipt_digest: string; checkpoint_digest: string; workflow_digest: string; comparability_digest: string; approval_reference: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainMultimodalEvidenceWorkflowFabricReceipt(receipt: BrainMultimodalEvidenceWorkflowFabricReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MULTIMODAL_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID || receipt.contract_version !== MULTIMODAL_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("multimodal workflow schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.scope.trim() || receipt.study_order.length < 2 || receipt.modality_order.length < 2 || !receipt.stage_order.length || !receipt.plan_order.length || !receipt.completed_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("multimodal workflow identity, study/modality floors, stages, plan, locality, budget, or effects are incomplete");
  if (receipt.qualified_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("multimodal workflow state is not covered by candidates");
  for (const values of [receipt.study_order, receipt.modality_order, receipt.stage_order, receipt.plan_order, receipt.completed_order, receipt.blocked_order, receipt.compensation_order, receipt.candidate_order, receipt.qualified_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal workflow ordering is invalid");
  for (const value of [receipt.evidence_receipt_digest, receipt.checkpoint_digest, receipt.workflow_digest, receipt.comparability_digest, receipt.approval_reference, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("multimodal workflow digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("schedule:research-work:") && !effect.startsWith("compensate:research-work:") && effect !== "block:unsafe-release")) throw new Error("multimodal workflow effect is outside schedule/compensation gate");
  if (receipt.disposition === "qualified" && !receipt.effect_receipts.some((effect) => effect.startsWith("schedule:research-work:"))) throw new Error("qualified multimodal workflow requires schedule receipt");
  if (receipt.disposition === "blocked" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("blocked multimodal workflow must be explicitly blocked");
}

export function brainMultimodalEvidenceWorkflowFabricReceiptDigest(receipt: BrainMultimodalEvidenceWorkflowFabricReceipt): string { validateBrainMultimodalEvidenceWorkflowFabricReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainHighThroughputEvidenceWorkflowFabricReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workflow_id: string; batch_id: string; partition: string; disposition: "qualified" | "partial" | "unknown" | "blocked";
  stage_order: string[]; plan_order: string[]; completed_order: string[]; blocked_order: string[]; compensation_order: string[]; candidate_order: string[]; admitted_order: string[]; unknown_order: string[]; checkpoint_seq: number; queue_digest: string; checkpoint_digest: string; workflow_digest: string; approval_reference: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainHighThroughputEvidenceWorkflowFabricReceipt(receipt: BrainHighThroughputEvidenceWorkflowFabricReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== HIGH_THROUGHPUT_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID || receipt.contract_version !== HIGH_THROUGHPUT_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("throughput workflow schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.batch_id.trim() || !receipt.partition.trim() || !receipt.candidate_order.length || !receipt.stage_order.length || !receipt.plan_order.length || !receipt.completed_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("throughput workflow identity, batch, stages, plan, locality, budget, or effects are incomplete");
  if (receipt.admitted_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("throughput workflow state is not covered by candidates");
  for (const values of [receipt.stage_order, receipt.plan_order, receipt.completed_order, receipt.blocked_order, receipt.compensation_order, receipt.candidate_order, receipt.admitted_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("throughput workflow ordering is invalid");
  for (const value of [receipt.queue_digest, receipt.checkpoint_digest, receipt.workflow_digest, receipt.approval_reference, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("throughput workflow digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("schedule:research-work:") && !effect.startsWith("compensate:research-work:") && effect !== "block:unsafe-release")) throw new Error("throughput workflow effect is outside schedule/compensation gate");
  if (receipt.disposition === "qualified" && !receipt.effect_receipts.some((effect) => effect.startsWith("schedule:research-work:"))) throw new Error("qualified throughput workflow requires schedule receipt");
  if (receipt.disposition === "blocked" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("blocked throughput workflow must be explicitly blocked");
}

export function brainHighThroughputEvidenceWorkflowFabricReceiptDigest(receipt: BrainHighThroughputEvidenceWorkflowFabricReceipt): string { validateBrainHighThroughputEvidenceWorkflowFabricReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainFederatedEvidenceWorkflowFabricReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workflow_id: string; federation_id: string; institution_id: string; purpose: string; endpoint: string; disposition: "qualified" | "partial" | "unknown" | "blocked";
  stage_order: string[]; plan_order: string[]; completed_order: string[]; blocked_order: string[]; compensation_order: string[]; candidate_order: string[]; admitted_order: string[]; unknown_order: string[]; aggregate_order: string[];
  checkpoint_digest: string; workflow_digest: string; approval_reference: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainFederatedEvidenceWorkflowFabricReceipt(receipt: BrainFederatedEvidenceWorkflowFabricReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== FEDERATED_EVIDENCE_WORKFLOW_FABRIC_FEATURE_ID || receipt.contract_version !== FEDERATED_EVIDENCE_WORKFLOW_FABRIC_CONTRACT_VERSION) throw new Error("federated workflow schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workflow_id.trim() || !receipt.federation_id.trim() || !receipt.institution_id.trim() || !receipt.purpose.trim() || !receipt.endpoint.trim() || !receipt.stage_order.length || !receipt.plan_order.length || !receipt.completed_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("federated workflow identity, stages, plan, locality, budget, or effects are incomplete");
  if (receipt.admitted_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("federated workflow state is not covered by candidates");
  for (const values of [receipt.stage_order, receipt.plan_order, receipt.completed_order, receipt.blocked_order, receipt.compensation_order, receipt.candidate_order, receipt.admitted_order, receipt.unknown_order, receipt.aggregate_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("federated workflow ordering is invalid");
  for (const value of [receipt.checkpoint_digest, receipt.workflow_digest, receipt.approval_reference, receipt.replay_identity, receipt.artifact.content_hash, ...receipt.aggregate_order]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("federated workflow digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("schedule:research-work:") && !effect.startsWith("compensate:research-work:") && effect !== "block:unsafe-release")) throw new Error("federated workflow effect is outside schedule/compensation gate");
  if (receipt.disposition === "qualified" && !receipt.effect_receipts.some((effect) => effect.startsWith("schedule:research-work:"))) throw new Error("qualified federated workflow requires schedule receipt");
  if (receipt.disposition === "blocked" && JSON.stringify(receipt.effect_receipts) !== JSON.stringify(["block:unsafe-release"])) throw new Error("blocked federated workflow must be explicitly blocked");
}

export function brainFederatedEvidenceWorkflowFabricReceiptDigest(receipt: BrainFederatedEvidenceWorkflowFabricReceipt): string { validateBrainFederatedEvidenceWorkflowFabricReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainEvidenceResearchWorkbenchReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workspace_id: string; study_id: string; scope: string; disposition: "qualified" | "partial" | "unknown" | "blocked";
  view_order: string[]; panel_order: string[]; action_receipts: string[]; candidate_order: string[]; qualified_order: string[]; blocked_order: string[]; unknown_order: string[]; evidence_digest: string; workbench_digest: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainEvidenceResearchWorkbenchReceipt(receipt: BrainEvidenceResearchWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== EVIDENCE_RESEARCH_WORKBENCH_FEATURE_ID || receipt.contract_version !== EVIDENCE_RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workspace_id.trim() || !receipt.study_id.trim() || !receipt.scope.trim() || !receipt.view_order.length || !receipt.panel_order.length || !receipt.action_receipts.length || !receipt.candidate_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("workbench identity, views, panels, evidence, locality, budget, or effects are incomplete");
  if (receipt.qualified_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("workbench evidence state is not covered by candidates");
  for (const values of [receipt.view_order, receipt.panel_order, receipt.action_receipts, receipt.candidate_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("workbench ordering is invalid");
  for (const value of [receipt.evidence_digest, receipt.workbench_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("workbench digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("view:local-research-artifacts:") && effect !== "block:unsafe-release")) throw new Error("workbench effect is not read-only");
}

export function brainEvidenceResearchWorkbenchReceiptDigest(receipt: BrainEvidenceResearchWorkbenchReceipt): string { validateBrainEvidenceResearchWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainMultimodalResearchWorkbenchReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workspace_id: string; scope: string; study_order: string[]; modality_order: string[]; disposition: "qualified" | "partial" | "unknown" | "blocked";
  view_order: string[]; panel_order: string[]; action_receipts: string[]; candidate_order: string[]; qualified_order: string[]; blocked_order: string[]; unknown_order: string[]; evidence_digest: string; comparability_digest: string; workbench_digest: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainMultimodalResearchWorkbenchReceipt(receipt: BrainMultimodalResearchWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== MULTIMODAL_RESEARCH_WORKBENCH_FEATURE_ID || receipt.contract_version !== MULTIMODAL_RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("multimodal workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workspace_id.trim() || !receipt.scope.trim() || receipt.study_order.length < 2 || receipt.modality_order.length < 2 || !receipt.view_order.length || !receipt.panel_order.length || !receipt.action_receipts.length || !receipt.candidate_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("multimodal workbench identity, study/modality views, evidence, locality, budget, or effects are incomplete");
  if (receipt.qualified_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("multimodal workbench state is not covered by candidates");
  for (const values of [receipt.study_order, receipt.modality_order, receipt.view_order, receipt.panel_order, receipt.action_receipts, receipt.candidate_order, receipt.qualified_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("multimodal workbench ordering is invalid");
  for (const value of [receipt.evidence_digest, receipt.comparability_digest, receipt.workbench_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("multimodal workbench digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("view:local-multimodal-artifacts:") && effect !== "block:unsafe-release")) throw new Error("multimodal workbench effect is not read-only");
}

export function brainMultimodalResearchWorkbenchReceiptDigest(receipt: BrainMultimodalResearchWorkbenchReceipt): string { validateBrainMultimodalResearchWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export interface BrainThroughputResearchWorkbenchReceipt {
  schema_version: string; contract_version: string; feature_id: string; request_id: string; workspace_id: string; batch_id: string; partition: string; disposition: "qualified" | "partial" | "unknown" | "blocked";
  view_order: string[]; panel_order: string[]; action_receipts: string[]; candidate_order: string[]; admitted_order: string[]; blocked_order: string[]; unknown_order: string[]; checkpoint_seq: number; queue_digest: string; evidence_digest: string; workbench_digest: string; replay_identity: string; budget_units: number; omissions: string[]; uncertainty: string[]; negative_evidence: string[]; effect_receipts: string[]; artifact: Record<string, unknown>; raw_data_local: boolean; boundary: string;
}

export function validateBrainThroughputResearchWorkbenchReceipt(receipt: BrainThroughputResearchWorkbenchReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION || receipt.feature_id !== THROUGHPUT_RESEARCH_WORKBENCH_FEATURE_ID || receipt.contract_version !== THROUGHPUT_RESEARCH_WORKBENCH_CONTRACT_VERSION) throw new Error("throughput workbench schema, feature, or version mismatch");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY || !receipt.raw_data_local || !receipt.request_id.trim() || !receipt.workspace_id.trim() || !receipt.batch_id.trim() || !receipt.partition.trim() || !receipt.view_order.length || !receipt.panel_order.length || !receipt.action_receipts.length || !receipt.candidate_order.length || !receipt.effect_receipts.length || !Number.isInteger(receipt.budget_units) || receipt.budget_units <= 0) throw new Error("throughput workbench identity, queue views, evidence, locality, budget, or effects are incomplete");
  if (receipt.admitted_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.blocked_order.some((value) => !receipt.candidate_order.includes(value)) || receipt.unknown_order.some((value) => !receipt.candidate_order.includes(value))) throw new Error("throughput workbench state is not covered by candidates");
  for (const values of [receipt.view_order, receipt.panel_order, receipt.action_receipts, receipt.candidate_order, receipt.admitted_order, receipt.blocked_order, receipt.unknown_order, receipt.omissions, receipt.uncertainty, receipt.negative_evidence, receipt.effect_receipts]) if (JSON.stringify([...new Set(values)].sort()) !== JSON.stringify(values)) throw new Error("throughput workbench ordering is invalid");
  for (const value of [receipt.queue_digest, receipt.evidence_digest, receipt.workbench_digest, receipt.replay_identity, receipt.artifact.content_hash]) if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new Error("throughput workbench digest is invalid");
  if (receipt.effect_receipts.some((effect) => !effect.startsWith("view:local-throughput-artifacts:") && effect !== "block:unsafe-release")) throw new Error("throughput workbench effect is not read-only");
}

export function brainThroughputResearchWorkbenchReceiptDigest(receipt: BrainThroughputResearchWorkbenchReceipt): string { validateBrainThroughputResearchWorkbenchReceipt(receipt); return digestJsonSync(receipt); }

export function validatePolicyReceipt(receipt: PolicyReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (!receipt.receipt_id.trim() || receipt.reasons.length === 0) throw new Error("policy receipt needs an id and reason");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if ((receipt.decision === "approval_required" || receipt.decision === "unresolved") && receipt.authority_reference) throw new Error("authority is premature for unresolved policy");
  if (receipt.decision === "allow" && receipt.reasons.some((reason) => reason === "unresolved")) throw new Error("unresolved policy cannot allow");
}

export function validateEvidenceReceipt(receipt: EvidenceReceipt): void {
  if (receipt.schema_version !== RESEARCH_CONTRACT_SCHEMA_VERSION) throw new Error("unsupported research contract schema");
  if (!receipt.receipt_id.trim() || !receipt.intent.trim() || receipt.derivation.length === 0) throw new Error("evidence receipt is incomplete");
  if (receipt.boundary !== PRECLINICAL_BOUNDARY) throw new Error("research boundary mismatch");
  if (receipt.sources.length === 0 && (receipt.conclusion_state !== "unknown" || receipt.omissions.length === 0 || receipt.uncertainty.length === 0)) throw new Error("empty evidence must be explicit unknown");
  if (receipt.conclusion_state === "proven" && receipt.omissions.some((omission) => omission.could_change_decision !== "no_known_impact")) throw new Error("protected omission blocks proven conclusion");
}

/** Hashes the same JSON payload that the Rust `TypedResearchArtifact` seals. */
export function researchArtifactDigest(payload: unknown): string {
  return digestJsonSync(payload);
}
