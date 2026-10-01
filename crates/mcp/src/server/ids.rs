//! MCP Interoperability and federated resource-discovery handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn ids_federated_resource_discovery_interoperability(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::interoperate_ids_resources_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_ids::IDS_RESOURCE_INTEROPERABILITY_FEATURE_ID,
            "contract_version": bioprism_ids::IDS_RESOURCE_INTEROPERABILITY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "resource endpoint and peer qualification is deterministic and replay-bound",
                "raw data stays institution-local and only aggregate permitted artifacts may cross federation boundaries",
                "unknown, stale, contradicted, policy-denied, and missing-capability resources remain explicit"
            ],
            "limitations": [
                "the route evaluates caller-supplied manifests and does not fetch endpoints or move raw data",
                "qualification is an interoperability and policy decision, not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_context_compilation_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_context_compilation_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_CONTEXT_COMPILATION_FEATURE_ID,
            "contract_version": IDS_CONTEXT_COMPILATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed facts are deterministically ranked with stable tie breaks and replay identity",
                "unknown, unmeasured, contradicted, omitted, and negative claims remain visible",
                "only aggregate-only permitted context summaries can cross an approved federation boundary"
            ],
            "limitations": [
                "the route consumes caller-supplied fact summaries and does not retrieve documents or raw measurements",
                "a qualified context section is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_knowledge_representation_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_knowledge_representation_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_KNOWLEDGE_REPRESENTATION_FEATURE_ID,
            "contract_version": IDS_KNOWLEDGE_REPRESENTATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal claims are deterministically ranked and bound to source, provenance, replay, and semantic identities",
                "unknown, unmeasured, contradicted, omitted, and negative claims remain explicit",
                "only aggregate-only permitted typed worlds can cross the approved federation boundary"
            ],
            "limitations": [
                "the route consumes caller-supplied claim summaries and does not access raw imaging or omics data",
                "a qualified typed world is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_multimodal_ingestion_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_multimodal_ingestion_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_MULTIMODAL_INGESTION_FEATURE_ID,
            "contract_version": IDS_MULTIMODAL_INGESTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "modality manifests are deterministically ordered and quality-gated with replay and provenance identities",
                "missing, unknown, unmeasured, contradicted, omitted, and negative modalities remain explicit",
                "raw modality data remains institution-local and the copilot emits only governed local capability receipts"
            ],
            "limitations": [
                "the route evaluates caller-supplied manifests and does not read files or control instruments",
                "a qualified harmonized object is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_quality_control_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_quality_control_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_QUALITY_CONTROL_FEATURE_ID,
            "contract_version": IDS_QUALITY_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "metric summaries are deterministically partitioned into passed, failed, unknown, unmeasured, and blocked states",
                "required modality closure, quality fraction, replay, provenance, policy, approval, and locality gates are explicit",
                "failed and negative QC evidence cannot be silently promoted to a release"
            ],
            "limitations": [
                "the route evaluates caller-supplied QC summaries and never reads raw arrays, images, sequencing reads, or instrument state",
                "a qualified QC report is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_mechanism_exploration_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_mechanism_exploration_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_MECHANISM_EXPLORATION_FEATURE_ID,
            "contract_version": IDS_MECHANISM_EXPLORATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "mechanism candidates are deterministically ranked with stable support/novelty tie breaks",
                "study and modality closure, peer quorum, replay, provenance, policy, approval, and locality gates are explicit",
                "competing explanations, contradictions, counterevidence, omissions, and negative results remain visible"
            ],
            "limitations": [
                "the route verifies caller-supplied summaries and does not fit models, execute experiments, or move raw data",
                "a qualified portfolio is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_experiment_design_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_experiment_design_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_EXPERIMENT_DESIGN_FEATURE_ID,
            "contract_version": IDS_EXPERIMENT_DESIGN_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "design candidates are deterministically ranked by power, effect, and stable identity",
                "required controls, power shortfalls, replay, authorization, locality, uncertainty, and negative evidence remain explicit",
                "the route only emits a preclinical design frontier and cannot enroll subjects, control instruments, or make clinical decisions"
            ],
            "limitations": [
                "the route evaluates caller-supplied design summaries and does not calculate power from raw measurements or execute laboratory work",
                "a qualified design frontier is not biological validity, statistical significance, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_protocol_simulation_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_protocol_simulation_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_PROTOCOL_SIMULATION_FEATURE_ID,
            "contract_version": IDS_PROTOCOL_SIMULATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "stage, scenario, batch, capacity, peer, recovery, and evidence partitions are deterministic and replay-bound",
                "fault scenarios, unknown evidence, contradictions, budget overflow, and federation omissions remain explicit",
                "the route produces protocol metadata only and cannot schedule animals, control instruments, or make clinical decisions"
            ],
            "limitations": [
                "the route simulates caller-supplied protocol summaries and does not execute protocols or inspect instrument state",
                "a qualified simulation report is not biological validity, operational approval, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_laboratory_integration_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_laboratory_integration_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_LABORATORY_INTEGRATION_FEATURE_ID,
            "contract_version": IDS_LABORATORY_INTEGRATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "instrument, action, interlock, emergency-stop, compensation, peer, and evidence states are deterministic and replay-bound",
                "policy, signed approval, federation quorum, provenance, locality, and no-hardware-effect gates are explicit",
                "the route emits only an integration preflight and cannot send commands, enroll subjects, or make clinical decisions"
            ],
            "limitations": [
                "the route evaluates caller-supplied instrument and action attestations and does not connect to hardware or telemetry",
                "a qualified preflight is not a physical execution authorization, instrument safety certification, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_computational_execution_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_computational_execution_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_COMPUTATIONAL_EXECUTION_FEATURE_ID,
            "contract_version": IDS_COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "workflow dependencies are deterministically planned with explicit cycles, missing dependencies, retries, and node partitions",
                "budget, replay, provenance, policy, federation quorum, authorization, locality, and negative evidence remain auditable",
                "the route emits only a dry-run capability receipt and cannot dispatch jobs, move raw data, or make clinical decisions"
            ],
            "limitations": [
                "the route evaluates caller-supplied workflow manifests and does not launch processes, containers, instruments, or networks",
                "a qualified dry-run plan is not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_statistical_causal_ml_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_statistical_causal_ml_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_STATISTICAL_CAUSAL_ML_FEATURE_ID,
            "contract_version": IDS_STATISTICAL_CAUSAL_ML_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "analysis candidates are deterministically scored with stable selected and fallback portfolios",
                "study identity, sample power, missingness, robustness, evidence, replay, provenance, policy, and locality gates remain explicit",
                "the route emits a bounded local planning receipt and cannot fit models, move raw data, publish conclusions, or make clinical decisions"
            ],
            "limitations": [
                "the route evaluates caller-supplied analysis manifests and does not execute statistical, causal, or ML computation",
                "a qualified analysis plan is not a scientific conclusion, causal identification proof, model performance claim, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_retrieval_synthesis_assurance_harness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_retrieval_synthesis_assurance_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_FEATURE_ID,
            "contract_version": IDS_RETRIEVAL_SYNTHESIS_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed evidence, source, and peer states are deterministically partitioned with stable ordering",
                "staleness, relevance, comparability, provenance, replay, policy, approval, federation, budget, and negative evidence remain explicit",
                "only digest-bound permitted summaries can be acknowledged and the route never performs network retrieval or exports raw evidence"
            ],
            "limitations": [
                "the route verifies caller-supplied retrieval manifests and does not fetch or independently validate source content",
                "a qualified synthesis assurance is not evidence truth, causal validity, publication approval, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_replication_negative_results_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_replication_interoperability_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_REPLICATION_INTEROPERABILITY_FEATURE_ID,
            "contract_version": IDS_REPLICATION_INTEROPERABILITY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal study, modality, observation, site, outcome, and peer states are deterministically partitioned",
                "positive, null, negative, inconclusive, incomparable, contradictory, omitted, and uncertain evidence remains visible with a robust effect median",
                "only aggregate digest summaries cross the interoperability boundary and the route never reruns experiments, exports raw observations, or makes clinical decisions"
            ],
            "limitations": [
                "the route validates caller-supplied replication attestations and does not independently reproduce experiments or inspect raw imaging/omics data",
                "a qualified replication record is not causal proof, publication approval, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_publication_research_object_release_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_publication_release_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_PUBLICATION_RELEASE_FEATURE_ID,
            "contract_version": IDS_PUBLICATION_RELEASE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "research-object artifacts are deterministically partitioned into selected, unresolved, and blocked release states",
                "provenance, evidence, explicit omissions, negative results, replay identity, peer quorum, policy, approval, federation, budget, and locality remain auditable",
                "the route emits only a digest-only signed release intent and cannot publish raw data, execute instruments, or make clinical decisions"
            ],
            "limitations": [
                "the route validates caller-supplied local manifests and peer summaries and does not upload objects or independently reproduce results",
                "a qualified release intent is not a publication, scientific truth claim, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_typed_determinism_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_typed_determinism_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_TYPED_DETERMINISM_FEATURE_ID,
            "contract_version": IDS_TYPED_DETERMINISM_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "version, canonical-field, input-digest, replay, provenance, evidence, and endpoint state negotiation is deterministic",
                "accepted, migrated, approval-required, incompatible, blocked, missing-version, omission, uncertainty, and negative evidence remain separately auditable",
                "the route emits only digest-bound artifact exchange or an unsafe-release block and never moves raw data or makes clinical decisions"
            ],
            "limitations": [
                "the gateway compares caller-supplied endpoint manifests and does not perform network negotiation or rewrite payloads",
                "a qualified compatibility receipt is not proof of scientific validity or publication approval"
            ]
        }))
    }

    pub(super) fn ids_typed_determinism_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let output =
            crate::research_contracts::operate_ids_typed_determinism_assurance_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_TYPED_DETERMINISM_ASSURANCE_FEATURE_ID,
            "contract_version": IDS_TYPED_DETERMINISM_ASSURANCE_CONTRACT_VERSION,
            "output": output,
            "guarantees": [
                "typed capability implementations are deterministically ordered and partitioned into verified, mismatch, unresolved, and blocked states",
                "canonical fields, input/output digests, replay identity, provenance, evidence, policy, protected closure, signed approval, locality, and adversarial gates fail closed",
                "omissions, uncertainty, contradictions, and negative results remain explicit in the content-addressed output",
                "the route verifies caller-supplied artifacts only and never executes providers, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the assurance harness does not independently recompute provider outputs or retrieve external evidence",
                "a qualified parity receipt is release evidence, not scientific validity or publication approval"
            ]
        }))
    }

    pub(super) fn ids_prospective_provenance_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let output = crate::research_contracts::operate_ids_prospective_provenance_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_PROSPECTIVE_PROVENANCE_FEATURE_ID,
            "contract_version": IDS_PROSPECTIVE_PROVENANCE_CONTRACT_VERSION,
            "output": output,
            "guarantees": [
                "artifact derivation lineage is deterministically ordered with verified, unresolved, blocked, missing-parent, cycle, signature, and root-mismatch partitions",
                "provenance, replay, evidence, policy, protected closure, signed approval, locality, and adversarial gates fail closed",
                "semantic loss, omissions, uncertainty, contradictions, and negative results remain in the signed-envelope metadata",
                "the route verifies local attestations only and never signs bytes, exports raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the harness does not independently retrieve parent artifacts or validate cryptographic signatures against an external trust service",
                "a qualified provenance envelope is release evidence, not scientific validity or publication approval"
            ]
        }))
    }

    pub(super) fn ids_provenance_signing_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_provenance_signing_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_PROVENANCE_SIGNING_FEATURE_ID,
            "contract_version": IDS_PROVENANCE_SIGNING_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "provenance parents, DAG cycles, detached signature attestations, root digests, evidence states, and semantic omissions are deterministic",
                "verified, unresolved, blocked, missing-parent, cycle, invalid-signature, root-mismatch, uncertainty, and negative evidence remain separately auditable",
                "the route emits only digest-bound provenance exchange or an unsafe-release block and never exports raw data or makes clinical decisions"
            ],
            "limitations": [
                "the verifier checks caller-supplied local attestations and does not sign bytes or independently retrieve lineage",
                "a qualified provenance receipt is not proof of scientific validity or publication acceptance"
            ]
        }))
    }

    pub(super) fn ids_performance_reliability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let result =
            crate::research_contracts::operate_ids_performance_reliability_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_PERFORMANCE_RELIABILITY_FEATURE_ID,
            "contract_version": IDS_PERFORMANCE_RELIABILITY_CONTRACT_VERSION,
            "result": result,
            "guarantees": [
                "bounded workload latency, completion, retry, duplicate-event, endpoint-approval, replay, and evidence checks are deterministic",
                "dependable, degraded, blocked, missing, retry, timeout, duplicate, omission, uncertainty, and negative evidence remain auditable",
                "the route emits only digest-bound exchange metadata and never connects, retries, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the gateway evaluates caller-supplied workload measurements and does not execute or retry provider work",
                "a qualified result is reliability evidence, not scientific validity or service-level certification"
            ]
        }))
    }

    pub(super) fn ids_interoperability_extensibility_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let output =
            crate::research_contracts::operate_ids_interoperability_extensibility_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_INTEROPERABILITY_EXTENSIBILITY_FEATURE_ID,
            "contract_version": IDS_INTEROPERABILITY_EXTENSIBILITY_CONTRACT_VERSION,
            "output": output,
            "guarantees": [
                "capability/provider/version/modality partitions are deterministic and preserve migration and semantic-loss witnesses",
                "replay, provenance, evidence, omission, uncertainty, negative-result, policy, protected-closure, signed/tool-approval, locality, and aggregate-only gates remain explicit",
                "the route emits only a content-addressed NegotiatedIntegration3 read-only artifact and never invokes tools, opens connections, moves raw data, asserts biology, or makes clinical decisions"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied capability manifests and does not invoke tools or rewrite migrations",
                "a qualified integration is approval metadata, not scientific validity or instrument authorization"
            ]
        }))
    }

    pub(super) fn ids_policy_autonomy_workbench(&self, arguments: &Value) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_policy_autonomy_workbench_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_POLICY_AUTONOMY_WORKBENCH_FEATURE_ID,
            "contract_version": IDS_POLICY_AUTONOMY_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed actions are deterministically partitioned into admitted, approval-required, and denied states",
                "scope, autonomy tier, revocation, budget, replay, evidence, policy, protected closure, signed approval, locality, and adversarial gates remain explicit",
                "omission, uncertainty, missing authority, over-budget, scope-mismatch, revocation, and negative-result witnesses are preserved",
                "the workbench provides a read-only authorization view and never executes actions, grants external authority, or makes clinical decisions"
            ],
            "limitations": [
                "the workbench evaluates caller-supplied action attestations and does not mint credentials or invoke tools",
                "a qualified policy receipt is not permission to operate instruments or provide clinical advice"
            ]
        }))
    }

    pub(super) fn ids_federation_security_contract(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let envelope = crate::research_contracts::operate_ids_federation_security_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_FEDERATION_SECURITY_FEATURE_ID,
            "contract_version": IDS_FEDERATION_SECURITY_CONTRACT_VERSION,
            "envelope": envelope,
            "guarantees": [
                "purpose, permission, semantic profile, replay, provenance, locality, aggregate-only, revocation, and network gates are deterministic",
                "accepted, denied, revoked, unresolved, missing, omission, uncertainty, and negative evidence remain separately auditable",
                "qualified output is a digest-only federation envelope and never exports raw research data or grants clinical authority"
            ],
            "limitations": [
                "the contract evaluates caller-supplied contribution manifests and does not open network connections or transfer artifacts",
                "a qualified envelope is exchange metadata, not scientific validity or publication approval"
            ]
        }))
    }

    pub(super) fn ids_policy_autonomy_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_policy_autonomy_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_POLICY_AUTONOMY_FEATURE_ID,
            "contract_version": IDS_POLICY_AUTONOMY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "actor scope, risk tier, authority, approval, revocation, budget, action, federation, and locality checks are deterministic",
                "admitted, approval-required, denied, revoked, over-budget, scope-mismatch, missing-authority, omission, uncertainty, and negative evidence remain auditable",
                "the route emits only digest-bound policy receipts or an unsafe-release block and never grants external authority, executes actions, or makes clinical decisions"
            ],
            "limitations": [
                "the gateway evaluates caller-supplied authority declarations and does not mint keys or invoke tools",
                "a qualified admission receipt is not permission to operate physical instruments or provide clinical advice"
            ]
        }))
    }

    pub(super) fn ids_federated_workflow_fabric(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_federated_workflow_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_FEDERATED_WORKFLOW_FEATURE_ID,
            "contract_version": IDS_FEDERATED_WORKFLOW_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "stage dependency, cycle, checkpoint, compensation, peer quorum, budget, evidence, replay, policy, federation, and locality checks are deterministic",
                "selected, unresolved, blocked, missing-dependency, cycle, omission, uncertainty, and negative evidence remain auditable",
                "the route emits only digest-bound workflow summaries or an unsafe-release block and never dispatches jobs, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the fabric compiles caller-supplied stage and peer attestations and does not schedule executors or contact instruments",
                "a qualified workflow plan is not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_reliability_copilot(&self, arguments: &Value) -> Result<Value, String> {
        let result = crate::research_contracts::operate_ids_reliability_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_RELIABILITY_COPILOT_FEATURE_ID,
            "contract_version": IDS_RELIABILITY_COPILOT_CONTRACT_VERSION,
            "result": result,
            "guarantees": [
                "idempotency, replay identity, retry budget, evidence, policy, locality, dry-run, and total budget checks are deterministic",
                "dry-run, ready, retry, unresolved, blocked, duplicate, replay-mismatch, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only reliability-plan observation or local capability-management receipts and never executes jobs, controls instruments, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied capability manifests and does not dispatch retries or guarantee executor availability",
                "a qualified reliability result is a preflight artifact, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_interoperability_gateway(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_interoperability_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_INTEROPERABILITY_GATEWAY_FEATURE_ID,
            "contract_version": IDS_INTEROPERABILITY_GATEWAY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "capability identity, version intersection, semantic profile, replay, evidence, policy, federation, signed approval, and locality checks are deterministic",
                "accepted, migrated, incompatible, unresolved, blocked, missing-capability, omission, uncertainty, migration-loss, and negative-evidence states remain auditable",
                "the route emits only digest-bound integration-manifest exchange or local capability management and never invokes endpoints, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the gateway evaluates caller-supplied manifests and does not connect to endpoints or perform migration",
                "a qualified negotiation receipt is compatibility evidence, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_evaluation_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let card = crate::research_contracts::operate_ids_evaluation_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_EVALUATION_ASSURANCE_FEATURE_ID,
            "contract_version": IDS_EVALUATION_ASSURANCE_CONTRACT_VERSION,
            "card": card,
            "guarantees": [
                "benchmark, metric, threshold, baseline, evidence, replay, policy, signed-approval, locality, aggregate-only, and protected-closure checks are deterministic",
                "passed, failed, unknown, unmeasured, contradicted, omitted, baseline-delta, uncertainty, and negative-evidence states remain auditable",
                "the route emits only evaluation-card measurement or local capability-management receipts and never reads raw studies, schedules work, or makes clinical decisions"
            ],
            "limitations": [
                "the harness verifies caller-supplied metric summaries and does not calculate metrics from raw data",
                "a qualified evaluation card is evidence for release review, not scientific validity, execution success, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_research_workbench(&self, arguments: &Value) -> Result<Value, String> {
        let workspace = crate::research_contracts::operate_ids_research_workbench_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_RESEARCH_WORKBENCH_FEATURE_ID,
            "contract_version": IDS_RESEARCH_WORKBENCH_CONTRACT_VERSION,
            "workspace": workspace,
            "guarantees": [
                "study, modality, comparability, evidence, provenance, replay, policy, approval, locality, and protected-closure gates are deterministic",
                "selected, unresolved, blocked, missing-study, missing-modality, omission, uncertainty, view, and negative-evidence states remain auditable",
                "the route exposes digest-bound workspace views and local capability management only; it never renders raw bytes, executes jobs, or makes clinical decisions"
            ],
            "limitations": [
                "the workbench compiles caller-supplied summaries and does not retrieve or transform raw imaging or omics data",
                "a qualified workspace is an interaction artifact, not scientific validity, execution success, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_contract_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let manifest = crate::research_contracts::operate_ids_contract_frontier_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_CONTRACT_FRONTIER_FEATURE_ID,
            "contract_version": IDS_CONTRACT_FRONTIER_CONTRACT_VERSION,
            "manifest": manifest,
            "guarantees": [
                "contract-family, effect, evidence, provenance, replay, policy, federation, approval, locality, and aggregate-only gates are deterministic",
                "accepted, migrated, unresolved, blocked, incompatible, missing-effect, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only digest-bound capability-manifest exchange or local capability management and never loads endpoints, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the harness verifies caller-supplied manifests and does not load implementations or execute tools",
                "a qualified capability manifest is compatibility evidence, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_limitation_closure(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_limitation_closure_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_LIMITATION_CLOSURE_FEATURE_ID,
            "contract_version": IDS_LIMITATION_CLOSURE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "limitation scope, closure criteria, evidence, replay, policy, federation, signed-approval, peer-quorum, locality, and aggregate-only gates are deterministic",
                "closed, partial, unknown, blocked, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only digest-bound permitted-limitation exchange or local capability management and never moves raw data or makes clinical decisions"
            ],
            "limitations": [
                "the gateway verifies caller-supplied limitation and peer attestations and does not independently measure a limitation",
                "a closed receipt is an interoperability and governance artifact, not scientific validity, execution success, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_dependency_composition(&self, arguments: &Value) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_ids_dependency_composition_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_DEPENDENCY_COMPOSITION_FEATURE_ID,
            "contract_version": IDS_DEPENDENCY_COMPOSITION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "capability, dependency, modality, study, evidence, replay, policy, protected-closure, approval, locality, and aggregate-only gates are deterministic",
                "qualified, unresolved, blocked, missing-capability, omission, uncertainty, and negative-evidence states remain auditable",
                "the route exposes only digest-bound composition views and local capability management and never loads implementations or moves raw data"
            ],
            "limitations": [
                "the workbench ranks caller-supplied capability manifests and does not execute providers or independently validate their scientific claims",
                "a qualified composition is a planning and interoperability artifact, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_semantic_parity(&self, arguments: &Value) -> Result<Value, String> {
        let witness = crate::research_contracts::operate_ids_semantic_parity_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_SEMANTIC_PARITY_FEATURE_ID,
            "contract_version": IDS_SEMANTIC_PARITY_CONTRACT_VERSION,
            "witness": witness,
            "guarantees": [
                "schema, semantic, study, modality, artifact, provenance, replay, policy, protected-closure, approval, locality, and aggregate-only checks are deterministic",
                "qualified, unresolved, blocked, missing-study, missing-modality, omission, uncertainty, and negative-evidence states remain auditable",
                "the route compares digest-only summaries and never imports raw imaging or omics outputs or makes clinical decisions"
            ],
            "limitations": [
                "the model compares caller-supplied parity fixtures and does not independently inspect producer implementations or raw measurements",
                "a qualified witness is compatibility evidence, not scientific validity, execution success, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_scale_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let report = crate::research_contracts::operate_ids_scale_frontier_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_SCALE_FRONTIER_FEATURE_ID,
            "contract_version": IDS_SCALE_FRONTIER_CONTRACT_VERSION,
            "report": report,
            "guarantees": [
                "typed workload cells are sorted and classified deterministically against declared concurrency, capacity, latency, cost, budget, evidence, replay, policy, approval, locality, and aggregate-only gates",
                "qualified, unresolved, blocked, capacity-exceeded, budget-exhausted, unknown, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only digest-bound local capacity previews and capability-management effects; it never schedules jobs, dispatches instruments, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "capacity, latency, cost, and evidence values are caller-declared summaries and are not independent measurements",
                "a qualified report is a planning and admission preview, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_adversarial_recovery(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_adversarial_recovery_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_ADVERSARIAL_RECOVERY_FEATURE_ID,
            "contract_version": IDS_ADVERSARIAL_RECOVERY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "hostile and failed events are ordered and classified deterministically with replay, checkpoint, evidence, authorization, recovery, and locality witnesses",
                "qualified, unresolved, blocked, hostile, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only digest-bound local recovery previews and capability-management effects; it never retries jobs, invokes connectors, moves raw data, or grants authority"
            ],
            "limitations": [
                "event payloads, checkpoints, and recovery flags are caller-declared digests and are not independently inspected",
                "a qualified receipt is a recovery posture preview, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_federated_commons(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_federated_commons_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_FEDERATED_COMMONS_FEATURE_ID,
            "contract_version": IDS_FEDERATED_COMMONS_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "peer capability, semantic-profile, evidence, replay, signature, federation, quorum, policy, protected-closure, approval, locality, and aggregate-only gates are deterministic",
                "qualified, unresolved, blocked, missing-peer, contradiction, omission, uncertainty, and negative-evidence states remain auditable",
                "the route exchanges only digest-bound federation summaries and local capability-management effects; it never moves raw data, executes remote providers, or makes clinical decisions"
            ],
            "limitations": [
                "peer capability, evidence, and artifact values are caller-declared summaries and are not independently measured",
                "a qualified receipt is a quorum and interoperability preview, not execution success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_bounded_evolution(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_ids_bounded_evolution_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": IDS_BOUNDED_EVOLUTION_FEATURE_ID,
            "contract_version": IDS_BOUNDED_EVOLUTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "capability evolution proposals are ordered and classified deterministically against compatibility, benchmark, safety, evidence, replay, signing, policy, protected-closure, approval, locality, and aggregate-only gates",
                "qualified, unresolved, blocked, incompatible, benchmark-failed, safety-failed, omission, uncertainty, and negative-evidence states remain auditable",
                "the route emits only digest-bound local evolution previews and capability-management effects; it never mutates implementations, grants authority, or publishes releases"
            ],
            "limitations": [
                "proposal artifacts, benchmarks, safety claims, and compatibility flags are caller-declared digests and are not independently evaluated",
                "a qualified receipt is a promotion preview, not deployment success, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_local_evidence_surveillance_inference(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an IDS EvidenceFeed1")?;
        let receipt =
            crate::research_contracts::run_ids_local_evidence_surveillance_inference_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_ids::IDS_LOCAL_EVIDENCE_SURVEILLANCE_FEATURE_ID,
            "contract_version": bioprism_ids::IDS_LOCAL_EVIDENCE_SURVEILLANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "one institution-local study is ranked deterministically by relevance and stable evidence identity",
                "qualified, unknown, contradicted, unmeasured, omitted, negative, replay, provenance, policy, protected-closure, and locality states remain explicit",
                "only a qualified read-only local artifact emits a read effect; no sources are fetched, raw data is exported, or clinical decision is made"
            ],
            "limitations": [
                "the inference engine consumes caller-supplied EvidenceFeed1 metadata and does not inspect source payloads",
                "a qualified evidence alert is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn ids_federated_interpretation_visualization_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized ids EvidenceBackedResult4")?;
        let receipt =
            crate::research_contracts::operate_ids_interpretation_visualization_assurance_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_ids::IDS_INTERPRETATION_VISUALIZATION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "candidate ordering, state partition, comparability, omission, uncertainty, and negative-evidence reporting are deterministic",
                "policy, protected-closure, signed-approval, locality, federation, replay, and adversarial gates remain explicit",
                "qualified interpretations emit a verification receipt and unresolved or blocked interpretations emit block:unsafe-release"
            ],
            "limitations": [
                "the tool evaluates typed evidence declarations without rendering, raw-data access, biological inference, or clinical decisions",
                "aggregate-only output requires independent scientific review and does not itself establish biological validity"
            ]
        }))
    }
}
