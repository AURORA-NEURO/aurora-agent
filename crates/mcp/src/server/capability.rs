//! MCP Capability discovery, audit, and dashboard handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    /// Batch multiple cross-domain needs into one digest-bound candidate report.
    ///
    /// A route is intentionally a proposal, not a mission: explicit tool filters are marked as
    /// explicit, free-text/domain/group searches remain ranked candidates, and no nested tool is
    /// called. This gives an agent one auditable hand-off point before it constructs an allow-listed
    /// `agent_mission` request with domain-appropriate arguments.
    pub(super) fn capability_route(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode capability route input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("capability route input exceeds the 20000000-byte safety bound".into());
        }
        let request: CapabilityRouteRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid capability route input: {error}"))?;
        request
            .validate()
            .map_err(|error| format!("capability route refused: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("capability catalogue refused: {error}"))?;
        let route_document = json!({
            "catalog_digest": catalogue.digest().to_string(),
            "goal": request.goal,
            "needs": request.needs,
            "max_candidates_per_need": request.max_candidates_per_need,
            "max_tools": request.max_tools,
            "include_tools": request.include_tools,
        });
        let route_id = bioprism_ids::ContentHash::of_value(&route_document)
            .map_err(|error| format!("cannot hash capability route: {error}"))?
            .to_string();

        let mut recommended = BTreeSet::new();
        let mut unresolved = Vec::new();
        let mut need_reports = Vec::new();
        let mut route_groups = BTreeSet::new();
        let mut route_group_domains = BTreeMap::<String, BTreeSet<String>>::new();
        let mut route_domains = BTreeSet::new();
        for need in request.needs {
            let mut query = need.query.clone();
            if query.query.is_none()
                && query.group_id.is_none()
                && query.domain.is_none()
                && query.tool.is_none()
            {
                query.query = Some(request.goal.clone());
            }
            query.max_items = query.max_items.min(request.max_candidates_per_need);
            query.include_tools = false;
            let search = catalogue
                .search(&query)
                .map_err(|error| format!("capability route need {:?} refused: {error}", need.id))?;
            let mut candidate_groups = BTreeSet::new();
            let mut candidate_domains = BTreeSet::new();
            let mut candidate_tools = BTreeSet::new();
            for matched in &search.matches {
                candidate_groups.insert(matched.group.id.clone());
                candidate_domains.extend(matched.group.domains.iter().cloned());
                candidate_tools.extend(matched.matched_tools.iter().cloned());
                route_group_domains
                    .entry(matched.group.id.clone())
                    .or_default()
                    .extend(matched.group.domains.iter().cloned());
            }
            route_groups.extend(candidate_groups.iter().cloned());
            route_domains.extend(candidate_domains.iter().cloned());
            recommended.extend(candidate_tools.iter().cloned());
            let resolution = if search.matches.is_empty() {
                unresolved.push(need.id.clone());
                "unresolved"
            } else if need.query.tool.is_some() {
                "explicit"
            } else {
                "ranked_candidates"
            };
            need_reports.push(json!({
                "id": need.id,
                "resolution": resolution,
                "candidate_groups": candidate_groups.into_iter().collect::<Vec<_>>(),
                "candidate_domains": candidate_domains.into_iter().collect::<Vec<_>>(),
                "candidate_tools": candidate_tools.into_iter().collect::<Vec<_>>(),
                "search": search,
            }));
        }

        let selected_groups = route_group_domains
            .iter()
            .map(|(id, domains)| (id.clone(), domains.iter().cloned().collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        let (artifact_registry_generation, artifact_registry_size, artifact_postures) = {
            let registry = self
                .artifact_registry
                .lock()
                .map_err(|_| "artifact registry lock is poisoned".to_string())?;
            let postures = selected_groups
                .iter()
                .map(|(id, domains)| (id.clone(), registry.domain_evidence_posture(id, domains)))
                .collect::<BTreeMap<_, _>>();
            (registry.generation(), registry.len(), postures)
        };
        let (
            workflow_reconciliation_registry_generation,
            workflow_reconciliation_registry_size,
            reconciliation_postures,
        ) = {
            let registry = self
                .workflow_reconciliation_registry
                .lock()
                .map_err(|_| "workflow reconciliation registry lock is poisoned".to_string())?;
            let postures = selected_groups
                .iter()
                .map(|(id, _)| (id.clone(), registry.workflow_posture(id)))
                .collect::<BTreeMap<_, _>>();
            (registry.generation(), registry.len(), postures)
        };
        let evidence_scope = "candidate_capability_groups_current_digest_verified_artifact_and_workflow_reconciliation_registries";
        let mut groups_with_artifact_evidence = 0usize;
        let mut artifact_evidence_records = 0usize;
        let mut groups_with_workflow_reconciliation = 0usize;
        let mut workflow_reconciliation_records = 0usize;
        let mut evidence_rows = Vec::new();
        let mut evidence_by_group = BTreeMap::new();
        for (id, _) in &selected_groups {
            let artifact_evidence = artifact_postures.get(id).cloned().unwrap_or_else(|| {
                json!({
                    "ok": true,
                    "state": "missing",
                    "group_id": id,
                    "matching_record_count": 0,
                    "readiness_claimed": false,
                    "execution": "not_started"
                })
            });
            let reconciliation_evidence =
                reconciliation_postures.get(id).cloned().unwrap_or_else(|| {
                    json!({
                        "workflow_id": id,
                        "state": "missing",
                        "record_count": 0,
                        "readiness_claimed": false,
                        "scope": "bounded_digest_valid_reconciliation_registry"
                    })
                });
            let artifact_records = artifact_evidence
                .get("matching_record_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let reconciliation_records = reconciliation_evidence
                .get("record_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if artifact_records > 0 {
                groups_with_artifact_evidence += 1;
                artifact_evidence_records =
                    artifact_evidence_records.saturating_add(artifact_records);
            }
            if reconciliation_records > 0 {
                groups_with_workflow_reconciliation += 1;
                workflow_reconciliation_records =
                    workflow_reconciliation_records.saturating_add(reconciliation_records);
            }
            let row = json!({
                "id": id,
                "artifact_evidence": artifact_evidence,
                "workflow_reconciliation_evidence": reconciliation_evidence
            });
            evidence_by_group.insert(id.clone(), row.clone());
            evidence_rows.push(row);
        }
        for need_report in &mut need_reports {
            let candidate_group_evidence = need_report
                .get("candidate_groups")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(|id| evidence_by_group.get(id).cloned())
                .collect::<Vec<_>>();
            need_report["candidate_group_evidence"] = Value::Array(candidate_group_evidence);
        }
        let evidence_document = json!({
            "scope": evidence_scope,
            "artifact_registry_generation": artifact_registry_generation,
            "artifact_registry_size": artifact_registry_size,
            "workflow_reconciliation_registry_generation": workflow_reconciliation_registry_generation,
            "workflow_reconciliation_registry_size": workflow_reconciliation_registry_size,
            "groups_with_artifact_evidence": groups_with_artifact_evidence,
            "artifact_evidence_records": artifact_evidence_records,
            "groups_with_workflow_reconciliation": groups_with_workflow_reconciliation,
            "workflow_reconciliation_records": workflow_reconciliation_records,
            "groups": evidence_rows
        });
        let evidence_digest = bioprism_ids::ContentHash::of_value(&evidence_document)
            .map_err(|error| format!("capability route evidence could not be hashed: {error}"))?;
        let evidence_summary = json!({
            "scope": evidence_scope,
            "evidence_digest": evidence_digest.to_string(),
            "artifact_registry_generation": artifact_registry_generation,
            "artifact_registry_size": artifact_registry_size,
            "workflow_reconciliation_registry_generation": workflow_reconciliation_registry_generation,
            "workflow_reconciliation_registry_size": workflow_reconciliation_registry_size,
            "candidate_group_count": selected_groups.len(),
            "groups_with_artifact_evidence": groups_with_artifact_evidence,
            "artifact_evidence_records": artifact_evidence_records,
            "groups_with_workflow_reconciliation": groups_with_workflow_reconciliation,
            "workflow_reconciliation_records": workflow_reconciliation_records,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "each candidate capability group has separate artifact and reconciliation posture rows",
                "postures are read from bounded current registries and retain their own verification semantics",
                "the evidence digest binds candidate group rows and registry snapshot metadata"
            ],
            "limitations": [
                "registry observations are not one atomic cross-store transaction",
                "evidence presence does not prove execution, scientific validity, release readiness, or external effect completion",
                "unresolved needs have no candidate-group evidence rows"
            ]
        });

        let recommended_tool_count = recommended.len();
        let recommended_tools = recommended
            .iter()
            .take(request.max_tools)
            .cloned()
            .collect::<Vec<_>>();
        let recommended_tool_overflow =
            recommended_tool_count.saturating_sub(recommended_tools.len());
        let mut output = json!({
            "ok": true,
            "workflow": "capability_route",
            "capability_schema_version": CAPABILITY_SCHEMA_VERSION,
            "route_id": route_id,
            "catalog_digest": catalogue.digest().to_string(),
            "evidence_digest": evidence_digest.to_string(),
            "evidence_scope": evidence_scope,
            "goal": request.goal,
            "needs": need_reports,
            "unresolved_needs": unresolved,
            "recommended_tools": recommended_tools,
            "recommended_tool_count": recommended_tool_count,
            "recommended_tool_overflow": recommended_tool_overflow,
            "route_coverage": {
                "needs_total": need_reports.len(),
                "needs_resolved": need_reports.len().saturating_sub(unresolved.len()),
                "needs_unresolved": unresolved.len(),
                "candidate_group_count": route_groups.len(),
                "candidate_groups": route_groups,
                "candidate_domain_count": route_domains.len(),
                "candidate_domains": route_domains,
                "candidate_tool_count": recommended_tool_count,
                "candidate_group_evidence_count": selected_groups.len(),
                "posture": "routing evidence only; caller must review domain contracts, arguments, policy, and authorization"
            },
            "evidence": evidence_summary,
            "execution": "not_started",
            "guarantees": [
                "each need retains its complete bounded ranked search result",
                "explicit tool filters are marked separately from ranked candidates",
                "no routed tool was executed and no permission was granted",
                "the route is reproducible only against the returned catalogue digest",
            ],
            "limitations": [
                "candidate ranking does not validate domain-specific arguments",
                "a route is not an agent_mission allow-list until the caller reviews it",
                "free-text matches are routing evidence, not scientific or readiness claims",
                "evidence posture is advisory and does not replace route review, mission preflight, authorization, or execution evidence",
            ],
        });
        if request.include_tools {
            let definitions = tool_definitions()
                .into_iter()
                .filter_map(|definition| {
                    let name = definition
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    name.map(|name| (name, definition))
                })
                .collect::<BTreeMap<_, _>>();
            let mut schemas = Vec::new();
            let mut missing = Vec::new();
            for tool in output["recommended_tools"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if let Some(definition) = definitions.get(tool) {
                    schemas.push(definition.clone());
                } else {
                    missing.push(tool.to_string());
                }
            }
            output["tool_schemas"] = Value::Array(schemas.clone());
            output["schema_attachment"] = json!({
                "requested": true,
                "returned": schemas.len(),
                "missing": missing,
                "authoritative_source": "tools/list definition set",
            });
        } else {
            output["schema_attachment"] = json!({
                "requested": false,
                "returned": 0,
                "missing": [],
                "authoritative_source": "tools/list definition set",
            });
        }
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure capability route result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err("capability route result exceeds the 20000000-byte safety bound".into());
        }
        Ok(output)
    }

    /// Review caller-selected candidates from one route and produce a non-executing mission handoff.
    ///
    /// The route remains discovery evidence. This checkpoint verifies that every need is selected
    /// exactly once, every selected tool came from that need's bounded candidate list, arguments
    /// are explicit JSON objects, and dependency references form deterministic waves. It does not
    /// validate domain semantics, grant authorization, or dispatch a nested tool.
    pub(super) fn capability_route_review(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode capability route review input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "capability route review input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let route = arguments
            .get("route")
            .and_then(Value::as_object)
            .ok_or("capability route review requires a route object")?;
        if route.get("workflow").and_then(Value::as_str) != Some("capability_route") {
            return Err("route.workflow must be capability_route".into());
        }
        let route_id = route
            .get("route_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("route.route_id must be a non-empty string")?;
        let catalog_digest = route
            .get("catalog_digest")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("route.catalog_digest must be a non-empty string")?;
        let route_evidence_digest = match route.get("evidence_digest") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let digest = value
                    .as_str()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or("route.evidence_digest must be a non-empty string")?;
                bioprism_ids::ContentHash::parse(digest.to_string())
                    .map_err(|_| "route.evidence_digest must be a valid content digest")?;
                Some(digest.to_string())
            }
        };
        let route_evidence_scope = match route.get("evidence_scope") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or("route.evidence_scope must be a non-empty string")?
                    .to_string(),
            ),
        };
        let route_evidence = route.get("evidence").cloned();
        if route_evidence_digest.is_some() != route_evidence.is_some()
            || route_evidence_digest.is_some() != route_evidence_scope.is_some()
        {
            return Err(
                "route evidence must provide evidence_digest, evidence_scope, and evidence together"
                    .into(),
            );
        }
        if let (Some(evidence_digest), Some(evidence_scope), Some(evidence)) = (
            route_evidence_digest.as_deref(),
            route_evidence_scope.as_deref(),
            route_evidence.as_ref(),
        ) {
            let evidence_object = evidence
                .as_object()
                .ok_or("route.evidence must be an object")?;
            if evidence_object
                .get("evidence_digest")
                .and_then(Value::as_str)
                != Some(evidence_digest)
                || evidence_object.get("scope").and_then(Value::as_str) != Some(evidence_scope)
            {
                return Err(
                    "route evidence summary does not match its top-level digest and scope".into(),
                );
            }
        }
        let evidence_binding = match (
            route_evidence_digest.as_deref(),
            route_evidence_scope.as_deref(),
            route_evidence.as_ref(),
        ) {
            (Some(digest), Some(scope), Some(evidence)) => json!({
                "present": true,
                "evidence_digest": digest,
                "scope": scope,
                "summary": evidence,
                "posture": "carried_forward_not_recomputed",
                "readiness_claimed": false,
                "execution": "not_started"
            }),
            _ => json!({
                "present": false,
                "posture": "not_supplied",
                "readiness_claimed": false,
                "execution": "not_started"
            }),
        };
        let goal = route
            .get("goal")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("route.goal must be a non-empty string")?;
        let raw_needs = route
            .get("needs")
            .and_then(Value::as_array)
            .ok_or("route.needs must be an array")?;
        if raw_needs.is_empty() || raw_needs.len() > 32 {
            return Err("route.needs must contain between 1 and 32 needs".into());
        }
        let unresolved_route = route
            .get("unresolved_needs")
            .and_then(Value::as_array)
            .ok_or("route.unresolved_needs must be an array")?;
        let unresolved_route = unresolved_route
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|item| !item.trim().is_empty())
                    .map(str::to_string)
                    .ok_or("route.unresolved_needs must contain non-empty strings")
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut need_ids = Vec::with_capacity(raw_needs.len());
        let mut candidates_by_need = BTreeMap::<String, BTreeSet<String>>::new();
        for raw_need in raw_needs {
            let need = raw_need
                .as_object()
                .ok_or("route.needs entries must be objects")?;
            let need_id = need
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or("route need id must be a non-empty string")?
                .to_string();
            if candidates_by_need.contains_key(&need_id) {
                return Err(format!("route contains duplicate need id: {need_id}"));
            }
            let candidate_tools = need
                .get("candidate_tools")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("route need {need_id:?} has no candidate_tools array"))?;
            let candidates = candidate_tools
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|item| !item.trim().is_empty())
                        .map(str::to_string)
                        .ok_or_else(|| {
                            format!("route need {need_id:?} has an invalid candidate tool")
                        })
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            candidates_by_need.insert(need_id.clone(), candidates);
            need_ids.push(need_id);
        }

        let mut findings = Vec::<Value>::new();
        let mut add_finding = |code: &str, message: String, need_id: Option<&str>| {
            let mut finding = json!({
                "code": code,
                "severity": "error",
                "message": message,
            });
            if let Some(need_id) = need_id {
                finding["need_id"] = json!(need_id);
            }
            findings.push(finding);
        };
        for need_id in &unresolved_route {
            add_finding(
                "unresolved_need",
                format!("route need {need_id:?} has no resolved candidate"),
                Some(need_id),
            );
        }

        let raw_selections = arguments
            .get("selections")
            .and_then(Value::as_array)
            .ok_or("capability route review requires a selections array")?;
        let validate_schemas = match arguments.get("validate_schemas") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("validate_schemas must be a boolean".into()),
        };
        let review_document = json!({
            "route_id": route_id,
            "catalog_digest": catalog_digest,
            "evidence_digest": route_evidence_digest,
            "selections": raw_selections,
            "validate_schemas": validate_schemas,
        });
        let review_id = bioprism_ids::ContentHash::of_value(&review_document)
            .map_err(|error| format!("cannot hash capability route review: {error}"))?
            .to_string();
        if raw_selections.len() > 32 {
            return Err("selections must contain at most 32 choices".into());
        }
        if raw_selections.len() != need_ids.len() {
            add_finding(
                "selection_count_mismatch",
                format!(
                    "route has {} needs but received {} selections",
                    need_ids.len(),
                    raw_selections.len()
                ),
                None,
            );
        }

        let mut selected_steps = BTreeMap::<String, Value>::new();
        let mut dependencies_by_need = BTreeMap::<String, Vec<String>>::new();
        for raw_selection in raw_selections {
            let Some(selection) = raw_selection.as_object() else {
                add_finding(
                    "invalid_selection",
                    "selection must be an object".into(),
                    None,
                );
                continue;
            };
            let Some(need_id) = selection
                .get("need_id")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            else {
                add_finding(
                    "invalid_selection",
                    "selection.need_id must be a non-empty string".into(),
                    None,
                );
                continue;
            };
            let Some(candidates) = candidates_by_need.get(need_id) else {
                add_finding(
                    "unknown_need",
                    format!("selection refers to unknown route need {need_id:?}"),
                    Some(need_id),
                );
                continue;
            };
            if selected_steps.contains_key(need_id) {
                add_finding(
                    "duplicate_selection",
                    format!("route need {need_id:?} has more than one selection"),
                    Some(need_id),
                );
                continue;
            }
            let Some(tool) = selection
                .get("tool")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            else {
                add_finding(
                    "invalid_selection",
                    "selection.tool must be a non-empty string".into(),
                    Some(need_id),
                );
                continue;
            };
            if !candidates.contains(tool) {
                add_finding(
                    "candidate_mismatch",
                    format!("tool {tool:?} is not a candidate for route need {need_id:?}"),
                    Some(need_id),
                );
                continue;
            }
            let mut valid = true;
            for field in ["domain", "capability", "objective"] {
                if selection
                    .get(field)
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .is_none()
                {
                    add_finding(
                        "invalid_selection",
                        format!("selection.{field} must be a non-empty string"),
                        Some(need_id),
                    );
                    valid = false;
                }
            }
            if !selection
                .get("arguments")
                .map(Value::is_object)
                .unwrap_or(false)
            {
                add_finding(
                    "invalid_arguments",
                    "selection.arguments must be an explicit JSON object".into(),
                    Some(need_id),
                );
                valid = false;
            }
            let depends_on = match selection.get("depends_on") {
                None => Vec::new(),
                Some(value) => match value.as_array() {
                    Some(values) => {
                        let mut dependencies = Vec::with_capacity(values.len());
                        for dependency in values {
                            let Some(dependency) =
                                dependency.as_str().filter(|value| !value.trim().is_empty())
                            else {
                                add_finding(
                                    "invalid_dependency",
                                    "selection.depends_on must contain non-empty strings".into(),
                                    Some(need_id),
                                );
                                valid = false;
                                continue;
                            };
                            if dependency == need_id || !candidates_by_need.contains_key(dependency)
                            {
                                add_finding(
                                    "unknown_dependency",
                                    format!("selection depends on unknown need {dependency:?}"),
                                    Some(need_id),
                                );
                                valid = false;
                            }
                            dependencies.push(dependency.to_string());
                        }
                        dependencies
                    }
                    None => {
                        add_finding(
                            "invalid_dependency",
                            "selection.depends_on must be an array".into(),
                            Some(need_id),
                        );
                        valid = false;
                        Vec::new()
                    }
                },
            };
            if let Some(bindings) = selection.get("bindings") {
                if !bindings
                    .as_array()
                    .map(|values| values.iter().all(Value::is_object))
                    .unwrap_or(false)
                {
                    add_finding(
                        "invalid_bindings",
                        "selection.bindings must be an array of JSON objects".into(),
                        Some(need_id),
                    );
                    valid = false;
                }
            }
            let required = match selection.get("required") {
                None => true,
                Some(Value::Bool(value)) => *value,
                Some(_) => {
                    add_finding(
                        "invalid_selection",
                        "selection.required must be a boolean".into(),
                        Some(need_id),
                    );
                    valid = false;
                    true
                }
            };
            if valid {
                let step = json!({
                    "id": need_id,
                    "domain": selection.get("domain").and_then(Value::as_str).unwrap_or_default(),
                    "capability": selection.get("capability").and_then(Value::as_str).unwrap_or_default(),
                    "objective": selection.get("objective").and_then(Value::as_str).unwrap_or_default(),
                    "tool": tool,
                    "arguments": selection.get("arguments").cloned().unwrap_or_else(|| json!({})),
                    "depends_on": depends_on,
                    "required": required,
                    "bindings": selection.get("bindings").cloned().unwrap_or_else(|| json!([])),
                });
                selected_steps.insert(need_id.to_string(), step);
                dependencies_by_need.insert(
                    need_id.to_string(),
                    selection
                        .get("depends_on")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
        }

        let missing_needs = need_ids
            .iter()
            .filter(|need_id| !selected_steps.contains_key(*need_id))
            .cloned()
            .collect::<Vec<_>>();
        for need_id in &missing_needs {
            add_finding(
                "missing_selection",
                format!("route need {need_id:?} has no valid explicit selection"),
                Some(need_id),
            );
        }

        let mut indegree = BTreeMap::<String, usize>::new();
        let mut dependents = BTreeMap::<String, Vec<String>>::new();
        for need_id in selected_steps.keys() {
            let dependencies = dependencies_by_need
                .get(need_id)
                .cloned()
                .unwrap_or_default();
            let known_dependencies = dependencies
                .iter()
                .filter(|dependency| selected_steps.contains_key(*dependency))
                .count();
            indegree.insert(need_id.clone(), known_dependencies);
            for dependency in dependencies {
                if selected_steps.contains_key(&dependency) {
                    dependents
                        .entry(dependency)
                        .or_default()
                        .push(need_id.clone());
                }
            }
        }
        let mut ready = indegree
            .iter()
            .filter_map(|(need_id, degree)| (*degree == 0).then_some(need_id.clone()))
            .collect::<BTreeSet<_>>();
        let mut dependency_waves = Vec::<Vec<String>>::new();
        let mut ordered_selected = Vec::<String>::new();
        while !ready.is_empty() {
            let wave = ready.iter().cloned().collect::<Vec<_>>();
            ready.clear();
            for need_id in &wave {
                ordered_selected.push(need_id.clone());
                if let Some(children) = dependents.get(need_id) {
                    for child in children {
                        if let Some(degree) = indegree.get_mut(child) {
                            *degree = degree.saturating_sub(1);
                            if *degree == 0 {
                                ready.insert(child.clone());
                            }
                        }
                    }
                }
            }
            dependency_waves.push(wave);
        }
        if ordered_selected.len() != selected_steps.len() {
            add_finding(
                "dependency_cycle",
                "valid selections contain a dependency cycle".into(),
                None,
            );
        }

        let selected_tools = selected_steps
            .values()
            .filter_map(|step| step.get("tool").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        let selected_domains = selected_steps
            .values()
            .filter_map(|step| step.get("domain").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        let ordered_steps = need_ids
            .iter()
            .filter_map(|need_id| selected_steps.get(need_id).cloned())
            .collect::<Vec<_>>();
        let mut schema_reports = Vec::<Value>::new();
        let mut schema_valid = true;
        if validate_schemas {
            for step in &ordered_steps {
                let tool = step.get("tool").and_then(Value::as_str).unwrap_or_default();
                let arguments = step.get("arguments").unwrap_or(&Value::Null);
                match validate_mission_tool_arguments(tool, arguments)? {
                    Some(report) => {
                        let issues = report
                            .issues
                            .iter()
                            .map(|issue| {
                                json!({
                                    "path": issue.path,
                                    "code": issue.code,
                                    "message": issue.message,
                                })
                            })
                            .collect::<Vec<_>>();
                        let ok = issues.is_empty();
                        schema_valid &= ok;
                        schema_reports.push(json!({
                            "need_id": step.get("id"),
                            "tool": tool,
                            "schema_digest": report.digest,
                            "ok": ok,
                            "fully_checked": true,
                            "issue_count": issues.len(),
                            "issues": issues,
                        }));
                    }
                    None => {
                        schema_valid = false;
                        schema_reports.push(json!({
                            "need_id": step.get("id"),
                            "tool": tool,
                            "schema_digest": Value::Null,
                            "ok": false,
                            "fully_checked": false,
                            "issue_count": 1,
                            "issues": [{
                                "path": "",
                                "code": "schema_missing",
                                "message": "selected tool has no authoritative tools/list schema"
                            }],
                        }));
                    }
                }
            }
            for report in &schema_reports {
                if report.get("ok") == Some(&Value::Bool(false)) {
                    add_finding(
                        "schema_mismatch",
                        format!(
                            "selected tool {:?} failed authoritative schema validation",
                            report
                                .get("tool")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                        ),
                        report.get("need_id").and_then(Value::as_str),
                    );
                }
            }
        }
        let ready_for_handoff = findings.is_empty() && (!validate_schemas || schema_valid);
        let mission_draft = ready_for_handoff.then(|| {
            json!({
                "goal": goal,
                "steps": ordered_steps,
                "dependency_waves": dependency_waves.clone(),
                "route_evidence_digest": route_evidence_digest,
                "route_evidence_scope": route_evidence_scope,
            })
        });
        let mut output = json!({
            "ok": true,
            "workflow": "capability_route_review",
            "review_id": review_id,
            "route_id": route_id,
            "catalog_digest": catalog_digest,
            "evidence_digest": route_evidence_digest,
            "evidence_scope": route_evidence_scope,
            "goal": goal,
            "need_count": need_ids.len(),
            "selection_count": raw_selections.len(),
            "missing_needs": missing_needs,
            "selected_tools": selected_tools,
            "selected_domains": selected_domains,
            "dependency_waves": dependency_waves,
            "findings": findings,
            "review_status": if ready_for_handoff { "ready" } else { "blocked" },
            "handoff_status": if ready_for_handoff { "mission_preflight_required" } else { "requires_caller_correction" },
            "mission_draft": mission_draft,
            "evidence_binding": evidence_binding,
            "schema_review": {
                "requested": validate_schemas,
                "checked": schema_reports.len(),
                "valid": validate_schemas.then_some(schema_valid),
                "fully_checked": validate_schemas && schema_reports.iter().all(|report| report["fully_checked"] == json!(true)),
                "reports": schema_reports,
                "authoritative_source": "tools/list definition set",
                "posture": "schema shape evidence only; domain semantics and authorization remain caller-owned",
            },
            "execution": "not_started",
            "guarantees": [
                "route candidates were reviewed without executing any tool",
                "each ready selection is present in its need's bounded candidate list",
                "dependency waves are deterministic and use only explicit caller selections",
                "when supplied, the route evidence digest and scope are carried into review provenance and the mission draft",
            ],
            "limitations": [
                "this review checks transport-shaped handoff structure, not domain-specific argument semantics",
                "the route catalogue digest is provenance and does not prove the live catalogue is unchanged",
                "mission_preflight remains required before an agent_mission request can be dispatched",
            ],
        });
        output["route_coverage"] = route
            .get("route_coverage")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure capability route review result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "capability route review result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }

    /// Compile an explicitly reviewed capability route into a mission preflight handoff.
    ///
    /// This is intentionally a composition boundary rather than a second planner: the route
    /// review remains responsible for candidate membership, dependency structure, and review
    /// evidence, while `preflight_agent_mission` remains authoritative for the normalized mission
    /// graph and every selected tool schema. The result is always planned and never dispatches a
    /// nested tool. Callers still choose every tool, domain label, objective, argument object, and
    /// dependency; route scores never become an allow-list implicitly.
    pub(super) fn capability_route_plan(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode capability route plan input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "capability route plan input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let object = arguments
            .as_object()
            .ok_or("capability route plan input must be an object")?;
        let mission_id = object
            .get("mission_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("capability route plan requires a non-empty mission_id")?;
        let route = object
            .get("route")
            .and_then(Value::as_object)
            .ok_or("capability route plan requires a route object")?;
        if route.get("workflow").and_then(Value::as_str) != Some("capability_route") {
            return Err("route.workflow must be capability_route".into());
        }
        let selections = object
            .get("selections")
            .and_then(Value::as_array)
            .ok_or("capability route plan requires a selections array")?;
        if selections.is_empty() || selections.len() > 128 {
            return Err("selections must contain between 1 and 128 choices".into());
        }
        let validate_schemas = match object.get("validate_schemas") {
            None => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("validate_schemas must be a boolean".into()),
        };
        if object
            .get("policy")
            .and_then(Value::as_object)
            .and_then(|policy| policy.get("execute"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Err(
                "capability_route_plan is non-executing; call agent_mission only after inspecting its preflight"
                    .into(),
            );
        }
        if object.get("policy").is_some_and(|value| !value.is_object()) {
            return Err("policy must be an object when supplied".into());
        }

        let review_arguments = json!({
            "route": route,
            "selections": selections,
            "validate_schemas": validate_schemas,
        });
        let route_input_digest = bioprism_ids::ContentHash::of_value(&Value::Object(route.clone()))
            .map_err(|error| format!("cannot hash capability route plan route input: {error}"))?
            .to_string();
        let selection_digest =
            bioprism_ids::ContentHash::of_value(&Value::Array(selections.clone()))
                .map_err(|error| format!("cannot hash capability route plan selections: {error}"))?
                .to_string();
        let review = self.capability_route_review(&review_arguments)?;
        let review_ready = review.get("review_status").and_then(Value::as_str) == Some("ready");

        let mut output = json!({
            "ok": true,
            "workflow": "capability_route_plan",
            "mission_id": mission_id,
            "route_id": review.get("route_id").cloned().unwrap_or(Value::Null),
            "review_id": review.get("review_id").cloned().unwrap_or(Value::Null),
            "catalog_digest": review.get("catalog_digest").cloned().unwrap_or(Value::Null),
            "goal": review.get("goal").cloned().unwrap_or(Value::Null),
            "review": review.clone(),
            "route_input_digest": route_input_digest,
            "selection_digest": selection_digest,
            "selection_count": selections.len(),
            "mission": Value::Null,
            "preflight": Value::Null,
            "plan_status": if review_ready { "preflight_pending" } else { "blocked_by_route_review" },
            "dispatch": "not_started",
            "execution": "not_started",
            "guarantees": [
                "route review and authoritative mission preflight are composed without dispatch",
                "caller-selected tools and arguments remain explicit and candidate-bound",
                "schema validation is requested by default and remains transport-shape evidence only",
                "a blocked route review never becomes a partial mission plan",
            ],
            "limitations": [
                "the compiler does not infer missing tools, arguments, dependencies, or domain meaning",
                "route and preflight digests are provenance and do not authorize execution",
                "domain semantics, provider availability, and scientific or clinical interpretation remain outside this boundary",
            ],
        });

        if !review_ready {
            return Ok(output);
        }
        let mission_draft = review
            .get("mission_draft")
            .and_then(Value::as_object)
            .ok_or("ready route review did not provide a mission_draft")?;
        let mut mission = Map::new();
        mission.insert("mission_id".into(), json!(mission_id));
        mission.insert(
            "goal".into(),
            mission_draft.get("goal").cloned().unwrap_or(Value::Null),
        );
        mission.insert(
            "steps".into(),
            mission_draft
                .get("steps")
                .cloned()
                .unwrap_or_else(|| json!([])),
        );
        mission.insert(
            "policy".into(),
            object.get("policy").cloned().unwrap_or_else(|| json!({})),
        );
        mission.insert("route_review".into(), output["review"].clone());
        for field in ["claim_requests", "evaluator_review", "workflow_binding"] {
            if let Some(value) = object.get(field) {
                mission.insert(field.into(), value.clone());
            }
        }
        let mission = Value::Object(mission);
        let preflight = match self.preflight_agent_mission(&mission) {
            Ok(report) => report,
            Err(error) => json!({
                "ok": false,
                "workflow": "agent_mission",
                "preflight": true,
                "dispatch": "not_started",
                "refusal": error,
                "fail_closed": true,
                "issues": [error],
            }),
        };
        let preflight_ok = preflight.get("ok").and_then(Value::as_bool) == Some(true);
        output["mission"] = mission;
        output["preflight"] = preflight.clone();
        output["plan_status"] = json!(if preflight_ok {
            "ready_for_caller_inspection"
        } else {
            "blocked_by_mission_preflight"
        });
        output["plan_digest"] = preflight
            .pointer("/plan/digest")
            .cloned()
            .unwrap_or(Value::Null);
        output["route_review_provenance"] = preflight
            .pointer("/plan/route_review_provenance")
            .cloned()
            .unwrap_or(Value::Null);
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure capability route plan result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "capability route plan result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }

    /// Verify a previously returned route plan against its content-addressed inputs and the live
    /// authoritative mission boundary without executing any nested tool.
    ///
    /// A caller may provide the original route and selections to replay the complete route-review
    /// and preflight composition. Without them, the verifier still checks the retained plan shape,
    /// nested identities, and mission preflight, but reports that route replay was not requested;
    /// it never upgrades that weaker check into proof that the original candidate membership is
    /// still current.
    pub(super) fn capability_route_plan_verify(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode capability route plan verification input: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err(
                "capability route plan verification input exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        let object = arguments
            .as_object()
            .ok_or("capability route plan verification input must be an object")?;
        let plan = object
            .get("plan")
            .and_then(Value::as_object)
            .ok_or("capability route plan verification requires a plan object")?;
        let plan_value = Value::Object(plan.clone());
        if plan.get("workflow").and_then(Value::as_str) != Some("capability_route_plan") {
            return Err("plan.workflow must be capability_route_plan".into());
        }
        let mission_id = plan
            .get("mission_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("plan.mission_id must be a non-empty string")?;
        let plan_status = plan
            .get("plan_status")
            .and_then(Value::as_str)
            .ok_or("plan.plan_status must be a non-empty string")?;
        let route_id = plan
            .get("route_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("plan.route_id must be a non-empty string")?;
        let review_id = plan
            .get("review_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("plan.review_id must be a non-empty string")?;
        let catalog_digest = plan
            .get("catalog_digest")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("plan.catalog_digest must be a non-empty string")?;
        let review = plan
            .get("review")
            .and_then(Value::as_object)
            .ok_or("plan.review must be an object")?;
        let mut mismatches = Vec::<Value>::new();
        for (field, expected) in [
            ("route_id", route_id),
            ("review_id", review_id),
            ("catalog_digest", catalog_digest),
        ] {
            if review.get(field).and_then(Value::as_str) != Some(expected) {
                mismatches.push(json!({
                    "code": "review_identity_mismatch",
                    "field": field,
                    "expected": expected,
                    "observed": review.get(field).cloned().unwrap_or(Value::Null),
                }));
            }
        }
        if plan.get("dispatch").and_then(Value::as_str) != Some("not_started") {
            mismatches.push(json!({
                "code": "dispatch_started",
                "message": "a route plan verifier accepts only non-dispatching plans"
            }));
        }
        if plan.get("execution").and_then(Value::as_str) != Some("not_started") {
            mismatches.push(json!({
                "code": "execution_started",
                "message": "a route plan verifier accepts only non-executing plans"
            }));
        }
        let route = object.get("route");
        let selections = object.get("selections");
        if route.is_some() != selections.is_some() {
            mismatches.push(json!({
                "code": "incomplete_route_replay_input",
                "message": "route and selections must be supplied together for route replay"
            }));
        }
        let validate_schemas = match object.get("validate_schemas") {
            None => review
                .get("schema_review")
                .and_then(Value::as_object)
                .and_then(|schema| schema.get("requested"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("validate_schemas must be a boolean".into()),
        };
        let mut route_replay = json!({
            "requested": false,
            "status": "not_requested",
            "matched": Value::Null,
        });
        if let (Some(route), Some(selections)) = (route, selections) {
            let route = route
                .as_object()
                .ok_or("route replay route must be an object")?;
            let selections = selections
                .as_array()
                .ok_or("route replay selections must be an array")?;
            let mut replay_arguments = json!({
                "mission_id": mission_id,
                "route": route,
                "selections": selections,
                "validate_schemas": validate_schemas,
                "policy": plan_value.pointer("/mission/policy").cloned().unwrap_or_else(|| json!({})),
                "claim_requests": plan_value.pointer("/mission/claim_requests").cloned().unwrap_or_else(|| json!([])),
            });
            for field in ["evaluator_review", "workflow_binding"] {
                if let Some(value) = plan_value.pointer(&format!("/mission/{field}")) {
                    if !value.is_null() {
                        replay_arguments[field] = value.clone();
                    }
                }
            }
            let replay = match self.capability_route_plan(&replay_arguments) {
                Ok(value) => value,
                Err(error) => {
                    mismatches.push(json!({
                        "code": "route_replay_blocked",
                        "message": error.clone(),
                    }));
                    route_replay = json!({
                        "requested": true,
                        "status": "blocked",
                        "matched": false,
                        "error": error,
                    });
                    Value::Null
                }
            };
            if replay.is_object() {
                let mut replay_mismatches = Vec::new();
                for field in [
                    "route_id",
                    "review_id",
                    "catalog_digest",
                    "plan_digest",
                    "plan_status",
                    "route_input_digest",
                    "selection_digest",
                    "selection_count",
                ] {
                    if plan.get(field) != replay.get(field) {
                        replay_mismatches.push(json!({
                            "code": "replay_field_mismatch",
                            "field": field,
                            "expected": plan.get(field).cloned().unwrap_or(Value::Null),
                            "observed": replay.get(field).cloned().unwrap_or(Value::Null),
                        }));
                    }
                }
                let matched = replay_mismatches.is_empty();
                route_replay = json!({
                    "requested": true,
                    "status": if matched { "matched" } else { "mismatched" },
                    "matched": matched,
                    "mismatches": replay_mismatches,
                    "route_input_digest": replay.get("route_input_digest").cloned().unwrap_or(Value::Null),
                    "selection_digest": replay.get("selection_digest").cloned().unwrap_or(Value::Null),
                });
                if !matched {
                    if let Some(rows) = route_replay.get("mismatches").and_then(Value::as_array) {
                        mismatches.extend(rows.iter().cloned());
                    }
                }
            }
        }
        let mut mission_preflight = json!({
            "requested": false,
            "status": "not_requested",
            "matched": Value::Null,
        });
        if let Some(mission) = plan.get("mission").and_then(Value::as_object) {
            let preflight = match self.preflight_agent_mission(&Value::Object(mission.clone())) {
                Ok(value) => value,
                Err(error) => json!({
                    "ok": false,
                    "dispatch": "not_started",
                    "error": error,
                    "fail_closed": true,
                }),
            };
            let preflight_ok = preflight.get("ok") == Some(&Value::Bool(true));
            let expected_digest = plan.get("plan_digest").cloned().unwrap_or(Value::Null);
            let observed_digest = preflight
                .pointer("/plan/digest")
                .cloned()
                .unwrap_or(Value::Null);
            let matched = preflight_ok && expected_digest == observed_digest;
            if !matched {
                mismatches.push(json!({
                    "code": if preflight_ok { "mission_plan_digest_mismatch" } else { "mission_preflight_blocked" },
                    "expected": expected_digest,
                    "observed": observed_digest,
                }));
            }
            mission_preflight = json!({
                "requested": true,
                "status": if matched { "matched" } else if preflight_ok { "mismatched" } else { "blocked" },
                "matched": matched,
                "ok": preflight_ok,
                "plan_digest": observed_digest,
                "dispatch": "not_started",
            });
        }
        let valid = mismatches.is_empty();
        let verification_status = if valid {
            if route_replay["requested"] == json!(true) {
                "verified"
            } else {
                "verified_without_route_replay"
            }
        } else if route_replay["status"] == json!("blocked") {
            "blocked_by_route_replay"
        } else if mission_preflight["status"] == json!("blocked") {
            "blocked_by_mission_preflight"
        } else {
            "mismatch"
        };
        Ok(json!({
            "ok": true,
            "workflow": "capability_route_plan_verify",
            "mission_id": mission_id,
            "route_id": route_id,
            "review_id": review_id,
            "catalog_digest": catalog_digest,
            "plan_status": plan_status,
            "plan_digest": plan.get("plan_digest").cloned().unwrap_or(Value::Null),
            "valid": valid,
            "verification_status": verification_status,
            "route_replay": route_replay,
            "mission_preflight": mission_preflight,
            "mismatches": mismatches,
            "dispatch": "not_started",
            "execution": "not_started",
            "guarantees": [
                "verification is non-executing and reruns only route review and mission preflight",
                "route replay is explicit; omitted route inputs never become a claim of current candidate membership",
                "identity and plan-digest mismatches remain visible instead of being coerced into validity"
            ],
            "limitations": [
                "verification cannot establish domain semantics, provider availability, scientific validity, or authorization",
                "without route and selections, only the retained plan shape and mission preflight are checked",
                "the verifier does not persist a distributed audit record or resume a mission"
            ]
        }))
    }
}
