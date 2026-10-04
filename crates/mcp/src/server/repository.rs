//! Repository documentation catalog, bundles, telemetry, and change impact.

use super::*;

impl Server {
    fn scan_repository(&self) -> Result<bioprism_docgraph::ScanReport, String> {
        let options = ScanOptions::default()
            .with_status("docs/", NodeStatus::Specification)
            .with_status(".agents/", NodeStatus::Guide)
            .with_status("README.md", NodeStatus::Guide)
            .with_status("AGENTS.md", NodeStatus::Guide);
        scan_markdown_tree(&self.root, &options).map_err(|error| error.to_string())
    }

    pub(super) fn repository_catalog(&self, arguments: &Value) -> Result<Value, String> {
        let prefix = arguments
            .get("prefix")
            .and_then(Value::as_str)
            .unwrap_or("");
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(200);
        if limit == 0 || limit > 1_000 {
            return Err("limit must be between 1 and 1000".into());
        }
        let include_briefs = arguments
            .get("include_briefs")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let include_findings = arguments
            .get("include_findings")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let report = self.scan_repository()?;
        let lint_report = lint(&report.graph, &[]);
        let matching: Vec<_> = report
            .graph
            .nodes()
            .filter(|node| node.id.as_str().starts_with(prefix))
            .collect();
        let total_matching = matching.len();
        let modules = matching
            .into_iter()
            .take(limit as usize)
            .map(|node| {
                let mut item = json!({
                    "id": node.id,
                    "path": node.path,
                    "title": node.title,
                    "status": node.status.as_str(),
                    "declared_profile": node.declared_profile.as_str(),
                    "protected_classes": node
                        .protected_classes
                        .iter()
                        .map(|class| class.as_str())
                        .collect::<Vec<_>>(),
                    "sha256": node.hash.as_ref().map(|hash| hash.as_str()),
                });
                if include_briefs {
                    item["brief"] = json!(node.body.brief);
                }
                item
            })
            .collect::<Vec<_>>();

        let mut result = json!({
            "ok": true,
            "root": self.root,
            "prefix": prefix,
            "files_read": report.files_read,
            "module_count": report.graph.node_count(),
            "edge_count": report.graph.edges().len(),
            "matching_modules": total_matching,
            "returned_modules": modules.len(),
            "omitted_modules": total_matching.saturating_sub(modules.len()),
            "truncated": modules.len() < total_matching,
            "modules": modules,
            "unresolved_links": report.unresolved_links,
            "out_of_corpus_links": report.out_of_corpus_links,
            "unreadable_front_matter": report.unreadable_front_matter,
            "lint": {
                "errors": lint_report.errors().count(),
                "warnings": lint_report.warnings().count(),
                "counts": lint_report.counts(),
            },
        });
        if include_findings {
            result["lint"]["findings"] = json!(lint_report.findings);
        }
        Ok(result)
    }

    pub(super) fn repository_bundle(&self, arguments: &Value) -> Result<Value, String> {
        let route_value = arguments
            .get("route")
            .cloned()
            .ok_or("route is required: provide id, intent and must_read")?;
        let route: TaskRoute = serde_json::from_value(route_value)
            .map_err(|error| format!("invalid documentation route: {error}"))?;
        let policy = documentation_policy(arguments)?;
        let include_markdown = arguments
            .get("include_markdown")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_chars = arguments
            .get("max_markdown_chars")
            .and_then(Value::as_u64)
            .unwrap_or(120_000);
        if max_chars == 0 || max_chars > 2_000_000 {
            return Err("max_markdown_chars must be between 1 and 2000000".into());
        }

        let report = self.scan_repository()?;
        let bundle =
            compile_bundle(&report.graph, &route, &policy).map_err(|error| error.to_string())?;
        let mut result = json!({
            "ok": true,
            "root": self.root,
            "bundle": bundle,
            "scan": {
                "files_read": report.files_read,
                "module_count": report.graph.node_count(),
                "edge_count": report.graph.edges().len(),
                "unresolved_links": report.unresolved_links,
                "unreadable_front_matter": report.unreadable_front_matter,
            },
        });
        if include_markdown {
            let markdown = bundle.render_markdown(&report.graph);
            if markdown.chars().count() > max_chars as usize {
                return Err(format!(
                    "rendered documentation bundle is {} characters, over max_markdown_chars {max_chars}; raise the limit or request metadata only",
                    markdown.chars().count()
                ));
            }
            result["markdown"] = json!(markdown);
        }
        Ok(result)
    }

    pub(super) fn telemetry_project(&self, arguments: &Value) -> Result<Value, String> {
        let raw_event = arguments
            .get("event")
            .cloned()
            .ok_or("event is required and must be a serialized DomainEvent")?;
        let raw_policy = arguments
            .get("policy")
            .cloned()
            .ok_or("policy is required and must be a serialized RedactionPolicy")?;
        let trace = arguments
            .get("trace")
            .and_then(Value::as_str)
            .ok_or("trace is required and must be a string")?;
        let encoded = serde_json::to_vec(&json!({
            "event": raw_event,
            "policy": raw_policy,
            "trace": trace,
            "metric": arguments.get("metric"),
            "observations": arguments.get("observations"),
        }))
        .map_err(|error| format!("cannot measure telemetry input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("telemetry input exceeds the 20000000-byte safety bound".into());
        }
        let event: DomainEvent = serde_json::from_value(raw_event)
            .map_err(|error| format!("invalid domain event: {error}"))?;
        if event.fields.len() > 10_000 {
            return Err("domain events are bounded at 10000 fields".into());
        }
        let policy: RedactionPolicy = serde_json::from_value(raw_policy)
            .map_err(|error| format!("invalid redaction policy: {error}"))?;
        let projected = match policy.project(&event, TraceId::new(trace)) {
            Ok(projected) => projected,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/telemetry-projection/0.1",
                    "stage": "telemetry_projection",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "record": Value::Null,
                    "loss": Value::Null,
                    "guarantees": [
                        "redaction denies by default when a field class has no policy treatment",
                        "unclassified fields cannot be emitted",
                        "no telemetry record is returned for an incomplete or invalid projection policy",
                    ],
                }));
            }
        };
        let metric_result = match (
            arguments.get("metric").cloned(),
            arguments.get("observations").cloned(),
        ) {
            (None, None) => Value::Null,
            (Some(raw_metric), Some(raw_observations)) => {
                let metric: MetricDefinition = serde_json::from_value(raw_metric)
                    .map_err(|error| format!("invalid metric definition: {error}"))?;
                let observations: TelemetryObservations = serde_json::from_value(raw_observations)
                    .map_err(|error| format!("invalid telemetry observations: {error}"))?;
                match metric.evaluate(&observations) {
                    Ok(value) => json!({
                        "ok": true,
                        "value": value,
                        "audit_statement": telemetry_audit_statement("mcp", &value),
                    }),
                    Err(error) => json!({
                        "ok": false,
                        "refusal": error.to_string(),
                        "asserted_signals": observations
                            .asserted_signals()
                            .into_iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>(),
                        "observed_sample_count": observations.len(),
                    }),
                }
            }
            (Some(_), None) => {
                return Err("observations is required when metric is supplied".into());
            }
            (None, Some(_)) => {
                return Err("metric is required when observations is supplied".into());
            }
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/telemetry-projection/0.1",
            "event_id": projected.record.event_id(),
            "event_kind": projected.record.kind(),
            "trace": projected.record.trace(),
            "policy_version": projected.record.policy(),
            "record": projected.record,
            "loss": projected.loss,
            "lossless": projected.loss.is_lossless(),
            "metric": metric_result,
            "guarantees": [
                "telemetry is a one-way projection of the canonical DomainEvent",
                "semantic loss is returned beside every projected record",
                "metric values exist only when every input signal is observed; asserted samples remain visible but do not support a value",
                "the call performs no OTLP export, backend write, sampling, span creation, clock read, or network operation",
            ],
        }))
    }

    pub(super) fn repository_impact(&self, arguments: &Value) -> Result<Value, String> {
        let changed = arguments
            .get("changed")
            .and_then(Value::as_str)
            .ok_or("changed is required and must be a repository module id")?;
        let changed = ModuleId::parse(changed.to_string()).map_err(|error| error.to_string())?;
        let raw_routes = if let Some(raw) = arguments.get("routes") {
            raw.as_array()
                .ok_or("routes must be an array of serialized TaskRoute values")?
                .clone()
        } else if let Some(raw) = arguments.get("route") {
            vec![raw.clone()]
        } else {
            Vec::new()
        };
        if raw_routes.len() > 1_000 {
            return Err("routes are bounded at 1000 entries".into());
        }
        let routes = raw_routes
            .into_iter()
            .map(|raw| {
                serde_json::from_value::<TaskRoute>(raw)
                    .map_err(|error| format!("invalid documentation route: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let report = self.scan_repository()?;
        if !report.graph.contains(&changed) {
            return Err(format!(
                "changed module {:?} is not in the scanned repository",
                changed
            ));
        }
        let impact = impact_of(&report.graph, &changed, &routes);
        let closure_count = impact.closure().len();
        let affected_module_count = impact.affected.len();
        let affected_route_count = impact.affected_routes.len();
        Ok(json!({
            "ok": true,
            "root": self.root,
            "changed": changed,
            "impact": impact,
            "closure_count": closure_count,
            "affected_module_count": affected_module_count,
            "affected_route_count": affected_route_count,
            "scan": {
                "files_read": report.files_read,
                "module_count": report.graph.node_count(),
                "edge_count": report.graph.edges().len(),
                "unresolved_links": report.unresolved_links,
                "unreadable_front_matter": report.unreadable_front_matter,
            },
            "guarantees": [
                "impact walks incoming dependents and preserves the typed propagation stop reasons",
                "schema- and governance-relevant downstream modules remain visible in the closure when graph edges declare them",
                "the report is conservative change impact, not a semantic diff or a claim that every affected module must change",
                "routes are only marked affected when their declared modules intersect the computed closure",
            ],
        }))
    }
}
