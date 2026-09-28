//! Domain evidence harmonization, provider normalization, and intake handlers.

use super::*;

impl Server {
    /// Harmonize explicit domain reports into a traceability artifact without interpreting the
    /// caller's claim. The catalogue checks here prevent a report row from smuggling in an
    /// undeclared source tool or domain label before the harmonization is indexed.
    pub(super) fn domain_evidence_harmonize(&self, arguments: &Value) -> Result<Value, String> {
        let harmonization = bioprism_devplat::harmonize_domain_evidence(arguments)
            .map_err(|error| format!("domain evidence harmonization refused: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let reports = harmonization
            .get("reports")
            .and_then(Value::as_array)
            .ok_or("harmonization omitted reports")?;
        let mut artifact_domains = BTreeSet::new();
        let mut parent_digests = Vec::new();
        for row in reports {
            let group_id = row
                .get("group_id")
                .and_then(Value::as_str)
                .ok_or("harmonization report row omitted group_id")?;
            let source_tool = row
                .get("source_tool")
                .and_then(Value::as_str)
                .ok_or("harmonization report row omitted source_tool")?;
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
            let domains = row
                .get("domains")
                .and_then(Value::as_array)
                .ok_or("harmonization report row omitted domains")?;
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
                artifact_domains.insert(domain.to_string());
            }
            if let Some(digest) = row.get("digest").and_then(Value::as_str) {
                parent_digests.push(digest.to_string());
            }
        }
        let subject_id = harmonization
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("harmonization omitted subject_id")?;
        let projection = self.index_artifact_projection(
            "domain_evidence_harmonization",
            subject_id,
            artifact_domains.into_iter().collect(),
            parent_digests,
            harmonization.clone(),
        );
        if projection.get("indexed") != Some(&Value::Bool(true)) {
            return Err(format!(
                "domain evidence harmonization could not be indexed: {}",
                projection
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown artifact registry error")
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
            "workflow": DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW,
            "harmonization": harmonization,
            "artifact_registry": projection,
            "catalogue_digest": catalogue.digest().to_string(),
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "all report source tools and domain labels were checked against the authoritative capability catalogue",
                "the harmonization artifact is indexed by exact JSON content digest with report digests as parents",
                "explicit support, qualification, contradiction, and context roles remain separate"
            ],
            "does_not_claim": [
                "traceability proves any claim or chooses between contradictory reports",
                "harmonization proves scientific, clinical, regulatory, publication, or release validity",
                "artifact indexing proves provenance completeness or external effect completion"
            ]
        }))
    }

    /// Apply a caller-owned structural readiness policy to reports from any capability group.
    ///
    /// The core audit deliberately knows nothing about the workspace catalogue. The MCP boundary
    /// adds that binding here, so a report cannot claim membership in a group or domain merely by
    /// spelling the label. The retained audit is content-addressed separately from this transport
    /// wrapper and remains a structural handoff, never a scientific or clinical conclusion.
    pub(super) fn domain_decision_readiness_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let audit = audit_domain_decision_readiness(arguments)
            .map_err(|error| format!("domain decision-readiness audit refused: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let reports = audit
            .pointer("/harmonization/reports")
            .and_then(Value::as_array)
            .ok_or("decision-readiness audit omitted harmonization reports")?;
        let mut artifact_domains = BTreeSet::new();
        let mut parent_digests = Vec::new();
        for row in reports {
            let group_id = row
                .get("group_id")
                .and_then(Value::as_str)
                .ok_or("decision-readiness report omitted group_id")?;
            let source_tool = row
                .get("source_tool")
                .and_then(Value::as_str)
                .ok_or("decision-readiness report omitted source_tool")?;
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
            let domains = row
                .get("domains")
                .and_then(Value::as_array)
                .ok_or("decision-readiness report omitted domains")?;
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
                artifact_domains.insert(domain.to_string());
            }
            if let Some(digest) = row.get("digest").and_then(Value::as_str) {
                parent_digests.push(digest.to_string());
            }
        }
        let subject_id = audit
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("decision-readiness audit omitted subject_id")?;
        let projection = self.index_artifact_projection(
            "domain_decision_readiness",
            subject_id,
            artifact_domains.into_iter().collect(),
            parent_digests,
            audit.clone(),
        );
        if projection.get("indexed") != Some(&Value::Bool(true)) {
            return Err(format!(
                "domain decision-readiness audit could not be indexed: {}",
                projection
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown artifact registry error")
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_DECISION_READINESS_SCHEMA_VERSION,
            "workflow": DOMAIN_DECISION_READINESS_WORKFLOW,
            "audit": audit,
            "artifact_registry": projection,
            "catalogue_digest": catalogue.digest().to_string(),
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "all report source tools and domain labels were checked against the authoritative 29-group catalogue",
                "the policy result preserves separate coverage, support, qualification, contradiction, refusal, review, and lineage states",
                "the retained audit is indexed by exact JSON content digest with source report digests as parents"
            ],
            "does_not_claim": [
                "a ready_for_human_review state proves a scientific, clinical, causal, regulatory, publication, or release conclusion",
                "catalogue membership proves that a source tool executed or that its response is authentic",
                "artifact retention proves external provenance, consent, identity, execution, or authority"
            ]
        }))
    }

    /// Query exact retained decision-readiness audits by structural state or policy result.
    ///
    /// This intentionally uses the same artifact registry that receives an audit at creation
    /// time. Querying never reruns harmonization, reinterprets a claim, or upgrades a retained
    /// `ready_for_human_review` state into scientific, clinical, release, or execution authority.
    pub(super) fn domain_decision_readiness_query(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let optional_string = |name: &str| -> Result<Option<&str>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| format!("{name} must be a non-empty string"))
                })
                .transpose()
        };
        let subject_id = optional_string("subject_id")?;
        let decision_state = optional_string("decision_state")?;
        let after = optional_string("after")?;
        let policy_satisfied = arguments
            .get("policy_satisfied")
            .map(|value| value.as_bool().ok_or("policy_satisfied must be a boolean"))
            .transpose()?;
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
        let include_audits = arguments
            .get("include_audits")
            .map(|value| value.as_bool().ok_or("include_audits must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        self.artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .domain_decision_readiness_query(
                subject_id,
                decision_state,
                policy_satisfied,
                after,
                max_items,
                include_audits,
            )
            .map_err(|error| format!("domain decision-readiness query refused: {error}"))
    }

    /// Query retained harmonization artifacts as a cross-domain observability surface.
    ///
    /// The registry is content-addressed and ordered by digest, so this route exposes bounded
    /// cursoring without reinterpreting any claim. Rows summarize traceability, bridge classes,
    /// contradiction posture, and lineage; full harmonization bodies remain available through the
    /// exact artifact digest when a caller explicitly asks for them.
    pub(super) fn domain_evidence_harmonization_coverage(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        const MAX_ITEMS: usize = bioprism_devplat::MAX_DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_ITEMS;
        let subject_filter = arguments.get("subject_id").and_then(Value::as_str);
        let domain_filter = arguments.get("domain").and_then(Value::as_str);
        let report_class_filter = arguments.get("report_class").and_then(Value::as_str);
        let bridge_mode_filter = arguments.get("bridge_mode").and_then(Value::as_str);
        let traceability_filter = arguments.get("traceability_state").and_then(Value::as_str);
        let after = arguments.get("after").and_then(Value::as_str);
        let max_items = arguments
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_items must be an integer".to_string())
                    .and_then(|value| {
                        usize::try_from(value).map_err(|_| "max_items is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(100);
        if !(1..=MAX_ITEMS).contains(&max_items) {
            return Err(format!("max_items must be between 1 and {MAX_ITEMS}"));
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
        for (name, value) in [
            ("subject_id", subject_filter),
            ("domain", domain_filter),
            ("report_class", report_class_filter),
            ("bridge_mode", bridge_mode_filter),
            ("traceability_state", traceability_filter),
            ("after", after),
        ] {
            if value.is_some_and(str::is_empty) {
                return Err(format!("{name} must be non-empty when supplied"));
            }
        }
        if let Some(after) = after {
            bioprism_ids::ContentHash::parse(after.to_string())
                .map_err(|_| "after must be a lowercase SHA-256 content digest".to_string())?;
        }
        if let Some(state) = traceability_filter {
            if !["complete", "requirements_missing", "links_missing"].contains(&state) {
                return Err(
                    "traceability_state must be complete, requirements_missing, or links_missing"
                        .into(),
                );
            }
        }
        let records = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .records_for_audit();
        let mut all_rows = Vec::new();
        let mut traceability_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut report_class_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut bridge_mode_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut domain_counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut subject_ids = BTreeSet::new();
        let mut contradiction_count = 0usize;
        let mut qualification_count = 0usize;
        let mut parent_digest_count = 0usize;
        let mut reports_with_lineage_parents = 0usize;
        let mut reports_without_lineage_parents = 0usize;
        for record in records.iter().filter(|record| {
            record.kind == "domain_evidence_harmonization"
                && record.artifact.get("schema").and_then(Value::as_str)
                    == Some(bioprism_devplat::DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION)
                && after.is_none_or(|cursor| record.content_digest.as_str() > cursor)
        }) {
            if subject_filter.is_some_and(|filter| record.subject_id != filter) {
                continue;
            }
            let artifact = &record.artifact;
            let traceability_state = artifact
                .pointer("/coverage/traceability_state")
                .and_then(Value::as_str)
                .unwrap_or("links_missing");
            if traceability_filter.is_some_and(|filter| traceability_state != filter) {
                continue;
            }
            let report_rows = artifact
                .get("reports")
                .and_then(Value::as_array)
                .ok_or("retained harmonization omitted reports")?;
            let matches_domain = domain_filter.is_none_or(|filter| {
                report_rows.iter().any(|row| {
                    row.get("domains")
                        .and_then(Value::as_array)
                        .is_some_and(|domains| {
                            domains
                                .iter()
                                .filter_map(Value::as_str)
                                .any(|domain| domain.eq_ignore_ascii_case(filter))
                        })
                })
            });
            if !matches_domain {
                continue;
            }
            let matches_class = report_class_filter.is_none_or(|filter| {
                report_rows
                    .iter()
                    .any(|row| row.get("report_class").and_then(Value::as_str) == Some(filter))
            });
            if !matches_class {
                continue;
            }
            let matches_mode = bridge_mode_filter.is_none_or(|filter| {
                report_rows
                    .iter()
                    .any(|row| row.get("bridge_mode").and_then(Value::as_str) == Some(filter))
            });
            if !matches_mode {
                continue;
            }
            let bridge_summary = artifact
                .pointer("/coverage/bridge_summary")
                .and_then(Value::as_object);
            let report_classes = bridge_summary
                .and_then(|summary| summary.get("report_classes"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            let bridge_modes = bridge_summary
                .and_then(|summary| summary.get("modes"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            for (class, count) in report_classes.as_object().into_iter().flatten() {
                let count = count.as_u64().unwrap_or(0) as usize;
                *report_class_counts.entry(class.clone()).or_default() += count;
            }
            for (mode, count) in bridge_modes.as_object().into_iter().flatten() {
                let count = count.as_u64().unwrap_or(0) as usize;
                *bridge_mode_counts.entry(mode.clone()).or_default() += count;
            }
            let lineage = bridge_summary
                .and_then(|summary| summary.get("lineage"))
                .and_then(Value::as_object);
            let harmonization_parent_count = record.parent_digests.len();
            let report_parent_count = lineage
                .and_then(|lineage| lineage.get("parent_digest_count"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            parent_digest_count += harmonization_parent_count;
            reports_with_lineage_parents += lineage
                .and_then(|lineage| lineage.get("reports_with_lineage_parents"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            reports_without_lineage_parents += lineage
                .and_then(|lineage| lineage.get("reports_without_lineage_parents"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let posture = artifact.get("posture").and_then(Value::as_object);
            let has_contradiction = posture
                .and_then(|posture| posture.get("explicit_contradiction_declared"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let has_qualification = posture
                .and_then(|posture| posture.get("qualification_declared"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            contradiction_count += usize::from(has_contradiction);
            qualification_count += usize::from(has_qualification);
            *traceability_counts
                .entry(traceability_state.to_string())
                .or_default() += 1;
            subject_ids.insert(record.subject_id.clone());
            let report_digests = report_rows
                .iter()
                .filter_map(|row| row.get("digest").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>();
            for report in report_rows {
                for domain in report
                    .get("domains")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    let summary = domain_counts.entry(domain.to_string()).or_default();
                    summary.0 += 1;
                    summary.1 += 1;
                }
            }
            let mut row = json!({
                "content_digest": record.content_digest,
                "subject_id": record.subject_id,
                "domains": record.domains,
                "claim_id": artifact.pointer("/claim/id"),
                "report_count": artifact.get("report_count"),
                "link_count": artifact.pointer("/links").and_then(Value::as_array).map(Vec::len),
                "traceability_state": traceability_state,
                "requirements_complete": artifact.pointer("/coverage/requirements_complete"),
                "all_reports_linked": artifact.pointer("/coverage/all_reports_linked"),
                "contradiction_declared": has_contradiction,
                "qualification_declared": has_qualification,
                "report_classes": report_classes,
                "bridge_modes": bridge_modes,
                "lineage": {
                    "harmonization_parent_count": harmonization_parent_count,
                    "report_parent_digest_count": report_parent_count,
                    "reports_with_lineage_parents": lineage
                        .and_then(|lineage| lineage.get("reports_with_lineage_parents")),
                    "reports_without_lineage_parents": lineage
                        .and_then(|lineage| lineage.get("reports_without_lineage_parents"))
                },
                "missing_group_ids": artifact.get("missing_group_ids"),
                "missing_domains": artifact.get("missing_domains")
            });
            if include_report_digests {
                row["report_digests"] = json!(report_digests);
            }
            all_rows.push((record.content_digest.clone(), row));
        }
        let matching_count = all_rows.len();
        let has_more = matching_count > max_items;
        let rows = all_rows
            .into_iter()
            .take(max_items)
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("content_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        let domain_summary = domain_counts
            .into_iter()
            .map(|(domain, (harmonization_count, report_count))| {
                (
                    domain,
                    json!({
                        "harmonization_count": harmonization_count,
                        "report_count": report_count
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let mut result = json!({
            "ok": true,
            "schema": bioprism_devplat::DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_SCHEMA_VERSION,
            "workflow": bioprism_devplat::DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_WORKFLOW,
            "filters": {
                "subject_id": subject_filter,
                "domain": domain_filter,
                "report_class": report_class_filter,
                "bridge_mode": bridge_mode_filter,
                "traceability_state": traceability_filter,
                "after": after,
                "max_items": max_items,
                "include_report_digests": include_report_digests
            },
            "registry_size": records.len(),
            "matching_count": matching_count,
            "returned_count": rows.len(),
            "has_more": has_more,
            "next_after": next_after,
            "rows": rows,
            "summary": {
                "subject_count": subject_ids.len(),
                "traceability_states": traceability_counts,
                "report_classes": report_class_counts,
                "bridge_modes": bridge_mode_counts,
                "lineage": {
                    "harmonization_parent_digest_count": parent_digest_count,
                    "reports_with_lineage_parents": reports_with_lineage_parents,
                    "reports_without_lineage_parents": reports_without_lineage_parents
                },
                "posture": {
                    "harmonizations_with_contradictions": contradiction_count,
                    "harmonizations_with_qualifications": qualification_count
                },
                "domain_summary": domain_summary
            },
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "rows are bounded, digest-ordered retained harmonization summaries",
                "filters match explicit subject, domain, bridge, and traceability fields only",
                "full claim bodies remain behind exact artifact digests and are not interpreted"
            ],
            "does_not_claim": [
                "indexed harmonization means the joined claim is true or scientifically valid",
                "complete traceability means every domain, source, or provenance obligation is satisfied",
                "absence from this bounded registry means an harmonization never existed"
            ]
        });
        let coverage_digest = bioprism_ids::ContentHash::of_value(&result)
            .map_err(|error| format!("harmonization coverage could not be hashed: {error}"))?;
        result["coverage_digest"] = json!(coverage_digest.to_string());
        Ok(result)
    }

    /// Plan a caller-managed external evidence connector without fetching or executing it.
    pub(super) fn domain_evidence_source_plan(&self, arguments: &Value) -> Result<Value, String> {
        let plan = plan_domain_evidence_source(arguments)
            .map_err(|error| format!("domain evidence source plan refused: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let group_id = plan
            .get("group_id")
            .and_then(Value::as_str)
            .ok_or("domain evidence source plan omitted group_id")?;
        let group = catalogue
            .groups()
            .iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("unknown capability group {group_id:?}"))?;
        if let Some(source_tool) = plan.get("source_tool").and_then(Value::as_str) {
            if !group.mcp_tools.iter().any(|tool| tool == source_tool) {
                return Err(format!(
                    "source_tool {source_tool:?} is not declared by capability group {group_id:?}"
                ));
            }
        }
        let domains = plan
            .get("domains")
            .and_then(Value::as_array)
            .ok_or("domain evidence source plan omitted domains")?;
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
        let subject_id = plan
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("domain evidence source plan omitted subject_id")?;
        let parent_digests = plan
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
            "domain_evidence_source_plan",
            subject_id,
            domains
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            parent_digests,
            plan.clone(),
        );
        if projection.get("indexed") != Some(&Value::Bool(true)) {
            return Err(format!(
                "domain evidence source plan could not be indexed: {}",
                projection
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown artifact registry error")
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
            "workflow": DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW,
            "plan_digest": plan.get("plan_digest"),
            "group_id": plan.get("group_id"),
            "domains": plan.get("domains"),
            "subject_id": plan.get("subject_id"),
            "source_tool": plan.get("source_tool"),
            "connector_kind": plan.get("connector_kind"),
            "locator_kind": plan.get("locator_kind"),
            "locator": plan.get("locator"),
            "retrieval_mode": plan.get("retrieval_mode"),
            "expected_content_digest": plan.get("expected_content_digest"),
            "parent_digests": plan.get("parent_digests"),
            "retrieval_policy": plan.get("retrieval_policy"),
            "plan": plan,
            "artifact_registry": projection,
            "catalogue_digest": catalogue.digest().to_string(),
            "readiness_claimed": false,
            "execution": "not_started",
            "retrieval_status": "not_started",
            "guarantees": [
                "connector kind, locator shape, policy, group, and domain membership were structurally checked",
                "the exact source plan is indexed and its plan digest can parent later caller-controlled intake",
                "credentials remain caller-managed and no external connector is invoked"
            ],
            "does_not_claim": [
                "the locator exists, was reachable, or identifies an authentic source",
                "a planned retrieval occurred or that future bytes will match the expected digest",
                "planning establishes scientific, clinical, causal, provenance, regulatory, or release validity"
            ]
        }))
    }

    /// Execute one retained source plan through the bounded in-process connector kernel.
    ///
    /// Execution is intentionally separate from planning and intake. The plan is looked up by
    /// its declared digest, the current catalogue is checked again, the connector returns an
    /// explicit observed/partial/refused/error outcome, and only then is that response passed
    /// through the ordinary intake path. This keeps a successful file or HTTP read from becoming
    /// a scientific or provenance claim merely because it was reachable.
    pub(super) fn domain_evidence_source_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let source_plan_digest = arguments
            .get("source_plan_digest")
            .and_then(Value::as_str)
            .ok_or("source_plan_digest is required")?;
        let plan_record = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .records_for_audit()
            .into_iter()
            .find(|record| {
                record.kind == "domain_evidence_source_plan"
                    && record.artifact.get("plan_digest").and_then(Value::as_str)
                        == Some(source_plan_digest)
            })
            .ok_or_else(|| {
                format!("source_plan_digest {source_plan_digest:?} is not a retained source plan")
            })?;
        let plan = plan_record.artifact;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let group_id = plan
            .get("group_id")
            .and_then(Value::as_str)
            .ok_or("retained source plan omitted group_id")?;
        let group = catalogue
            .groups()
            .iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("unknown capability group {group_id:?}"))?;
        let domains = plan
            .get("domains")
            .and_then(Value::as_array)
            .ok_or("retained source plan omitted domains")?;
        for domain in domains.iter().filter_map(Value::as_str) {
            if !group
                .domains
                .iter()
                .any(|declared| declared.eq_ignore_ascii_case(domain))
            {
                return Err(format!(
                    "retained source plan domain {domain:?} is not declared by capability group {group_id:?}"
                ));
            }
        }
        let source_tool = arguments
            .get("source_tool")
            .and_then(Value::as_str)
            .or_else(|| plan.get("source_tool").and_then(Value::as_str))
            .ok_or("source_tool is required when the retained source plan has no source_tool")?;
        if !group.mcp_tools.iter().any(|tool| tool == source_tool) {
            return Err(format!(
                "source_tool {source_tool:?} is not declared by capability group {group_id:?}"
            ));
        }
        if plan
            .get("source_tool")
            .and_then(Value::as_str)
            .is_some_and(|planned| planned != source_tool)
        {
            return Err("execution source_tool does not match retained source plan".into());
        }
        let execution = execute_domain_evidence_source(self.root(), &plan)
            .map_err(|error| format!("domain evidence source execution refused: {error}"))?;
        let request = arguments.get("request").cloned().unwrap_or_else(|| {
            json!({
                "connector_kind": plan.get("connector_kind"),
                "locator_kind": plan.get("locator_kind"),
                "retrieval_mode": plan.get("retrieval_mode"),
                "execution": DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW
            })
        });
        let claim_posture = arguments.get("claim_posture").cloned().unwrap_or_else(|| {
            json!({
                "status": "review_required",
                "does_not_claim": [
                    "source authenticity or scientific truth",
                    "clinical, regulatory, causal, or release validity",
                    "provenance completeness or external authorization"
                ],
                "limitations": [
                    "bounded connector output is retained as caller-declared evidence only"
                ]
            })
        });
        let mut parent_digests = arguments
            .get("parent_digests")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let parents = parent_digests
            .as_array_mut()
            .ok_or("parent_digests must be an array when supplied")?;
        if !parents
            .iter()
            .any(|value| value.as_str() == Some(plan_record.content_digest.as_str()))
        {
            parents.push(json!(plan_record.content_digest));
        }
        let intake_arguments = json!({
            "group_id": plan.get("group_id"),
            "domains": plan.get("domains"),
            "subject_id": plan.get("subject_id"),
            "source_tool": source_tool,
            "request": request,
            "response": execution.get("response"),
            "outcome": execution.get("outcome"),
            "claim_posture": claim_posture,
            "source_plan_digest": source_plan_digest,
            "parent_digests": parents
        });
        let intake = self.domain_evidence_intake(&intake_arguments)?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA_VERSION,
            "workflow": DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW,
            "source_plan_digest": source_plan_digest,
            "group_id": plan.get("group_id"),
            "domains": plan.get("domains"),
            "subject_id": plan.get("subject_id"),
            "source_tool": source_tool,
            "outcome": execution.get("outcome"),
            "retrieval_status": execution.get("retrieval_status"),
            "execution": execution.get("execution"),
            "raw_content_digest": execution.get("raw_content_digest"),
            "response_digest": execution.get("response_digest"),
            "byte_length": execution.get("byte_length"),
            "content_type": execution.get("content_type"),
            "source_plan": plan,
            "execution_result": execution,
            "intake": intake,
            "artifact_registry": intake.get("artifact_registry"),
            "catalogue_digest": catalogue.digest().to_string(),
            "readiness_claimed": false,
            "guarantees": [
                "the retained plan and current capability catalogue were checked before connector execution",
                "the connector response carries separate raw-byte and canonical-JSON digests",
                "the response is indexed through domain_evidence_intake with the source-plan digest and exact plan artifact as parents"
            ],
            "does_not_claim": [
                "a successful read proves source authenticity, scientific validity, clinical validity, or provenance completeness",
                "the named source_tool was executed or interpreted by this connector",
                "a refused, error, or partial transport outcome is equivalent to an observed result"
            ]
        }))
    }

    /// Intake one raw request/response envelope and bind it to the authoritative catalogue.
    /// Intake is deliberately separate from execution: callers may submit a retained tool
    /// response, a refusal, or an externally produced envelope, but this operation never calls
    /// the named source tool on their behalf.
    pub(super) fn domain_evidence_intake(&self, arguments: &Value) -> Result<Value, String> {
        let mut expected_content_digest = None;
        let mut source_plan_content_digest = None;
        if let Some(source_plan_digest) =
            arguments.get("source_plan_digest").and_then(Value::as_str)
        {
            let group_id = arguments
                .get("group_id")
                .and_then(Value::as_str)
                .ok_or("source-plan-bound intake omitted group_id")?;
            let subject_id = arguments
                .get("subject_id")
                .and_then(Value::as_str)
                .ok_or("source-plan-bound intake omitted subject_id")?;
            let source_tool = arguments
                .get("source_tool")
                .and_then(Value::as_str)
                .ok_or("source-plan-bound intake omitted source_tool")?;
            let domains = arguments
                .get("domains")
                .and_then(Value::as_array)
                .ok_or("source-plan-bound intake omitted domains")?;
            let records = self
                .artifact_registry
                .lock()
                .map_err(|_| "artifact registry lock is poisoned".to_string())?
                .records_for_audit();
            let plan = records
                .iter()
                .find(|record| {
                    record.kind == "domain_evidence_source_plan"
                        && record.artifact.get("plan_digest").and_then(Value::as_str)
                            == Some(source_plan_digest)
                })
                .ok_or_else(|| {
                    format!(
                        "source_plan_digest {source_plan_digest:?} is not a retained source plan"
                    )
                })?;
            if plan.artifact.get("group_id").and_then(Value::as_str) != Some(group_id)
                || plan.artifact.get("subject_id").and_then(Value::as_str) != Some(subject_id)
            {
                return Err("source plan group_id or subject_id does not match intake".into());
            }
            if plan
                .artifact
                .get("source_tool")
                .and_then(Value::as_str)
                .is_some_and(|planned_tool| planned_tool != source_tool)
            {
                return Err("source plan source_tool does not match intake".into());
            }
            let planned_domains = plan
                .artifact
                .get("domains")
                .and_then(Value::as_array)
                .ok_or("retained source plan omitted domains")?;
            for domain in domains.iter().filter_map(Value::as_str) {
                if !planned_domains
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|planned| planned.eq_ignore_ascii_case(domain))
                {
                    return Err(format!(
                        "source plan does not cover intake domain {domain:?}"
                    ));
                }
            }
            expected_content_digest = plan
                .artifact
                .get("expected_content_digest")
                .and_then(Value::as_str)
                .map(str::to_string);
            source_plan_content_digest = Some(plan.content_digest.clone());
        }
        let mut intake_arguments = arguments.clone();
        if let Some(source_plan_content_digest) = source_plan_content_digest {
            let parents = intake_arguments
                .as_object_mut()
                .ok_or("domain evidence intake arguments must be an object")?
                .entry("parent_digests")
                .or_insert_with(|| json!([]));
            let parents = parents
                .as_array_mut()
                .ok_or("parent_digests must be an array")?;
            if !parents
                .iter()
                .any(|value| value.as_str() == Some(source_plan_content_digest.as_str()))
            {
                parents.push(json!(source_plan_content_digest));
            }
        }
        let intake = bioprism_devplat::intake_domain_evidence(&intake_arguments)
            .map_err(|error| format!("domain evidence intake refused: {error}"))?;
        if let Some(expected_content_digest) = expected_content_digest {
            let outcome = intake.get("outcome").and_then(Value::as_str);
            if matches!(outcome, Some("observed" | "partial"))
                && intake.get("response_digest").and_then(Value::as_str)
                    != Some(expected_content_digest.as_str())
            {
                return Err(format!(
                    "source plan expected response digest {expected_content_digest}, but intake response digest differs"
                ));
            }
        }
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("workspace capability catalogue is invalid: {error}"))?;
        let group_id = intake
            .get("group_id")
            .and_then(Value::as_str)
            .ok_or("domain evidence intake omitted group_id")?;
        let source_tool = intake
            .get("source_tool")
            .and_then(Value::as_str)
            .ok_or("domain evidence intake omitted source_tool")?;
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
        let domains = intake
            .get("domains")
            .and_then(Value::as_array)
            .ok_or("domain evidence intake omitted domains")?;
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
        let subject_id = intake
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or("domain evidence intake omitted subject_id")?;
        let parent_digests = intake
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
            "domain_evidence_intake",
            subject_id,
            domains
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            parent_digests,
            intake.clone(),
        );
        if projection.get("indexed") != Some(&Value::Bool(true)) {
            return Err(format!(
                "domain evidence intake could not be indexed: {}",
                projection
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown artifact registry error")
            ));
        }
        let report = intake
            .get("report")
            .cloned()
            .ok_or("domain evidence intake omitted canonical report")?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION,
            "workflow": DOMAIN_EVIDENCE_INTAKE_WORKFLOW,
            "group_id": intake.get("group_id"),
            "domains": intake.get("domains"),
            "subject_id": intake.get("subject_id"),
            "source_tool": intake.get("source_tool"),
            "request_supplied": intake.get("request_supplied"),
            "request_digest": intake.get("request_digest"),
            "response_digest": intake.get("response_digest"),
            "intake_digest": intake.get("intake_digest"),
            "outcome": intake.get("outcome"),
            "source_plan_digest": intake.get("source_plan_digest"),
            "parent_digests": intake.get("parent_digests"),
            "report": report,
            "intake": intake,
            "artifact_registry": projection,
            "catalogue_digest": catalogue.digest().to_string(),
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "the source tool and domains were checked against the authoritative workspace capability catalogue",
                "request and response JSON remain recoverable through the indexed canonical intake artifact",
                "the intake records a supplied envelope and never executes the named source tool"
            ],
            "does_not_claim": [
                "intake proves that a source tool was executed or that its response is true",
                "catalogue membership or exact digests prove scientific, clinical, causal, provenance, or release validity",
                "a refusal or partial response is silently equivalent to a successful observation"
            ]
        }))
    }

    /// Project retained intake artifacts against the authoritative catalogue without treating
    /// presence as execution coverage. Group rows retain subjects, source tools, outcomes, and
    /// exact artifact digests; missing groups and domain-level gaps remain explicit.
    pub(super) fn domain_evidence_coverage(&self, arguments: &Value) -> Result<Value, String> {
        const MAX_GROUPS: usize = 128;
        let group_filter = arguments.get("group_id").and_then(Value::as_str);
        let domain_filter = arguments.get("domain").and_then(Value::as_str);
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
        let include_intake_digests = arguments
            .get("include_intake_digests")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| "include_intake_digests must be a boolean".to_string())
            })
            .transpose()?
            .unwrap_or(false);
        if group_filter.is_some_and(str::is_empty) || domain_filter.is_some_and(str::is_empty) {
            return Err("group_id and domain filters must be non-empty".into());
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
        let artifact_registry = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?;
        let artifact_registry_generation = artifact_registry.generation();
        let artifact_registry_size = artifact_registry.len();
        let records = artifact_registry.records_for_audit();
        let mut group_intakes: BTreeMap<String, Vec<&bioprism_devplat::ArtifactRecord>> =
            BTreeMap::new();
        for record in &records {
            if record.kind != "domain_evidence_intake"
                || record.artifact.get("schema").and_then(Value::as_str)
                    != Some(bioprism_devplat::DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION)
            {
                continue;
            }
            let Some(group_id) = record.artifact.get("group_id").and_then(Value::as_str) else {
                continue;
            };
            if selected_ids.contains(group_id) {
                group_intakes
                    .entry(group_id.to_string())
                    .or_default()
                    .push(record);
            }
        }

        let mut groups = Vec::new();
        let mut missing_group_ids = Vec::new();
        let mut missing_tool_group_ids = Vec::new();
        let mut missing_domain_group_ids = Vec::new();
        let mut domain_summary: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
        let mut groups_with_artifact_evidence = 0usize;
        let mut artifact_evidence_records = 0usize;
        for group in selected {
            let intakes = group_intakes.get(&group.id).cloned().unwrap_or_default();
            let artifact_evidence =
                artifact_registry.domain_evidence_posture(&group.id, &group.domains);
            let matching_artifact_records = artifact_evidence
                .get("matching_record_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if matching_artifact_records > 0 {
                groups_with_artifact_evidence += 1;
                artifact_evidence_records =
                    artifact_evidence_records.saturating_add(matching_artifact_records);
            }
            if intakes.is_empty() {
                missing_group_ids.push(group.id.clone());
            }
            let mut subject_ids = BTreeSet::new();
            let mut source_tools = BTreeSet::new();
            let mut outcomes = BTreeSet::new();
            let mut reported_domains = BTreeSet::new();
            let mut intake_digests = BTreeSet::new();
            let mut tool_intakes: BTreeMap<String, Vec<&bioprism_devplat::ArtifactRecord>> =
                BTreeMap::new();
            for record in &intakes {
                subject_ids.insert(record.subject_id.clone());
                if let Some(source_tool) =
                    record.artifact.get("source_tool").and_then(Value::as_str)
                {
                    source_tools.insert(source_tool.to_string());
                    tool_intakes
                        .entry(source_tool.to_string())
                        .or_default()
                        .push(record);
                }
                if let Some(outcome) = record.artifact.get("outcome").and_then(Value::as_str) {
                    outcomes.insert(outcome.to_string());
                }
                if let Some(domains) = record.artifact.get("domains").and_then(Value::as_array) {
                    reported_domains
                        .extend(domains.iter().filter_map(Value::as_str).map(str::to_string));
                }
                intake_digests.insert(record.content_digest.clone());
            }
            for domain in &group.domains {
                let domain_intakes = intakes
                    .iter()
                    .filter(|record| {
                        record
                            .artifact
                            .get("domains")
                            .and_then(Value::as_array)
                            .is_some_and(|domains| {
                                domains
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .any(|candidate| candidate.eq_ignore_ascii_case(domain))
                            })
                    })
                    .count();
                let summary = domain_summary.entry(domain.clone()).or_default();
                summary.0 += 1;
                if domain_intakes > 0 {
                    summary.1 += 1;
                }
                summary.2 += domain_intakes;
            }
            let missing_domains = group
                .domains
                .iter()
                .filter(|domain| {
                    !reported_domains
                        .iter()
                        .any(|reported| reported.eq_ignore_ascii_case(domain))
                })
                .cloned()
                .collect::<Vec<_>>();
            let missing_source_tools = group
                .mcp_tools
                .iter()
                .filter(|tool| !tool_intakes.contains_key(*tool))
                .cloned()
                .collect::<Vec<_>>();
            let source_tool_coverage = group
                .mcp_tools
                .iter()
                .map(|tool| {
                    let rows = tool_intakes.get(tool);
                    let tool_outcomes = rows
                        .into_iter()
                        .flat_map(|records| records.iter())
                        .filter_map(|record| record.artifact.get("outcome").and_then(Value::as_str))
                        .collect::<BTreeSet<_>>();
                    json!({
                        "tool": tool,
                        "intake_count": rows.map_or(0, Vec::len),
                        "outcomes": tool_outcomes.into_iter().collect::<Vec<_>>(),
                        "coverage_state": if rows.is_some() { "reported" } else { "missing" }
                    })
                })
                .collect::<Vec<_>>();
            let tool_coverage_state = if missing_source_tools.is_empty() {
                "complete"
            } else if tool_intakes.is_empty() {
                "missing"
            } else {
                "partial"
            };
            let domain_coverage_state = if missing_domains.is_empty() {
                "complete"
            } else if reported_domains.is_empty() {
                "missing"
            } else {
                "partial"
            };
            if !missing_source_tools.is_empty() {
                missing_tool_group_ids.push(group.id.clone());
            }
            if !missing_domains.is_empty() {
                missing_domain_group_ids.push(group.id.clone());
            }
            let mut row = json!({
                "id": group.id,
                "domains": group.domains,
                "status": group.status,
                "declared_tool_count": group.mcp_tools.len(),
                "declared_tools": group.mcp_tools,
                "intake_count": intakes.len(),
                "subject_ids": subject_ids.into_iter().collect::<Vec<_>>(),
                "source_tools": source_tools.into_iter().collect::<Vec<_>>(),
                "outcomes": outcomes.into_iter().collect::<Vec<_>>(),
                "reported_domains": reported_domains.into_iter().collect::<Vec<_>>(),
                "missing_source_tools": missing_source_tools,
                "source_tool_coverage": source_tool_coverage,
                "missing_domains": missing_domains,
                "tool_coverage_state": tool_coverage_state,
                "domain_coverage_state": domain_coverage_state,
                "coverage_state": if intakes.is_empty() { "missing" } else { "reported" },
                "artifact_evidence": artifact_evidence,
                "artifact_evidence_scope": "current_digest_verified_artifact_registry_exact_declared_matches"
            });
            if include_intake_digests {
                row["intake_digests"] = json!(intake_digests.into_iter().collect::<Vec<_>>());
            }
            groups.push(row);
        }
        let domain_summary = domain_summary
            .into_iter()
            .map(
                |(domain, (group_count, reported_group_count, intake_count))| {
                    (
                        domain,
                        json!({
                            "group_count": group_count,
                            "reported_group_count": reported_group_count,
                            "missing_group_count": group_count.saturating_sub(reported_group_count),
                            "intake_count": intake_count
                        }),
                    )
                },
            )
            .collect::<serde_json::Map<_, _>>();
        let mut result = json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_INTAKE_COVERAGE_SCHEMA_VERSION,
            "workflow": DOMAIN_EVIDENCE_INTAKE_COVERAGE_WORKFLOW,
            "catalogue_digest": catalogue.digest().to_string(),
            "filters": {
                "group_id": group_filter,
                "domain": domain_filter,
                "max_groups": max_groups,
                "include_intake_digests": include_intake_digests
            },
            "group_count": groups.len(),
            "reported_group_count": groups.iter().filter(|group| group["coverage_state"] == "reported").count(),
            "missing_group_count": missing_group_ids.len(),
            "missing_group_ids": missing_group_ids,
            "complete": missing_group_ids.is_empty(),
            "tool_coverage_complete": missing_tool_group_ids.is_empty(),
            "missing_tool_group_ids": missing_tool_group_ids,
            "domain_coverage_complete": missing_domain_group_ids.is_empty(),
            "missing_domain_group_ids": missing_domain_group_ids,
            "groups_with_artifact_evidence": groups_with_artifact_evidence,
            "artifact_evidence_records": artifact_evidence_records,
            "artifact_registry_generation": artifact_registry_generation,
            "artifact_registry_size": artifact_registry_size,
            "artifact_evidence_scope": "current_digest_verified_artifact_registry_exact_declared_matches",
            "groups": groups,
            "domain_summary": domain_summary,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "coverage counts only retained, structurally verified domain-evidence-intake artifacts",
                "group, domain, outcome, subject, source-tool, and digest rows remain separately inspectable",
                "declared source-tool and domain gaps remain explicit instead of being hidden by one intake",
                "missing intake remains visible instead of being inferred as absent capability",
                "artifact-family evidence is joined for every selected capability group without changing intake coverage semantics"
            ],
            "does_not_claim": [
                "intake presence proves that every tool was executed or that a response is true",
                "artifact-family presence proves that a provider, adapter, source, report, or workflow was executed",
                "complete local intake coverage proves scientific, clinical, causal, provenance, release, or readiness validity",
                "missing intake proves that a capability or external source does not exist"
            ]
        });
        let coverage_digest = bioprism_ids::ContentHash::of_value(&result).map_err(|error| {
            format!("domain evidence intake coverage could not be hashed: {error}")
        })?;
        result["coverage_digest"] = json!(coverage_digest.to_string());
        Ok(result)
    }
}
