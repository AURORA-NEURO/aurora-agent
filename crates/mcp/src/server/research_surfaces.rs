//! MCP projections for typed cross-domain research contracts.

use super::*;

impl Server {
    pub(super) fn worldgen_multimodal_ingestion(&self, arguments: &Value) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_worldgen_multimodal_ingestion_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": WORLDGEN_MULTIMODAL_INGESTION_FEATURE_ID,
            "contract_version": WORLDGEN_MULTIMODAL_INGESTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "world identity, required modality closure, semantic profile, quality floor, evidence state, replay, provenance, policy, signed approval, protected closure, locality, and aggregate-only checks are deterministic",
                "qualified, unresolved, blocked, missing-modality, quality-failed, contradiction, omission, uncertainty, and negative-evidence states remain auditable",
                "the route exchanges only digest-bound harmonized summaries or manages local capability and never imports raw data, executes pipelines, moves raw data, asserts scientific truth, or makes clinical decisions"
            ],
            "limitations": [
                "the assurance harness evaluates caller-supplied modality summaries and does not read images, omics, instruments, or remote stores",
                "a qualified receipt is a governed ingestion preflight artifact, not scientific validity, workflow execution, or clinical advice"
            ]
        }))
    }

    pub(super) fn worldgen_multimodal_execution(&self, arguments: &Value) -> Result<Value, String> {
        let run = crate::research_contracts::operate_worldgen_multimodal_execution_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": WORLDGEN_MULTIMODAL_EXECUTION_FEATURE_ID,
            "contract_version": WORLDGEN_MULTIMODAL_EXECUTION_CONTRACT_VERSION,
            "run": run,
            "guarantees": [
                "dependency topology, study and modality closure, evidence state, budget, replay, provenance, policy, federation, signed approval, locality, and aggregate-only checks are deterministic",
                "qualified, unresolved, blocked, cycle, missing-dependency, missing-study, missing-modality, budget, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only digest-bound execution-run exchange or local capability management and never dispatches code, reads raw data, moves raw data, asserts scientific truth, or makes clinical decisions"
            ],
            "limitations": [
                "the assurance harness verifies a caller-supplied ResearchWorkflowSpec graph and does not execute nodes or guarantee executor availability",
                "a qualified run is an execution preflight artifact, not completed computation, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn atlasx_mechanism_contract(&self, arguments: &Value) -> Result<Value, String> {
        let portfolio =
            crate::research_contracts::operate_atlasx_mechanism_contract_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": ATLASX_MECHANISM_FEATURE_ID,
            "contract_version": ATLASX_MECHANISM_CONTRACT_VERSION,
            "portfolio": portfolio,
            "guarantees": [
                "candidate and peer identity, semantic profile, replay, provenance, authorization, quorum, policy, federation, signed approval, locality, aggregate-only, and preclinical checks are deterministic",
                "selected, unresolved, blocked, missing-peer, contradiction, omission, uncertainty, and negative-evidence states remain auditable",
                "the route exposes only digest-bound portfolio viewing and local capability management and never infers biology, executes experiments, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the contract model normalizes caller-supplied summaries and does not calculate support from raw imaging, omics, or laboratory data",
                "a qualified portfolio is compatibility and evidence metadata, not a validated mechanism, experiment result, or clinical advice"
            ]
        }))
    }

    pub(super) fn routing_execution_copilot(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_routing_execution_copilot_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATED_EXECUTION_COPILOT_FEATURE_ID,
            "contract_version": FEDERATED_EXECUTION_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "candidate ranking is deterministic from typed discovery, risk, peer utility, evidence, and canonical replay inputs",
                "peer quorum, semantic and replay compatibility, policy, protected closure, signed approval, federation approval, locality, and aggregate-only gates remain explicit",
                "selected, unresolved, blocked, missing-study, missing-modality, omission, uncertainty, negative-evidence, and effect states are preserved in the signed receipt",
                "the tool emits a routing receipt only and never dispatches code, moves raw data, asserts scientific truth, or makes clinical decisions"
            ],
            "limitations": [
                "the copilot ranks caller-supplied execution-plan summaries and does not execute workflows or independently reproduce peer utility claims",
                "a qualified receipt is an approval-aware routing decision, not completed computation, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn routing_laboratory_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_routing_laboratory_inference_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": LABORATORY_INFERENCE_FEATURE_ID,
            "contract_version": LABORATORY_INFERENCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed instrument actions are deterministically partitioned into selected, unresolved, blocked, and missing states",
                "protocol readiness, evidence, provenance, replay, policy, protected closure, federation, locality, and no-hardware-effect gates fail closed",
                "qualified output is a read-only local authorization artifact and retains omissions, uncertainty, and negative evidence",
                "the route never contacts instruments, executes actions, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the engine evaluates caller-supplied attestations and does not independently run laboratory protocols",
                "qualified output requires downstream signed preflight and institutional governance before any physical execution"
            ]
        }))
    }

    pub(super) fn devx_context_compilation_contract(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_devx_context_compilation_contract_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": CONTEXT_COMPILATION_CONTRACT_FEATURE_ID,
            "contract_version": CONTEXT_COMPILATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed facts and context types are deterministically ordered and partitioned",
                "scope, evidence, replay, policy, protected closure, agent authority, locality, and adversarial gates fail closed",
                "omissions, uncertainty, contradictions, and negative findings remain explicit",
                "the contract emits a local compiled-context artifact without retrieving evidence or invoking tools"
            ],
            "limitations": [
                "the route validates caller-supplied context attestations and does not inspect raw studies",
                "qualified output is release-readiness evidence, not scientific truth or clinical advice"
            ]
        }))
    }

    pub(super) fn devx_evidence_surveillance_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a DevxEvidenceFeed5")?;
        let receipt =
            crate::research_contracts::run_devx_evidence_surveillance_control_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_devx::DEVX_EVIDENCE_SURVEILLANCE_CONTROL_FEATURE_ID,
            "contract_version": bioprism_devx::DEVX_EVIDENCE_SURVEILLANCE_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "local single-study evidence is deterministically ranked and partitioned into qualified, unresolved, blocked, and missing states",
                "relevance, freshness, evidence-state, replay, provenance, signature, policy, protected closure, approval, locality, omission, uncertainty, negative, and adversarial evidence remain explicit",
                "only a fully qualified read-only local artifact emits a read effect; incomplete or unsafe states block"
            ],
            "limitations": [
                "the control plane evaluates caller-supplied metadata and does not retrieve documents or inspect raw experimental payloads",
                "a qualified evidence receipt is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn resource_workbench_discover(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must contain need and candidates")?;
        let receipt = crate::research_contracts::discover_resources_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_fiber::RESOURCE_WORKBENCH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "candidate ranking is deterministic by trust score and resource identity",
                "availability, locality, federation, origin, capability, and result-limit omissions remain explicit",
                "raw research data stays institution-local and the route performs no connector or network effect"
            ],
            "limitations": [
                "the route qualifies caller-supplied registry manifests and does not fetch or inspect raw resources",
                "trust and capability declarations are evidence inputs, not a biological validity or clinical decision"
            ]
        }))
    }

    pub(super) fn resource_discovery_contract_v2(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResourceDiscoveryContractRequest")?;
        let response = crate::research_contracts::resource_discovery_contract_v2_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": crate::RESOURCE_DISCOVERY_CONTRACT_FEATURE_ID,
            "contract_version": crate::RESOURCE_DISCOVERY_CONTRACT_VERSION,
            "receipt": response,
            "guarantees": [
                "the MCP compatibility envelope preserves FIBER qualification semantics and explicit omissions",
                "v1 semantic fields remain stable while migration notes make version assumptions machine-readable",
                "canonical response bytes and artifact digests are deterministic across replays"
            ],
            "limitations": [
                "the route validates caller-supplied manifests and does not fetch resources or execute external effects",
                "compatibility is a protocol guarantee, not a biological validity or clinical decision"
            ]
        }))
    }

    pub(super) fn worldfactory_protocol_simulation_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::simulate_worldfactory_protocol_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_worldfactory::PROTOCOL_SIMULATION_FEDERATED_CONTROL_FEATURE_ID,
            "contract_version": bioprism_worldfactory::PROTOCOL_SIMULATION_FEDERATED_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "protocol stages and fault scenarios are simulated deterministically with checkpoint and replay identities",
                "policy, quorum, protected-closure, approval, federation, locality, aggregate-only, and evidence gates fail closed",
                "simulation produces no instrument, network, or raw-data effect"
            ],
            "limitations": [
                "the route evaluates caller-supplied protocol declarations and does not run laboratory actions",
                "simulation robustness is evidence for planning, not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn worldfactory_computational_execution_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::authorize_worldfactory_computational_execution_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_worldfactory::COMPUTATIONAL_EXECUTION_FEDERATED_CONTROL_FEATURE_ID,
            "contract_version": bioprism_worldfactory::COMPUTATIONAL_EXECUTION_FEDERATED_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "declared computational tasks are admitted deterministically with task, peer, budget, replay, and provenance closure",
                "policy, approval, federation, locality, aggregate-only, evidence, and adversarial gates fail closed",
                "qualified output is an authorization receipt only; no process, instrument, network, or raw-data effect is performed"
            ],
            "limitations": [
                "the route evaluates caller-supplied manifests and does not execute code or validate biological conclusions",
                "a separate institution-governed runtime must enforce the authorization and independent resource limits"
            ]
        }))
    }

    pub(super) fn evalengine_federated_protocol_simulation_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvalengineProtocolDraft")?;
        let receipt =
            crate::research_contracts::operate_evalengine_protocol_simulation_copilot_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_evalengine::EVALENGINE_PROTOCOL_SIMULATION_COPILOT_FEATURE_ID,
            "contract_version": bioprism_evalengine::EVALENGINE_PROTOCOL_SIMULATION_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A2 federated continual protocol simulation ranks declared steps and peer attestations deterministically with replay-bound evidence",
                "missing steps, unknown/speculative/contradictory evidence, policy/approval/quorum/provenance failures, adversarial events, and negative results remain explicit",
                "qualified output is only a bounded invoke:declared-tool receipt; the copilot never executes protocols, contacts instruments, or moves raw preclinical data"
            ],
            "limitations": [
                "the route evaluates caller-supplied state-machine summaries and does not run a workflow engine or infer biological truth",
                "declared-tool invocation remains subject to an independent policy-authorized runtime and human research governance"
            ]
        }))
    }

    pub(super) fn evalengine_local_mechanism_exploration_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvalengineMechanismQuestion1")?;
        let receipt = crate::research_contracts::operate_evalengine_local_mechanism_exploration_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_evalengine::EVALENGINE_LOCAL_MECHANISM_EXPLORATION_FEATURE_ID,
            "contract_version": bioprism_evalengine::EVALENGINE_LOCAL_MECHANISM_EXPLORATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "local single-study mechanism candidates are deterministically ranked and partitioned without mechanism invention",
                "evidence, provenance, comparability, replay, omission, uncertainty, contradiction, negative-result, policy, protected-closure, approval, budget, and locality gates remain explicit",
                "qualified output claims no external effect; every incomplete or unsafe posture emits block:unsafe-release"
            ],
            "limitations": [
                "the harness verifies caller-supplied mechanism summaries and does not discover literature, fit models, or execute experiments",
                "a qualified portfolio is an auditable release posture, not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn packs_local_quality_control_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a Packs ResearchObject1")?;
        let observations = arguments
            .get("observations")
            .ok_or("observations are required and must be Packs QualityObservation2 records")?;
        let mut envelope = serde_json::Map::new();
        envelope.insert("request".into(), request.clone());
        envelope.insert("observations".into(), observations.clone());
        let receipt = crate::research_contracts::operate_packs_local_quality_control_json(
            &Value::Object(envelope),
        )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_packs::PACKS_LOCAL_QUALITY_CONTROL_FEATURE_ID,
            "contract_version": bioprism_packs::PACKS_LOCAL_QUALITY_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "local single-study quality observations are deterministically partitioned into passed, failed, unknown, unmeasured, and blocked states",
                "required modality closure, threshold evidence, provenance, replay, policy, protected-closure, approval, locality, and aggregate-only gates remain explicit",
                "qualified output is a typed quality verdict; unresolved or blocked postures emit block:unsafe-release"
            ],
            "limitations": [
                "the harness evaluates caller-supplied metric summaries and does not inspect raw arrays, images, sequencing reads, or instrument state",
                "a qualified quality verdict does not establish biological validity or make a clinical decision"
            ]
        }))
    }

    pub(super) fn atlashub_replication_negative_results_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_atlashub_replication_control_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlashub::REPLICATION_CONTROL_FEATURE_ID,
            "contract_version": bioprism_atlashub::REPLICATION_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "independent replication observations are deterministically partitioned by outcome and evidence state",
                "null, negative, inconclusive, contradictory, incomparable, and omitted results remain visible",
                "only aggregate summaries can cross federation and all policy, provenance, replay, approval, and locality gates fail closed"
            ],
            "limitations": [
                "the route consumes caller-supplied replication summaries and never reads raw measurements",
                "a qualified replication posture is not biological validity or a clinical decision"
            ]
        }))
    }

    pub(super) fn mcp_replication_negative_results_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ClaimAndProtocol3")?;
        let receipt =
            crate::research_contracts::run_mcp_replication_negative_results_assurance_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": crate::REPLICATION_ASSURANCE_FEATURE_ID,
            "contract_version": crate::REPLICATION_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "prospective batches are deterministically ordered and every observation is classified",
                "null, negative, inconclusive, contradictory, incomparable, omitted, and unresolved evidence remain visible",
                "only a qualified aggregate release receipt is emitted; raw measurements, protocol execution, and clinical decisions are out of scope"
            ],
            "limitations": [
                "the assurance harness evaluates caller-supplied aggregate replication summaries and does not rerun experiments",
                "a qualified replication release posture is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn prism_protocol_simulation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a PRISM ProtocolDraft2")?;
        let receipt =
            crate::research_contracts::run_prism_protocol_simulation_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_prism::PROTOCOL_SIMULATION_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_prism::PROTOCOL_SIMULATION_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal multi-study protocol steps and peer attestations are deterministically partitioned",
                "missing steps, unresolved or contradictory evidence, adversarial events, and policy failures remain explicit",
                "qualified output is verification evidence only; no protocol, instrument, raw-data, or clinical effect is executed"
            ],
            "limitations": [
                "the route evaluates caller-supplied protocol summaries and does not run a simulator or infer biological validity",
                "release evidence remains subject to institutional review and independent replication"
            ]
        }))
    }

    pub(super) fn scale_quality_control_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a Scale QualityControlContractRequest")?;
        let receipt =
            crate::research_contracts::run_scale_quality_control_contract_model_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_scale::QUALITY_CONTROL_CONTRACT_MODEL_FEATURE_ID,
            "contract_version": bioprism_scale::QUALITY_CONTROL_CONTRACT_MODEL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "local single-study metric and modality contracts are canonicalized without calculating metrics",
                "missing, unmeasured, unknown, contradicted, negative, replay, provenance, and policy states remain explicit",
                "the contract model is read-only and cannot execute tools, move raw data, or make clinical decisions"
            ],
            "limitations": [
                "the model checks caller-supplied envelopes and does not inspect raw arrays or image bytes",
                "a qualified contract is compatibility evidence, not a quality or biological-validity conclusion"
            ]
        }))
    }

    pub(super) fn packs_protocol_simulation_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a Packs ProtocolWorkbenchRequest5")?;
        let receipt =
            crate::research_contracts::run_packs_protocol_simulation_workbench_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_packs::PACKS_PROTOCOL_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_packs::PACKS_PROTOCOL_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal multi-study protocol stages, fault scenarios, peer attestations, and batch capacity are deterministically partitioned",
                "missing, unresolved, contradictory, negative, replay, provenance, policy, approval, federation, locality, and recovery evidence remain explicit",
                "the workbench is simulation metadata only and never executes protocols, instruments, raw-data transfers, or clinical decisions"
            ],
            "limitations": [
                "the workbench replays caller-supplied protocol summaries and does not execute a simulator or laboratory workflow",
                "a qualified report is a preflight compatibility artifact, not proof of biological validity"
            ]
        }))
    }

    pub(super) fn oracle_evidence_surveillance_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be an Oracle EvidenceSurveillanceWorkflowRequest",
        )?;
        let receipt =
            crate::research_contracts::run_oracle_evidence_surveillance_workflow_fabric_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oracle::FEATURE_ID,
            "contract_version": bioprism_oracle::CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "checkpoint, federation admission, surveillance, and envelope sealing stages are deterministic and replayable",
                "relevance, semantic, signature, provenance, omission, uncertainty, negative evidence, quorum, policy, approval, locality, and budget states remain explicit",
                "only a qualified aggregate evidence-surveillance receipt can emit an execute effect; raw data, source fetching, tool execution, and clinical decisions are out of scope"
            ],
            "limitations": [
                "the fabric schedules caller-supplied signed aggregate evidence contributions and does not fetch literature or inspect raw payloads",
                "a qualified surveillance receipt is not a scientific conclusion, biological validity claim, or clinical advice"
            ]
        }))
    }

    pub(super) fn governance_research_release_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ValidatedResearchRun")?;
        let receipt = crate::research_contracts::compile_governance_research_release_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_governance::RESEARCH_RELEASE_CONTRACT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "policy allow, complete provenance, local-data retention, schema migration, and detached Ed25519 verification gate release",
                "artifact and evidence identifiers are canonicalized while omissions remain visible",
                "the route creates local metadata only and never accepts or exports raw experimental bytes"
            ],
            "limitations": [
                "the route verifies a caller-supplied signature over a content digest and does not hold private keys",
                "a signed research object is a reproducibility and governance artifact, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn release_assurance_harness(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ReleaseHarnessRequest")?;
        let receipt = crate::research_contracts::assess_release_harness_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_obligation::RELEASE_HARNESS_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "signed-object governance, required provenance, protected closure, replay identity, and benchmark admission remain separate checks",
                "missing replay evidence is unknown and cannot become a pass",
                "blocked checks and omissions are retained in a deterministic local receipt"
            ],
            "limitations": [
                "the route evaluates caller-supplied signed metadata and does not execute workflows or inspect raw experimental data",
                "a passed assurance receipt is a release gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn obligation_prospective_release_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ProspectiveReleaseAssuranceRequest")?;
        let receipt =
            crate::research_contracts::operate_obligation_prospective_release_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_obligation::PROSPECTIVE_RELEASE_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "prospective high-throughput runs are partitioned into selected, unresolved, blocked, and missing states",
                "protected closure, provenance, replay, signer, evidence, locality, and policy gates fail closed",
                "omissions, contradictions, negative results, and adversarial declarations remain portable and content-addressed"
            ],
            "limitations": [
                "the route verifies caller-supplied typed attestations and does not read raw experimental data",
                "a qualified receipt is a publication admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn obligation_knowledge_representation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_obligation_knowledge_representation_assurance_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_obligation::KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_obligation::KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "federated continual claims and peer summaries are deterministically partitioned into selected, unresolved, blocked, and missing states",
                "semantic, replay, provenance, evidence, omission, uncertainty, negative-result, policy, protected-closure, signed-approval, federation, locality, aggregate-only, and adversarial gates fail closed",
                "the TypedKnowledgeWorld7 artifact is content-addressed and raw preclinical data remains local"
            ],
            "limitations": [
                "the route verifies caller-supplied summaries and does not retrieve sources, inspect raw data, or open federation connections",
                "a qualified knowledge world is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn obligation_security_federation_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::run_obligation_security_federation_interoperability_gateway_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_obligation::SECURITY_FEDERATION_INTEROPERABILITY_GATEWAY_FEATURE_ID,
            "contract_version": bioprism_obligation::SECURITY_FEDERATION_INTEROPERABILITY_GATEWAY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "versioned peer capabilities are negotiated deterministically with complete selected, unresolved, denied, and missing partitions",
                "semantic-profile migration, replay, provenance, evidence, policy, approval, protected-closure, locality, negative-result, and adversarial states remain explicit",
                "qualified effects authorize only permitted aggregate artifacts; raw preclinical data remains institution-local"
            ],
            "limitations": [
                "the route validates caller-supplied capability metadata and does not open network connections or dereference raw artifacts",
                "a qualified federation envelope is an exchange authorization receipt, not a biological conclusion or clinical decision"
            ]
        }))
    }

    pub(super) fn atlasx_federated_execution_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResearchWorkflowSpec4")?;
        let receipt = crate::research_contracts::operate_atlasx_federated_execution_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlasx::FEDERATED_EXECUTION_CONTROL_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "dependency, checkpoint, peer capability, replay, provenance, policy, approval, capacity, federation, and locality gates are explicit",
                "unknown, contradictory, stale, revoked, missing, omitted, negative, and adversarial states cannot become a pass",
                "the control plane emits only digest-bound run metadata and never dispatches code"
            ],
            "limitations": [
                "the route plans caller-supplied declarations and does not execute workflows or inspect raw data",
                "a qualified run image is an admission decision, not a scientific or clinical conclusion"
            ]
        }))
    }

    pub(super) fn atlasx_computational_execution_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResearchWorkflowSpec3")?;
        let receipt =
            crate::research_contracts::operate_atlasx_computational_execution_assurance_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlasx::COMPUTATIONAL_EXECUTION_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_atlasx::COMPUTATIONAL_EXECUTION_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "prospective execution plans are deterministically topologically ordered and retain cycle, missing-dependency, budget, replay, provenance, evidence, policy, protected-closure, locality, and adversarial witnesses",
                "completed, unresolved, and blocked node states form a complete partition with omission and negative-result evidence",
                "the route emits only digest-bound verification metadata and never dispatches code, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the harness verifies caller-supplied graph attestations and does not execute nodes or contact providers",
                "a qualified run is a release-evidence gate, not proof of scientific validity or workflow completion"
            ]
        }))
    }

    pub(super) fn policy_federated_analysis_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AnalysisQuestion4")?;
        let receipt = crate::research_contracts::operate_policy_analysis_copilot_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_policy::ANALYSIS_COPILOT_FEATURE_ID,
            "contract_version": bioprism_policy::ANALYSIS_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "declared analysis candidates are ranked deterministically by evidence state, uncertainty, baseline delta, and stable identity",
                "policy, protected-closure, signed approval, federation, locality, aggregate-only, replay, and adversarial gates fail closed",
                "unknown, stale, contradictory, omitted, negative, missing, and revoked analysis states remain explicit and no model is executed"
            ],
            "limitations": [
                "the copilot qualifies caller-supplied analysis capability attestations and does not execute tools or inspect raw data",
                "a qualified receipt is a bounded analysis admission artifact, not a scientific conclusion, workflow completion, or clinical advice"
            ]
        }))
    }

    pub(super) fn prism_analysis_workbench(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AnalysisWorkbenchRequest5")?;
        let receipt = crate::research_contracts::operate_prism_analysis_workbench_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_prism::ANALYSIS_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_prism::ANALYSIS_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "analysis jobs are ranked deterministically by evidence state, freshness, and stable identity",
                "identification, comparability, quality, policy, protected closure, signed approval, replay, provenance, locality, and adversarial gates fail closed",
                "unknown, stale, speculative, contradictory, omitted, negative, missing, and revoked states remain explicit and no statistical, causal, or ML model is executed"
            ],
            "limitations": [
                "the workbench qualifies caller-supplied analysis job attestations and does not run models or inspect raw data",
                "a qualified receipt is an analysis admission artifact, not a scientific conclusion, workflow completion, or clinical advice"
            ]
        }))
    }

    pub(super) fn atlasx_context_compilation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ContextCompilationQuestion4")?;
        let receipt = crate::research_contracts::operate_atlasx_context_compilation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlasx::CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_atlasx::CONTEXT_COMPILATION_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "context fragments are ranked deterministically by evidence state, freshness, uncertainty, and stable identity",
                "required context/source closure, policy, protected closure, signed approval, federation, aggregate locality, replay, provenance, and adversarial gates fail closed",
                "missing, stale, unknown, speculative, contradictory, omitted, negative, and revoked context remains explicit and no raw source is read or exported"
            ],
            "limitations": [
                "the assurance plane evaluates caller-supplied typed fragments and does not fetch sources or compile a Decision Section",
                "a qualified receipt is a bounded context-admission artifact, not scientific validity, workflow execution, or clinical advice"
            ]
        }))
    }

    pub(super) fn atlashub_quality_control_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a QualityControlRequest3")?;
        let verdict =
            crate::research_contracts::operate_atlashub_quality_control_copilot_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlashub::QUALITY_CONTROL_COPILOT_FEATURE_ID,
            "contract_version": bioprism_atlashub::QUALITY_CONTROL_COPILOT_CONTRACT_VERSION,
            "verdict": verdict,
            "guarantees": [
                "quality metrics are deterministically partitioned into passed, failed, unknown, unmeasured, blocked, and missing states",
                "modality closure, replay, provenance, evidence, policy, protected-closure, signed/tool-approval, locality, aggregate-only, omission, uncertainty, negative-result, and adversarial gates remain explicit",
                "the route emits only a content-addressed QualityVerdict3 read-only artifact and never invokes tools, reads raw bytes, moves data, or makes clinical decisions"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied metric summaries and does not calculate metrics or invoke external tools",
                "a qualified verdict is a bounded quality admission artifact, not scientific validity or release authorization"
            ]
        }))
    }

    pub(super) fn atlashub_quality_control_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a QualityControlContractRequest")?;
        let verdict =
            crate::research_contracts::operate_atlashub_quality_control_contract_model_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlashub::PROSPECTIVE_QUALITY_CONTROL_CONTRACT_FEATURE_ID,
            "contract_version": bioprism_atlashub::PROSPECTIVE_QUALITY_CONTROL_CONTRACT_VERSION,
            "verdict": verdict,
            "guarantees": [
                "the typed contract model canonicalizes metric and modality closure into a byte-stable QualityVerdict2 artifact",
                "missing, unmeasured, unknown, contradicted, negative, policy, locality, and replay states remain explicit",
                "raw preclinical data remains local and the capability grants no execution or federation authority"
            ],
            "limitations": [
                "the model validates caller-supplied quality summaries and does not calculate metrics or execute tools",
                "a qualified contract verdict is a schema and compatibility artifact, not scientific validity or release authorization"
            ]
        }))
    }

    pub(super) fn bioworlds_resource_discovery_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResourceNeed5")?;
        let receipt =
            crate::research_contracts::operate_bioworlds_resource_discovery_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioworlds::RESOURCE_DISCOVERY_COPILOT_FEATURE_ID,
            "contract_version": bioprism_bioworlds::RESOURCE_DISCOVERY_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "resource candidates are ranked deterministically by evidence, trust, availability, and stable identity",
                "resource, capability, site, policy, federation, locality, aggregate-only, replay, provenance, and adversarial gates fail closed",
                "missing, stale, unknown, speculative, contradictory, omitted, negative, revoked, and low-trust states remain explicit and no resource is fetched"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied resource attestations and does not contact registries or remote institutions",
                "a qualified resource set is a discovery and admission artifact, not resource availability proof, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn routing_limitation_closure_workflow(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a LimitationClosureWorkflowRequest5")?;
        let receipt = crate::research_contracts::operate_routing_limitation_closure_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_routing::LIMITATION_CLOSURE_WORKFLOW_FEATURE_ID,
            "contract_version": bioprism_routing::LIMITATION_CLOSURE_WORKFLOW_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "limitation declarations are ranked deterministically by evidence state, freshness, and stable identity",
                "closure criteria, counterexamples, policy, protected closure, signed approval, replay/provenance, locality, capacity, and adversarial gates fail closed",
                "unknown, stale, speculative, contradictory, omitted, negative, missing, and revoked limitations remain explicit and unresolved constraints cannot be promoted"
            ],
            "limitations": [
                "the fabric evaluates caller-supplied closure attestations and does not execute workflows or establish scientific truth",
                "a qualified receipt is a bounded limitation-admission artifact, not a release guarantee or clinical advice"
            ]
        }))
    }

    pub(super) fn devplat_multimodal_limitation_closure_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a DevplatLimitationCase2")?;
        let receipt = crate::research_contracts::operate_devplat_multimodal_limitation_closure_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": DEVPLAT_MULTIMODAL_LIMITATION_CLOSURE_FEATURE_ID,
            "contract_version": DEVPLAT_MULTIMODAL_LIMITATION_CLOSURE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A1 multimodal multi-study limitation closure is deterministic, omission-aware, and fail-closed",
                "unknown, stale, contradicted, unmeasured, omitted, negative, policy, protected-closure, replay, provenance, and locality states remain explicit",
                "qualified output is a content-addressed verification artifact with no external side effect"
            ],
            "limitations": [
                "the harness validates caller-supplied limitation attestations only; it does not close unknown claims by assumption",
                "no raw preclinical data leaves the institution and no clinical decisions are made"
            ]
        }))
    }

    pub(super) fn interweave_federated_interpretation_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an InterpretationInferenceRequest")?;
        let receipt =
            crate::research_contracts::operate_interweave_federated_interpretation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_interweave::federated_interpretation_engine::FEATURE_ID,
            "contract_version": bioprism_interweave::federated_interpretation_engine::CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal observations are ordered deterministically and partitioned into selected, missing, contradictory, uncertain, and negative evidence",
                "study/modality closure, policy, protected closure, signed approval, federation, replay/provenance, aggregate-only locality, and adversarial gates fail closed",
                "visualization-ready interpretations retain omissions, uncertainty, negative results, and provenance without moving raw data or asserting clinical conclusions"
            ],
            "limitations": [
                "the engine evaluates caller-supplied typed observations and does not render charts, read source stores, or execute analysis models",
                "a qualified receipt is an auditable interpretation input, not scientific truth, diagnosis, treatment, or clinical advice"
            ]
        }))
    }

    pub(super) fn interweave_federated_commons_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_interweave_federated_commons_assurance_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATED_COMMONS_ASSURANCE_FEATURE_ID,
            "contract_version": FEDERATED_COMMONS_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "signed institution-local capability declarations are deterministically partitioned into selected, unresolved, blocked, and missing states",
                "replay, signature, protected-closure, policy, federation authorization, aggregate-only locality, provenance, and adversarial gates fail closed",
                "omissions, uncertainty, contradictions, and negative evidence remain explicit; qualified output is a verification envelope only",
                "the harness never opens federation connections, transfers raw data, executes capabilities, or makes clinical decisions"
            ],
            "limitations": [
                "the harness evaluates caller-supplied capability attestations and does not authenticate remote identities or validate scientific content",
                "an InterweaveFederationEnvelope7 is release evidence for a bounded capability, not deployment approval or scientific truth"
            ]
        }))
    }

    pub(super) fn bioworlds_knowledge_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a KnowledgeWorkflowRequest5")?;
        let receipt =
            crate::research_contracts::operate_bioworlds_knowledge_workflow_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioworlds::KNOWLEDGE_WORKFLOW_FEATURE_ID,
            "contract_version": bioprism_bioworlds::KNOWLEDGE_WORKFLOW_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal observation attestations are ranked deterministically by evidence state, freshness, and stable identity",
                "semantic compatibility, quality, policy, protected closure, signed approval, replay/provenance, locality, capacity, and adversarial gates fail closed",
                "unknown, stale, speculative, contradictory, omitted, negative, missing, and revoked states remain explicit and no raw observation is read or exported"
            ],
            "limitations": [
                "the fabric compiles caller-supplied observation attestations and does not access source stores or build a biological knowledge graph itself",
                "a qualified receipt is a bounded knowledge-admission artifact, not scientific truth, workflow completion, or clinical advice"
            ]
        }))
    }

    pub(super) fn bioworlds_federated_context_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedContextWorkbenchRequest")?;
        let receipt =
            crate::research_contracts::operate_bioworlds_federated_context_research_workbench_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioworlds::FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_bioworlds::FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "federated DecisionQuery context is deterministically partitioned by semantic profile, freshness, provenance, replay identity, permission, and quorum",
                "missing, stale, unknown, contradictory, omitted, negative, and incomplete protected-closure states remain visible in the researcher workbench",
                "raw preclinical data remains institution-local and only permitted aggregate artifacts can appear in a CertifiedDecisionSection"
            ],
            "limitations": [
                "the workbench evaluates caller-supplied typed peer attestations and does not fetch evidence, move raw data, or execute laboratory actions",
                "a qualified receipt is bounded context evidence, not scientific truth, diagnosis, treatment, or clinical advice"
            ]
        }))
    }

    pub(super) fn mutation_family(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a JSON world document relative to the server root)")?;
        let path = self.resolve(relative)?;
        if path.is_dir() {
            return Err(
                "mutation_family requires a JSON world document, not an indexed store".into(),
            );
        }
        let suite_name = arguments
            .get("suite")
            .and_then(Value::as_str)
            .unwrap_or("standard");
        if suite_name != "standard" {
            return Err(format!(
                "unknown mutation suite {suite_name:?}; only the deterministic standard suite is available"
            ));
        }
        let raw = self.read_json(&path)?;
        let mutations = standard_suite();
        let family = generate_mutations(&raw, &mutations).map_err(|error| error.to_string())?;
        let diversity = measure_diversity(std::slice::from_ref(&family));
        let attempted = family.accepted.len() + family.rejected.len() + family.duplicates.len();
        let include_worlds = arguments
            .get("include_worlds")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut result = json!({
            "ok": true,
            "suite": suite_name,
            "world": relative,
            "parent": {
                "id": family.parent_id,
                "sha256": family.parent_sha256,
            },
            "counts": {
                "attempted": attempted,
                "accepted": family.accepted.len(),
                "rejected": family.rejected.len(),
                "duplicates": family.duplicates.len(),
            },
            "yield_rate": family.yield_rate(),
            "accepted": family.accepted,
            "rejected": family.rejected,
            "duplicates": family.duplicates,
            "diversity": diversity,
        });

        if include_worlds {
            let max_worlds = arguments
                .get("max_worlds")
                .and_then(Value::as_u64)
                .unwrap_or(100);
            if max_worlds == 0 || max_worlds > 1_000 {
                return Err("max_worlds must be between 1 and 1000".into());
            }
            let worlds = family
                .worlds
                .iter()
                .take(max_worlds as usize)
                .map(|(id, world)| json!({ "id": id, "world": world }))
                .collect::<Vec<_>>();
            result["worlds"] = json!(worlds);
            result["omitted_worlds"] = json!(family.worlds.len().saturating_sub(worlds.len()));
        }
        Ok(result)
    }

    pub(super) fn influence_analyze(&self, arguments: &Value) -> Result<Value, String> {
        let label = arguments
            .get("label")
            .and_then(Value::as_str)
            .ok_or("label is required")?;
        let raw_variables = arguments
            .get("variables")
            .and_then(Value::as_object)
            .ok_or("variables is required and must map variable names to positive cardinalities")?;
        if raw_variables.is_empty() || raw_variables.len() > 10_000 {
            return Err("variables must contain between 1 and 10000 entries".into());
        }
        let assumed = arguments
            .get("assumed_variables")
            .map(|raw| -> Result<BTreeSet<String>, String> {
                let values = raw.as_array().ok_or("assumed_variables must be an array")?;
                let assumed = values
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_string)
                            .ok_or("assumed_variables must contain strings")
                    })
                    .collect::<Result<BTreeSet<_>, _>>()?;
                if assumed.len() != values.len() {
                    return Err("assumed_variables must not contain duplicates".into());
                }
                Ok(assumed)
            })
            .transpose()?;
        let assumed = assumed.unwrap_or_default();
        if let Some(variable) = assumed
            .iter()
            .find(|variable| !raw_variables.contains_key(*variable))
        {
            return Err(format!(
                "assumed_variables names undeclared variable {variable:?}"
            ));
        }
        let mut builder = QueryRegion::builder(label);
        for (name, raw_cardinality) in raw_variables {
            let cardinality = raw_cardinality.as_u64().ok_or_else(|| {
                format!("cardinality for variable {name:?} must be a positive integer")
            })?;
            if cardinality == 0 || cardinality > 1_000_000 {
                return Err(format!(
                    "cardinality for variable {name:?} must be between 1 and 1000000"
                ));
            }
            if assumed.contains(name) {
                builder = builder.assumed_variable(name.clone(), cardinality as usize);
            } else {
                builder = builder.observed_variable(name.clone(), cardinality as usize);
            }
        }

        let raw_factors = arguments
            .get("factors")
            .and_then(Value::as_array)
            .ok_or("factors is required and must be an array")?;
        if raw_factors.is_empty() || raw_factors.len() > 1_000 {
            return Err("factors must contain between 1 and 1000 entries".into());
        }
        for (index, raw_factor) in raw_factors.iter().enumerate() {
            let object = raw_factor
                .as_object()
                .ok_or_else(|| format!("factor at index {index} must be an object"))?;
            let id = object
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("factor at index {index} is missing id"))?;
            let raw_scope = object
                .get("scope")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("factor {id:?} is missing scope"))?;
            if raw_scope.is_empty() || raw_scope.len() > 128 {
                return Err(format!(
                    "factor {id:?} scope must contain between 1 and 128 variables"
                ));
            }
            let scope = raw_scope
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("factor {id:?} scope must contain strings"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let factor = match object.get("table") {
                None | Some(Value::Null) => RegionFactor::structural(id, scope),
                Some(raw_table) => {
                    let raw_table = raw_table
                        .as_array()
                        .ok_or_else(|| format!("factor {id:?} table must be an array"))?;
                    if raw_table.len() > 4_194_304 {
                        return Err(format!(
                            "factor {id:?} table exceeds the 4194304-entry safety bound"
                        ));
                    }
                    let table = raw_table
                        .iter()
                        .map(|item| {
                            item.as_f64().ok_or_else(|| {
                                format!("factor {id:?} table must contain finite numbers")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    RegionFactor::with_table(id, scope, table)
                }
            };
            builder = builder.factor(factor);
        }

        let raw_free = arguments
            .get("free")
            .and_then(Value::as_array)
            .ok_or("free is required and must be an array of variable names")?;
        if raw_free.is_empty() || raw_free.len() > raw_variables.len() {
            return Err(
                "free must contain at least one and no more than the declared variables".into(),
            );
        }
        let mut seen_free = BTreeSet::new();
        for item in raw_free {
            let variable = item.as_str().ok_or("free must contain strings")?;
            if !seen_free.insert(variable) {
                return Err(format!(
                    "free must not contain duplicate variable {variable:?}"
                ));
            }
            builder = builder.free(variable);
        }
        let region = builder
            .build()
            .map_err(|error| format!("invalid influence region: {error}"))?;

        let raw_perturbation = arguments
            .get("perturbation")
            .cloned()
            .ok_or("perturbation is required and must name removal or multiplicative_range")?;
        let perturbation: Perturbation = serde_json::from_value(raw_perturbation)
            .map_err(|error| format!("invalid perturbation: {error}"))?;
        let subjects = match (arguments.get("factor"), arguments.get("factor_group")) {
            (Some(_), Some(_)) => return Err("provide factor or factor_group, not both".into()),
            (Some(raw), None) => vec![raw.as_str().ok_or("factor must be a string")?.to_string()],
            (None, Some(raw)) => {
                let values = raw
                    .as_array()
                    .ok_or("factor_group must be an array of factor ids")?;
                if values.is_empty() || values.len() > 1_000 {
                    return Err("factor_group must contain between 1 and 1000 ids".into());
                }
                let group = values
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(str::to_string)
                            .ok_or("factor_group must contain strings")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if group.iter().collect::<BTreeSet<_>>().len() != group.len() {
                    return Err("factor_group must not contain duplicate factor ids".into());
                }
                group
            }
            (None, None) => return Err("factor or factor_group is required".into()),
        };

        let mut budget = InfluenceBudget::default();
        if let Some(raw_budget) = arguments.get("budget") {
            let object = raw_budget.as_object().ok_or("budget must be an object")?;
            if let Some(value) = object.get("max_induced_width") {
                let width = value
                    .as_u64()
                    .ok_or("budget.max_induced_width must be an integer")?;
                if width > 1024 {
                    return Err("budget.max_induced_width must be at most 1024".into());
                }
                budget = budget.with_max_induced_width(width as usize);
            }
            if let Some(value) = object.get("max_peak_entries") {
                let entries = value
                    .as_f64()
                    .ok_or("budget.max_peak_entries must be a number")?;
                if !entries.is_finite() || entries <= 0.0 {
                    return Err("budget.max_peak_entries must be a finite positive number".into());
                }
                budget = budget.with_max_peak_entries(entries);
            }
            if let Some(value) = object.get("max_ops") {
                let ops = value.as_f64().ok_or("budget.max_ops must be a number")?;
                if !ops.is_finite() || ops <= 0.0 {
                    return Err("budget.max_ops must be a finite positive number".into());
                }
                budget = budget.with_max_ops(ops);
            }
        }
        let execute = arguments
            .get("execute")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let analyzer = InfluenceAnalyzer::new(budget);
        let analyzer = if execute {
            analyzer
        } else {
            analyzer.structural_only()
        };
        let analysis = if subjects.len() == 1 {
            analyzer
                .analyse_factor(&region, &subjects[0], &perturbation)
                .map_err(|error| error.to_string())?
        } else {
            analyzer
                .analyse_group(&region, &subjects, &perturbation)
                .map_err(|error| error.to_string())?
        };
        let factor_summary = region
            .factors()
            .iter()
            .map(|factor| {
                json!({
                    "id": factor.id(),
                    "scope": factor.scope(),
                    "arity": factor.arity(),
                    "has_table": factor.table().is_some(),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "region": {
                "label": region.label(),
                "variables": region.cardinality(),
                "free": region.free_variables(),
                "bound": region.bound_variables(),
                "factors": factor_summary,
                "has_tables": region.has_tables(),
                "joint_entries": region.joint_entries(),
                "free_entries": region.free_entries(),
                "assumed_cardinality_fraction": region.assumed_cardinality_fraction(),
            },
            "subjects": subjects,
            "perturbation": perturbation,
            "execute": execute,
            "analysis": analysis,
            "looseness": analysis.looseness(),
            "guarantees": [
                "unknown influence remains an InfluenceEstimate::Unknown rather than a numeric infinity",
                "the default is structural-only and does not execute a query",
                "every reported bound carries its metric, method, approximation direction, and validity scope",
            ],
        }))
    }
}
