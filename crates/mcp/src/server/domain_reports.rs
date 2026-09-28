//! Domain report composition and validated evidence adapters.

use super::*;

impl Server {
    /// Compose one validated adapter-evidence observation into the canonical domain-report
    /// envelope. The adapter remains caller-owned: this operation indexes the evidence handoff
    /// and the report projection, but never imports, executes, or certifies the adapter.
    pub(super) fn project_adapter_execution_domain_report(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let evidence = arguments
            .get("evidence")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or("from_adapter_execution requires an evidence object")?;
        let evidence_result = self.adapter_execution_evidence(&evidence)?;
        let evidence_artifact = evidence_result
            .get("evidence")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or("adapter evidence response omitted evidence")?;
        let group_id = evidence_artifact
            .get("group_id")
            .and_then(Value::as_str)
            .ok_or("adapter evidence omitted group_id")?;
        let domains = evidence_artifact
            .get("domains")
            .filter(|value| value.is_array())
            .cloned()
            .ok_or("adapter evidence omitted domains")?;
        let subject_id = evidence_artifact
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("adapter evidence omitted subject_id")?;
        let execution_status = evidence_artifact
            .get("execution_status")
            .and_then(Value::as_str)
            .ok_or("adapter evidence omitted execution_status")?;
        let claim_status = match execution_status {
            "succeeded" | "partial" => "observed",
            "refused" | "failed" => "refused",
            _ => "review_required",
        };
        let mut parent_digests = evidence_artifact
            .get("parent_digests")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(content_digest) = evidence_result
            .get("artifact_registry")
            .and_then(|registry| registry.get("content_digest"))
            .and_then(Value::as_str)
        {
            if !parent_digests.iter().any(|parent| parent == content_digest) {
                parent_digests.push(content_digest.to_string());
            }
        }
        let mut report = json!({
            "kind": "adapter_execution",
            "evidence": evidence_artifact,
            "adapter": evidence_result.get("adapter"),
        });
        if let Some(conformance) = arguments.get("conformance") {
            if !conformance.is_object() {
                return Err("conformance must be an object when supplied".into());
            }
            report["conformance"] = conformance.clone();
        }
        let report_request = json!({
            "group_id": group_id,
            "domains": domains,
            "subject_id": subject_id,
            "source_tool": "adapter_execution_evidence",
            "report": report,
            "claim_posture": {
                "status": claim_status,
                "does_not_claim": [
                    "adapter correctness beyond the caller-supplied observation",
                    "scientific, clinical, causal, provenance, regulatory, or release validity",
                    "MCP-core adapter execution or readiness"
                ],
                "limitations": [
                    "adapter execution remains caller-owned",
                    "the evidence is structural and caller-attested"
                ]
            },
            "parent_digests": parent_digests
        });
        let domain_report = self.project_domain_report(&report_request)?;
        Ok(json!({
            "ok": true,
            "schema": ADAPTER_DOMAIN_REPORT_SCHEMA_VERSION,
            "workflow": ADAPTER_DOMAIN_REPORT_WORKFLOW,
            "evidence": evidence_result,
            "domain_report": domain_report,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "adapter evidence is validated and indexed before the canonical domain report is projected",
                "adapter identity remains inside the evidence payload while the declared MCP tool anchors catalogue scope",
                "the evidence artifact digest is retained as a parent of the domain report"
            ],
            "does_not_claim": [
                "the MCP core executed or imported the adapter",
                "caller-attested conformance proves scientific, clinical, provenance, regulatory, or release validity",
                "report indexing proves readiness or external effects"
            ]
        }))
    }

    fn provider_domain_report_parents(
        arguments: &Value,
        normalization: &Value,
        external: bool,
    ) -> Result<Vec<String>, String> {
        let mut parents = Vec::new();
        let mut append = |value: Option<&Value>, field: &str| -> Result<(), String> {
            let Some(value) = value else {
                return Ok(());
            };
            let values = value
                .as_array()
                .ok_or_else(|| format!("{field} must be an array"))?;
            for parent in values {
                let parent = parent
                    .as_str()
                    .ok_or_else(|| format!("{field} must contain strings"))?;
                if !parents.iter().any(|existing| existing == parent) {
                    parents.push(parent.to_string());
                }
            }
            Ok(())
        };
        append(arguments.get("parent_digests"), "parent_digests")?;
        append(
            normalization
                .get("intake")
                .and_then(|intake| intake.get("parent_digests")),
            "normalization.intake.parent_digests",
        )?;
        for digest in [normalization
            .get("artifact_registry")
            .and_then(|registry| registry.get("content_digest"))]
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        {
            if !parents.iter().any(|existing| existing == digest) {
                parents.push(digest.to_string());
            }
        }
        if external {
            if let Some(digest) = normalization
                .get("receipt_artifact_registry")
                .and_then(|registry| registry.get("content_digest"))
                .and_then(Value::as_str)
            {
                if !parents.iter().any(|existing| existing == digest) {
                    parents.push(digest.to_string());
                }
            }
        }
        Ok(parents)
    }

    pub(super) fn project_provider_normalization_domain_report(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let normalization_arguments = arguments
            .get("normalization")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or("from_provider_normalization requires a normalization object")?;
        let normalization = self.domain_evidence_provider_normalize(&normalization_arguments)?;
        self.compose_provider_domain_report(arguments, normalization, "inline")
    }

    pub(super) fn project_external_provider_normalization_domain_report(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let normalization_arguments = arguments
            .get("normalization")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or("from_external_provider_normalization requires a normalization object")?;
        let normalization =
            self.domain_evidence_provider_external_payload_normalize(&normalization_arguments)?;
        self.compose_provider_domain_report(arguments, normalization, "external_payload")
    }

    pub(super) fn compose_provider_domain_report(
        &self,
        arguments: &Value,
        normalization: Value,
        mode: &str,
    ) -> Result<Value, String> {
        let external = mode == "external_payload";
        let group_id = normalization
            .get("group_id")
            .and_then(Value::as_str)
            .ok_or("provider normalization omitted group_id")?;
        let domains = normalization
            .get("domains")
            .filter(|value| value.is_array())
            .cloned()
            .ok_or("provider normalization omitted domains")?;
        let subject_id = normalization
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("provider normalization omitted subject_id")?;
        let outcome = normalization
            .get("outcome")
            .and_then(Value::as_str)
            .ok_or("provider normalization omitted outcome")?;
        let claim_status = match outcome {
            "observed" | "partial" => "observed",
            "refused" | "error" => "refused",
            _ => "review_required",
        };
        let parent_digests =
            Self::provider_domain_report_parents(arguments, &normalization, external)?;
        let mut evidence = json!({
            "kind": "provider_normalization",
            "mode": mode,
            "schema": normalization.get("schema"),
            "workflow": normalization.get("workflow"),
            "group_id": group_id,
            "domains": domains,
            "subject_id": subject_id,
            "source_tool": normalization.get("source_tool"),
            "connector_kind": normalization.get("connector_kind"),
            "provider": normalization.get("provider"),
            "outcome": outcome,
            "payload_digest": normalization.get("payload_digest"),
            "request_digest": normalization.get("request_digest"),
            "shape_audit": normalization.get("shape_audit"),
            "record_index": normalization.get("record_index"),
            "intake_digest": normalization.get("intake").and_then(|intake| intake.get("intake_digest")),
            "artifact_content_digest": normalization.get("artifact_registry").and_then(|registry| registry.get("content_digest")),
        });
        if external {
            evidence["receipt"] = normalization.get("receipt").cloned().unwrap_or(Value::Null);
            evidence["materialization"] = normalization
                .get("materialization")
                .cloned()
                .unwrap_or(Value::Null);
            evidence["receipt_artifact_content_digest"] = normalization
                .get("receipt_artifact_registry")
                .and_then(|registry| registry.get("content_digest"))
                .cloned()
                .unwrap_or(Value::Null);
        }
        let report_request = json!({
            "group_id": group_id,
            "domains": domains,
            "subject_id": subject_id,
            "source_tool": if external {
                "domain_evidence_provider_external_payload_normalize"
            } else {
                "domain_evidence_provider_normalize"
            },
            "report": evidence,
            "claim_posture": {
                "status": claim_status,
                "does_not_claim": [
                    "provider authenticity, signature validity, or remote execution",
                    "scientific, clinical, causal, provenance, regulatory, or release validity",
                    "retrieval completeness, terminology resolution, or external-effect completion"
                ],
                "limitations": if external {
                    [
                        "external payload materialization remains caller-supplied and the locator remains unopened",
                        "receipt and normalization lineage do not establish payload or provider authenticity"
                    ]
                } else {
                    [
                        "provider normalization remains caller-supplied and does not authenticate the provider",
                        "payload shape and record indexing remain structural observations"
                    ]
                }
            },
            "parent_digests": parent_digests
        });
        let domain_report = self.project_domain_report(&report_request)?;
        Ok(json!({
            "ok": true,
            "schema": PROVIDER_DOMAIN_REPORT_SCHEMA_VERSION,
            "workflow": PROVIDER_DOMAIN_REPORT_WORKFLOW,
            "mode": mode,
            "normalization": normalization,
            "domain_report": domain_report,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "provider normalization is validated and indexed before the canonical domain report is projected",
                "provider payload bytes remain outside the domain-report copy when the report is composed",
                "normalization and receipt artifact digests remain explicit report lineage parents"
            ],
            "does_not_claim": [
                "provider authenticity, scientific, clinical, provenance, regulatory, or release validity",
                "that normalization or receipt indexing executed a provider, connector, or adapter",
                "that report composition proves readiness or external effects"
            ]
        }))
    }

    pub(super) fn project_domain_report(&self, arguments: &Value) -> Result<Value, String> {
        let report = bioprism_devplat::project_domain_report(arguments)
            .map_err(|error| format!("domain report projection refused: {error}"))?;
        let group_id = report
            .get("group_id")
            .and_then(Value::as_str)
            .ok_or("projected domain report omitted group_id")?;
        let source_tool = report
            .get("source_tool")
            .and_then(Value::as_str)
            .ok_or("projected domain report omitted source_tool")?;
        let domains = report
            .get("domains")
            .and_then(Value::as_array)
            .ok_or("projected domain report omitted domains")?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let group = catalogue
            .groups()
            .iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("unknown capability group {group_id:?}"))?;
        if !group.mcp_tools.iter().any(|tool| tool == source_tool) {
            return Err(format!(
                "source_tool {source_tool:?} is not declared by capability group {group_id:?}"
            ));
        }
        for domain in domains.iter().filter_map(Value::as_str) {
            if !group
                .domains
                .iter()
                .any(|declared| declared.eq_ignore_ascii_case(domain))
            {
                return Err(format!(
                    "domain label {domain:?} is not declared by capability group {group_id:?}"
                ));
            }
        }
        let subject_id = report
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("projected domain report omitted subject_id")?;
        let parent_digests = report
            .get("parent_digests")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let projection = self.index_artifact_projection(
            "domain_report",
            subject_id,
            domains
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            parent_digests,
            report.clone(),
        );
        if projection.get("indexed") != Some(&Value::Bool(true)) {
            return Err(format!(
                "domain report projection could not be indexed: {}",
                projection
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown artifact registry error")
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_REPORT_PROJECT_SCHEMA_VERSION,
            "workflow": DOMAIN_REPORT_PROJECT_WORKFLOW,
            "report": report,
            "artifact_registry": projection,
            "coverage": {
                "group_id": group.id,
                "source_tool": source_tool,
                "domains": domains,
                "catalogue_digest": catalogue.digest().to_string(),
                "group_status": group.status,
                "declared_tool_count": group.mcp_tools.len()
            },
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "the source tool and domains were checked against the authoritative workspace capability catalogue",
                "the canonical report was indexed by its exact JSON content digest",
                "the projection remains caller-supplied and does not execute the source tool"
            ],
            "does_not_claim": [
                "catalogue membership proves the report is scientifically or clinically valid",
                "artifact indexing proves completeness, provenance, readiness, or external effects"
            ]
        }))
    }
}
