//! Read-only MCP adapters for the typed Glioma federation contracts.

use super::*;
use bioprism_research::{
    analyze_federated_aggregate_anomalies, assess_glioma_cross_site_protocol_conformance,
    build_glioma_federation_operations_snapshot, compile_glioma_federated_release,
    compile_glioma_site_capability_envelope, evaluate_glioma_federated_release_sharing,
    exchange_glioma_compute_capacity, exchange_glioma_federated_instrument_operations,
    exchange_glioma_federated_workflow_template, execute_federated_benchmark_dry_run,
    execute_glioma_federated_benchmark_record, explore_glioma_cross_site_evidence,
    package_glioma_federated_decision_capsule, plan_federated_glioma_sites,
    plan_federation_capacity, publish_glioma_federated_device_capability_manifest,
    review_glioma_site_participation, scan_glioma_federated_replay_discrepancy,
    steward_glioma_consortium_publication, verify_glioma_federated_replay_conformance,
    ConsortiumPublicationRequest, CrossSiteEvidenceExplorerRequest,
    FederatedAggregateAnomalyRequest, FederatedBenchmarkDryRunRequest,
    FederatedBenchmarkExecutionRequest, FederatedComputeCostExchangeRequest,
    FederatedDecisionCapsuleRequest, FederatedDeviceCapabilityRequest,
    FederatedInstrumentOperationsRequest, FederatedReleaseRequest, FederatedReleaseSharingRequest,
    FederatedReplayConformanceRequest, FederatedReplayDiscrepancyScanRequest,
    FederatedSiteSelectionRequest, FederatedWorkflowTemplateExchangeRequest,
    FederationCapacityRequest, FederationOperationsRequest, ProtocolConformanceRequest,
    SiteCapabilityRequest, SiteParticipationRequest,
};
use serde::de::DeserializeOwned;

fn decode_request<T: DeserializeOwned>(arguments: &Value, tool: &str) -> Result<T, String> {
    let request = arguments
        .get("request")
        .cloned()
        .ok_or_else(|| format!("{tool} requires request"))?;
    serde_json::from_value(request).map_err(|error| format!("invalid {tool} request: {error}"))
}

fn refusal(tool: &str, error: impl std::fmt::Display) -> String {
    format!("{tool} refused: {error}")
}

impl Server {
    pub(super) fn glioma_federated_aggregate_anomaly_detect(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_aggregate_anomaly_detect";
        let request: FederatedAggregateAnomalyRequest = decode_request(arguments, tool)?;
        let assessment = analyze_federated_aggregate_anomalies(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "assessment": assessment,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["evaluates caller-supplied aggregate observations; contacts no federation site"]
        }))
    }

    pub(super) fn glioma_federated_site_selection_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_site_selection_plan";
        let request: FederatedSiteSelectionRequest = decode_request(arguments, tool)?;
        let plan = plan_federated_glioma_sites(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["ranks supplied site capability envelopes and does not reserve site capacity"]
        }))
    }

    pub(super) fn glioma_federation_capacity_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federation_capacity_plan";
        let request: FederationCapacityRequest = decode_request(arguments, tool)?;
        let plan = plan_federation_capacity(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["forecasts capacity from supplied site windows and does not schedule work"]
        }))
    }

    pub(super) fn glioma_federated_benchmark_dry_run(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_benchmark_dry_run";
        let request: FederatedBenchmarkDryRunRequest = decode_request(arguments, tool)?;
        let report =
            execute_federated_benchmark_dry_run(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["evaluates supplied dry-run fixtures and dispatches no benchmark work"]
        }))
    }

    pub(super) fn glioma_federated_replay_discrepancy_scan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_replay_discrepancy_scan";
        let request: FederatedReplayDiscrepancyScanRequest = decode_request(arguments, tool)?;
        let report = scan_glioma_federated_replay_discrepancy(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["analyzes supplied replay attestations and does not rerun remote computations"]
        }))
    }

    pub(super) fn glioma_federated_workflow_template_exchange(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_workflow_template_exchange";
        let request: FederatedWorkflowTemplateExchangeRequest = decode_request(arguments, tool)?;
        let package = exchange_glioma_federated_workflow_template(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "package": package,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["validates and packages caller-supplied metadata without transmitting workflow payloads"]
        }))
    }

    pub(super) fn glioma_federated_replay_conformance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_replay_conformance";
        let request: FederatedReplayConformanceRequest = decode_request(arguments, tool)?;
        let report = verify_glioma_federated_replay_conformance(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["verifies supplied replay attestations and does not execute or fetch site workloads"]
        }))
    }

    pub(super) fn glioma_federated_compute_capacity_exchange(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_compute_capacity_exchange";
        let request: FederatedComputeCostExchangeRequest = decode_request(arguments, tool)?;
        let envelope =
            exchange_glioma_compute_capacity(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "envelope": envelope,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["compares supplied aggregate capacity and cost summaries; reserves no compute resources"]
        }))
    }

    pub(super) fn glioma_federated_decision_capsule(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_decision_capsule";
        let request: FederatedDecisionCapsuleRequest = decode_request(arguments, tool)?;
        let capsule = package_glioma_federated_decision_capsule(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "capsule": capsule,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["packages and checks a caller-supplied decision context; it does not authorize downstream actions"]
        }))
    }

    pub(super) fn glioma_cross_site_protocol_conformance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_cross_site_protocol_conformance";
        let request: ProtocolConformanceRequest = decode_request(arguments, tool)?;
        let matrix = assess_glioma_cross_site_protocol_conformance(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "matrix": matrix,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["compares supplied protocol descriptors and does not execute protocols at participating sites"]
        }))
    }

    pub(super) fn glioma_federated_device_capability_manifest(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_device_capability_manifest";
        let request: FederatedDeviceCapabilityRequest = decode_request(arguments, tool)?;
        let manifest = publish_glioma_federated_device_capability_manifest(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "manifest": manifest,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["validates the supplied signed capability description and does not probe or reserve hardware"]
        }))
    }

    pub(super) fn glioma_federated_instrument_operations(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_instrument_operations";
        let request: FederatedInstrumentOperationsRequest = decode_request(arguments, tool)?;
        let snapshot = exchange_glioma_federated_instrument_operations(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "snapshot": snapshot,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["aggregates caller-supplied operational summaries and performs no hardware or network actions"]
        }))
    }

    pub(super) fn glioma_federated_release_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_release_compile";
        let request: FederatedReleaseRequest = decode_request(arguments, tool)?;
        let object =
            compile_glioma_federated_release(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "object": object,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["compiles supplied aggregate release metadata and does not publish or transmit a release"]
        }))
    }

    pub(super) fn glioma_federated_release_sharing_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_release_sharing_check";
        let request: FederatedReleaseSharingRequest = decode_request(arguments, tool)?;
        let decision = evaluate_glioma_federated_release_sharing(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "decision": decision,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["evaluates declared recipients and fields but does not share or redact release artifacts"]
        }))
    }

    pub(super) fn glioma_consortium_publication_steward(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_consortium_publication_steward";
        let request: ConsortiumPublicationRequest = decode_request(arguments, tool)?;
        let state = steward_glioma_consortium_publication(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "state": state,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["evaluates supplied site decisions and does not publish the research object"]
        }))
    }

    pub(super) fn glioma_site_capability_envelope_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_site_capability_envelope_compile";
        let request: SiteCapabilityRequest = decode_request(arguments, tool)?;
        let envelope = compile_glioma_site_capability_envelope(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "envelope": envelope,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["compiles caller-declared capability and attestation metadata; it does not inspect site systems"]
        }))
    }

    pub(super) fn glioma_cross_site_evidence_explore(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_cross_site_evidence_explore";
        let request: CrossSiteEvidenceExplorerRequest = decode_request(arguments, tool)?;
        let view =
            explore_glioma_cross_site_evidence(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "view": view,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["explores supplied aggregate observations and does not query remote sites or raw records"]
        }))
    }

    pub(super) fn glioma_site_participation_review(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_site_participation_review";
        let request: SiteParticipationRequest = decode_request(arguments, tool)?;
        let review =
            review_glioma_site_participation(&request).map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "review": review,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["reviews the supplied participation request and does not dispatch its proposed query"]
        }))
    }

    pub(super) fn glioma_federation_operations_snapshot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federation_operations_snapshot";
        let request: FederationOperationsRequest = decode_request(arguments, tool)?;
        let snapshot = build_glioma_federation_operations_snapshot(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "snapshot": snapshot,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["summarizes supplied site heartbeats and queue state without contacting those sites"]
        }))
    }

    pub(super) fn glioma_federated_benchmark_record_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let tool = "glioma_federated_benchmark_record_execute";
        let request: FederatedBenchmarkExecutionRequest = decode_request(arguments, tool)?;
        let record = execute_glioma_federated_benchmark_record(&request)
            .map_err(|error| refusal(tool, error))?;
        Ok(json!({
            "record": record,
            "dispatch": "not_started",
            "simulation_only": true,
            "limitations": ["folds supplied contribution attestations into a record; it does not run a benchmark"]
        }))
    }
}
