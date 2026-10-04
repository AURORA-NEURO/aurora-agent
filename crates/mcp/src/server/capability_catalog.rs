//! Capability ranking, discovery, audit, and dashboard projections.

use super::*;

impl Server {
    pub(super) fn capability_rank(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let raw_vectors = arguments
            .get("vectors")
            .and_then(Value::as_array)
            .ok_or("vectors is required and must contain at least two capability vectors")?;
        if raw_vectors.len() < 2 || raw_vectors.len() > 100 {
            return Err("vectors must contain between 2 and 100 capability vectors".into());
        }
        let vectors = raw_vectors
            .iter()
            .enumerate()
            .map(|(index, value)| {
                serde_json::from_value::<CapabilityVector>(value.clone())
                    .map_err(|error| format!("invalid capability vector at index {index}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let waiver_values = arguments
            .get("waived_dimensions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut policy = MetricsComparabilityPolicy::strict();
        for (index, value) in waiver_values.iter().enumerate() {
            let dimension = value.as_str().ok_or_else(|| {
                format!("waived_dimensions[{index}] must be a string dimension name")
            })?;
            policy = policy.waiving(dimension);
        }
        let ranking = PartialRanking::over_under(vectors, &policy)
            .map_err(|error| format!("capability ranking refused: {error}"))?;
        let vector_rows = ranking
            .vectors()
            .iter()
            .take(max_items)
            .map(|vector| {
                let holes = vector
                    .grid
                    .holes()
                    .map(|(capability, reason)| {
                        json!({ "capability": capability, "reason": reason })
                    })
                    .collect::<Vec<_>>();
                json!({
                    "system": vector.system,
                    "grid": vector.grid.label,
                    "cells": vector.grid.len(),
                    "measured_cells": vector.grid.measured().count(),
                    "holes": holes,
                    "unrecorded_coordinates": vector.grid.conditions.unrecorded_coordinates(),
                })
            })
            .collect::<Vec<_>>();
        let relation_rows = ranking
            .relations()
            .iter()
            .take(max_items)
            .map(|relation| {
                json!({
                    "left": relation.left,
                    "right": relation.right,
                    "dominance": relation.dominance,
                })
            })
            .collect::<Vec<_>>();
        let unresolved_rows = ranking
            .unresolved()
            .into_iter()
            .take(max_items)
            .map(|relation| {
                json!({
                    "left": relation.left,
                    "right": relation.right,
                    "dominance": relation.dominance,
                })
            })
            .collect::<Vec<_>>();
        let relation_count = ranking.relations().len();
        let unresolved_count = ranking.unresolved().len();
        let omitted_vectors = ranking.vectors().len().saturating_sub(max_items);
        let omitted_relations = relation_count.saturating_sub(max_items);
        let omitted_unresolved = unresolved_count.saturating_sub(max_items);
        let mut output = json!({
            "ok": true,
            "metrics_schema_version": METRICS_SCHEMA_VERSION,
            "max_items": max_items,
            "waived_dimensions": policy.waived().collect::<Vec<_>>(),
            "partial_order": {
                "system_count": ranking.vectors().len(),
                "relation_count": relation_count,
                "unresolved_count": unresolved_count,
                "is_total": ranking.is_total(),
                "maximal_systems": ranking.maximal(),
                "vectors": vector_rows,
                "relations": relation_rows,
                "unresolved": unresolved_rows,
                "omitted_vectors": omitted_vectors,
                "omitted_relations": omitted_relations,
                "omitted_unresolved": omitted_unresolved,
            },
            "guarantees": [
                "unmeasured cells remain incomparable rather than becoming zero",
                "different conditions are blocked unless the caller names a waiver",
                "a partial order is not silently converted into a total order",
            ],
        });

        if let Some(raw_weighting) = arguments.get("weighting") {
            let weighting: DeclaredWeighting = serde_json::from_value(raw_weighting.clone())
                .map_err(|error| format!("invalid declared weighting: {error}"))?;
            let total = ranking
                .totalise(&weighting)
                .map_err(|error| format!("weighted ranking refused: {error}"))?;
            let instability = RankInstability::measure(&ranking, &weighting)
                .map_err(|error| format!("weight sensitivity refused: {error}"))?;
            output["declared_weighting"] = json!({
                "intended_use": weighting.intended_use(),
                "digest": weighting.digest(),
                "capabilities": weighting.capabilities().collect::<Vec<_>>(),
                "weights": weighting.weights().map(|(capability, weight)| json!({
                    "capability": capability,
                    "weight": weight,
                })).collect::<Vec<_>>(),
            });
            output["total_order"] = json!({
                "headline": total.headline(),
                "leaders": total.leaders(),
                "overwrote_a_refusal": total.overwrote_a_refusal(),
                "order": total.order.iter().take(max_items).map(|row| json!({
                    "rank": row.rank,
                    "system": row.system,
                    "aggregate": row.aggregate,
                })).collect::<Vec<_>>(),
                "collapsed": total.collapsed.iter().take(max_items).map(|pair| json!({
                    "left": pair.left,
                    "right": pair.right,
                    "dominance": pair.dominance,
                })).collect::<Vec<_>>(),
                "omitted_order": total.order.len().saturating_sub(max_items),
                "omitted_collapsed": total.collapsed.len().saturating_sub(max_items),
            });
            output["rank_instability"] = json!(instability);
        }
        Ok(output)
    }

    /// Search the complete workspace capability catalogue and optionally attach authoritative MCP
    /// schemas for the matched tools.
    ///
    /// This is intentionally a discovery operation, not a semantic router or permission grant.
    /// Scores only reflect explicit label matches; callers still need to inspect the returned
    /// domain, evidence, policy, and tool contracts before constructing a mission.
    pub(super) fn capability_discover(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode capability discovery input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("capability discovery input exceeds the 20000000-byte safety bound".into());
        }
        let query: CapabilityQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid capability discovery input: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("capability catalogue refused: {error}"))?;
        let search = catalogue
            .search(&query)
            .map_err(|error| format!("capability discovery refused: {error}"))?;
        let mut output = serde_json::to_value(search)
            .map_err(|error| format!("cannot encode capability discovery result: {error}"))?;
        let mut schema_missing = Vec::new();
        let mut schema_returned = 0usize;
        if query.include_tools {
            let definitions = tool_definitions();
            let definitions = definitions
                .into_iter()
                .filter_map(|definition| {
                    let name = definition
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    name.map(|name| (name, definition))
                })
                .collect::<BTreeMap<_, _>>();
            if let Some(matches) = output.get_mut("matches").and_then(Value::as_array_mut) {
                for matched in matches {
                    let tool_names = matched
                        .get("matched_tools")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let mut schemas = Vec::new();
                    for tool_name in tool_names.iter().filter_map(Value::as_str) {
                        if let Some(definition) = definitions.get(tool_name) {
                            schemas.push(definition.clone());
                        } else {
                            schema_missing.push(tool_name.to_string());
                        }
                    }
                    schema_returned += schemas.len();
                    matched["tool_schemas"] = Value::Array(schemas);
                }
            }
        }
        output["ok"] = json!(true);
        output["workflow"] = json!("capability_discover");
        output["capability_schema_version"] = json!(CAPABILITY_SCHEMA_VERSION);
        output["schema_attachment"] = json!({
            "requested": query.include_tools,
            "returned": schema_returned,
            "missing": schema_missing,
            "authoritative_source": "tools/list definition set"
        });
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure capability discovery result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "capability discovery result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }

    /// Verify that the explicit cross-domain catalogue and authoritative MCP tool definitions
    /// describe the same callable surface.
    ///
    /// A catalogue can be useful for routing while still drifting from the transport. This audit
    /// makes that failure visible without treating intentional multi-group membership as a
    /// defect. `healthy` is therefore about schema/name coverage and duplicate definitions; the
    /// returned membership report remains useful even when the invariant is broken.
    pub(super) fn capability_audit(&self, arguments: &Value) -> Result<Value, String> {
        let include_groups = arguments
            .get("include_groups")
            .map(|value| value.as_bool().ok_or("include_groups must be a boolean"))
            .transpose()?
            .unwrap_or(true);
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("capability catalogue refused: {error}"))?;
        let definitions = tool_definitions();
        let mut definition_names = BTreeMap::new();
        let mut duplicate_schema_names = BTreeSet::new();
        for definition in definitions {
            let name = definition
                .get("name")
                .and_then(Value::as_str)
                .ok_or("tool definition has no string name")?
                .to_string();
            if definition_names.insert(name.clone(), definition).is_some() {
                duplicate_schema_names.insert(name);
            }
        }
        const MAX_SCHEMA_BYTES: usize = 1_000_000;
        let mut schema_findings = Vec::new();
        let mut valid_schema_count = 0usize;
        let mut schema_bytes = 0usize;
        for (name, definition) in &definition_names {
            let Some(schema) = definition.get("inputSchema") else {
                schema_findings.push(json!({
                    "tool": name,
                    "code": "missing_input_schema",
                    "detail": "tool definition has no inputSchema member",
                }));
                continue;
            };
            let encoded_schema = serde_json::to_vec(schema)
                .map_err(|error| format!("cannot encode schema for {name:?}: {error}"))?;
            schema_bytes = schema_bytes.saturating_add(encoded_schema.len());
            if encoded_schema.len() > MAX_SCHEMA_BYTES {
                schema_findings.push(json!({
                    "tool": name,
                    "code": "input_schema_too_large",
                    "bytes": encoded_schema.len(),
                    "maximum": MAX_SCHEMA_BYTES,
                }));
            }
            let Some(schema_object) = schema.as_object() else {
                schema_findings.push(json!({
                    "tool": name,
                    "code": "input_schema_not_object",
                    "detail": "inputSchema must be a JSON object",
                }));
                continue;
            };
            if schema_object.get("type").and_then(Value::as_str) != Some("object") {
                schema_findings.push(json!({
                    "tool": name,
                    "code": "input_schema_wrong_type",
                    "detail": "inputSchema.type must be object",
                }));
            }
            let properties = schema_object.get("properties").and_then(Value::as_object);
            let mut schema_valid = true;
            if let Some(required) = schema_object.get("required") {
                let Some(required) = required.as_array() else {
                    schema_findings.push(json!({
                        "tool": name,
                        "code": "required_not_array",
                        "detail": "inputSchema.required must be an array when supplied",
                    }));
                    continue;
                };
                for required_name in required {
                    let Some(required_name) = required_name.as_str() else {
                        schema_findings.push(json!({
                            "tool": name,
                            "code": "required_name_not_string",
                            "detail": "every inputSchema.required member must be a string",
                        }));
                        schema_valid = false;
                        continue;
                    };
                    if properties.is_none_or(|properties| !properties.contains_key(required_name)) {
                        schema_findings.push(json!({
                            "tool": name,
                            "code": "required_property_missing",
                            "field": required_name,
                            "detail": "required field is absent from inputSchema.properties",
                        }));
                        schema_valid = false;
                    }
                }
            }
            if schema_object.get("type").and_then(Value::as_str) != Some("object") {
                schema_valid = false;
            }
            if encoded_schema.len() > MAX_SCHEMA_BYTES {
                schema_valid = false;
            }
            if schema_valid {
                valid_schema_count += 1;
            }
        }

        let mut memberships: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut group_reports = Vec::new();
        for group in catalogue.groups() {
            let mut unique_tools = BTreeSet::new();
            for tool in &group.mcp_tools {
                unique_tools.insert(tool.clone());
                memberships
                    .entry(tool.clone())
                    .or_default()
                    .push(group.id.clone());
            }
            let missing_tools = unique_tools
                .iter()
                .filter(|tool| !definition_names.contains_key(*tool))
                .cloned()
                .collect::<Vec<_>>();
            group_reports.push(json!({
                "id": group.id,
                "domains": group.domains,
                "status": group.status,
                "declared_tool_memberships": group.mcp_tools.len(),
                "unique_tools": unique_tools.len(),
                "schemas_found": unique_tools.len() - missing_tools.len(),
                "missing_schemas": missing_tools,
            }));
        }

        let catalog_tools = memberships.keys().cloned().collect::<BTreeSet<_>>();
        let advertised_tools = definition_names.keys().cloned().collect::<BTreeSet<_>>();
        let catalog_only_tools = catalog_tools
            .difference(&advertised_tools)
            .cloned()
            .collect::<Vec<_>>();
        let advertised_only_tools = advertised_tools
            .difference(&catalog_tools)
            .cloned()
            .collect::<Vec<_>>();
        let every_catalog_tool_has_schema = catalog_only_tools.is_empty();
        let every_advertised_tool_is_catalogued = advertised_only_tools.is_empty();
        let schemas_are_well_formed = schema_findings.is_empty();
        let duplicate_memberships = memberships
            .into_iter()
            .filter_map(|(tool, groups)| {
                (groups.len() > 1).then_some(json!({
                    "tool": tool,
                    "group_count": groups.len(),
                    "groups": groups,
                }))
            })
            .collect::<Vec<_>>();
        let healthy = every_catalog_tool_has_schema
            && every_advertised_tool_is_catalogued
            && duplicate_schema_names.is_empty()
            && schemas_are_well_formed;
        let schema_names_are_unique = duplicate_schema_names.is_empty();
        let mut output = json!({
            "ok": true,
            "workflow": "capability_audit",
            "capability_schema_version": CAPABILITY_SCHEMA_VERSION,
            "catalog_digest": catalogue.digest().to_string(),
            "healthy": healthy,
            "total_groups": catalogue.groups().len(),
            "catalog_tool_memberships": catalogue.groups().iter().map(|group| group.mcp_tools.len()).sum::<usize>(),
            "unique_catalog_tools": catalog_tools.len(),
            "advertised_tool_count": advertised_tools.len(),
            "catalog_only_tools": catalog_only_tools,
            "advertised_only_tools": advertised_only_tools,
            "duplicate_schema_names": duplicate_schema_names.into_iter().collect::<Vec<_>>(),
            "duplicate_group_memberships": duplicate_memberships,
            "schema_quality": {
                "checked": definition_names.len(),
                "valid": valid_schema_count,
                "total_bytes": schema_bytes,
                "maximum_schema_bytes": MAX_SCHEMA_BYTES,
                "findings": schema_findings,
            },
            "invariants": {
                "every_catalog_tool_has_authoritative_schema": every_catalog_tool_has_schema,
                "every_advertised_tool_is_catalogued": every_advertised_tool_is_catalogued,
                "schema_names_are_unique": schema_names_are_unique,
                "all_input_schemas_are_well_formed": schemas_are_well_formed,
                "multi_group_membership_is_allowed": true,
            },
        });
        if include_groups {
            output["groups"] = Value::Array(group_reports);
        }
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure capability audit result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err("capability audit result exceeds the 20000000-byte safety bound".into());
        }
        Ok(output)
    }

    /// Produce a bounded operator dashboard over catalogue groups and the authoritative MCP
    /// schema set. This is a coverage projection, not a permission or scientific-readiness gate.
    pub(super) fn capability_dashboard(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode capability dashboard input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("capability dashboard input exceeds the 20000000-byte safety bound".into());
        }
        let query: CapabilityDashboardQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid capability dashboard input: {error}"))?;
        let catalogue = CapabilityCatalogue::from_value(&workspace_capabilities())
            .map_err(|error| format!("capability catalogue refused: {error}"))?;
        let mut schema_quality = BTreeMap::new();
        let mut duplicate_names = BTreeSet::new();
        for definition in tool_definitions() {
            let Some(name) = definition.get("name").and_then(Value::as_str) else {
                continue;
            };
            let quality = dashboard_schema_is_valid(&definition);
            if schema_quality.insert(name.to_string(), quality).is_some() {
                duplicate_names.insert(name.to_string());
            }
        }
        let duplicate_schema_names = duplicate_names.iter().cloned().collect::<Vec<_>>();
        for name in duplicate_names {
            schema_quality.insert(name, false);
        }
        let audit = build_dashboard(&catalogue, &schema_quality, &query)
            .map_err(|error| format!("capability dashboard refused: {error}"))?;
        let mut audit_value = serde_json::to_value(&audit).map_err(|error| {
            format!("capability dashboard audit could not be serialized: {error}")
        })?;
        let selected_groups = audit_value
            .get("groups")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| {
                let id = group.get("id").and_then(Value::as_str)?.to_string();
                let domains = group
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Some((id, domains))
            })
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
        let mut groups_with_artifact_evidence = 0usize;
        let mut artifact_evidence_records = 0usize;
        let mut groups_with_workflow_reconciliation = 0usize;
        let mut workflow_reconciliation_records = 0usize;
        let mut evidence_rows = Vec::new();
        if let Some(groups) = audit_value.get_mut("groups").and_then(Value::as_array_mut) {
            for group in groups {
                let Some(group_object) = group.as_object_mut() else {
                    continue;
                };
                let Some(id) = group_object
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                else {
                    continue;
                };
                let artifact_evidence = artifact_postures.get(&id).cloned().unwrap_or_else(|| {
                    json!({
                        "ok": true,
                        "state": "missing",
                        "group_id": id,
                        "matching_record_count": 0,
                        "readiness_claimed": false,
                        "execution": "not_started"
                    })
                });
                let reconciliation_evidence = reconciliation_postures
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| {
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
                group_object.insert("artifact_evidence".into(), artifact_evidence.clone());
                group_object.insert(
                    "workflow_reconciliation_evidence".into(),
                    reconciliation_evidence.clone(),
                );
                evidence_rows.push(json!({
                    "id": id,
                    "artifact_evidence": artifact_evidence,
                    "workflow_reconciliation_evidence": reconciliation_evidence
                }));
            }
        }
        let evidence_scope = "selected_capability_groups_current_digest_verified_artifact_and_workflow_reconciliation_registries";
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
        let evidence_digest =
            bioprism_ids::ContentHash::of_value(&evidence_document).map_err(|error| {
                format!("capability dashboard evidence could not be hashed: {error}")
            })?;
        audit_value["evidence"] = json!({
            "scope": evidence_scope,
            "evidence_digest": evidence_digest.to_string(),
            "artifact_registry_generation": artifact_registry_generation,
            "artifact_registry_size": artifact_registry_size,
            "workflow_reconciliation_registry_generation": workflow_reconciliation_registry_generation,
            "workflow_reconciliation_registry_size": workflow_reconciliation_registry_size,
            "groups_with_artifact_evidence": groups_with_artifact_evidence,
            "artifact_evidence_records": artifact_evidence_records,
            "groups_with_workflow_reconciliation": groups_with_workflow_reconciliation,
            "workflow_reconciliation_records": workflow_reconciliation_records,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "every returned capability group receives an independent artifact and reconciliation posture",
                "postures are joined only from the bounded current registries and preserve their own verification semantics",
                "the evidence digest binds the selected group rows and registry snapshot metadata"
            ],
            "limitations": [
                "registry observations are not one atomic cross-store transaction",
                "artifact or reconciliation presence does not prove execution, scientific validity, release readiness, or external effect completion",
                "bounded or filtered output does not describe unselected capability groups"
            ]
        });
        let output = json!({
            "ok": true,
            "workflow": "capability_dashboard",
            "schema": bioprism_devplat::CAPABILITY_DASHBOARD_SCHEMA,
            "catalog_digest": audit.catalog_digest,
            "dashboard_digest": audit.dashboard_digest,
            "evidence_digest": evidence_digest.to_string(),
            "evidence_scope": evidence_scope,
            "capability_dashboard_ready": audit.ready,
            "audit": audit_value,
            "schema_source": "authoritative tools/list definitions",
            "duplicate_schema_names": duplicate_schema_names,
            "guarantees": [
                "domain rows are sorted and bound to the catalogue digest and query",
                "MCP callable status requires a present, well-formed authoritative input schema",
                "crate, CLI, Python, and MCP surfaces are reported independently rather than collapsed into one score",
                "artifact and workflow-reconciliation evidence are advisory joins and never promote the dashboard to readiness"
            ],
            "limitations": [
                "declared crate, CLI, and Python surfaces are not executed or import-checked",
                "callable means transport-schema availability, not permission, scientific validity, or execution success",
                "filtered or bounded output must not be interpreted as a complete inventory without its warnings",
                "evidence registries are observed independently and do not provide an atomic cross-store snapshot"
            ]
        });
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure capability dashboard result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err(
                "capability dashboard result exceeds the 20000000-byte safety bound".into(),
            );
        }
        Ok(output)
    }
}
