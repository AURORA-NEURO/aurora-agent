//! Mission evaluator discovery and retained evidence-bundle handlers.

use super::*;

impl Server {
    /// Search the explicit evaluator candidate catalogue across every workspace domain.
    ///
    /// The returned rows are discovery evidence only. They help a caller choose an
    /// `adapter_id` for a mission claim, but they do not execute an evaluator, inspect a step
    /// result, grant permission, or establish the truth of the claim.
    pub(super) fn mission_evaluator_discover(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode mission evaluator discovery input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "mission evaluator discovery input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let query: MissionEvaluatorQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid mission evaluator discovery input: {error}"))?;
        let catalogue = MissionEvaluatorCatalogue::standard();
        let search = catalogue
            .search(&query)
            .map_err(|error| format!("mission evaluator discovery refused: {error}"))?;
        let capability_catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("capability catalogue refused: {error}"))?;
        let evaluator_groups = catalogue
            .adapters()
            .iter()
            .map(|adapter| adapter.group_id.clone())
            .collect::<BTreeSet<_>>();
        let capability_groups = capability_catalogue
            .groups()
            .iter()
            .map(|group| group.id.clone())
            .collect::<BTreeSet<_>>();
        let uncovered_groups = capability_groups
            .difference(&evaluator_groups)
            .cloned()
            .collect::<Vec<_>>();
        let unbound_groups = evaluator_groups
            .difference(&capability_groups)
            .cloned()
            .collect::<Vec<_>>();
        let coverage_complete = uncovered_groups.is_empty() && unbound_groups.is_empty();
        let mut output = serde_json::to_value(search).map_err(|error| {
            format!("cannot encode mission evaluator discovery result: {error}")
        })?;
        output["ok"] = json!(true);
        output["workflow"] = json!("mission_evaluator_discover");
        output["mission_evaluator_schema_version"] = json!(MISSION_EVALUATOR_SCHEMA_VERSION);
        output["selection_posture"] = json!("candidate_only");
        output["coverage"] = json!({
            "capability_group_count": capability_groups.len(),
            "evaluator_group_count": evaluator_groups.len(),
            "uncovered_groups": uncovered_groups,
            "unbound_groups": unbound_groups,
            "complete": coverage_complete,
            "posture": "catalogue coverage evidence only; no evaluator was executed",
        });
        output["guarantees"] = json!([
            "every returned adapter is an explicit catalogue row with a stable content digest",
            "ranking is deterministic and uses labels only",
            "candidate tools are suggestions and were not executed",
            "a caller must bind an adapter explicitly to a mission claim and step output pointer",
        ]);
        output["limitations"] = json!([
            "the catalogue does not execute adapters or validate domain semantics",
            "candidate_tools do not prove that a concrete result contains the suggested fields",
            "adapter discovery does not make a claim reviewed, calibrated, causal, clinical, or release-ready",
        ]);
        let output_bytes = serde_json::to_vec(&output).map_err(|error| {
            format!("cannot measure mission evaluator discovery result: {error}")
        })?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "mission evaluator discovery result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }

    /// Review caller-selected evaluator candidates and produce a non-executing claim-binding
    /// scaffold. The normal agent_mission validator remains the authority for final step and
    /// claim validation.
    pub(super) fn mission_evaluator_review(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode mission evaluator review input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "mission evaluator review input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let request: MissionEvaluatorReviewRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid mission evaluator review input: {error}"))?;
        let catalogue = MissionEvaluatorCatalogue::standard();
        let output = catalogue
            .review(&request)
            .map_err(|error| format!("mission evaluator review refused: {error}"))?;
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure mission evaluator review result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "mission evaluator review result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }

    /// Replay retained mission evaluator lineage against the current catalogue without dispatch.
    pub(super) fn mission_evaluator_replay(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode mission evaluator replay input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "mission evaluator replay input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let request: MissionEvaluatorReplayRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid mission evaluator replay input: {error}"))?;
        let catalogue = MissionEvaluatorCatalogue::standard();
        let output = catalogue
            .replay(&request)
            .map_err(|error| format!("mission evaluator replay refused: {error}"))?;
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure mission evaluator replay result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "mission evaluator replay result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }

    /// Compare retained evaluator evidence with the current catalogue without dispatch.
    pub(super) fn mission_evaluator_replay_compare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode mission evaluator replay comparison input: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err(
                "mission evaluator replay comparison input exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        let request: MissionEvaluatorReplayCompareRequest =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                format!("invalid mission evaluator replay comparison input: {error}")
            })?;
        let output = MissionEvaluatorCatalogue::standard()
            .compare(&request)
            .map_err(|error| format!("mission evaluator replay comparison refused: {error}"))?;
        let output_bytes = serde_json::to_vec(&output).map_err(|error| {
            format!("cannot measure mission evaluator replay comparison result: {error}")
        })?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "mission evaluator replay comparison result exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        Ok(output)
    }

    /// Verify a portable mission evidence bundle without executing any contained workflow.
    pub(super) fn mission_evidence_bundle_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode mission evidence bundle verification input: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err(
                "mission evidence bundle verification input exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        let bundle = arguments.get("bundle").ok_or("bundle is required")?;
        let output = verify_mission_evidence_bundle(bundle)
            .map_err(|error| format!("mission evidence bundle verification refused: {error}"))?;
        let output_bytes = serde_json::to_vec(&output).map_err(|error| {
            format!("cannot measure mission evidence bundle verification result: {error}")
        })?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "mission evidence bundle verification result exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        Ok(output)
    }

    /// Import a verified evidence bundle into the bounded in-process registry.
    ///
    /// The MCP surface intentionally shares the same verifier and registry kernel as REST. It
    /// is a process-local index (REST can add restart persistence); importing never turns an
    /// evidence artifact into execution state or a scientific/release claim.
    pub(super) fn mission_evidence_bundle_import(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode mission evidence bundle import input: {error}")
        })?;
        if encoded.len() > bioprism_devplat::MAX_EVIDENCE_REGISTRY_BYTES {
            return Err(format!(
                "mission evidence bundle import input exceeds the {}-byte safety bound",
                bioprism_devplat::MAX_EVIDENCE_REGISTRY_BYTES
            ));
        }
        let bundle = arguments.get("bundle").ok_or("bundle is required")?;
        self.evidence_registry
            .lock()
            .map_err(|_| "mission evidence registry lock is poisoned".to_string())?
            .import(bundle)
            .map_err(|error| format!("mission evidence bundle import refused: {error}"))
    }

    /// Query deterministic, digest-ordered registry rows without executing anything.
    pub(super) fn mission_evidence_bundle_query(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode mission evidence bundle query input: {error}")
        })?;
        if encoded.len() > 1_000_000 {
            return Err(
                "mission evidence bundle query input exceeds the 1000000-byte safety bound".into(),
            );
        }
        let optional_string = |name: &str| -> Result<Option<&str>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| format!("{name} must be a string"))
                })
                .transpose()
        };
        let mission_id = optional_string("mission_id")?;
        let domain = optional_string("domain")?;
        let after = optional_string("after")?;
        let max_items = arguments
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_items must be an integer".to_string())
                    .and_then(|number| {
                        usize::try_from(number).map_err(|_| "max_items is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(100);
        if !(1..=MAX_EVIDENCE_REGISTRY_QUERY_ITEMS).contains(&max_items) {
            return Err(format!(
                "max_items must be between 1 and {MAX_EVIDENCE_REGISTRY_QUERY_ITEMS}"
            ));
        }
        let include_bundles = arguments
            .get("include_bundles")
            .map(|value| value.as_bool().ok_or("include_bundles must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        self.evidence_registry
            .lock()
            .map_err(|_| "mission evidence registry lock is poisoned".to_string())?
            .query(mission_id, domain, after, max_items, include_bundles)
            .map_err(|error| format!("mission evidence bundle query refused: {error}"))
    }

    /// Fetch one imported bundle by its content hash.
    pub(super) fn mission_evidence_bundle_get(&self, arguments: &Value) -> Result<Value, String> {
        let digest = arguments
            .get("bundle_digest")
            .and_then(Value::as_str)
            .ok_or("bundle_digest is required and must be a content hash")?;
        bioprism_ids::ContentHash::parse(digest.to_string())
            .map_err(|error| format!("bundle_digest is invalid: {error}"))?;
        let bundle = self
            .evidence_registry
            .lock()
            .map_err(|_| "mission evidence registry lock is poisoned".to_string())?
            .get(digest)
            .ok_or_else(|| format!("bundle {digest:?} is not present in the registry"))?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/mission-evidence-bundle-record/0.1",
            "workflow": "mission_evidence_bundle_get",
            "bundle_digest": digest,
            "bundle": bundle,
            "execution": "not_started",
            "guarantees": [
                "the returned bundle was accepted by the shared independent verifier before import",
                "lookup does not execute a mission, evaluator, domain tool, or external effect"
            ],
            "limitations": [
                "the process-local registry is bounded and is not a distributed object store",
                "bundle presence does not establish scientific validity or release approval"
            ]
        }))
    }

    /// Reconcile a returned mission report with delegated check evidence without executing it.
    pub(super) fn execution_provenance_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode execution provenance input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("execution provenance input exceeds the 20000000-byte safety bound".into());
        }
        let request: ExecutionProvenanceRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid execution provenance input: {error}"))?;
        let audit = audit_execution_provenance(&request)?;
        let mut output = serde_json::to_value(&audit)
            .map_err(|error| format!("cannot encode execution provenance audit: {error}"))?;
        output["ok"] = json!(true);
        output["workflow"] = json!("execution_provenance_audit");
        output["valid"] = json!(audit.structurally_valid);
        output["provenance_ready"] = json!(audit.release_candidate);
        Ok(output)
    }

    /// Plan or execute an explicit, bounded DAG of existing domain tools.
    ///
    /// Planning is the default. Execution invokes the same internal tool dispatcher used by the
    /// MCP boundary, but only after the typed mission contract has required an allow-list and
    /// applied side-effect and output budgets. Nested results remain raw MCP envelopes so a
    /// refusal cannot be silently converted into a successful scientific conclusion.
    pub(super) fn attach_workflow_reconciliation(
        &self,
        arguments: &Value,
        result: Result<Value, String>,
    ) -> Result<Value, String> {
        let Ok(mut result) = result else {
            return result;
        };
        if arguments
            .pointer("/policy/execute")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Ok(result);
        }
        let Some(binding) = arguments.get("workflow_binding") else {
            return Ok(result);
        };
        let Some(binding_object) = binding.as_object() else {
            return Ok(result);
        };
        let instantiation = json!({
            "ok": true,
            "schema": bioprism_devplat::DOMAIN_WORKFLOW_INSTANTIATE_SCHEMA_VERSION,
            "workflow": "domain_workflow_instantiate",
            "workflow_id": binding_object.get("workflow_id").cloned().unwrap_or(Value::Null),
            "workflow_digest": binding_object.get("workflow_digest").cloned().unwrap_or(Value::Null),
            "catalog_digest": binding_object.get("catalog_digest").cloned().unwrap_or(Value::Null),
            "domain_contract_digest": binding_object.get("domain_contract_digest").cloned().unwrap_or(Value::Null),
            "domain_contract": binding_object.get("domain_contract").cloned().unwrap_or(Value::Null),
            "mission": arguments,
            "evidence_plan": binding_object.get("evidence_plan").cloned().unwrap_or(Value::Null),
        });
        let reconciliation_request = json!({
            "instantiation": instantiation,
            "mission_report": result,
        });
        let reconciliation = match reconcile_domain_workflow(&reconciliation_request) {
            Ok(record) => record,
            Err(error) => {
                result["workflow_reconciliation"] = json!({
                    "present": false,
                    "automatic": true,
                    "error": error.to_string(),
                    "fail_closed": true,
                    "readiness_claimed": false,
                });
                return Ok(result);
            }
        };
        let import_response = match self.workflow_reconciliation_registry.lock() {
            Ok(mut registry) => registry.import(&reconciliation),
            Err(_) => Err(
                bioprism_devplat::DomainWorkflowReconciliationRegistryError::InvalidRecord(
                    "workflow reconciliation registry lock is poisoned".into(),
                ),
            ),
        };
        match import_response {
            Ok(import_response) => {
                let digest = reconciliation
                    .get("reconciliation_digest")
                    .cloned()
                    .unwrap_or(Value::Null);
                let artifact_projection = self.index_artifact_projection(
                    "workflow_reconciliation",
                    reconciliation
                        .get("mission_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown-mission"),
                    // Reconciliation's canonical digest intentionally contains workflow and
                    // mission identities, not a second inferred domain taxonomy. Mission-report
                    // parents retain the concrete step domains separately.
                    Vec::new(),
                    Vec::new(),
                    reconciliation.clone(),
                );
                result["workflow_reconciliation"] = json!({
                    "present": true,
                    "automatic": true,
                    "reconciliation_digest": digest,
                    "canonical_record": reconciliation.clone(),
                    "workflow_id": reconciliation.get("workflow_id").cloned().unwrap_or(Value::Null),
                    "mission_id": reconciliation.get("mission_id").cloned().unwrap_or(Value::Null),
                    "completion": reconciliation.get("completion").cloned().unwrap_or(Value::Null),
                    "evidence": {
                        "evidence_valid": reconciliation.pointer("/evidence/evidence_valid").cloned().unwrap_or(Value::Null),
                        "required_success": reconciliation.pointer("/evidence/required_success").cloned().unwrap_or(Value::Null),
                        "required_outputs_retained": reconciliation.pointer("/evidence/required_outputs_retained").cloned().unwrap_or(Value::Null),
                    },
                    "integrity": reconciliation.get("integrity").cloned().unwrap_or(Value::Null),
                    "registry_import": import_response,
                    "artifact_registry": artifact_projection,
                    "lookup": format!(
                        "/v1/domain-workflows/reconciliations/{}",
                        reconciliation.get("reconciliation_digest").and_then(Value::as_str).unwrap_or("unknown")
                    ),
                    "readiness_claimed": false,
                });
            }
            Err(error) => {
                result["workflow_reconciliation"] = json!({
                    "present": false,
                    "automatic": true,
                    "error": error.to_string(),
                    "fail_closed": true,
                    "readiness_claimed": false,
                });
            }
        }
        Ok(result)
    }
}
