//! MCP Adapter execution and evidence handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    /// Retain caller-supplied execution and semantic-loss observations for one declared adapter.
    /// The core validates catalogue/group scope and indexes the evidence, but never runs the
    /// adapter or imports a delegated dependency.
    pub(super) fn adapter_execution_evidence(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode adapter execution evidence: {error}"))?;
        if encoded.len() > 1_000_000 {
            return Err("adapter execution evidence exceeds the 1000000-byte safety bound".into());
        }
        let request: AdapterExecutionEvidenceRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid adapter execution evidence: {error}"))?;
        let adapter_registry = AdapterRegistry::default();
        let adapter = adapter_registry
            .descriptors()
            .iter()
            .find(|descriptor| descriptor.id == request.adapter_id)
            .ok_or_else(|| {
                format!(
                    "adapter_id {:?} is not in the declared adapter registry",
                    request.adapter_id
                )
            })?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let group = catalogue
            .groups()
            .iter()
            .find(|group| group.id == request.group_id)
            .ok_or_else(|| format!("unknown capability group {:?}", request.group_id))?;
        for domain in &request.domains {
            if !group
                .domains
                .iter()
                .any(|declared| declared.eq_ignore_ascii_case(domain))
            {
                return Err(format!(
                    "domain label {:?} is not declared by capability group {:?}",
                    domain, request.group_id
                ));
            }
        }
        let subject_id = request.subject_id.clone();
        let domains = request.domains.clone();
        let parent_digests = request.parent_digests.clone();
        let adapter_summary = json!({
            "id": adapter.id,
            "version": adapter.version,
            "execution": adapter.execution,
            "conformance_level": adapter.conformance_level,
            "optional_dependency": adapter.optional_dependency,
            "declared_loss_kinds": adapter.declared_loss_kinds,
            "scope_dimensions": adapter.scope_dimensions,
        });
        let evidence = record_adapter_execution_evidence(request)
            .map_err(|error| format!("adapter execution evidence refused: {error}"))?;
        let evidence_artifact = evidence
            .get("evidence")
            .cloned()
            .ok_or("adapter execution evidence omitted nested evidence")?;
        let projection = self.index_artifact_projection(
            "adapter_execution_evidence",
            &subject_id,
            domains,
            parent_digests,
            evidence_artifact,
        );
        let mut result = evidence;
        result["adapter"] = adapter_summary;
        result["artifact_registry"] = projection;
        Ok(result)
    }

    /// Query retained adapter observations and classify only explicit parent joins to source or
    /// workflow projections. The query never executes adapters or infers provenance from labels.
    pub(super) fn adapter_execution_evidence_query(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode adapter execution evidence query: {error}"))?;
        if encoded.len() > 1_000_000 {
            return Err(
                "adapter execution evidence query exceeds the 1000000-byte safety bound".into(),
            );
        }
        let request: AdapterExecutionEvidenceQueryRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid adapter execution evidence query: {error}"))?;
        let (records, generation) = {
            let registry = self
                .artifact_registry
                .lock()
                .map_err(|_| "artifact registry lock is poisoned".to_string())?;
            (registry.records_for_audit(), registry.generation())
        };
        let report = query_adapter_execution_evidence(&records, generation, request)
            .map_err(|error| format!("adapter execution evidence query refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode adapter execution evidence query: {error}"))
    }

    pub(super) fn adapter_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: AdapterPlanRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid adapter plan request: {error}"))?;
        let plan = AdapterRegistry::default()
            .plan(request)
            .map_err(|error| format!("adapter plan refused: {error}"))?;
        let plan_value = serde_json::to_value(&plan)
            .map_err(|error| format!("could not serialize adapter plan: {error}"))?;
        let plan_id = bioprism_ids::ContentHash::of_value(&plan_value)
            .map_err(|error| format!("could not hash adapter plan: {error}"))?;
        let selected = plan.selected_adapter.as_ref().map(|adapter| {
            json!({
                "id": adapter.id,
                "execution": adapter.execution,
                "version": adapter.version,
                "conformance_level": adapter.conformance_level,
                "optional_dependency": adapter.optional_dependency,
                "declared_loss_kinds": adapter.declared_loss_kinds,
                "scope_dimensions": adapter.scope_dimensions,
            })
        });
        Ok(json!({
            "ok": true,
            "workflow": "adapter_plan",
            "plan_id": plan_id,
            "registry": bioprism_adapter::ADAPTER_REGISTRY_SCHEMA_VERSION,
            "executable": plan.executable,
            "selected_adapter": selected,
            "plan": plan,
            "execution": "not_started",
            "guarantees": [
                "format matching is explicit and content sniffing is refused",
                "native adapters and Python-delegated biological adapters share one declared loss vocabulary",
                "optional dependency absence or uncertainty is surfaced before execution",
                "the planner does not fetch bytes, import packages, execute adapters, or grant credentials",
            ],
        }))
    }

    pub(super) fn adapter_federated_context_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedContextQuestion5")?;
        let receipt =
            crate::research_contracts::operate_adapter_federated_context_copilot_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::FEDERATED_CONTEXT_COPILOT_FEATURE_ID,
            "contract_version": bioprism_adapter::FEDERATED_CONTEXT_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "institution-local context facts are ranked deterministically by evidence, freshness, and stable identity",
                "scope, policy, protected closure, signed approval, federation, replay/provenance, aggregate-only locality, capacity, and adversarial gates fail closed",
                "unknown, stale, speculative, contradictory, omitted, negative, missing, and revoked facts remain explicit and no raw source is read or exported"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied fact attestations and does not retrieve sources or compile a Decision Section",
                "a qualified receipt is a bounded context-admission artifact, not scientific truth, workflow completion, or clinical advice"
            ]
        }))
    }

    pub(super) fn adapter_local_evidence_surveillance_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalEvidenceSurveillanceResearchCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_evidence_surveillance_research_copilot_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "only declared tools can be requested and max_tool_calls remains bounded",
                "dry-run and bounded invocation effects are distinct and replay-addressed",
                "unknown, stale, missing, contradicted, required, and negative evidence remain explicit",
                "raw preclinical source payloads remain institution-local and policy/closure failures block"
            ],
            "limitations": [
                "the copilot qualifies caller-supplied evidence metadata and does not infer a clinical or treatment conclusion",
                "tool hosts remain responsible for connector sandboxing, key revocation, and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_multimodal_evidence_surveillance_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a MultimodalEvidenceSurveillanceResearchCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::run_multimodal_evidence_surveillance_research_copilot_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "study-by-modality closure is deterministic and missing cells remain explicit",
                "semantic-profile mismatches are incomparable and cannot be silently merged",
                "dry-run and signed bounded invocation effects are distinct and replay-addressed",
                "unknown, stale, contradicted, negative, and denied evidence remain visible",
                "raw preclinical source payloads remain institution-local and policy failures block"
            ],
            "limitations": [
                "the copilot qualifies caller-supplied evidence metadata and does not infer a clinical or treatment conclusion",
                "tool hosts remain responsible for connector sandboxing, key revocation, and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_throughput_evidence_surveillance_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputEvidenceSurveillanceResearchCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_evidence_surveillance_research_copilot_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "checkpoint and queue identity are deterministic",
                "capacity overflow, unknown, negative, and denied evidence remain explicit",
                "dry-run and signed bounded invocation effects are distinct",
                "raw preclinical source payloads remain institution-local"
            ],
            "limitations": [
                "the copilot qualifies caller-supplied evidence metadata and does not make clinical decisions",
                "tool hosts remain responsible for connector isolation, key revocation, and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_federated_continual_evidence_surveillance_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedContinualEvidenceSurveillanceResearchCopilotRequest",
        )?;
        let receipt = crate::research_contracts::run_federated_continual_evidence_surveillance_research_copilot_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_COPILOT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "only signed, permitted aggregate-only artifacts cross the federation boundary",
                "purpose, semantic profile, peer quorum, replay, and envelope identity are explicit",
                "unknown, contradicted, negative, denied, and incomplete evidence remain visible",
                "raw observations remain institution-local and policy or closure failures block"
            ],
            "limitations": [
                "the copilot qualifies caller-supplied aggregate metadata and does not infer clinical conclusions",
                "institutions remain responsible for signer rotation, connector isolation, and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_local_evidence_surveillance_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a LocalEvidenceSurveillanceWorkflowRequest")?;
        let receipt =
            crate::research_contracts::run_local_evidence_surveillance_workflow_fabric_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "canonical workflow stages, checkpoint identity, budget admission, and replay are explicit",
                "unknown, unavailable, negative, and unresolved evidence remain visible",
                "schedule, compensation, and unsafe-release effects are distinct",
                "raw preclinical data remains institution-local"
            ],
            "limitations": [
                "the fabric orchestrates caller-supplied evidence metadata and does not make clinical decisions",
                "operators remain responsible for local artifact retention and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_multimodal_evidence_surveillance_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a MultimodalEvidenceSurveillanceWorkflowRequest",
        )?;
        let receipt =
            crate::research_contracts::run_multimodal_evidence_surveillance_workflow_fabric_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "comparability, modality closure, signed approval, checkpoint identity, budget admission, and replay are explicit",
                "unknown, incomparable, missing-cell, negative, and unresolved evidence remain visible",
                "schedule, compensation, and unsafe-release effects are distinct",
                "raw preclinical data remains institution-local"
            ],
            "limitations": [
                "the fabric orchestrates caller-supplied evidence metadata and does not make clinical decisions",
                "operators remain responsible for local artifact retention and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_throughput_evidence_surveillance_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputEvidenceSurveillanceWorkflowRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_evidence_surveillance_workflow_fabric_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "queue checkpoint, capacity admission, overflow compensation, and replay are explicit",
                "unknown, denied, negative, and unresolved evidence remain visible",
                "schedule, compensation, and unsafe-release effects are distinct",
                "raw preclinical data remains institution-local"
            ],
            "limitations": [
                "the fabric orchestrates caller-supplied evidence metadata and does not make clinical decisions",
                "operators remain responsible for queue retention, signer rotation, and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_federated_continual_evidence_surveillance_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedContinualEvidenceSurveillanceWorkflowRequest",
        )?;
        let receipt = crate::research_contracts::run_federated_continual_evidence_surveillance_workflow_fabric_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "purpose-bound peer quorum, checkpoint identity, aggregate-only federation, and replay are explicit",
                "unknown, denied, negative, quorum-incomplete, and unresolved evidence remain visible",
                "schedule, compensation, and unsafe-release effects are distinct",
                "raw preclinical observations remain institution-local"
            ],
            "limitations": [
                "the fabric orchestrates caller-supplied aggregate metadata and does not make clinical decisions",
                "institutions remain responsible for signer rotation, endpoint isolation, and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_local_evidence_surveillance_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalEvidenceSurveillanceResearchWorkbenchRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_evidence_surveillance_research_workbench_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_LOCAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "A0 local read-only workbench preserves qualified, unknown, negative, omission, and provenance views",
                "canonical view and panel order is replayable across clients",
                "raw preclinical data remains institution-local and no external effect is scheduled"
            ],
            "limitations": [
                "the workbench renders caller-supplied evidence metadata and does not make clinical decisions",
                "operators remain responsible for source authorization and independent replication"
            ]
        }))
    }

    pub(super) fn adapter_multimodal_evidence_surveillance_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a MultimodalEvidenceSurveillanceResearchWorkbenchRequest")?;
        let receipt = crate::research_contracts::run_multimodal_evidence_surveillance_research_workbench_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 multimodal workbench preserves study/modality comparability, qualified, unknown, incomparable, missing, negative, and provenance views", "canonical view and panel order is replayable across clients", "raw preclinical data remains institution-local and no external effect is scheduled"], "limitations": ["the workbench renders caller-supplied evidence metadata and does not make clinical decisions", "operators remain responsible for semantic-profile governance and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_evidence_surveillance_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a ThroughputEvidenceSurveillanceResearchWorkbenchRequest")?;
        let receipt = crate::research_contracts::run_throughput_evidence_surveillance_research_workbench_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 throughput workbench preserves queue, capacity, checkpoint, qualified, unknown, overflow, negative, and provenance views", "canonical view and panel order is replayable across clients", "raw preclinical data remains institution-local and no external effect is scheduled"], "limitations": ["the workbench renders caller-supplied evidence metadata and does not make clinical decisions", "operators remain responsible for capacity policy, queue retention, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_continual_evidence_surveillance_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest")?;
        let receipt = crate::research_contracts::run_federated_continual_evidence_surveillance_research_workbench_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 federated continual workbench exposes peer, aggregate, omission, denied, negative, qualified, unknown, and provenance views without moving raw observations", "canonical view and panel order is replayable across clients", "only permitted signed aggregate evidence is represented and raw preclinical data remains institution-local"], "limitations": ["the workbench renders caller-supplied aggregate evidence metadata and does not make clinical decisions", "operators remain responsible for federation policy, signer governance, and independent replication"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalRetrievalSynthesisInferenceEngineRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_inference_engine_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_FEATURE_ID, "receipt": receipt, "guarantees": ["A0 local retrieval engine computes a deterministic single-study evidence corpus from typed candidate metadata", "omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external effects are scheduled"], "limitations": ["the engine is not a clinical decision system", "operators remain responsible for source scope and replication"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalRetrievalSynthesisContractModelRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_contract_model_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_FEATURE_ID, "receipt": receipt, "guarantees": ["A0 local typed data primitive validates schema profile, canonicalization, consumer identity, and deterministic single-study evidence corpus", "omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external effects are scheduled"], "limitations": ["the contract model is not a clinical decision system", "operators remain responsible for schema evolution, source scope, and replication"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalRetrievalSynthesisResearchCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_research_copilot_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 local research copilot ranks scoped evidence for a named agent and emits read-only evidence-ranked recommendations", "omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no clinical decision or external effect is scheduled"], "limitations": ["the copilot is advisory and not a clinical decision system", "operators remain responsible for source scope, approval, schema evolution, and independent replication"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a MultimodalRetrievalSynthesisResearchCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::run_multimodal_retrieval_synthesis_research_copilot_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 multimodal research copilot compares at least two institution-local modalities across studies and emits an approval-gated tool intent", "omissions, uncertainty, contradiction, comparability gaps, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external tool effect is scheduled without a non-empty approval token"], "limitations": ["the copilot is not a clinical decision system", "operators remain responsible for tool approval, source scope, schema evolution, and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputRetrievalSynthesisResearchCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_retrieval_synthesis_research_copilot_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 prospective high-throughput copilot admits bounded queue capacity with checkpoint identity and compares at least two institution-local modalities", "overflow, omissions, uncertainty, contradiction, comparability gaps, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external tool effect is scheduled without a non-empty approval token"], "limitations": ["the copilot is not a clinical decision system", "operators remain responsible for queue capacity, checkpoint durability, tool approval, source scope, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_continual_retrieval_synthesis_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a FederatedContinualRetrievalSynthesisResearchCopilotRequest")?;
        let receipt = crate::research_contracts::run_federated_continual_retrieval_synthesis_research_copilot_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_COPILOT_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 federated continual copilot enforces purpose-bound peer quorum, aggregate-only exchange, bounded capacity, and checkpoint identity while comparing at least two institution-local modalities", "overflow, omissions, uncertainty, contradiction, quorum gaps, comparability gaps, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external tool effect is scheduled without approval, quorum, and aggregate-only gates"], "limitations": ["the copilot is not a clinical decision system", "operators remain responsible for federation governance, queue capacity, checkpoint durability, tool approval, and independent replication"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a LocalRetrievalSynthesisWorkflowRequest")?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_workflow_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 local single-study workflow executes canonical retrieval, synthesis, persistence, and validation stages with durable checkpoints, budgets, compensation, and replay identity", "omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical data stays institution-local and clinical decisions are out of scope"], "limitations": ["the workflow is not a clinical decision system", "operators remain responsible for approvals, replication, schema migration, and local artifact retention"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a MultimodalRetrievalSynthesisWorkflowRequest",
        )?;
        let receipt =
            crate::research_contracts::run_multimodal_retrieval_synthesis_workflow_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 multimodal multi-study workflow executes canonical checkpoint, comparability, synthesis, and artifact stages with budgets, compensation, and replay identity", "incomparable, omitted, uncertain, contradictory, and negative evidence remain explicit", "raw preclinical data stays institution-local and clinical decisions are out of scope"], "limitations": ["the workflow is not a clinical decision system", "operators remain responsible for approvals, replication, schema migration, and local artifact retention"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputRetrievalSynthesisWorkflowRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_retrieval_synthesis_workflow_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 prospective high-throughput workflow enforces bounded queue admission, overflow retention, checkpoint continuity, budget compensation, replay identity, and output validation", "overflow, omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical data stays institution-local and clinical decisions are out of scope"], "limitations": ["the workflow is not a clinical decision system", "operators remain responsible for approvals, queue retention, replication, schema migration, and local artifact retention"]}),
        )
    }

    pub(super) fn adapter_federated_continual_retrieval_synthesis_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedContinualRetrievalSynthesisWorkflowRequest",
        )?;
        let receipt =
            crate::research_contracts::run_federated_continual_retrieval_synthesis_workflow_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 federated continual workflow enforces purpose-bound peer quorum, aggregate-only exchange, checkpoint continuity, bounded budget, compensation, replay identity, and output validation", "federation denials, omissions, uncertainty, contradiction, overflow, and negative evidence remain explicit", "raw preclinical data stays institution-local and clinical decisions are out of scope"], "limitations": ["the workflow is not a clinical decision system", "operators remain responsible for federation approvals, key rotation, replay, schema migration, and local artifact retention"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalRetrievalSynthesisResearchWorkbenchRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_research_workbench_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A0 local read-only workbench renders canonical overview, evidence, omission, and provenance views over typed retrieval synthesis", "unknown, omitted, contradictory, and negative evidence remain explicit with deterministic replay and provenance digests", "raw preclinical data stays institution-local and no external or clinical decision effect is scheduled"], "limitations": ["the workbench is not a clinical decision system", "operators remain responsible for scope, local retention, independent replication, and schema migration"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a MultimodalRetrievalSynthesisResearchWorkbenchRequest")?;
        let receipt =
            crate::research_contracts::run_multimodal_retrieval_synthesis_research_workbench_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 multimodal workbench renders canonical overview, evidence, omission, provenance, comparability, negative, and unknown views over multiple studies and modalities", "incomparable, omitted, uncertain, contradictory, and negative evidence remain explicit with replay and provenance digests", "raw preclinical data stays institution-local and no external or clinical decision effect is scheduled"], "limitations": ["the workbench is not a clinical decision system", "operators remain responsible for semantic-profile governance, scope, local retention, and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a ThroughputRetrievalSynthesisResearchWorkbenchRequest")?;
        let receipt =
            crate::research_contracts::run_throughput_retrieval_synthesis_research_workbench_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 throughput workbench renders queue, overflow, omission, and provenance views over bounded retrieval synthesis", "capacity overflow, uncertainty, contradiction, and negative evidence remain explicit with replay and queue digests", "raw preclinical data stays institution-local and no external or clinical decision effect is scheduled"], "limitations": ["the workbench is not a clinical decision system", "operators remain responsible for queue retention, scope, local capacity, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_continual_retrieval_synthesis_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a FederatedContinualRetrievalSynthesisResearchWorkbenchRequest")?;
        let receipt = crate::research_contracts::run_federated_continual_retrieval_synthesis_research_workbench_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_RESEARCH_WORKBENCH_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 federated continual workbench renders purpose-bound peer, aggregate, quorum, omission, negative, and provenance views", "quorum, partition, contradiction, uncertainty, negative evidence, and federation denials remain explicit with deterministic replay digests", "raw preclinical data remains institution-local; only permitted aggregate views are exposed and no clinical decision effect is scheduled"], "limitations": ["the workbench is not a clinical decision system", "operators remain responsible for consortium membership, purpose policy, local retention, and independent replication"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a LocalRetrievalSynthesisInteroperabilityGatewayRequest")?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_interoperability_gateway_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 local gateway negotiates pinned retrieval/synthesis protocol versions and records capability and semantic-loss receipts", "incompatible schemas, missing capabilities, protected-closure gaps, and policy denials fail closed", "raw preclinical data remains institution-local and only content-addressed permitted artifacts may be exchanged"], "limitations": ["the gateway is not a clinical decision system", "operators remain responsible for endpoint trust, migration review, and independent replication"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a MultimodalRetrievalSynthesisInteroperabilityGatewayRequest")?;
        let receipt = crate::research_contracts::run_multimodal_retrieval_synthesis_interoperability_gateway_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 multimodal gateway negotiates pinned retrieval/synthesis protocol versions with comparability and semantic-loss receipts", "modality gaps, incompatible schemas, protected-closure gaps, and policy denials fail closed", "raw preclinical data remains institution-local and only content-addressed permitted artifacts may be exchanged"], "limitations": ["the gateway is not a clinical decision system", "operators remain responsible for comparability governance, endpoint trust, migration review, and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a ThroughputRetrievalSynthesisInteroperabilityGatewayRequest")?;
        let receipt = crate::research_contracts::run_throughput_retrieval_synthesis_interoperability_gateway_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 throughput gateway negotiates pinned retrieval/synthesis protocol versions with bounded batch and checkpoint receipts", "capacity, migration loss, omissions, protected-closure gaps, and policy denials fail closed", "raw preclinical data remains institution-local and only content-addressed permitted artifacts may be exchanged"], "limitations": ["the gateway is not a clinical decision system", "operators remain responsible for queue capacity, endpoint trust, migration review, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_continual_retrieval_synthesis_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a FederatedContinualRetrievalSynthesisInteroperabilityGatewayRequest")?;
        let receipt = crate::research_contracts::run_federated_continual_retrieval_synthesis_interoperability_gateway_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_INTEROPERABILITY_GATEWAY_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 federated continual gateway negotiates purpose-bound pinned retrieval/synthesis schemas with quorum and aggregate-only controls", "peer capability gaps, semantic loss, quorum failures, protected-closure gaps, and policy denials fail closed", "raw preclinical observations remain institution-local and only content-addressed permitted artifacts may be exchanged"], "limitations": ["the gateway is not a clinical decision system", "operators remain responsible for consortium membership, purpose policy, endpoint trust, migration review, and independent replication"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_assurance_harness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalRetrievalSynthesisAssuranceHarnessRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_assurance_harness_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 assurance harness evaluates a local retrieval/synthesis workbench against explicit policy, protected-closure, provenance, and evidence predicates", "counterexamples, omissions, uncertainty, negative evidence, and contradictory evidence remain visible", "raw preclinical data remains institution-local and failed release predicates emit a blocking effect"], "limitations": ["the harness is not a clinical decision system", "operators remain responsible for baseline selection, independent replication, and institutional release governance"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_assurance_harness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a MultimodalRetrievalSynthesisAssuranceHarnessRequest",
        )?;
        let receipt =
            crate::research_contracts::run_multimodal_retrieval_synthesis_assurance_harness_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 assurance harness evaluates multimodal retrieval/synthesis against explicit modality, comparability, policy, protected-closure, provenance, and evidence predicates", "counterexamples, omissions, uncertainty, negative evidence, and contradictory evidence remain visible", "raw preclinical data remains institution-local and failed release predicates emit a blocking effect"], "limitations": ["the harness is not a clinical decision system", "operators remain responsible for comparability baselines, independent replication, and institutional release governance"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_assurance_harness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputRetrievalSynthesisAssuranceHarnessRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_retrieval_synthesis_assurance_harness_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 assurance harness evaluates throughput retrieval/synthesis against batch, checkpoint, capacity, policy, protected-closure, provenance, and evidence predicates", "overflow, counterexamples, omissions, uncertainty, negative evidence, and contradictory evidence remain visible", "raw preclinical data remains institution-local and failed release predicates emit a blocking effect"], "limitations": ["the harness is not a clinical decision system", "operators remain responsible for queue baselines, independent replication, and institutional release governance"]}),
        )
    }

    pub(super) fn adapter_federated_continual_retrieval_synthesis_assurance_harness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a FederatedContinualRetrievalSynthesisAssuranceHarnessRequest")?;
        let receipt = crate::research_contracts::run_federated_continual_retrieval_synthesis_assurance_harness_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_ASSURANCE_HARNESS_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 assurance harness evaluates federated continual retrieval/synthesis against purpose, peer quorum, aggregate-only locality, policy, protected-closure, provenance, and evidence predicates", "overflow, federation counterexamples, omissions, uncertainty, negative evidence, and contradictory evidence remain visible", "raw preclinical observations remain institution-local and failed release predicates emit a blocking effect"], "limitations": ["the harness is not a clinical decision system", "operators remain responsible for consortium baselines, independent replication, and institutional release governance"]}),
        )
    }

    pub(super) fn adapter_local_retrieval_synthesis_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a LocalRetrievalSynthesisFederatedControlPlaneRequest",
        )?;
        let receipt =
            crate::research_contracts::run_local_retrieval_synthesis_federated_control_plane_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 control plane admits institution-local retrieval/synthesis service work only under policy, federation-permission, signed-approval, health, and capacity gates", "degraded, approval-required, blocked, and saturated states remain explicit with omission and uncertainty receipts", "raw preclinical observations remain local and control-plane effects are content-addressed"], "limitations": ["the control plane is not a clinical decision system", "operators remain responsible for service health attestations, capacity policy, and independent release governance"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a MultimodalRetrievalSynthesisFederatedControlPlaneRequest")?;
        let receipt = crate::research_contracts::run_multimodal_retrieval_synthesis_federated_control_plane_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 control plane admits multimodal multi-study retrieval/synthesis only when peer quorum, modality comparability, capacity, health, federation, signed approval, and protected-closure gates pass", "aggregate-only federation keeps raw preclinical imaging and omics data institution-local while omission, uncertainty, counterexample, degraded, approval-required, and blocked states remain explicit", "replay and content-addressed control receipts support deterministic audit and recovery"], "limitations": ["the control plane is not a clinical decision system", "operators remain responsible for peer trust, comparability profiles, health attestations, signed authorization, and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a ThroughputRetrievalSynthesisFederatedControlPlaneRequest")?;
        let receipt = crate::research_contracts::run_throughput_retrieval_synthesis_federated_control_plane_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 control plane admits prospective high-throughput retrieval/synthesis only under purpose-bound peer quorum, queue/checkpoint continuity, capacity, health, policy, federation, signed approval, and protected-closure gates", "aggregate-only federation keeps raw preclinical data institution-local while overflow, omissions, uncertainty, counterexamples, degraded, approval-required, and blocked states remain explicit", "content-addressed queue, checkpoint, workbench, health, replay, and control digests support deterministic audit and recovery"], "limitations": ["the control plane is not a clinical decision system", "operators remain responsible for queue policy, peer trust, checkpoint retention, health attestations, signed authorization, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_continual_retrieval_synthesis_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or("request is required and must be a FederatedContinualRetrievalSynthesisFederatedControlPlaneRequest")?;
        let receipt = crate::research_contracts::run_federated_continual_retrieval_synthesis_federated_control_plane_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_PLANE_FEATURE_ID, "receipt": receipt, "guarantees": ["A2 control plane admits federated continual retrieval/synthesis only under purpose-bound peer quorum, checkpoint continuity, capacity, health, policy, federation, signed approval, and protected-closure gates", "aggregate-only locality keeps raw preclinical observations institution-local and preserves omissions, uncertainty, counterexamples, negative evidence, degraded, approval-required, and blocked states", "content-addressed workflow, workbench, health, replay, and control receipts support deterministic audit and bounded autonomy"], "limitations": ["the control plane is not a clinical decision system", "operators remain responsible for consortium governance, peer trust, checkpoint retention, health attestations, signed authorization, and independent replication"]}),
        )
    }

    pub(super) fn adapter_multimodal_retrieval_synthesis_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a MultimodalRetrievalSynthesisInferenceEngineRequest",
        )?;
        let receipt =
            crate::research_contracts::run_multimodal_retrieval_synthesis_inference_engine_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_MULTIMODAL_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 multimodal retrieval engine computes a deterministic imaging/omics multi-study evidence corpus with explicit comparability", "missing modalities, incomparable evidence, contradiction, uncertainty, and negative results remain explicit", "raw preclinical source payloads remain institution-local and no external effects are scheduled"], "limitations": ["the engine is not a clinical decision system", "operators remain responsible for semantic-profile governance and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputRetrievalSynthesisInferenceEngineRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_retrieval_synthesis_inference_engine_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 prospective high-throughput retrieval engine admits bounded batches with capacity and checkpoint receipts", "queue overflow, omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external effects are scheduled"], "limitations": ["the engine is not a clinical decision system", "operators remain responsible for queue retention, capacity policy, and independent replication"]}),
        )
    }

    pub(super) fn adapter_throughput_retrieval_synthesis_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a ThroughputRetrievalSynthesisContractModelRequest",
        )?;
        let receipt =
            crate::research_contracts::run_throughput_retrieval_synthesis_contract_model_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_THROUGHPUT_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 prospective high-throughput typed data primitive validates schema profile, canonicalization, consumer identity, bounded batch, capacity, checkpoint, and deterministic retrieval corpus", "queue overflow, omissions, uncertainty, contradiction, and negative evidence remain explicit", "raw preclinical source payloads remain institution-local and no external effects are scheduled"], "limitations": ["the contract model is not a clinical decision system", "operators remain responsible for queue retention, capacity policy, schema evolution, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_retrieval_synthesis_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedRetrievalSynthesisInferenceEngineRequest",
        )?;
        let receipt =
            crate::research_contracts::run_federated_retrieval_synthesis_inference_engine_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_RETRIEVAL_SYNTHESIS_INFERENCE_ENGINE_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 federated continual retrieval engine enforces purpose, peer quorum, aggregate-only, locality, and policy gates", "omissions, uncertainty, contradiction, negative evidence, and denied federation remain explicit", "raw preclinical source payloads remain institution-local and only permitted aggregate metadata is represented"], "limitations": ["the engine is not a clinical decision system", "operators remain responsible for federation governance, signer policy, and independent replication"]}),
        )
    }

    pub(super) fn adapter_federated_retrieval_synthesis_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedRetrievalSynthesisContractModelRequest",
        )?;
        let receipt =
            crate::research_contracts::run_federated_retrieval_synthesis_contract_model_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_adapter::ADAPTER_FEDERATED_RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 federated continual typed data primitive validates schema profile, canonicalization, consumer identity, purpose, peer quorum, aggregate-only, locality, policy, checkpoint, capacity, and deterministic retrieval corpus", "omissions, uncertainty, contradiction, negative evidence, and denied federation remain explicit", "raw preclinical source payloads remain institution-local and only permitted aggregate metadata is represented"], "limitations": ["the contract model is not a clinical decision system", "operators remain responsible for federation governance, schema evolution, signer policy, and independent replication"]}),
        )
    }

    pub(super) fn adapter_context_compilation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ContextCompilationRequest")?;
        let receipt = crate::research_contracts::assure_adapter_context_compilation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::CONTEXT_COMPILATION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "required decision facts and derivation receipt are explicit",
                "missing facts remain unknown rather than certified",
                "prospective admission and protected-closure gates are deterministic"
            ],
            "limitations": [
                "the harness verifies a caller-supplied context and does not infer missing facts",
                "a passed receipt is not a clinical or treatment decision"
            ]
        }))
    }

    pub(super) fn adapter_resource_workbench(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::discover_adapter_resources_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::RESOURCE_WORKBENCH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "resource qualification is ranked deterministically",
                "stale, protected, non-local, and out-of-scope resources remain omissions",
                "raw resource bytes never enter the workbench receipt"
            ],
            "limitations": [
                "the workbench qualifies metadata and does not fetch or execute a resource",
                "unknown and partial results require downstream policy admission"
            ]
        }))
    }

    pub(super) fn adapter_ingestion_gateway(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an IngestionGatewayRequest")?;
        let receipt = crate::research_contracts::run_ingestion_gateway_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::INGESTION_GATEWAY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "raw modality payloads remain institution-local",
                "prospective admission requires policy allow and an independent authorization reference",
                "comparability omissions and semantic loss remain explicit",
                "every admitted bundle has a deterministic effect receipt"
            ],
            "limitations": [
                "the gateway accepts metadata descriptors and does not fetch or execute instruments",
                "partial and blocked admissions require downstream workflow policy"
            ]
        }))
    }

    pub(super) fn adapter_quality_envelope(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a QualityEnvelopeRequest")?;
        let receipt = crate::research_contracts::evaluate_quality_envelope_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::QUALITY_ENVELOPE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "multi-study modality coverage is counted deterministically",
                "protocol and instrument comparability conflicts are explicit blockers",
                "protected quality closure and raw-data locality are fail-closed",
                "study-level QC verdicts remain linked to their source receipts"
            ],
            "limitations": [
                "the envelope compares typed QC receipts and does not inspect raw experimental bytes",
                "partial, unknown, and blocked verdicts require downstream workflow policy"
            ]
        }))
    }

    pub(super) fn adapter_experiment_design_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedExperimentDesignRequest")?;
        let receipt = crate::research_contracts::compile_experiment_design_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::EXPERIMENT_DESIGN_CONTROL_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "site capability and modality replication assignments are deterministic",
                "authorization, protected closure, locality, and instrument comparability are fail-closed",
                "missing site capability remains an explicit partial design",
                "the control plane emits design metadata and never executes instruments"
            ],
            "limitations": [
                "the receipt is an executable-design proposal, not physical execution approval",
                "protocol simulation and signed instrument preflight remain downstream gates"
            ]
        }))
    }

    pub(super) fn adapter_protocol_simulation(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ProtocolDraft")?;
        let receipt = crate::research_contracts::simulate_protocol_draft_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::PROTOCOL_SIMULATION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "scenario ordering and protocol state transitions are deterministic",
                "budget exhaustion and injected failures fail closed",
                "instrument execution requires an earlier preflight",
                "partitions and external effects remain approval-required"
            ],
            "limitations": [
                "the simulator does not contact instruments or remote institutions",
                "simulation success is not a physical execution or scientific result"
            ]
        }))
    }

    pub(super) fn adapter_instrument_mesh(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::integrate_instrument_mesh_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::INSTRUMENT_MESH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "instrument capability selection is deterministic across candidate ordering",
                "policy, protected closure, authorization, interlocks, locality, and partition gates are explicit",
                "missing capability remains unknown rather than a positive scientific conclusion",
                "the receipt authorizes no physical execution and keeps raw data local"
            ],
            "limitations": [
                "the mesh consumes capability metadata and never contacts hardware or remote institutions",
                "admitted selection is only a downstream preflight input; signed operator authorization remains required"
            ]
        }))
    }

    pub(super) fn adapter_execution_control(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ComputationalExecutionRequest")?;
        let receipt = crate::research_contracts::admit_computational_execution_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::EXECUTION_CONTROL_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "workflow validation and topological ordering are deterministic",
                "policy, autonomy authority, locality, checkpoints, and replay identity remain explicit",
                "authorized effects are planned local-computation receipts and are never reported as executed",
                "cycles, denied policy, missing approval, and malformed boundaries fail closed"
            ],
            "limitations": [
                "the control plane admits a plan but does not execute code, instruments, or federation exports",
                "an institution-local executor must independently enforce the retained run and effect receipts"
            ]
        }))
    }

    pub(super) fn adapter_analysis_portfolio(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AnalysisPortfolioRequest")?;
        let receipt = crate::research_contracts::qualify_analysis_portfolio_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ANALYSIS_PORTFOLIO_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "candidate ordering is deterministic by declared score and candidate id",
                "estimand, method, input-artifact, identification, uncertainty, and locality gates are explicit",
                "protected omissions and negative evidence remain first-class and can only lower the verdict",
                "the portfolio qualifies declared candidates and never fits a model or asserts biology"
            ],
            "limitations": [
                "the adapter does not execute statistical code or estimate parameters",
                "a qualified candidate still requires a separately governed analysis runner and independent evaluation"
            ]
        }))
    }

    pub(super) fn adapter_interpretation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceBackedResult")?;
        let receipt = crate::research_contracts::assure_interpretation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::INTERPRETATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "interpretation claims are ordered and checked against declared local evidence",
                "required modality omissions, uncertainty, and negative evidence remain explicit",
                "unsupported interpretation claims are blocked rather than visualized as fact",
                "the assurance surface creates no plot, model, or external effect"
            ],
            "limitations": [
                "the adapter verifies metadata and does not inspect raw imaging or omics bytes",
                "a qualified receipt is not a biological conclusion or publication approval"
            ]
        }))
    }

    pub(super) fn adapter_replication_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ReplicationAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_replication_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::REPLICATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "replication observations are canonically ordered and counted by independent site",
                "null, negative, contradictory, inconclusive, protected, and partitioned evidence remain explicit",
                "raw experimental data remain institution-local and only typed receipts are returned",
                "an unmet independent-site floor cannot be promoted to replicated"
            ],
            "limitations": [
                "the assurance harness verifies caller-supplied observation receipts and does not inspect raw data",
                "a replication verdict is preclinical research evidence, not a clinical or treatment decision"
            ]
        }))
    }

    pub(super) fn adapter_release_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ValidatedResearchRun")?;
        let receipt = crate::research_contracts::assure_release_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::RELEASE_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "multimodal study manifests, artifact identifiers, and evidence receipts are canonically ordered",
                "policy, provenance, signer, comparability, omission, negative-evidence, and modality gates are explicit",
                "raw imaging and omics bytes remain institution-local and only signed metadata receipts are returned",
                "unsafe or incomplete releases are blocked or conditional rather than promoted"
            ],
            "limitations": [
                "the harness verifies caller-supplied release metadata and does not sign or publish raw data",
                "a released receipt is preclinical research-object metadata, not a clinical or treatment decision"
            ]
        }))
    }

    pub(super) fn adapter_determinism_gateway(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a TypedCapabilityInput")?;
        let receipt = crate::research_contracts::negotiate_determinism_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::DETERMINISM_GATEWAY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "capability fields are canonically ordered through a version-pinned contract",
                "legacy migration and unsupported versions remain explicit rather than silently coerced",
                "only content hashes and typed metadata cross the gateway; raw data remains local",
                "permission denial and incompatible negotiation produce blocked effects"
            ],
            "limitations": [
                "the gateway validates metadata and canonicalization but does not execute the remote capability",
                "canonical parity is not scientific validity or a clinical decision"
            ]
        }))
    }

    pub(super) fn adapter_provenance_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an ArtifactAndDerivation")?;
        let receipt = crate::research_contracts::assure_provenance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::PROVENANCE_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "derivation references are checked for duplicates, missing inputs, and cycles",
                "tool determinism, study/modality coverage, signer evidence, omissions, and negative evidence remain explicit",
                "only content-addressed lineage metadata crosses the boundary; raw artifacts remain local",
                "unsafe or incomplete signing is blocked or unresolved rather than promoted"
            ],
            "limitations": [
                "the verifier checks supplied metadata and does not inspect raw payload bytes or create a cryptographic signature",
                "a signed lineage envelope is not a scientific conclusion or clinical decision"
            ]
        }))
    }

    pub(super) fn adapter_policy_gateway(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an ActionAndAuthority")?;
        let receipt = crate::research_contracts::admit_policy_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::POLICY_GATEWAY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "actor, scope, permitted action, policy decision, autonomy tier, and resource budget are checked together",
                "A2-A4 approval, signed-preflight, independent-gate, locality, unresolved-policy, and revocation states remain explicit",
                "the gateway admits or blocks a policy receipt and never executes an external effect",
                "raw research data remains local and the boundary excludes clinical decisions"
            ],
            "limitations": [
                "the gateway evaluates authority metadata but does not perform the admitted action",
                "approval and institutional governance remain required for physical or federated effects"
            ]
        }))
    }

    pub(super) fn adapter_federation_workflow(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederationRequest")?;
        let receipt = crate::research_contracts::schedule_federation_workflow_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::FEDERATION_WORKFLOW_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "high-throughput tasks are canonically ordered with checkpoints, budgets, and compensation actions",
                "foundation FederationEnvelope policy constraints and integrity evidence are retained",
                "partitions, missing authority, missing signatures, and denied policy remain partial or blocked",
                "the route schedules metadata only and never executes remote work or moves raw data"
            ],
            "limitations": [
                "destination admission and task execution remain institution-local downstream responsibilities",
                "a scheduled workflow is not a scientific result or clinical decision"
            ]
        }))
    }

    pub(super) fn adapter_reliability_copilot(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a CapabilityWorkload")?;
        let receipt = crate::research_contracts::plan_reliable_capability_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::RELIABILITY_COPILOT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "only declared, approved, non-revoked tools enter the deterministic invocation plan",
                "dry-run, retries, timeouts, budget, simulated failures, and degraded nondeterminism remain explicit",
                "every planned effect is a non-executed bounded-tool receipt and raw data remains local",
                "partial or blocked work is retained rather than silently retried or reported complete"
            ],
            "limitations": [
                "the copilot plans tool execution and does not invoke connectors or remote code",
                "dependable operation must be established by downstream replay and independent evaluation"
            ]
        }))
    }

    pub(super) fn adapter_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an InteroperabilityRequest")?;
        let receipt = crate::research_contracts::negotiate_interoperability_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::INTEROPERABILITY_GATEWAY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "source and target capability sets are canonicalized before version negotiation",
                "only approved artifact digests and replay metadata are eligible for federation",
                "migration loss, missing capabilities, denied policy, and incomplete protected closure remain explicit",
                "unsupported or ambiguous integrations fail closed without executing remote code or moving raw data"
            ],
            "limitations": [
                "the gateway negotiates protocol metadata and does not fetch or execute a remote capability",
                "institution-local authorization, independent conformance, and scientific validation remain downstream gates"
            ]
        }))
    }

    pub(super) fn adapter_evaluation_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a CapabilityRun")?;
        let receipt = crate::research_contracts::assure_evaluation_run_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::EVALUATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "baseline deltas, metric measurements, protected closure, provenance, replay identity, and witness coverage are independent gates",
                "counterexamples, negative evidence, omissions, and uncertainty remain ordered and inspectable",
                "only a complete witness-bearing run can become passed; conditional, unknown, and blocked states retain their reasons",
                "the harness produces release evidence without executing benchmarks, moving raw data, or making biological or clinical decisions"
            ],
            "limitations": [
                "the harness evaluates caller-supplied aggregate observations and does not run the underlying capability",
                "independent replication, domain review, and consortium release authority remain required"
            ]
        }))
    }

    pub(super) fn adapter_research_workbench(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResearchWorkspaceState")?;
        let receipt = crate::research_contracts::compile_research_workbench_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::RESEARCH_WORKBENCH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "authorized studies, modalities, artifact digests, and views are canonically ordered",
                "cross-study comparability, missing modalities, provenance gaps, negative results, and policy denials remain visible",
                "the workbench compiles a local interaction projection without moving raw data or executing an external effect",
                "partial, blocked, and local-only workspaces retain action receipts rather than appearing ready"
            ],
            "limitations": [
                "the route compiles workbench state and does not render a browser UI or infer scientific conclusions",
                "study authorization, comparability certification, and institution-local storage remain external controls"
            ]
        }))
    }

    pub(super) fn adapter_contract_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AdapterContractInput")?;
        let receipt = crate::research_contracts::compile_adapter_capability_manifest_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::CONTRACT_FRONTIER_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "adapter identity, input/output schemas, modality order, effects, permissions, and artifact digests form a canonical capability manifest",
                "version migration loss and unsupported contract states remain explicit",
                "policy, protected-closure, comparability, and locality gates fail closed before artifact exchange",
                "the gateway exchanges manifests and digests only and never executes an extension or moves raw data"
            ],
            "limitations": [
                "the route compiles and validates a capability manifest but does not execute adapter code or certify biological validity",
                "endpoint conformance, independent replication, and institutional authorization remain required"
            ]
        }))
    }

    pub(super) fn adapter_limitation_closure(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a LimitationClosureRequest")?;
        let receipt = crate::research_contracts::close_adapter_limitations_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::LIMITATION_CLOSURE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "resolved, open, measured, blocked, and unknown limitations remain explicitly classified",
                "closure criteria, evidence digests, omissions, uncertainty, and negative evidence are retained",
                "only permitted limitation digests may be exchanged and raw experimental data remains local",
                "federation denial, incomplete protected closure, and policy denial fail closed"
            ],
            "limitations": [
                "the gateway compiles limitation metadata and does not execute remote code or infer biological or clinical conclusions",
                "institutional evidence review, endpoint authorization, and independent replication remain external requirements"
            ]
        }))
    }

    pub(super) fn adapter_dependency_composition(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AdapterCompositionRequest")?;
        let receipt =
            crate::research_contracts::infer_adapter_dependency_composition_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::DEPENDENCY_COMPOSITION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "typed adapter dependencies are resolved in canonical component order with deterministic provider ranking",
                "missing capabilities, ambiguous providers, protected-closure gaps, negative evidence, and policy denials remain explicit",
                "the route emits a local-only composition manifest and digest-only exchange receipt without invoking adapter code",
                "partial and unknown compositions cannot be promoted to executable research effects"
            ],
            "limitations": [
                "the inference engine plans declared contracts and does not execute adapters or certify biological validity",
                "runtime scheduling, endpoint authorization, and independent conformance remain required before execution"
            ]
        }))
    }

    pub(super) fn adapter_semantic_parity(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AdapterSemanticParityRequest")?;
        let receipt = crate::research_contracts::evaluate_adapter_semantic_parity_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_SEMANTIC_PARITY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "independent adapter schema, semantic, modality, study, and artifact identities are compared without moving raw outputs",
                "missing modalities, semantic disagreement, protected closure gaps, policy denial, and negative evidence remain explicit",
                "only parity digests and typed receipts may cross the local boundary",
                "unknown parity cannot be promoted to a scientific conclusion or executable effect"
            ],
            "limitations": [
                "the route compares declared summaries and does not inspect raw data or certify biological equivalence",
                "independent adapter conformance, replication, and institutional authorization remain required"
            ]
        }))
    }

    pub(super) fn adapter_scale_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ScaleFrontierRequest")?;
        let receipt = crate::research_contracts::plan_adapter_scale_frontier_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADAPTER_SCALE_FRONTIER_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "prospective workload, concurrency, budget, failure, policy, and protected-closure cells are evaluated deterministically",
                "admissible and blocked frontier cells remain separately inspectable with omissions and negative evidence",
                "the route emits a planning manifest and never schedules work or authorizes an external effect",
                "resource exhaustion, policy denial, and incomplete closure cannot become a ready scale plan"
            ],
            "limitations": [
                "the frontier is a bounded scenario projection and does not execute adapters or establish an operational SLO",
                "institution-local scheduler admission, instrument capacity, and independent benchmark evidence remain required"
            ]
        }))
    }

    pub(super) fn adapter_adversarial_recovery(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AdversarialRecoveryRequest")?;
        let receipt = crate::research_contracts::recover_adversarial_events_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::ADVERSARIAL_RECOVERY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "crash, partition, duplicate, revoked-key, poisoned-artifact, prompt-injection, and resource-exhaustion events remain typed and replay-visible",
                "checkpoint, recovery, blocked, omission, uncertainty, and negative-evidence states are retained across federation boundaries",
                "only permitted checkpoint and digest metadata may be exchanged; raw data and hostile payloads remain local",
                "partial, unknown, and blocked recovery cannot be promoted to successful execution"
            ],
            "limitations": [
                "the gateway compiles recovery metadata and does not execute compensation or remote code",
                "institution-local key revocation, scheduler recovery, and independent adversarial testing remain required"
            ]
        }))
    }

    pub(super) fn adapter_federated_commons(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedCommonsRequest")?;
        let receipt = crate::research_contracts::admit_federated_commons_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::FEDERATED_COMMONS_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "institution contributions are purpose-bound, aggregate-only, locality-preserving, and semantically profiled",
                "admitted and denied institutions, artifact digests, omissions, uncertainty, and negative evidence remain separately inspectable",
                "raw experimental bytes never cross the gateway and policy/protected-closure gates fail closed",
                "partial and unknown commons states cannot be represented as complete federation consent"
            ],
            "limitations": [
                "the gateway admits metadata and aggregate contribution rights but does not move raw data or execute federation jobs",
                "institutional legal agreements, key management, semantic validation, and independent governance remain required"
            ]
        }))
    }

    pub(super) fn adapter_bounded_evolution(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a BoundedEvolutionRequest")?;
        let receipt = crate::research_contracts::admit_bounded_evolution_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::BOUNDED_EVOLUTION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "high-throughput candidate admission is bounded by replay, determinism, evidence, safety, policy, budget, and preclinical-boundary gates",
                "candidate ordering, baseline/artifact replay digests, blocked reasons, omissions, uncertainty, and negative evidence are deterministic",
                "the gateway emits an admission receipt only and never mutates source code or deploys a candidate",
                "unknown, partial, blocked, clinical-surface, and incomplete-closure states cannot become a production evolution approval"
            ],
            "limitations": [
                "the gateway does not execute, benchmark, sign, or deploy candidate artifacts",
                "independent review, sandbox replay, institutional authorization, and release governance remain required"
            ]
        }))
    }
}
