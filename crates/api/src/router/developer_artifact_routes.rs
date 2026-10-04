//! Developer workbench and CI evidence registry routes.
//!
//! These report projections share the router-owned registries and checkpoint policy.

use super::*;

impl ApiRouter {
    pub(super) fn developer_workbench_verify(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(
            request_id,
            "developer_workbench_verify",
            Value::Object(arguments),
        )
    }

    pub(super) fn import_workbench_report(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let report = match arguments.get("report") {
            Some(report) => report,
            None => return self.error(400, "invalid_json", "report is required", request_id),
        };
        let mut registry = match self.workbench_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "workbench_registry_unavailable",
                    "workbench registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let result = match registry.import(report) {
            Ok(result) => result,
            Err(error) => return self.error(422, "invalid_report", &error.to_string(), request_id),
        };
        if self.config.workbench_state_path.is_some() && result["created"] == true {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "workbench_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.workbench_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(503, "workbench_persistence_unavailable", &error, request_id);
            }
        }
        HttpResponse::json(
            if result["created"].as_bool().unwrap_or(false) {
                201
            } else {
                200
            },
            &result,
        )
    }

    pub(super) fn query_workbench_reports(
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
                "session_digest"
                    | "domain"
                    | "capability"
                    | "state"
                    | "release_ready"
                    | "after"
                    | "limit"
                    | "include_reports"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "workbench query accepts only session_digest, domain, capability, state, release_ready, after, limit, and include_reports",
                    request_id,
                );
            }
        }
        let session_digest = query.get("session_digest").map(String::as_str);
        let domain = query.get("domain").map(String::as_str);
        let capability = query.get("capability").map(String::as_str);
        let state = query.get("state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value) if (1..=bioprism_devplat::MAX_WORKBENCH_QUERY_ITEMS).contains(&value) => {
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
        let release_ready = match query_bool(&query, "release_ready", false) {
            Ok(value) if query.contains_key("release_ready") => Some(value),
            Ok(_) => None,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_reports = match query_bool(&query, "include_reports", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.workbench_registry.lock() {
            Ok(registry) => registry.query(
                session_digest,
                domain,
                capability,
                state,
                release_ready,
                after,
                max_items,
                include_reports,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "workbench_registry_unavailable",
                    "workbench registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn get_workbench_report(
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
            || segments[1] != "developer-workbench"
            || segments[2] != "reports"
        {
            return self.error(
                404,
                "not_found",
                "workbench report route does not exist",
                request_id,
            );
        }
        let digest = &segments[3];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "workbench_report_digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let result = match self.workbench_registry.lock() {
            Ok(registry) => registry.get_response(digest),
            Err(_) => {
                return self.error(
                    500,
                    "workbench_registry_unavailable",
                    "workbench registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(_) => self.error(
                404,
                "not_found",
                "workbench report does not exist",
                request_id,
            ),
        }
    }

    pub(super) fn workbench_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.workbench_state_path.is_some();
        let file_bytes = self
            .config
            .workbench_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.workbench_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .workbench_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = WorkbenchReportRegistry::from_snapshot(&document).is_ok();
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
                "schema": bioprism_devplat::WORKBENCH_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_reports": bioprism_devplat::MAX_WORKBENCH_REPORTS,
                "max_file_bytes": MAX_WORKBENCH_REGISTRY_STATE_BYTES,
                "recovery_policy": "only structurally valid digest-bound reports restore; retained reports never resume execution",
                "flush": "/v1/developer-workbench/reports/persistence/flush"
            }),
        )
    }

    pub(super) fn flush_workbench_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.workbench_state_path.is_none() {
            return self.error(
                409,
                "workbench_persistence_disabled",
                "configure workbench_state_path before flushing a workbench report snapshot",
                request_id,
            );
        }
        match self.persist_workbench_registry() {
            Ok(_) => self.workbench_persistence_status(),
            Err(error) => self.error(503, "workbench_persistence_unavailable", &error, request_id),
        }
    }

    pub(super) fn import_ci_provider_evidence(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mut registry = match self.ci_provider_evidence_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "ci_provider_evidence_registry_unavailable",
                    "CI provider evidence registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let result = match registry.import(&Value::Object(arguments)) {
            Ok(result) => result,
            Err(error) => {
                return self.error(
                    422,
                    "invalid_ci_provider_evidence",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        if self.config.ci_provider_evidence_state_path.is_some() && result["created"] == true {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "ci_provider_evidence_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self
                .ci_provider_evidence_persistence
                .persist_snapshot(&snapshot)
            {
                *registry = before;
                return self.error(
                    503,
                    "ci_provider_evidence_persistence_unavailable",
                    &error,
                    request_id,
                );
            }
        }
        HttpResponse::json(
            if result["created"].as_bool().unwrap_or(false) {
                201
            } else {
                200
            },
            &result,
        )
    }

    pub(super) fn query_ci_provider_evidence(
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
                "provider"
                    | "run_id"
                    | "plan_digest"
                    | "structurally_valid"
                    | "conformance_ready"
                    | "min_local_byte_hash_artifacts"
                    | "min_local_byte_hash_logs"
                    | "min_attestation_subject_digest_bindings"
                    | "after"
                    | "limit"
                    | "max_items"
                    | "include_records"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "CI provider evidence query accepts only provider, run_id, plan_digest, structurally_valid, conformance_ready, minimum digest-binding thresholds, after, limit or max_items, and include_records",
                    request_id,
                );
            }
        }
        let structurally_valid = match query_bool(&query, "structurally_valid", false) {
            Ok(value) if query.contains_key("structurally_valid") => Some(value),
            Ok(_) => None,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let conformance_ready = match query_bool(&query, "conformance_ready", false) {
            Ok(value) if query.contains_key("conformance_ready") => Some(value),
            Ok(_) => None,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let optional_minimum = |name: &str| -> Result<Option<usize>, String> {
            match query_usize(&query, name, 0) {
                Ok(_) if !query.contains_key(name) => Ok(None),
                Ok(value) if value <= 128 => Ok(Some(value)),
                Ok(_) => Err(format!("{name} must be between 0 and 128")),
                Err(error) => Err(error),
            }
        };
        let min_local_byte_hash_artifacts = match optional_minimum("min_local_byte_hash_artifacts")
        {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let min_local_byte_hash_logs = match optional_minimum("min_local_byte_hash_logs") {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let min_attestation_subject_digest_bindings =
            match optional_minimum("min_attestation_subject_digest_bindings") {
                Ok(value) => value,
                Err(error) => return self.error(422, "invalid_query", &error, request_id),
            };
        if query.contains_key("limit") && query.contains_key("max_items") {
            return self.error(
                400,
                "invalid_query",
                "CI provider evidence query accepts either limit or max_items, not both",
                request_id,
            );
        }
        let item_limit_key = if query.contains_key("max_items") {
            "max_items"
        } else {
            "limit"
        };
        let include_records = match query_bool(&query, "include_records", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let max_items = match query_usize(&query, item_limit_key, 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS)
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
        let result = match self.ci_provider_evidence_registry.lock() {
            Ok(registry) => registry.query(
                query.get("provider").map(String::as_str),
                query.get("run_id").map(String::as_str),
                query.get("plan_digest").map(String::as_str),
                structurally_valid,
                conformance_ready,
                min_local_byte_hash_artifacts,
                min_local_byte_hash_logs,
                min_attestation_subject_digest_bindings,
                query.get("after").map(String::as_str),
                max_items,
                include_records,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "ci_provider_evidence_registry_unavailable",
                    "CI provider evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn get_ci_provider_evidence(
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
            || segments[1] != "ci"
            || segments[2] != "provider-evidence"
        {
            return self.error(
                404,
                "not_found",
                "CI provider evidence route does not exist",
                request_id,
            );
        }
        let digest = &segments[3];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "provider_evidence_digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let result = match self.ci_provider_evidence_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "ci_provider_evidence_registry_unavailable",
                    "CI provider evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(_) => self.error(
                404,
                "not_found",
                "CI provider evidence record does not exist",
                request_id,
            ),
        }
    }

    pub(super) fn ci_provider_evidence_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.ci_provider_evidence_state_path.is_some();
        let file_bytes = self
            .config
            .ci_provider_evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let (registry_size, generation) = self
            .ci_provider_evidence_registry
            .lock()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .ci_provider_evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = CiProviderEvidenceRegistry::from_snapshot(&document).is_ok();
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
                "schema": bioprism_devplat::CI_PROVIDER_EVIDENCE_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_records": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_RECORDS,
                "max_query_items": bioprism_devplat::MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS,
                "max_file_bytes": MAX_CI_PROVIDER_EVIDENCE_REGISTRY_STATE_BYTES,
                "recovery_policy": "only re-audited provider evidence records restore; failed and unknown provider runs remain explicit and never resume execution",
                "lineage_policy": "artifact, log, and attestation record digests are retained as provider-observed joins; remote bytes and signatures are not verified",
                "flush": "/v1/ci/provider-evidence/persistence/flush"
            }),
        )
    }

    pub(super) fn flush_ci_provider_evidence_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.ci_provider_evidence_state_path.is_none() {
            return self.error(
                409,
                "ci_provider_evidence_persistence_disabled",
                "configure ci_provider_evidence_state_path before flushing a CI provider evidence snapshot",
                request_id,
            );
        }
        match self.persist_ci_provider_evidence_registry() {
            Ok(_) => self.ci_provider_evidence_persistence_status(),
            Err(error) => self.error(
                503,
                "ci_provider_evidence_persistence_unavailable",
                &error,
                request_id,
            ),
        }
    }
}
