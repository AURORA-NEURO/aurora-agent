//! MCP Domain catalogue and evidence workflow handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    /// Loads a domain pack and reports its declared surface without compiling anything.
    pub(super) fn domain_validate(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments
            .get("domain")
            .and_then(Value::as_str)
            .ok_or("domain is required (a path relative to the server root)")?;
        let path = self.resolve(relative)?;
        let raw = self.read_json(&path)?;
        let pack = DomainPack::from_json(&raw).map_err(|error| error.to_string())?;
        Ok(json!({
            "ok": true,
            "name": pack.name(),
            "description": pack.description(),
            "oracle_kind": pack.oracle().kind(),
            "required_variables": pack.oracle().required_variables(),
            "checks": pack
                .oracle()
                .checks()
                .iter()
                .map(|check| json!({ "name": check.name, "description": check.description }))
                .collect::<Vec<_>>(),
            "protected_tags": pack.protected_tags(),
            "scope_dimensions_declared": raw.get("scope_dimensions").is_some(),
        }))
    }

    /// Project one explicitly requested report through the shared domain-report contract.
    ///
    /// The catalogue check is intentionally performed at the transport boundary: a structurally
    /// valid envelope is not enough if the named tool is not declared under the named capability
    /// group. This keeps all capability groups usable while refusing invented routes and silently
    /// broadened domain labels.
    pub(super) fn domain_report_project(&self, arguments: &Value) -> Result<Value, String> {
        let operation = arguments
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("project");
        match operation {
            "project" => self.project_domain_report(arguments),
            "coverage" => self.domain_report_coverage(arguments),
            "from_adapter_execution" => self.project_adapter_execution_domain_report(arguments),
            "from_provider_normalization" => {
                self.project_provider_normalization_domain_report(arguments)
            }
            "from_external_provider_normalization" => {
                self.project_external_provider_normalization_domain_report(arguments)
            }
            other => Err(format!(
                "unknown domain report operation {other:?}; choose project, coverage, from_adapter_execution, from_provider_normalization, or from_external_provider_normalization"
            )),
        }
    }

    pub(super) fn domain_report_coverage(&self, arguments: &Value) -> Result<Value, String> {
        const MAX_GROUPS: usize = 128;
        let group_filter = arguments.get("group_id").and_then(Value::as_str);
        let domain_filter = arguments.get("domain").and_then(Value::as_str);
        let report_class_filter = arguments.get("report_class").and_then(Value::as_str);
        let bridge_mode_filter = arguments.get("bridge_mode").and_then(Value::as_str);
        let max_groups = arguments
            .get("max_groups")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_groups must be an integer".to_string())
                    .and_then(|value| {
                        usize::try_from(value).map_err(|_| "max_groups is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(64);
        if !(1..=MAX_GROUPS).contains(&max_groups) {
            return Err(format!("max_groups must be between 1 and {MAX_GROUPS}"));
        }
        let include_report_digests = arguments
            .get("include_report_digests")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| "include_report_digests must be a boolean".to_string())
            })
            .transpose()?
            .unwrap_or(false);
        if group_filter.is_some_and(str::is_empty)
            || domain_filter.is_some_and(str::is_empty)
            || report_class_filter.is_some_and(str::is_empty)
            || bridge_mode_filter.is_some_and(str::is_empty)
        {
            return Err(
                "group_id, domain, report_class, and bridge_mode filters must be non-empty".into(),
            );
        }
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let selected = catalogue
            .groups()
            .iter()
            .filter(|group| group_filter.is_none_or(|filter| group.id == filter))
            .filter(|group| {
                domain_filter.is_none_or(|filter| {
                    group
                        .domains
                        .iter()
                        .any(|domain| domain.eq_ignore_ascii_case(filter))
                })
            })
            .take(max_groups)
            .collect::<Vec<_>>();
        let selected_ids = selected
            .iter()
            .map(|group| group.id.as_str())
            .collect::<BTreeSet<_>>();
        let records = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .records_for_audit();
        let mut group_reports: BTreeMap<String, Vec<&bioprism_devplat::ArtifactRecord>> =
            BTreeMap::new();
        for record in &records {
            if record.kind != "domain_report"
                || record.artifact.get("schema").and_then(Value::as_str)
                    != Some(DOMAIN_REPORT_SCHEMA_VERSION)
            {
                continue;
            }
            let Some(group_id) = record.artifact.get("group_id").and_then(Value::as_str) else {
                continue;
            };
            let bridge_metadata = classify_domain_report_bridge(&record.artifact);
            if report_class_filter.is_some_and(|filter| bridge_metadata.report_class != filter)
                || bridge_mode_filter
                    .is_some_and(|filter| bridge_metadata.mode.as_deref() != Some(filter))
            {
                continue;
            }
            if selected_ids.contains(group_id) {
                group_reports
                    .entry(group_id.to_string())
                    .or_default()
                    .push(record);
            }
        }
        let mut groups = Vec::new();
        let mut missing_group_ids = Vec::new();
        let mut domain_summary: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
        let mut report_class_summary: BTreeMap<String, usize> = BTreeMap::new();
        let mut bridge_parent_digest_count = 0usize;
        let mut bridge_reports_with_parents = 0usize;
        let mut bridge_reports_without_parents = 0usize;
        for group in selected {
            let reports = group_reports.get(&group.id).cloned().unwrap_or_default();
            let report_count = reports.len();
            let coverage_state = if report_count > 0 {
                "reported"
            } else {
                missing_group_ids.push(group.id.clone());
                "missing"
            };
            let mut subject_ids = BTreeSet::new();
            let mut source_tools = BTreeSet::new();
            let mut claim_statuses = BTreeSet::new();
            let mut report_digests = BTreeSet::new();
            let mut report_classes: BTreeMap<String, usize> = BTreeMap::new();
            let mut bridge_modes = BTreeSet::new();
            let mut lineage_parent_count = 0usize;
            let mut reports_with_lineage_parents = 0usize;
            for record in &reports {
                subject_ids.insert(record.subject_id.clone());
                if let Some(source_tool) =
                    record.artifact.get("source_tool").and_then(Value::as_str)
                {
                    source_tools.insert(source_tool.to_string());
                }
                if let Some(status) = record
                    .artifact
                    .pointer("/claim_posture/status")
                    .and_then(Value::as_str)
                {
                    claim_statuses.insert(status.to_string());
                }
                report_digests.insert(record.content_digest.clone());
                let bridge_metadata = classify_domain_report_bridge(&record.artifact);
                let report_class = bridge_metadata.report_class;
                let report_mode = bridge_metadata.mode;
                *report_classes.entry(report_class.to_string()).or_default() += 1;
                *report_class_summary
                    .entry(report_class.to_string())
                    .or_default() += 1;
                if let Some(mode) = report_mode {
                    bridge_modes.insert(mode.to_string());
                }
                let parent_count = record
                    .artifact
                    .get("parent_digests")
                    .and_then(Value::as_array)
                    .map(|parents| parents.len())
                    .unwrap_or(0);
                lineage_parent_count += parent_count;
                bridge_parent_digest_count += parent_count;
                if parent_count > 0 {
                    reports_with_lineage_parents += 1;
                    bridge_reports_with_parents += 1;
                } else {
                    bridge_reports_without_parents += 1;
                }
            }
            for domain in &group.domains {
                let summary = domain_summary.entry(domain.clone()).or_default();
                summary.0 += 1;
                if report_count > 0 {
                    summary.1 += 1;
                }
                summary.2 += report_count;
            }
            let mut row = json!({
                "id": group.id,
                "domains": group.domains,
                "status": group.status,
                "declared_tool_count": group.mcp_tools.len(),
                "report_count": report_count,
                "subject_ids": subject_ids.into_iter().collect::<Vec<_>>(),
                "source_tools": source_tools.into_iter().collect::<Vec<_>>(),
                "claim_statuses": claim_statuses.into_iter().collect::<Vec<_>>(),
                "report_classes": report_classes,
                "bridge_modes": bridge_modes.into_iter().collect::<Vec<_>>(),
                "lineage_parent_count": lineage_parent_count,
                "reports_with_lineage_parents": reports_with_lineage_parents,
                "coverage_state": coverage_state
            });
            if include_report_digests {
                row["report_digests"] = json!(report_digests.into_iter().collect::<Vec<_>>());
            }
            groups.push(row);
        }
        let domain_summary = domain_summary
            .into_iter()
            .map(
                |(domain, (group_count, reported_group_count, report_count))| {
                    (
                        domain,
                        json!({
                            "group_count": group_count,
                            "reported_group_count": reported_group_count,
                            "missing_group_count": group_count.saturating_sub(reported_group_count),
                            "report_count": report_count
                        }),
                    )
                },
            )
            .collect::<serde_json::Map<_, _>>();
        let mut result = json!({
            "ok": true,
            "schema": DOMAIN_REPORT_COVERAGE_SCHEMA_VERSION,
            "workflow": DOMAIN_REPORT_COVERAGE_WORKFLOW,
            "catalogue_digest": catalogue.digest().to_string(),
            "filters": {
                "group_id": group_filter,
                "domain": domain_filter,
                "report_class": report_class_filter,
                "bridge_mode": bridge_mode_filter,
                "max_groups": max_groups,
                "include_report_digests": include_report_digests
            },
            "group_count": groups.len(),
            "reported_group_count": groups.iter().filter(|group| group["coverage_state"] == "reported").count(),
            "missing_group_count": missing_group_ids.len(),
            "missing_group_ids": missing_group_ids,
            "complete": missing_group_ids.is_empty(),
            "groups": groups,
            "domain_summary": domain_summary,
            "bridge_summary": {
                "report_classes": report_class_summary,
                "lineage": {
                    "parent_digest_count": bridge_parent_digest_count,
                    "reports_with_lineage_parents": bridge_reports_with_parents,
                    "reports_without_lineage_parents": bridge_reports_without_parents
                }
            },
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "coverage counts only retained, structurally valid domain-report projections in this local artifact registry",
                "catalogue membership and report presence remain separate dimensions",
                "the projection does not infer scientific truth or readiness from report count"
            ],
            "does_not_claim": [
                "reported coverage proves every capability group was executed or validated",
                "missing coverage proves a capability is absent from the repository",
                "report count proves report quality, provenance, reproducibility, or external effect completion"
            ]
        });
        let coverage_digest = bioprism_ids::ContentHash::of_value(&result)
            .map_err(|error| format!("domain report coverage could not be hashed: {error}"))?;
        result["coverage_digest"] = json!(coverage_digest.to_string());
        Ok(result)
    }
}
