//! Workflow reconciliation import, query, and persistence routes.
//!
//! Reconciliation records remain caller-evidence projections, not proof of external execution.

use super::*;

impl ApiRouter {
    pub(super) fn import_workflow_reconciliation(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let record = match arguments.get("record") {
            Some(record) => record,
            None => return self.error(422, "invalid_record", "record is required", request_id),
        };
        let mut registry = match self.reconciliation_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let report = match registry.import(record) {
            Ok(report) => report,
            Err(error) => return self.error(422, "invalid_record", &error.to_string(), request_id),
        };
        if self.config.reconciliation_state_path.is_some() {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "reconciliation_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.reconciliation_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(
                    503,
                    "reconciliation_persistence_unavailable",
                    &error,
                    request_id,
                );
            }
        }
        let artifact_projection = self.automatic_artifact_projection(
            "workflow_reconciliation",
            record
                .get("mission_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown-mission"),
            Vec::new(),
            strip_artifact_transport_fields(record),
        );
        let mut report = report;
        report["artifact_registry"] = artifact_projection;
        HttpResponse::json(
            if report["created"].as_bool().unwrap_or(false) {
                201
            } else {
                200
            },
            &report,
        )
    }

    pub(super) fn query_workflow_reconciliations(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "mission_id"
                    | "workflow_id"
                    | "mission_plan_digest"
                    | "completion_status"
                    | "decision_readiness_state"
                    | "decision_readiness_gate_satisfied"
                    | "after"
                    | "limit"
                    | "include_records"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "workflow reconciliation query accepts only mission_id, workflow_id, mission_plan_digest, completion_status, decision_readiness_state, decision_readiness_gate_satisfied, after, limit, and include_records",
                    request_id,
                );
            }
        }
        let mission_id = query.get("mission_id").map(String::as_str);
        let workflow_id = query.get("workflow_id").map(String::as_str);
        let mission_plan_digest = query.get("mission_plan_digest").map(String::as_str);
        let completion_status = query.get("completion_status").map(String::as_str);
        let decision_readiness_state = query.get("decision_readiness_state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS)
                    .contains(&value) =>
            {
                value
            }
            Ok(_) => {
                return self.error(
                    422,
                    "invalid_query",
                    "limit must be between 1 and 256",
                    request_id,
                )
            }
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_records = match query_bool(&query, "include_records", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let decision_readiness_gate_satisfied = match query
            .get("decision_readiness_gate_satisfied")
            .map(|_| query_bool(&query, "decision_readiness_gate_satisfied", false))
        {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => return self.error(422, "invalid_query", &error, request_id),
            None => None,
        };
        let result = match self.reconciliation_registry.lock() {
            Ok(registry) => registry.query(
                mission_id,
                workflow_id,
                mission_plan_digest,
                completion_status,
                decision_readiness_state,
                decision_readiness_gate_satisfied,
                after,
                max_items,
                include_records,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn get_workflow_reconciliation(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 4
            || segments[0] != "v1"
            || segments[1] != "domain-workflows"
            || segments[2] != "reconciliations"
        {
            return self.error(
                404,
                "not_found",
                "workflow reconciliation route does not exist",
                request_id,
            );
        }
        let digest = &segments[3];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "reconciliation_digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let record = match self.reconciliation_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "reconciliation_registry_unavailable",
                    "workflow reconciliation registry is unavailable",
                    request_id,
                )
            }
        };
        match record {
            Some(record) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-api/domain-workflow-reconciliation-record/0.1",
                    "workflow": "domain_workflow_reconciliation_get",
                    "reconciliation_digest": digest,
                    "record": record,
                    "execution": "not_started"
                }),
            ),
            None => self.error(
                404,
                "not_found",
                "workflow reconciliation does not exist",
                request_id,
            ),
        }
    }

    pub(super) fn reconciliation_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.reconciliation_state_path.is_some();
        let file_bytes = self
            .config
            .reconciliation_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.reconciliation_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .reconciliation_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = DomainWorkflowReconciliationRegistry::from_snapshot(&document).is_ok();
                (digest, Value::Bool(valid))
            })
            .unwrap_or((Value::Null, Value::Null));
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "enabled": enabled,
                "file_present": file_bytes.is_some(),
                "file_bytes": file_bytes,
                "schema": bioprism_devplat::DOMAIN_WORKFLOW_RECONCILIATION_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_reconciliations": bioprism_devplat::MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
                "max_file_bytes": MAX_WORKFLOW_RECONCILIATION_STATE_BYTES,
                "recovery_policy": "only digest-valid reconciliation reports restore; imported audit records never resume execution",
                "flush": "/v1/domain-workflows/reconciliations/persistence/flush"
            }),
        )
    }

    pub(super) fn flush_reconciliation_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.reconciliation_state_path.is_none() {
            return self.error(
                409,
                "reconciliation_persistence_disabled",
                "configure --reconciliation-state before flushing a workflow reconciliation snapshot",
                request_id,
            );
        }
        match self.persist_reconciliation_registry() {
            Ok(_) => self.reconciliation_persistence_status(),
            Err(error) => self.error(
                503,
                "reconciliation_persistence_unavailable",
                &error,
                request_id,
            ),
        }
    }
}
