//! Domain workflow, report, evidence, and capability routes.
//!
//! These adapters preserve the router as the single transport boundary while keeping each
//! domain-facing request path reviewable apart from transport and persistence mechanics.

use super::*;

impl ApiRouter {
    pub(super) fn domain_workflow_catalogue(&self, request_id: &str) -> HttpResponse {
        self.domain_workflow_tool(request_id, "domain_workflow_catalogue", json!({}))
    }

    pub(super) fn domain_workflow_instantiate(
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
            "domain_workflow_instantiate",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_workflow_portfolio(
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
            "domain_workflow_portfolio",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_workflow_portfolio_verify(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let mut arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        // REST adds request_id to every response envelope. It is transport metadata, not part of
        // the retained content-addressed portfolio, so accept a directly round-tripped report.
        if let Some(portfolio) = arguments
            .get_mut("portfolio")
            .and_then(Value::as_object_mut)
        {
            portfolio.remove("request_id");
        }
        self.domain_workflow_tool(
            request_id,
            "domain_workflow_portfolio_verify",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_workflow_verify(
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
            "domain_workflow_verify",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_workflow_scaffold(
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
            "domain_workflow_scaffold",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_workflow_reconcile(
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
            "domain_workflow_reconcile",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_report_project(
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
            "domain_report_project",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_report_coverage(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_groups = match query_usize(&query, "max_groups", 64) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_report_digests = match query_bool(&query, "include_report_digests", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("operation".into(), json!("coverage"));
        arguments.insert("max_groups".into(), json!(max_groups));
        arguments.insert(
            "include_report_digests".into(),
            json!(include_report_digests),
        );
        if let Some(group_id) = query.get("group_id") {
            arguments.insert("group_id".into(), json!(group_id));
        }
        if let Some(domain) = query.get("domain") {
            arguments.insert("domain".into(), json!(domain));
        }
        if let Some(report_class) = query.get("report_class") {
            arguments.insert("report_class".into(), json!(report_class));
        }
        if let Some(bridge_mode) = query.get("bridge_mode") {
            arguments.insert("bridge_mode".into(), json!(bridge_mode));
        }
        self.domain_workflow_tool(
            request_id,
            "domain_report_project",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_evidence_harmonize(
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
            "domain_evidence_harmonize",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_evidence_harmonization_coverage(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_items = match query_usize(&query, "max_items", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_report_digests = match query_bool(&query, "include_report_digests", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_items".into(), json!(max_items));
        arguments.insert(
            "include_report_digests".into(),
            json!(include_report_digests),
        );
        for name in [
            "subject_id",
            "domain",
            "report_class",
            "bridge_mode",
            "traceability_state",
            "after",
        ] {
            if let Some(value) = query.get(name) {
                arguments.insert(name.into(), json!(value));
            }
        }
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_harmonization_coverage",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_evidence_intake(
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
            "domain_evidence_intake",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_evidence_lineage(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        const ALLOWED: &[&str] = &[
            "content_digest",
            "group_id",
            "domain",
            "subject_id",
            "source_tool",
            "outcome",
            "request_digest",
            "response_digest",
            "intake_digest",
            "source_plan_digest",
            "after",
            "max_items",
            "include_children",
        ];
        if let Some(unknown) = query.keys().find(|key| !ALLOWED.contains(&key.as_str())) {
            return self.error(
                400,
                "invalid_query",
                &format!("unknown domain evidence lineage query parameter {unknown:?}"),
                request_id,
            );
        }
        let max_items = match query_usize(&query, "max_items", 100) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let include_children = match query_bool(&query, "include_children", true) {
            Ok(value) => value,
            Err(error) => return self.error(422, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_items".into(), json!(max_items));
        arguments.insert("include_children".into(), json!(include_children));
        for name in [
            "content_digest",
            "group_id",
            "domain",
            "subject_id",
            "source_tool",
            "outcome",
            "request_digest",
            "response_digest",
            "intake_digest",
            "source_plan_digest",
            "after",
        ] {
            if let Some(value) = query.get(name) {
                arguments.insert(name.into(), json!(value));
            }
        }
        let result = match self.artifact_registry.lock() {
            Ok(registry) => registry.domain_evidence_lineage(&Value::Object(arguments)),
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
            Err(ArtifactRegistryError::NotFound { .. }) => self.error(
                404,
                "not_found",
                "domain evidence intake does not exist",
                request_id,
            ),
            Err(error) => self.error(422, "invalid_query", &error.to_string(), request_id),
        }
    }

    pub(super) fn domain_evidence_source_plan(
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
            "domain_evidence_source_plan",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_evidence_source_execute(
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
            "domain_evidence_source_execute",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_evidence_coverage(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_groups = match query_usize(&query, "max_groups", 64) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_intake_digests = match query_bool(&query, "include_intake_digests", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_groups".into(), json!(max_groups));
        arguments.insert(
            "include_intake_digests".into(),
            json!(include_intake_digests),
        );
        if let Some(group_id) = query.get("group_id") {
            arguments.insert("group_id".into(), json!(group_id));
        }
        if let Some(domain) = query.get("domain") {
            arguments.insert("domain".into(), json!(domain));
        }
        self.domain_workflow_tool(
            request_id,
            "domain_evidence_coverage",
            Value::Object(arguments),
        )
    }

    pub(super) fn capability_dashboard(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let max_groups = match query_usize(&query, "max_groups", 128) {
            Ok(value) if (1..=512).contains(&value) => value,
            Ok(_) => {
                return self.error(
                    400,
                    "invalid_query",
                    "max_groups must be between 1 and 512",
                    request_id,
                )
            }
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_tools = match query_bool(&query, "include_tools", false) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let include_gaps = match query_bool(&query, "include_gaps", true) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let mut arguments = serde_json::Map::new();
        arguments.insert("max_groups".into(), json!(max_groups));
        arguments.insert("include_tools".into(), json!(include_tools));
        arguments.insert("include_gaps".into(), json!(include_gaps));
        for name in ["group_id", "domain", "status"] {
            if let Some(value) = query.get(name) {
                if value.trim().is_empty() {
                    return self.error(
                        400,
                        "invalid_query",
                        &format!("{name} must be non-empty when supplied"),
                        request_id,
                    );
                }
                arguments.insert(name.into(), json!(value));
            }
        }
        self.domain_workflow_tool(request_id, "capability_dashboard", Value::Object(arguments))
    }

    pub(super) fn capability_route(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let arguments = match self.json_object(request) {
            Ok(arguments) => arguments,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        self.domain_workflow_tool(request_id, "capability_route", Value::Object(arguments))
    }

    pub(super) fn capability_route_review(
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
            "capability_route_review",
            Value::Object(arguments),
        )
    }

    pub(super) fn capability_route_plan(
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
            "capability_route_plan",
            Value::Object(arguments),
        )
    }

    pub(super) fn capability_route_plan_verify(
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
            "capability_route_plan_verify",
            Value::Object(arguments),
        )
    }

    pub(super) fn domain_workflow_tool(
        &self,
        request_id: &str,
        tool: &str,
        arguments: Value,
    ) -> HttpResponse {
        let call = Request {
            id: Some(Value::String(request_id.to_string())),
            method: "tools/call".into(),
            params: json!({ "name": tool, "arguments": arguments }),
        };
        let mut server = self.server.clone();
        let Some(response) = server.handle(&call) else {
            return self.error(
                500,
                "dispatch_failed",
                "domain workflow call produced no response",
                request_id,
            );
        };
        let wire = response.to_json();
        self.record_tool_event(request_id, tool, &wire);
        let _ = self.reconciliation_persistence.persist();
        let _ = self.artifact_persistence.persist();
        let _ = self.workflow_execution_evidence_persistence.persist();
        let is_error = wire
            .pointer("/result/isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let payload = wire
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or_else(|| {
                json!({
                    "ok": false,
                    "error": "domain workflow dispatcher returned no structured payload"
                })
            });
        if is_error {
            let message = payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("domain workflow request was refused");
            return self.error(422, "invalid_domain_workflow", message, request_id);
        }
        let mut payload = payload;
        payload["request_id"] = json!(request_id);
        HttpResponse::json(200, &payload)
    }
}
