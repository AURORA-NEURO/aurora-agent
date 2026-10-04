//! Evidence bundle, artifact, and readiness registry routes.
//!
//! The handlers share the router state but keep this route family reviewable on its own.

use super::*;

impl ApiRouter {
    pub(super) fn verify_evidence_bundle(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(bundle) = arguments.get("bundle") else {
            return self.error(
                422,
                "invalid_evidence_bundle",
                "request body must contain a bundle object",
                request_id,
            );
        };
        match verify_mission_evidence_bundle(bundle) {
            Ok(value) => HttpResponse::json(200, &value),
            Err(EvidenceBundleError::TooLarge { actual, maximum }) => self.error(
                413,
                "evidence_bundle_too_large",
                &format!("bundle is {actual} bytes; maximum is {maximum} bytes"),
                request_id,
            ),
            Err(error) => self.error(
                422,
                "invalid_evidence_bundle",
                &error.to_string(),
                request_id,
            ),
        }
    }

    pub(super) fn import_evidence_bundle(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(bundle) = arguments.get("bundle") else {
            return self.error(
                422,
                "invalid_evidence_bundle",
                "request body must contain a bundle object",
                request_id,
            );
        };
        let mut registry = match self.evidence_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "evidence_registry_unavailable",
                    "evidence registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let report = registry.import(bundle);
        let report = match report {
            Ok(report) => report,
            Err(EvidenceRegistryError::Full { maximum }) => {
                return self.error(
                    413,
                    "evidence_registry_full",
                    &format!("evidence registry has reached its {maximum}-bundle limit"),
                    request_id,
                )
            }
            Err(EvidenceRegistryError::Verification(reason)) => {
                return self.error(422, "evidence_bundle_not_verified", &reason, request_id)
            }
            Err(error) => {
                return self.error(
                    422,
                    "invalid_evidence_bundle",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        let created = report
            .get("created")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if created {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "evidence_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.evidence_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(503, "evidence_persistence_unavailable", &error, request_id);
            }
        }
        let artifact_projection = self.automatic_artifact_projection(
            "mission_evidence_bundle",
            bundle
                .get("mission_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown-mission"),
            evaluator_domains_for_artifact(bundle),
            bundle.clone(),
        );
        let mut report = report;
        report["artifact_registry"] = artifact_projection;
        let status = if created { 201 } else { 200 };
        HttpResponse::json(status, &report)
    }

    pub(super) fn query_evidence_bundles(
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
                "mission_id" | "domain" | "after" | "limit" | "include_bundles"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "evidence bundle query accepts only mission_id, domain, after, limit, and include_bundles",
                    request_id,
                );
            }
        }
        let mission_id = query.get("mission_id").map(String::as_str);
        let domain = query.get("domain").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_EVIDENCE_REGISTRY_QUERY_ITEMS).contains(&value) =>
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
        let include_bundles = match query_bool(&query, "include_bundles", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.evidence_registry.lock() {
            Ok(registry) => registry.query(mission_id, domain, after, max_items, include_bundles),
            Err(_) => {
                return self.error(
                    500,
                    "evidence_registry_unavailable",
                    "evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn get_evidence_bundle(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 3 || segments[0] != "v1" || segments[1] != "evidence-bundles" {
            return self.error(
                404,
                "not_found",
                "evidence bundle route does not exist",
                request_id,
            );
        }
        let digest = &segments[2];
        if ContentHash::parse(digest.clone()).is_err() {
            return self.error(
                422,
                "invalid_digest",
                "bundle digest must be a 64-character SHA-256 digest",
                request_id,
            );
        }
        let bundle = match self.evidence_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "evidence_registry_unavailable",
                    "evidence registry is unavailable",
                    request_id,
                )
            }
        };
        match bundle {
            Some(bundle) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "schema": "bioprism-api/evidence-bundle-record/0.1",
                    "workflow": "mission_evidence_bundle_get",
                    "bundle_digest": digest,
                    "bundle": bundle,
                    "execution": "not_started"
                }),
            ),
            None => self.error(
                404,
                "not_found",
                "verified evidence bundle does not exist",
                request_id,
            ),
        }
    }

    pub(super) fn evidence_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.evidence_state_path.is_some();
        let file_bytes = self
            .config
            .evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.evidence_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .evidence_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = EvidenceBundleRegistry::from_snapshot(&document).is_ok();
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
                "schema": bioprism_devplat::EVIDENCE_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_bundles": bioprism_devplat::MAX_EVIDENCE_REGISTRY_BUNDLES,
                "max_file_bytes": MAX_EVIDENCE_REGISTRY_BYTES,
                "recovery_policy": "only independently verified bundles restore; imported evidence never resumes execution",
                "flush": "/v1/evidence-bundles/persistence/flush"
            }),
        )
    }

    pub(super) fn flush_evidence_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.evidence_state_path.is_none() {
            return self.error(
                409,
                "evidence_persistence_disabled",
                "configure --evidence-state before flushing an evidence registry snapshot",
                request_id,
            );
        }
        match self.persist_evidence_registry() {
            Ok(_) => self.evidence_persistence_status(),
            Err(error) => self.error(503, "evidence_persistence_unavailable", &error, request_id),
        }
    }

    /// Register a projection produced by an explicit verification boundary and report checkpoint
    /// health separately from in-memory registration. The source registry operation remains
    /// successful when this auxiliary projection is unavailable; the response makes that gap
    /// visible instead of presenting the cross-domain index as complete.
    pub(super) fn automatic_artifact_projection(
        &self,
        kind: &str,
        subject_id: &str,
        domains: Vec<String>,
        artifact: Value,
    ) -> Value {
        let registration = json!({
            "kind": kind,
            "subject_id": subject_id,
            "domains": domains,
            "parent_digests": [],
            "artifact": artifact,
        });
        let result = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry is unavailable".to_string())
            .and_then(|mut registry| {
                registry
                    .register(&registration)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(report) => {
                let checkpoint = self.artifact_persistence.persist();
                let digest = report.get("content_digest").cloned().unwrap_or(Value::Null);
                json!({
                    "indexed": true,
                    "kind": kind,
                    "subject_id": subject_id,
                    "content_digest": digest,
                    "created": report.get("created").cloned().unwrap_or(Value::Null),
                    "already_present": report.get("already_present").cloned().unwrap_or(Value::Null),
                    "verification": report.get("verification").cloned().unwrap_or(Value::Null),
                    "lookup": digest.as_str().map(|value| format!("/v1/artifacts/{value}")).unwrap_or_default(),
                    "persistence": {
                        "enabled": self.config.artifact_state_path.is_some(),
                        "checkpointed": checkpoint.is_ok(),
                        "error": checkpoint.err()
                    },
                    "execution": "not_started",
                    "does_not_claim": [
                        "artifact integrity establishes scientific, clinical, regulatory, publication, or external-effect validity",
                        "automatic indexing establishes causal provenance or external storage authority"
                    ]
                })
            }
            Err(error) => json!({
                "indexed": false,
                "kind": kind,
                "subject_id": subject_id,
                "error": error,
                "persistence": {
                    "enabled": self.config.artifact_state_path.is_some(),
                    "checkpointed": false
                },
                "execution": "not_started",
                "does_not_claim": [
                    "the failed projection means the source record was invalid",
                    "absence from the artifact registry means the source record never existed"
                ]
            }),
        }
    }

    pub(super) fn register_artifact(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let registration = match self.json_object(request) {
            Ok(arguments) => Value::Object(arguments),
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let mut registry = match self.artifact_registry.lock() {
            Ok(registry) => registry,
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        let before = registry.clone();
        let report = match registry.register(&registration) {
            Ok(report) => report,
            Err(ArtifactRegistryError::Full { maximum }) => {
                return self.error(
                    413,
                    "artifact_registry_full",
                    &format!("artifact registry has reached its {maximum}-record limit"),
                    request_id,
                )
            }
            Err(error @ ArtifactRegistryError::ArtifactTooLarge { .. }) => {
                return self.error(413, "artifact_too_large", &error.to_string(), request_id)
            }
            Err(error) => {
                return self.error(
                    422,
                    "invalid_artifact_registration",
                    &error.to_string(),
                    request_id,
                )
            }
        };
        let created = report
            .get("created")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if created {
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    *registry = before;
                    return self.error(
                        503,
                        "artifact_persistence_unavailable",
                        &error.to_string(),
                        request_id,
                    );
                }
            };
            if let Err(error) = self.artifact_persistence.persist_snapshot(&snapshot) {
                *registry = before;
                return self.error(503, "artifact_persistence_unavailable", &error, request_id);
            }
        }
        HttpResponse::json(if created { 201 } else { 200 }, &report)
    }

    /// Compare the three bounded local registries by exact digest identity.
    ///
    /// The stores are sampled independently, so the response includes each generation and
    /// checkpoint digest rather than implying a cross-store transaction.
    pub(super) fn cross_store_artifact_audit(&self, request_id: &str) -> HttpResponse {
        let (artifact_records, artifact_generation, artifact_state_digest) = {
            let registry = match self.artifact_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "artifact_registry_unavailable",
                        "artifact registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("artifact registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.records_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (evidence_digests, evidence_generation, evidence_state_digest) = {
            let registry = match self.evidence_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "evidence_registry_unavailable",
                        "evidence registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("evidence registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (reconciliation_digests, reconciliation_generation, reconciliation_state_digest) = {
            let registry = match self.reconciliation_registry.lock() {
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
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("workflow reconciliation registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        let (
            workflow_execution_evidence_digests,
            workflow_execution_evidence_generation,
            workflow_execution_evidence_state_digest,
        ) = {
            let registry = match self.workflow_execution_evidence_registry.lock() {
                Ok(registry) => registry,
                Err(_) => {
                    return self.error(
                        500,
                        "workflow_execution_evidence_registry_unavailable",
                        "workflow execution evidence registry is unavailable",
                        request_id,
                    )
                }
            };
            let snapshot = match registry.snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return self.error(
                        500,
                        "cross_domain_audit_unavailable",
                        &format!("workflow execution evidence registry snapshot failed: {error}"),
                        request_id,
                    )
                }
            };
            (
                registry.digests_for_audit(),
                registry.generation(),
                snapshot
                    .get("state_digest")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            )
        };
        HttpResponse::json(
            200,
            &build_cross_domain_audit(
                &artifact_records,
                &evidence_digests,
                &reconciliation_digests,
                &workflow_execution_evidence_digests,
                artifact_generation,
                evidence_generation,
                reconciliation_generation,
                workflow_execution_evidence_generation,
                artifact_state_digest,
                evidence_state_digest,
                reconciliation_state_digest,
                workflow_execution_evidence_state_digest,
            ),
        )
    }

    pub(super) fn query_domain_decision_readiness(
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
                "subject_id"
                    | "decision_state"
                    | "policy_satisfied"
                    | "after"
                    | "limit"
                    | "include_audits"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "decision-readiness query accepts only subject_id, decision_state, policy_satisfied, after, limit, and include_audits",
                    request_id,
                );
            }
        }
        let subject_id = query.get("subject_id").map(String::as_str);
        let decision_state = query.get("decision_state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let policy_satisfied = match query
            .get("policy_satisfied")
            .map(|_| query_bool(&query, "policy_satisfied", false))
        {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => return self.error(422, "invalid_query", &error, request_id),
            None => None,
        };
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&value) =>
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
        let include_audits = match query_bool(&query, "include_audits", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.domain_decision_readiness_query(
                subject_id,
                decision_state,
                policy_satisfied,
                after,
                max_items,
                include_audits,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn control_plane_readiness_audit(
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
            "control_plane_readiness_audit",
            Value::Object(arguments),
        )
    }

    pub(super) fn control_plane_readiness_compare(
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
            "control_plane_readiness_compare",
            Value::Object(arguments),
        )
    }

    pub(super) fn control_plane_readiness_compare_retained(
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
            "control_plane_readiness_compare_retained",
            Value::Object(arguments),
        )
    }

    pub(super) fn query_control_plane_readiness(
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
                "subject_id"
                    | "control_plane_state"
                    | "policy_satisfied"
                    | "after"
                    | "limit"
                    | "include_audits"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "control-plane readiness query accepts only subject_id, control_plane_state, policy_satisfied, after, limit, and include_audits",
                    request_id,
                );
            }
        }
        let subject_id = query.get("subject_id").map(String::as_str);
        let control_plane_state = query.get("control_plane_state").map(String::as_str);
        let after = query.get("after").map(String::as_str);
        let policy_satisfied = match query
            .get("policy_satisfied")
            .map(|_| query_bool(&query, "policy_satisfied", false))
        {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => return self.error(422, "invalid_query", &error, request_id),
            None => None,
        };
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&value) =>
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
        let include_audits = match query_bool(&query, "include_audits", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.control_plane_readiness_query(
                subject_id,
                control_plane_state,
                policy_satisfied,
                after,
                max_items,
                include_audits,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn query_artifacts(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        for key in query.keys() {
            if !matches!(
                key.as_str(),
                "kind" | "domain" | "subject_id" | "after" | "limit" | "include_artifacts"
            ) {
                return self.error(
                    400,
                    "invalid_query",
                    "artifact query accepts only kind, domain, subject_id, after, limit, and include_artifacts",
                    request_id,
                );
            }
        }
        let max_items = match query_usize(&query, "limit", 100) {
            Ok(value)
                if (1..=bioprism_devplat::MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&value) =>
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
        let include_artifacts = match query_bool(&query, "include_artifacts", false) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.query(
                query.get("kind").map(String::as_str),
                query.get("domain").map(String::as_str),
                query.get("subject_id").map(String::as_str),
                query.get("after").map(String::as_str),
                max_items,
                include_artifacts,
            ),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn get_artifact(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 3 || segments[0] != "v1" || segments[1] != "artifacts" {
            return self.error(
                404,
                "not_found",
                "artifact route does not exist",
                request_id,
            );
        }
        let digest = &segments[2];
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.get(digest),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(ArtifactRegistryError::NotFound { .. }) => {
                self.error(404, "not_found", "artifact does not exist", request_id)
            }
            Err(error) => self.error(422, "invalid_digest", &error.to_string(), request_id),
        }
    }

    pub(super) fn artifact_lineage(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let segments = match request.path_segments() {
            Ok(segments) => segments,
            Err(error) => return self.error(400, "invalid_path", &error.to_string(), request_id),
        };
        if segments.len() != 4
            || segments[0] != "v1"
            || segments[1] != "artifacts"
            || segments[3] != "lineage"
        {
            return self.error(
                404,
                "not_found",
                "artifact lineage route does not exist",
                request_id,
            );
        }
        let digest = &segments[2];
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.lineage(digest),
            Err(_) => {
                return self.error(
                    500,
                    "artifact_registry_unavailable",
                    "artifact registry is unavailable",
                    request_id,
                )
            }
        };
        match result {
            Ok(value) => HttpResponse::json(200, &value),
            Err(ArtifactRegistryError::NotFound { .. }) => {
                self.error(404, "not_found", "artifact does not exist", request_id)
            }
            Err(error) => self.error(422, "invalid_digest", &error.to_string(), request_id),
        }
    }

    pub(super) fn artifact_persistence_status(&self) -> HttpResponse {
        let enabled = self.config.artifact_state_path.is_some();
        let file_bytes = self
            .config
            .artifact_state_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len());
        let registry = self.artifact_registry.lock();
        let (registry_size, generation) = registry
            .as_ref()
            .map(|registry| (registry.len(), registry.generation()))
            .unwrap_or((0, 0));
        let (state_digest, integrity_verified) = self
            .config
            .artifact_state_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .map(|document| {
                let digest = document.get("state_digest").cloned().unwrap_or(Value::Null);
                let valid = ArtifactRegistry::from_snapshot(&document).is_ok();
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
                "schema": bioprism_devplat::ARTIFACT_REGISTRY_SCHEMA_VERSION,
                "state_digest": state_digest,
                "integrity_verified": integrity_verified,
                "registry_size": registry_size,
                "registry_generation": generation,
                "max_records": bioprism_devplat::MAX_ARTIFACT_REGISTRY_RECORDS,
                "max_file_bytes": MAX_ARTIFACT_REGISTRY_BYTES,
                "recovery_policy": "only digest-valid artifact records restore; indexed artifacts never resume execution",
                "flush": "/v1/artifacts/persistence/flush"
            }),
        )
    }

    pub(super) fn flush_artifact_persistence(&self, request_id: &str) -> HttpResponse {
        if self.config.artifact_state_path.is_none() {
            return self.error(
                409,
                "artifact_persistence_disabled",
                "configure --artifact-state before flushing an artifact registry snapshot",
                request_id,
            );
        }
        match self.persist_artifact_registry() {
            Ok(_) => self.artifact_persistence_status(),
            Err(error) => self.error(503, "artifact_persistence_unavailable", &error, request_id),
        }
    }
}
