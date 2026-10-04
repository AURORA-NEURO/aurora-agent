//! MCP Laboratory design and acquisition handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn lab_plan(&self, arguments: &Value) -> Result<Value, String> {
        let graph_value = arguments
            .get("graph")
            .cloned()
            .ok_or("graph is required and must be a serialized ObligationGraph")?;
        let graph: bioprism_obligation::ObligationGraph = serde_json::from_value(graph_value)
            .map_err(|error| format!("invalid obligation graph: {error}"))?;
        if graph.len() > 10_000 {
            return Err("graph exceeds the 10000-obligation safety bound".into());
        }

        let action_values = arguments
            .get("actions")
            .cloned()
            .ok_or("actions is required and must be an array of AcquisitionAction values")?;
        let action_values = action_values.as_array().ok_or("actions must be an array")?;
        if action_values.len() > 1_000 {
            return Err("actions exceeds the 1000-action safety bound".into());
        }
        let actions: Vec<AcquisitionAction> = action_values
            .iter()
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("invalid acquisition action: {error}"))
            })
            .collect::<Result<_, _>>()?;

        let budget: AcquisitionCost = serde_json::from_value(
            arguments
                .get("budget")
                .cloned()
                .ok_or("budget is required and must be an AcquisitionCost")?,
        )
        .map_err(|error| format!("invalid acquisition budget: {error}"))?;
        let marginal_value_floor = arguments
            .get("marginal_value_floor")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if !marginal_value_floor.is_finite() || marginal_value_floor < 0.0 {
            return Err("marginal_value_floor must be finite and non-negative".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let separation = if let Some(raw) = arguments.get("hypotheses") {
            let set: LabHypothesisSet = serde_json::from_value(raw.clone())
                .map_err(|error| format!("invalid hypothesis set: {error}"))?;
            if set.len() > 1_000 {
                return Err("hypotheses exceeds the 1000-hypothesis safety bound".into());
            }
            let observations: Observations = arguments
                .get("observations")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("invalid observations map: {error}"))?
                .unwrap_or_default();
            match separate_hypotheses(&set, &graph, &observations) {
                Ok(verdict) => Some(verdict),
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "stage": "separation",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "guarantee": "a lab plan never treats an unseparated hypothesis set as settled"
                    }));
                }
            }
        } else if arguments.get("observations").is_some() {
            return Err("observations may only be supplied with hypotheses".into());
        } else {
            None
        };

        let frontier = graph
            .frontier()
            .map_err(|error| format!("invalid obligation graph: {error}"))?;
        let expansion = match expand_acquisitions(
            &actions,
            &graph,
            budget,
            marginal_value_floor,
            separation.as_ref(),
        ) {
            Ok(expansion) => expansion,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "planning",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "privacy boundaries, obligation reachability, budget, and marginal value are enforced by the in-tree lab contract"
                }));
            }
        };

        Ok(json!({
            "ok": true,
            "goal": graph.goal.as_str(),
            "obligation_count": graph.len(),
            "frontier": frontier.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_frontier": frontier.len().saturating_sub(max_items),
            "separation": separation,
            "ordered": expansion.ordered.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_ordered": expansion.ordered.len().saturating_sub(max_items),
            "excluded": expansion.excluded.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_excluded": expansion.excluded.len().saturating_sub(max_items),
            "spent": expansion.spent,
            "stop": expansion.stop,
            "should_escalate": expansion.should_escalate(),
            "guarantees": [
                "evidence acquisition is ordered by caller-declared value per unit cost, not fabricated information gain",
                "crossing a privacy or permission boundary is exclusion rather than a discount",
                "the obligation graph's effective dependency states gate what can be planned",
                "a robust single surviving hypothesis stops expansion instead of consuming context budget",
            ],
            "limitations": [
                "the tool plans named acquisitions; it does not read files, query databases, run tests, or ask a user",
                "values, costs, privacy declarations, and observations are caller-supplied",
                "hypothesis generation, posterior probabilities, confidence intervals, and execution scheduling are outside the crate",
            ]
        }))
    }

    pub(super) fn lab_space_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure architecture-space input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("architecture-space input exceeds the 20000000-byte safety bound".into());
        }
        let cost_ceiling = arguments
            .get("cost_ceiling")
            .and_then(Value::as_u64)
            .ok_or("cost_ceiling is required and must be a non-negative integer")?;
        if cost_ceiling > 1_000_000_000 {
            return Err("cost_ceiling must be at most 1000000000".into());
        }
        let raw_candidates = arguments
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or("candidates is required and must be an array")?;
        if raw_candidates.is_empty() || raw_candidates.len() > 512 {
            return Err("candidates must contain between 1 and 512 architecture bundles".into());
        }
        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }
        let max_rows = max_rows as usize;
        let include_components = arguments
            .get("include_components")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut space = ArchitectureSpace::new();
        let mut candidate_rows = Vec::with_capacity(raw_candidates.len());
        for (index, raw_candidate) in raw_candidates.iter().enumerate() {
            let candidate: CandidateArchitecture = match serde_json::from_value(
                raw_candidate.clone(),
            ) {
                Ok(candidate) => candidate,
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/lab-space-audit/0.1",
                        "stage": "candidate_decode",
                        "candidate_index": index,
                        "refusal": format!("candidates[{index}] is not a valid CandidateArchitecture: {error}"),
                        "fail_closed": true,
                        "candidate_rows": candidate_rows.iter().take(max_rows).collect::<Vec<_>>(),
                        "candidate_rows_omitted": candidate_rows.len().saturating_sub(max_rows),
                        "candidate_count": raw_candidates.len(),
                        "registered_count": 0,
                        "space_committed": false,
                        "max_rows": max_rows,
                        "guarantees": [
                            "a malformed bundle cannot enter the immutable architecture registry",
                            "a failed audit never advertises a usable partial space"
                        ]
                    }));
                }
            };
            let component_kinds = candidate
                .kinds()
                .into_iter()
                .map(|kind| kind.as_str())
                .collect::<Vec<_>>();
            let edge_count = candidate
                .components
                .iter()
                .map(|component| component.feeds.len())
                .sum::<usize>();
            let parameter_count = candidate
                .components
                .iter()
                .map(|component| component.parameters.len())
                .sum::<usize>();
            if let Err(error) = candidate.validate(cost_ceiling) {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "space_error_serialization_failed"
                    })
                });
                candidate_rows.push(json!({
                    "index": index,
                    "id": candidate.id,
                    "derived_from": candidate.derived_from,
                    "cost_units": candidate.cost_units,
                    "component_count": candidate.components.len(),
                    "edge_count": edge_count,
                    "parameter_count": parameter_count,
                    "component_kinds": component_kinds,
                    "protected_surfaces": candidate.touches_protected,
                    "validation": "refused",
                    "registration": "not_attempted",
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                }));
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-space-audit/0.1",
                    "stage": "candidate_validation",
                    "candidate_index": index,
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "candidate_rows": candidate_rows.iter().take(max_rows).collect::<Vec<_>>(),
                    "candidate_rows_omitted": candidate_rows.len().saturating_sub(max_rows),
                    "candidate_count": raw_candidates.len(),
                    "registered_count": 0,
                    "space_committed": false,
                    "max_rows": max_rows,
                    "guarantees": [
                        "required components, graph acyclicity, dangling edges, protected surfaces, and cost are kernel-validated",
                        "a failed audit never advertises a usable partial space"
                    ],
                    "limitations": [
                        "component implementations and provider availability are declarations outside this crate"
                    ]
                }));
            }
            if let Err(error) = space.register(candidate.clone()) {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "space_error_serialization_failed"
                    })
                });
                candidate_rows.push(json!({
                    "index": index,
                    "id": candidate.id,
                    "derived_from": candidate.derived_from,
                    "cost_units": candidate.cost_units,
                    "component_count": candidate.components.len(),
                    "edge_count": edge_count,
                    "parameter_count": parameter_count,
                    "component_kinds": component_kinds,
                    "protected_surfaces": candidate.touches_protected,
                    "validation": "valid",
                    "registration": "refused",
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                }));
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-space-audit/0.1",
                    "stage": "candidate_registration",
                    "candidate_index": index,
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "candidate_rows": candidate_rows.iter().take(max_rows).collect::<Vec<_>>(),
                    "candidate_rows_omitted": candidate_rows.len().saturating_sub(max_rows),
                    "candidate_count": raw_candidates.len(),
                    "registered_count": 0,
                    "space_committed": false,
                    "max_rows": max_rows,
                    "guarantees": [
                        "duplicate ids and unregistered parents are refused before a space is committed",
                        "a failed audit never advertises a usable partial space"
                    ]
                }));
            }
            let mut row = json!({
                "index": index,
                "id": candidate.id,
                "derived_from": candidate.derived_from,
                "cost_units": candidate.cost_units,
                "component_count": candidate.components.len(),
                "edge_count": edge_count,
                "parameter_count": parameter_count,
                "component_kinds": component_kinds,
                "protected_surfaces": candidate.touches_protected,
                "validation": "valid",
                "registration": "registered",
            });
            if include_components {
                row["components"] =
                    serde_json::to_value(candidate.components).map_err(|error| {
                        format!("cannot serialize architecture components: {error}")
                    })?;
            }
            candidate_rows.push(row);
        }

        let raw_inspect = arguments
            .get("inspect")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| {
                space
                    .iter()
                    .map(|candidate| json!(candidate.id))
                    .collect::<Vec<_>>()
            });
        if raw_inspect.len() > 512 {
            return Err("inspect must contain at most 512 configuration ids".into());
        }
        let mut inspection_rows = Vec::with_capacity(raw_inspect.len());
        for (index, raw_id) in raw_inspect.iter().enumerate() {
            let id = raw_id
                .as_str()
                .ok_or_else(|| format!("inspect[{index}] must be a string"))?;
            if id.is_empty() {
                return Err(format!("inspect[{index}] must not be empty"));
            }
            let configuration = ConfigurationId::new(id);
            let candidate = space.get(&configuration).ok_or_else(|| {
                format!("inspect[{index}] names unknown configuration `{configuration}`")
            })?;
            let lineage = space.lineage(&configuration).map_err(|error| {
                format!("cannot resolve lineage for `{configuration}`: {error}")
            })?;
            let root = lineage.last().cloned();
            let mut row = json!({
                "index": index,
                "configuration": configuration,
                "lineage": lineage,
                "lineage_depth": lineage.len(),
                "root": root,
                "derived_from": candidate.derived_from,
                "component_ids": candidate.components.iter().map(|component| component.id.clone()).collect::<Vec<_>>(),
                "cost_units": candidate.cost_units,
            });
            if include_components {
                row["components"] = serde_json::to_value(&candidate.components)
                    .map_err(|error| format!("cannot serialize inspected components: {error}"))?;
            }
            inspection_rows.push(row);
        }

        let raw_comparisons = arguments
            .get("comparisons")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_comparisons.len() > 512 {
            return Err("comparisons must contain at most 512 pairs".into());
        }
        let mut comparison_rows = Vec::with_capacity(raw_comparisons.len());
        for (index, raw_comparison) in raw_comparisons.iter().enumerate() {
            let object = raw_comparison
                .as_object()
                .ok_or_else(|| format!("comparisons[{index}] must be an object"))?;
            let before = object
                .get("before")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("comparisons[{index}].before must be a string"))?;
            let after = object
                .get("after")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("comparisons[{index}].after must be a string"))?;
            let before_id = ConfigurationId::new(before);
            let after_id = ConfigurationId::new(after);
            let before_candidate = space.get(&before_id).ok_or_else(|| {
                format!("comparisons[{index}] names unknown before configuration `{before_id}`")
            })?;
            let after_candidate = space.get(&after_id).ok_or_else(|| {
                format!("comparisons[{index}] names unknown after configuration `{after_id}`")
            })?;
            comparison_rows.push(json!({
                "index": index,
                "before": before_id,
                "after": after_id,
                "derived_relation": after_candidate.derived_from.as_ref() == Some(&before_id),
                "before_lineage": space.lineage(&before_id).map_err(|error| error.to_string())?,
                "after_lineage": space.lineage(&after_id).map_err(|error| error.to_string())?,
                "changes": before_candidate.diff(after_candidate),
                "change_count": before_candidate.diff(after_candidate).len(),
            }));
        }
        let mut roots = BTreeSet::new();
        let mut lineage_depth_max = 0usize;
        for candidate in space.iter() {
            let lineage = space
                .lineage(&candidate.id)
                .map_err(|error| format!("cannot resolve registered lineage: {error}"))?;
            lineage_depth_max = lineage_depth_max.max(lineage.len());
            if let Some(root) = lineage.last() {
                roots.insert(root.clone());
            }
        }
        let output = json!({
            "ok": true,
            "schema": "bioprism-mcp/lab-space-audit/0.1",
            "cost_ceiling": cost_ceiling,
            "candidate_count": raw_candidates.len(),
            "registered_count": space.len(),
            "space_committed": true,
            "space": {
                "registered_ids": space.iter().map(|candidate| candidate.id.clone()).collect::<Vec<_>>(),
                "root_ids": roots,
                "root_count": roots.len(),
                "lineage_depth_max": lineage_depth_max,
                "required_component_kinds": bioprism_lab::space::ComponentKind::REQUIRED.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(),
                "protected_surfaces": bioprism_lab::space::ProtectedSurface::ALL.iter().map(|surface| json!({
                    "surface": surface.as_str(),
                    "rationale": surface.rationale(),
                })).collect::<Vec<_>>(),
            },
            "candidate_rows": candidate_rows.iter().take(max_rows).collect::<Vec<_>>(),
            "candidate_rows_omitted": candidate_rows.len().saturating_sub(max_rows),
            "inspection_rows": inspection_rows.iter().take(max_rows).collect::<Vec<_>>(),
            "inspection_count": inspection_rows.len(),
            "inspection_rows_omitted": inspection_rows.len().saturating_sub(max_rows),
            "comparison_rows": comparison_rows.iter().take(max_rows).collect::<Vec<_>>(),
            "comparison_count": comparison_rows.len(),
            "comparison_rows_omitted": comparison_rows.len().saturating_sub(max_rows),
            "max_rows": max_rows,
            "guarantees": [
                "every registered bundle passed the kernel's graph, required-kind, protected-surface, and cost checks",
                "parent registration and lineage are resolved from the immutable architecture space",
                "component changes are computed by CandidateArchitecture::diff rather than inferred from ids"
            ],
            "limitations": [
                "component implementations, provider availability, runtime behavior, and declared cost calibration are outside this audit",
                "a valid architecture space is admissible structure, not evidence that any candidate performs well",
                "protected-surface declarations are caller-supplied and require an independent review gate"
            ]
        });
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure architecture-space result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err("architecture-space result exceeds the 20000000-byte safety bound".into());
        }
        Ok(output)
    }

    pub(super) fn lab_pareto_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure Pareto-audit input: {error}"))?;
        if encoded.len() > 10_000_000 {
            return Err("Pareto-audit input exceeds the 10000000-byte safety bound".into());
        }
        let raw_objectives = arguments
            .get("objectives")
            .and_then(Value::as_array)
            .ok_or("objectives is required and must be an array of Objective values")?;
        if raw_objectives.is_empty() || raw_objectives.len() > 64 {
            return Err("objectives must contain between 1 and 64 values".into());
        }
        let objectives: Vec<bioprism_lab::pareto::Objective> = raw_objectives
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let objective: bioprism_lab::pareto::Objective =
                    serde_json::from_value(value.clone())
                        .map_err(|error| format!("invalid objectives[{index}]: {error}"))?;
                if objective.axis.trim().is_empty() || objective.axis.len() > 256 {
                    return Err(format!(
                        "objectives[{index}].axis must contain between 1 and 256 bytes"
                    ));
                }
                Ok(objective)
            })
            .collect::<Result<_, String>>()?;

        let raw_profiles = arguments
            .get("profiles")
            .and_then(Value::as_array)
            .ok_or("profiles is required and must be an array of Profile values")?;
        if raw_profiles.is_empty() || raw_profiles.len() > 512 {
            return Err("profiles must contain between 1 and 512 values".into());
        }
        let profiles: Vec<LabParetoProfile> = raw_profiles
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let profile: LabParetoProfile = serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid profiles[{index}]: {error}"))?;
                if profile.candidate.as_str().trim().is_empty()
                    || profile.candidate.as_str().len() > 512
                {
                    return Err(format!(
                        "profiles[{index}].candidate must contain between 1 and 512 bytes"
                    ));
                }
                if profile.values.len() > 64 {
                    return Err(format!(
                        "profiles[{index}].values must contain at most 64 axes"
                    ));
                }
                if profile
                    .values
                    .keys()
                    .any(|axis| axis.trim().is_empty() || axis.len() > 256)
                {
                    return Err(format!(
                        "profiles[{index}].values axis names must contain between 1 and 256 bytes"
                    ));
                }
                Ok(profile)
            })
            .collect::<Result<_, String>>()?;

        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }
        let max_rows = max_rows as usize;

        let mut front = match ParetoFront::new(objectives.clone()) {
            Ok(front) => front,
            Err(error) => {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "pareto_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-pareto-audit/0.1",
                    "stage": "objective_validation",
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "an empty or duplicate objective set never becomes a vacuous front",
                        "no partial Pareto archive is returned after objective validation fails"
                    ]
                }));
            }
        };

        let mut admissions = Vec::with_capacity(profiles.len());
        for (index, profile) in profiles.into_iter().enumerate() {
            let candidate = profile.candidate.to_string();
            let admission = match front.insert(profile) {
                Ok(admission) => admission,
                Err(error) => {
                    let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                        json!({
                            "error": "pareto_error_serialization_failed"
                        })
                    });
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/lab-pareto-audit/0.1",
                        "stage": "profile_insertion",
                        "profile_index": index,
                        "candidate": candidate,
                        "refusal": error.to_string(),
                        "error": detail,
                        "fail_closed": true,
                        "inserted_profiles": index,
                        "guarantees": [
                            "a profile with an absent or unknown objective axis is refused rather than imputed",
                            "a failed archive build never returns the partial front as if it were complete"
                        ]
                    }));
                }
            };
            admissions.push(json!({
                "input_index": index,
                "candidate": candidate,
                "admission": admission,
            }));
        }

        let raw_relations = arguments
            .get("relations")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_relations = raw_relations
            .as_array()
            .ok_or("relations must be an array when supplied")?;
        if raw_relations.len() > 256 {
            return Err("relations exceeds the 256-relation safety bound".into());
        }
        let mut relations = Vec::with_capacity(raw_relations.len());
        for (index, raw_relation) in raw_relations.iter().enumerate() {
            let object = raw_relation
                .as_object()
                .ok_or_else(|| format!("relations[{index}] must be an object"))?;
            let left = object
                .get("left")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("relations[{index}].left must be a string"))?;
            let right = object
                .get("right")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("relations[{index}].right must be a string"))?;
            if left.trim().is_empty() || right.trim().is_empty() {
                return Err(format!("relations[{index}] identifiers must be non-empty"));
            }
            let left_id = bioprism_lab::space::ConfigurationId::new(left);
            let right_id = bioprism_lab::space::ConfigurationId::new(right);
            let relation = match front.relation(&left_id, &right_id) {
                Ok(relation) => relation,
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/lab-pareto-audit/0.1",
                        "stage": "relation_projection",
                        "relation_index": index,
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "guarantees": [
                            "pairwise relations are emitted only for candidates that remain on the final front",
                            "an unavailable relation never becomes an inferred dominance result"
                        ]
                    }));
                }
            };
            relations.push(json!({
                "left": left,
                "right": right,
                "relation": relation,
            }));
        }

        let archived = front
            .archived()
            .iter()
            .take(max_rows)
            .map(|(profile, dominated_by)| {
                json!({
                    "profile": profile,
                    "dominated_by": dominated_by,
                })
            })
            .collect::<Vec<_>>();
        let unresolved = front.unresolved();
        let selection = front.select();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/lab-pareto-audit/0.1",
            "objective_count": front.objectives().len(),
            "profile_count": admissions.len(),
            "objectives": front.objectives(),
            "admissions": admissions.iter().take(max_rows).collect::<Vec<_>>(),
            "admissions_omitted": admissions.len().saturating_sub(max_rows),
            "front": {
                "count": front.members().len(),
                "members": front.members(),
                "unresolved_count": unresolved.len(),
                "unresolved": unresolved,
                "selection": selection,
            },
            "archived_count": front.archived().len(),
            "archived": archived,
            "archived_omitted": front.archived().len().saturating_sub(max_rows),
            "relations": relations.iter().take(max_rows).collect::<Vec<_>>(),
            "relations_omitted": relations.len().saturating_sub(max_rows),
            "max_rows": max_rows,
            "guarantees": [
                "dominance is computed by the in-tree Pareto kernel over every declared objective",
                "trade-offs remain incomparable and survive on the front without a caller-hidden scalarizer",
                "unmeasured axes remain unresolved rather than being treated as zero, worst-case, or missing-at-random",
                "dominated profiles remain archived with their dominator and are not erased from the audit",
                "a multi-member front returns ambiguous selection rather than an invented deployment choice"
            ],
            "limitations": [
                "objective directions and profile values are caller-declared point measurements; no statistical uncertainty or replication model is inferred",
                "the archive describes candidate declarations and does not execute components, select providers, or validate biological performance",
                "a Pareto front is a decision surface, not a release approval, safety gate, or evidence of a deployable architecture"
            ]
        }))
    }

    pub(super) fn lab_branch_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure branch-audit input: {error}"))?;
        if encoded.len() > 10_000_000 {
            return Err("branch-audit input exceeds the 10000000-byte safety bound".into());
        }
        let raw_policy = arguments
            .get("policy")
            .cloned()
            .ok_or("policy is required and must be a serialized BranchPolicy")?;
        let serialized_policy: BranchPolicy = serde_json::from_value(raw_policy)
            .map_err(|error| format!("invalid branch policy: {error}"))?;
        let policy = match BranchPolicy::new(
            serialized_policy.ceiling,
            serialized_policy.on_undetermined,
            serialized_policy.rules().to_vec(),
        ) {
            Ok(policy) => policy,
            Err(error) => {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "branch_policy_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-branch-audit/0.1",
                    "stage": "policy_validation",
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "vacuous triggers and rules over hard ceilings are rejected before any decision is planned",
                        "a policy-validation refusal emits no partial branch ledger"
                    ]
                }));
            }
        };

        let raw_decisions = arguments
            .get("decisions")
            .and_then(Value::as_array)
            .ok_or("decisions is required and must be an array")?;
        if raw_decisions.is_empty() || raw_decisions.len() > 512 {
            return Err("decisions must contain between 1 and 512 objects".into());
        }
        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }
        let max_rows = max_rows as usize;
        let mut ledger = BranchLedger::new();
        let mut rows = Vec::with_capacity(raw_decisions.len());
        for (index, raw_decision) in raw_decisions.iter().enumerate() {
            let object = raw_decision
                .as_object()
                .ok_or_else(|| format!("decisions[{index}] must be an object"))?;
            let decision = object
                .get("decision")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("decisions[{index}].decision must be a string"))?;
            if decision.trim().is_empty() || decision.len() > 512 {
                return Err(format!(
                    "decisions[{index}].decision must contain between 1 and 512 bytes"
                ));
            }
            let features: RiskFeatures = serde_json::from_value(
                object
                    .get("features")
                    .cloned()
                    .ok_or_else(|| format!("decisions[{index}].features is required"))?,
            )
            .map_err(|error| format!("invalid decisions[{index}].features: {error}"))?;
            if features
                .historical_failure_rate
                .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
            {
                return Err(format!(
                    "decisions[{index}].features.historical_failure_rate must be finite and lie in [0, 1]"
                ));
            }
            let plan = policy.plan(&features);
            let mut outcome = BranchOutcome::new(decision, plan);
            if let Some(caught) = object.get("caught") {
                let caught = caught
                    .as_object()
                    .ok_or_else(|| format!("decisions[{index}].caught must be an object"))?;
                let what = caught
                    .get("what")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("decisions[{index}].caught.what must be a string"))?;
                let would_have_been = caught
                    .get("would_have_been")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        format!("decisions[{index}].caught.would_have_been must be a string")
                    })?;
                if what.trim().is_empty() || would_have_been.trim().is_empty() {
                    return Err(format!(
                        "decisions[{index}].caught fields must be non-empty"
                    ));
                }
                outcome = outcome.catching(what, would_have_been);
            }
            if let Some(escaped) = object.get("escaped") {
                let escaped = escaped
                    .as_str()
                    .ok_or_else(|| format!("decisions[{index}].escaped must be a string"))?;
                if escaped.trim().is_empty() {
                    return Err(format!("decisions[{index}].escaped must be non-empty"));
                }
                outcome = outcome.with_escape(escaped);
            }
            rows.push(json!({
                "index": index,
                "decision": decision,
                "features": features,
                "outcome": outcome,
            }));
            ledger.record(outcome);
        }

        let yielded = ledger.report();
        let verdict = yielded.verdict();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/lab-branch-audit/0.1",
            "policy": policy,
            "decision_count": yielded.decisions,
            "yield": yielded,
            "verdict": verdict,
            "rows": rows.iter().take(max_rows).collect::<Vec<_>>(),
            "rows_omitted": rows.len().saturating_sub(max_rows),
            "max_rows": max_rows,
            "guarantees": [
                "the first matching stated rule controls each decision and its trigger prose remains attached",
                "undetermined historical risk is reported separately and follows the declared escalation policy",
                "the denominator includes decisions where no rule fired, not only escalations",
                "spent branches, verifier calls, catches, wasted escalations, and escaped harms remain separate",
                "a caught harm carries the counterfactual single-path outcome it claims to have prevented"
            ],
            "limitations": [
                "this endpoint plans and audits branching; it does not fork execution, invoke a verifier, or execute a tool",
                "risk features, trigger thresholds, catches, escapes, and counterfactual descriptions are caller-supplied",
                "the ledger measures declared branch utility and does not calibrate a learned risk model or infer causal benefit"
            ]
        }))
    }

    pub(super) fn lab_holdout_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure holdout-audit input: {error}"))?;
        if encoded.len() > 10_000_000 {
            return Err("holdout-audit input exceeds the 10000000-byte safety bound".into());
        }
        let cost_ceiling = arguments
            .get("cost_ceiling")
            .and_then(Value::as_u64)
            .ok_or("cost_ceiling is required and must be a non-negative integer")?;
        if cost_ceiling > 1_000_000_000 {
            return Err("cost_ceiling must be at most 1000000000".into());
        }
        let raw_candidates = arguments
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or("candidates is required and must be an array of CandidateArchitecture values")?;
        if raw_candidates.is_empty() || raw_candidates.len() > 512 {
            return Err("candidates must contain between 1 and 512 values".into());
        }
        let mut space = ArchitectureSpace::new();
        for (index, raw_candidate) in raw_candidates.iter().enumerate() {
            let candidate: CandidateArchitecture = serde_json::from_value(raw_candidate.clone())
                .map_err(|error| format!("invalid candidates[{index}]: {error}"))?;
            if candidate.id.as_str().trim().is_empty() || candidate.id.as_str().len() > 512 {
                return Err(format!(
                    "candidates[{index}].id must contain between 1 and 512 bytes"
                ));
            }
            if let Err(error) = candidate.validate(cost_ceiling) {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "space_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-holdout-audit/0.1",
                    "stage": "architecture_validation",
                    "candidate_index": index,
                    "candidate": candidate.id,
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "no deployment state is created from an invalid architecture bundle",
                        "required components, graph integrity, protected surfaces, and cost ceiling are checked before holdout operations"
                    ]
                }));
            }
            if let Err(error) = space.register(candidate) {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "space_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-holdout-audit/0.1",
                    "stage": "architecture_registration",
                    "candidate_index": index,
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "duplicate bundles and unregistered parents are refused rather than rebound or silently orphaned"
                    ]
                }));
            }
        }

        let raw_holdouts = arguments
            .get("holdouts")
            .and_then(Value::as_array)
            .ok_or("holdouts is required and must be an array")?;
        if raw_holdouts.is_empty() || raw_holdouts.len() > 128 {
            return Err("holdouts must contain between 1 and 128 values".into());
        }
        let mut holdouts = HoldoutLedger::new();
        for (index, raw_holdout) in raw_holdouts.iter().enumerate() {
            let object = raw_holdout
                .as_object()
                .ok_or_else(|| format!("holdouts[{index}] must be an object"))?;
            let id = object
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("holdouts[{index}].id must be a string"))?;
            if id.trim().is_empty() || id.len() > 512 {
                return Err(format!(
                    "holdouts[{index}].id must contain between 1 and 512 bytes"
                ));
            }
            let partition: Partition = serde_json::from_value(
                object
                    .get("partition")
                    .cloned()
                    .ok_or_else(|| format!("holdouts[{index}].partition is required"))?,
            )
            .map_err(|error| format!("invalid holdouts[{index}].partition: {error}"))?;
            let query_budget = object
                .get("query_budget")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    format!("holdouts[{index}].query_budget must be a non-negative integer")
                })?;
            if query_budget > u32::MAX as u64 {
                return Err(format!("holdouts[{index}].query_budget exceeds u32"));
            }
            holdouts
                .register(Holdout::new(id, partition, query_budget as u32))
                .map_err(|error| {
                    format!("holdout registration refused holdouts[{index}]: {error}")
                })?;
        }

        let current = arguments
            .get("current")
            .and_then(Value::as_str)
            .ok_or("current is required and must name a registered configuration")?;
        let current = ConfigurationId::new(current);
        let mut deployment = match Deployment::new(space, holdouts, current) {
            Ok(deployment) => deployment,
            Err(error) => {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "rollback_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-holdout-audit/0.1",
                    "stage": "deployment_initialization",
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "deployment state is created only at a registered configuration"
                    ]
                }));
            }
        };
        let raw_operations = arguments
            .get("operations")
            .and_then(Value::as_array)
            .ok_or("operations is required and must be an array")?;
        if raw_operations.is_empty() || raw_operations.len() > 2_000 {
            return Err("operations must contain between 1 and 2000 values".into());
        }
        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }
        let max_rows = max_rows as usize;
        let mut checkpoints: BTreeMap<String, Checkpoint> = BTreeMap::new();
        let mut rows = Vec::with_capacity(raw_operations.len());
        let mut measurement_count = 0usize;
        let mut measurement_refusal_count = 0usize;
        let mut rollback_count = 0usize;
        for (index, raw_operation) in raw_operations.iter().enumerate() {
            let object = raw_operation
                .as_object()
                .ok_or_else(|| format!("operations[{index}] must be an object"))?;
            let kind = object
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("operations[{index}].kind must be a string"))?;
            let row = match kind {
                "checkpoint" => {
                    let label = object
                        .get("label")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("operations[{index}].label must be a string"))?;
                    if label.trim().is_empty() || label.len() > 512 {
                        return Err(format!(
                            "operations[{index}].label must contain between 1 and 512 bytes"
                        ));
                    }
                    if checkpoints.contains_key(label) {
                        return Err(format!(
                            "operations[{index}] duplicates checkpoint label {label:?}"
                        ));
                    }
                    let checkpoint = deployment.checkpoint(label);
                    checkpoints.insert(label.to_string(), checkpoint.clone());
                    json!({
                        "index": index,
                        "kind": kind,
                        "result": "accepted",
                        "checkpoint": checkpoint,
                    })
                }
                "promote" => {
                    let configuration = object
                        .get("configuration")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("operations[{index}].configuration must be a string")
                        })?;
                    let configuration = ConfigurationId::new(configuration);
                    let selected_using = object
                        .get("selected_using")
                        .map(|value| {
                            value.as_str().map(HoldoutId::new).ok_or_else(|| {
                                format!("operations[{index}].selected_using must be a string")
                            })
                        })
                        .transpose()?;
                    let rationale = object
                        .get("rationale")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("operations[{index}].rationale must be a string"))?;
                    deployment
                        .promote(&configuration, selected_using.as_ref(), rationale)
                        .map_err(|error| {
                            format!("operations[{index}] promotion refused: {error}")
                        })?;
                    json!({
                        "index": index,
                        "kind": kind,
                        "result": "accepted",
                        "current": deployment.current(),
                        "selected_using": selected_using,
                    })
                }
                "search" => {
                    let holdout = object
                        .get("holdout")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("operations[{index}].holdout must be a string"))?;
                    let raw_configurations = object
                        .get("configurations")
                        .and_then(Value::as_array)
                        .ok_or_else(|| {
                            format!("operations[{index}].configurations must be an array")
                        })?;
                    if raw_configurations.is_empty() || raw_configurations.len() > 512 {
                        return Err(format!(
                            "operations[{index}].configurations must contain between 1 and 512 ids"
                        ));
                    }
                    let configurations: Vec<ConfigurationId> = raw_configurations
                        .iter()
                        .enumerate()
                        .map(|(item_index, value)| {
                            let id = value.as_str().ok_or_else(|| {
                                format!("operations[{index}].configurations[{item_index}] must be a string")
                            })?;
                            let id = ConfigurationId::new(id);
                            if !deployment.space.contains(&id) {
                                return Err(format!("operations[{index}] names unknown configuration {id}"));
                            }
                            Ok(id)
                        })
                        .collect::<Result<_, String>>()?;
                    deployment
                        .holdouts
                        .record_search(&HoldoutId::new(holdout), &configurations)
                        .map_err(|error| format!("operations[{index}] search refused: {error}"))?;
                    json!({
                        "index": index,
                        "kind": kind,
                        "result": "accepted",
                        "holdout": holdout,
                        "configurations": configurations,
                    })
                }
                "measure" => {
                    let holdout = object
                        .get("holdout")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("operations[{index}].holdout must be a string"))?;
                    let configuration = object
                        .get("configuration")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("operations[{index}].configuration must be a string")
                        })?;
                    let metric = object
                        .get("metric")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("operations[{index}].metric must be a string"))?;
                    let value = object
                        .get("value")
                        .and_then(Value::as_f64)
                        .ok_or_else(|| format!("operations[{index}].value must be a number"))?;
                    let result = deployment.holdouts.measure(
                        &HoldoutId::new(holdout),
                        &deployment.space,
                        &ConfigurationId::new(configuration),
                        metric,
                        value,
                    );
                    match result {
                        Ok(measurement) => {
                            measurement_count += 1;
                            json!({
                                "index": index,
                                "kind": kind,
                                "result": "clean_measurement",
                                "measurement": measurement,
                            })
                        }
                        Err(error) => {
                            measurement_refusal_count += 1;
                            let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                                json!({
                                    "error": "holdout_error_serialization_failed"
                                })
                            });
                            json!({
                                "index": index,
                                "kind": kind,
                                "result": "measurement_refused",
                                "refusal": error.to_string(),
                                "error": detail,
                                "fail_closed": true,
                            })
                        }
                    }
                }
                "rollback" => {
                    let label = object
                        .get("checkpoint")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("operations[{index}].checkpoint must be a string")
                        })?;
                    let checkpoint = checkpoints.get(label).cloned().ok_or_else(|| {
                        format!("operations[{index}] names unknown checkpoint {label:?}")
                    })?;
                    let receipt = deployment.rollback(&checkpoint).map_err(|error| {
                        format!("operations[{index}] rollback refused: {error}")
                    })?;
                    rollback_count += 1;
                    json!({
                        "index": index,
                        "kind": kind,
                        "result": "accepted",
                        "receipt": receipt,
                        "complete_restoration": receipt.is_complete_restoration(),
                    })
                }
                _ => {
                    return Err(format!(
                        "operations[{index}].kind {kind:?} is not one of checkpoint, promote, search, measure, rollback"
                    ));
                }
            };
            rows.push(row);
        }

        let holdout_rows = deployment
            .holdouts
            .iter()
            .map(|holdout| {
                json!({
                    "id": holdout.id,
                    "partition": holdout.partition,
                    "certifies": holdout.partition.certifies(),
                    "reuse_note": holdout.partition.reuse_note(),
                    "query_budget": holdout.query_budget,
                    "queries_used": holdout.queries_used(),
                    "remaining_budget": holdout.query_budget.saturating_sub(holdout.queries_used()),
                    "retired": holdout.is_retired(),
                    "watermark": holdout.watermark(),
                    "exposure": holdout.exposure(),
                })
            })
            .collect::<Vec<_>>();
        let permanently_burned = holdout_rows
            .iter()
            .flat_map(|row| {
                row.get("exposure")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|event| {
                        let consumes = event
                            .get("kind")
                            .and_then(|kind| kind.get("kind"))
                            .and_then(Value::as_str)
                            .map(|kind| kind != "rollback")
                            .unwrap_or(false);
                        if consumes {
                            Some(json!({
                                "holdout": row.get("id"),
                                "configuration": event.get("configuration"),
                                "event": event.get("seq"),
                            }))
                        } else {
                            None
                        }
                    })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/lab-holdout-audit/0.1",
            "current": deployment.current(),
            "space": {
                "candidate_count": deployment.space.len(),
                "registered_ids": deployment.space.iter().map(|candidate| candidate.id.clone()).collect::<Vec<_>>(),
            },
            "holdouts": holdout_rows,
            "remaining_certification_budget": deployment.holdouts.remaining_certification_budget(),
            "checkpoints": checkpoints.values().collect::<Vec<_>>(),
            "checkpoint_count": checkpoints.len(),
            "history": deployment.history(),
            "operations": rows.iter().take(max_rows).collect::<Vec<_>>(),
            "operations_omitted": rows.len().saturating_sub(max_rows),
            "operation_count": rows.len(),
            "measurement_count": measurement_count,
            "measurement_refusal_count": measurement_refusal_count,
            "rollback_count": rollback_count,
            "permanently_burned": permanently_burned,
            "max_rows": max_rows,
            "guarantees": [
                "candidate architecture validation and parent registration happen before deployment state is created",
                "clean measurements can only come from HoldoutLedger::measure after lineage and exposure checks",
                "measurement refusals remain typed rows and are never emitted as clean measurements",
                "selection and search exposure propagate through lineage and survive rollback",
                "rollback restores the complete known configuration but reports exposure retained since the checkpoint",
                "rollback never rewinds the append-only holdout ledger"
            ],
            "limitations": [
                "architecture components and metric values are caller-declared; no component or benchmark execution occurs",
                "a successful clean point measurement is not a confidence interval or a release approval",
                "the endpoint records the residual human-use risk of reading a baseline without an exposure event",
                "operations are an offline audit program and do not deploy traffic, trigger live rollback, or alter external state"
            ]
        }))
    }

    pub(super) fn lab_evolution_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure evolution-audit input: {error}"))?;
        if encoded.len() > 10_000_000 {
            return Err("evolution-audit input exceeds the 10000000-byte safety bound".into());
        }
        let cost_ceiling = arguments
            .get("cost_ceiling")
            .and_then(Value::as_u64)
            .ok_or("cost_ceiling is required and must be a non-negative integer")?;
        if cost_ceiling > 1_000_000_000 {
            return Err("cost_ceiling must be at most 1000000000".into());
        }
        let raw_candidates = arguments
            .get("candidates")
            .and_then(Value::as_array)
            .ok_or("candidates is required and must be an array")?;
        if raw_candidates.len() != 2 {
            return Err(
                "candidates must contain exactly baseline and candidate architecture bundles"
                    .into(),
            );
        }
        let mut space = ArchitectureSpace::new();
        for (index, raw_candidate) in raw_candidates.iter().enumerate() {
            let candidate: CandidateArchitecture = serde_json::from_value(raw_candidate.clone())
                .map_err(|error| format!("invalid candidates[{index}]: {error}"))?;
            if let Err(error) = candidate.validate(cost_ceiling) {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "space_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                    "stage": "architecture_validation",
                    "candidate_index": index,
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "an evolution card cannot be built from an invalid architecture bundle"
                    ]
                }));
            }
            if let Err(error) = space.register(candidate) {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "space_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                    "stage": "architecture_registration",
                    "candidate_index": index,
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "guarantees": [
                        "duplicate bundles and unregistered parents are refused before measurement"
                    ]
                }));
            }
        }

        let baseline = arguments
            .get("baseline")
            .and_then(Value::as_str)
            .ok_or("baseline is required and must name one candidate")?;
        let candidate = arguments
            .get("candidate")
            .and_then(Value::as_str)
            .ok_or("candidate is required and must name one candidate")?;
        let baseline = ConfigurationId::new(baseline);
        let candidate = ConfigurationId::new(candidate);
        if !space.contains(&baseline) || !space.contains(&candidate) || baseline == candidate {
            return Err("baseline and candidate must be distinct registered configurations".into());
        }
        let holdout_value = arguments
            .get("holdout")
            .cloned()
            .ok_or("holdout is required and must contain id, partition, and query_budget")?;
        let holdout_object = holdout_value
            .as_object()
            .ok_or("holdout must be an object")?;
        let holdout_id = holdout_object
            .get("id")
            .and_then(Value::as_str)
            .ok_or("holdout.id must be a string")?;
        let partition: Partition = serde_json::from_value(
            holdout_object
                .get("partition")
                .cloned()
                .ok_or("holdout.partition is required")?,
        )
        .map_err(|error| format!("invalid holdout.partition: {error}"))?;
        let query_budget = holdout_object
            .get("query_budget")
            .and_then(Value::as_u64)
            .ok_or("holdout.query_budget must be a non-negative integer")?;
        if query_budget > u32::MAX as u64 {
            return Err("holdout.query_budget exceeds u32".into());
        }
        let mut holdouts = HoldoutLedger::new();
        holdouts
            .register(Holdout::new(holdout_id, partition, query_budget as u32))
            .map_err(|error| format!("holdout registration refused: {error}"))?;

        let raw_proposal = arguments
            .get("proposal")
            .cloned()
            .ok_or("proposal is required and must be a serialized ChangeProposal")?;
        let proposal: ChangeProposal = serde_json::from_value(raw_proposal)
            .map_err(|error| format!("invalid evolution proposal: {error}"))?;
        let card_id = arguments
            .get("card_id")
            .and_then(Value::as_str)
            .ok_or("card_id is required and must be a string")?;
        let rollback_handle = ConfigurationId::new(
            arguments
                .get("rollback_handle")
                .and_then(Value::as_str)
                .ok_or("rollback_handle is required and must be a string")?,
        );
        let direction: LabParetoDirection = serde_json::from_value(
            arguments
                .get("direction")
                .cloned()
                .ok_or("direction is required and must be higher_is_better or lower_is_better")?,
        )
        .map_err(|error| format!("invalid improvement direction: {error}"))?;
        let defeaters = arguments
            .get("would_have_to_be_true")
            .and_then(Value::as_array)
            .ok_or("would_have_to_be_true is required and must be an array")?;
        if defeaters.is_empty() || defeaters.len() > 128 {
            return Err("would_have_to_be_true must contain between 1 and 128 statements".into());
        }
        let defeaters: Vec<String> = defeaters
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let statement = value
                    .as_str()
                    .ok_or_else(|| format!("would_have_to_be_true[{index}] must be a string"))?;
                if statement.len() > 2_000 {
                    return Err(format!("would_have_to_be_true[{index}] exceeds 2000 bytes"));
                }
                Ok(statement.to_string())
            })
            .collect::<Result<_, String>>()?;
        let raw_measurements = arguments
            .get("measurements")
            .and_then(Value::as_array)
            .ok_or("measurements is required and must be an array")?;
        if raw_measurements.is_empty() || raw_measurements.len() > 256 {
            return Err("measurements must contain between 1 and 256 values".into());
        }
        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }
        let max_rows = max_rows as usize;
        let mut before_measurement = None;
        let mut after_measurement = None;
        let mut first_refusal: Option<(ConfigurationId, bioprism_lab::error::HoldoutError)> = None;
        let mut measurement_rows = Vec::with_capacity(raw_measurements.len());
        for (index, raw_measurement) in raw_measurements.iter().enumerate() {
            let object = raw_measurement
                .as_object()
                .ok_or_else(|| format!("measurements[{index}] must be an object"))?;
            let configuration = object
                .get("configuration")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("measurements[{index}].configuration must be a string"))?;
            let metric = object
                .get("metric")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("measurements[{index}].metric must be a string"))?;
            let value = object
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("measurements[{index}].value must be a number"))?;
            let configuration_id = ConfigurationId::new(configuration);
            let result = holdouts.measure(
                &HoldoutId::new(holdout_id),
                &space,
                &configuration_id,
                metric,
                value,
            );
            match result {
                Ok(measurement) => {
                    if configuration_id == baseline && before_measurement.is_none() {
                        before_measurement = Some(measurement.clone());
                    }
                    if configuration_id == candidate && after_measurement.is_none() {
                        after_measurement = Some(measurement.clone());
                    }
                    measurement_rows.push(json!({
                        "index": index,
                        "configuration": configuration_id,
                        "result": "clean_measurement",
                        "measurement": measurement,
                    }));
                }
                Err(error) => {
                    if first_refusal.is_none() {
                        first_refusal = Some((configuration_id.clone(), error.clone()));
                    }
                    let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                        json!({
                            "error": "holdout_error_serialization_failed"
                        })
                    });
                    measurement_rows.push(json!({
                        "index": index,
                        "configuration": configuration_id,
                        "result": "measurement_refused",
                        "refusal": error.to_string(),
                        "error": detail,
                        "fail_closed": true,
                    }));
                }
            }
        }

        if let Some((refused_configuration, refusal)) = first_refusal {
            let card = EvolutionCard::contaminated(
                card_id,
                proposal,
                &baseline,
                &candidate,
                ContaminationRecord {
                    holdout: HoldoutId::new(holdout_id),
                    configuration: refused_configuration,
                    refusal: refusal.clone(),
                },
                &rollback_handle,
                defeaters,
            );
            let claim_refusal = card
                .claim_improvement(direction)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "contaminated card unexpectedly yielded a claim".to_string());
            return Ok(json!({
                "ok": true,
                "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                "status": "contaminated",
                "claimable": false,
                "card": card,
                "claim_refusal": claim_refusal,
                "measurement_count": measurement_rows.len(),
                "max_rows": max_rows,
                "measurement_rows": measurement_rows.iter().take(max_rows).collect::<Vec<_>>(),
                "measurement_rows_omitted": measurement_rows.len().saturating_sub(max_rows),
                "guarantees": [
                    "a contaminated measurement is retained as a card but cannot become an improvement claim",
                    "the clean-measurement type is minted only by the holdout ledger and is never deserialized"
                ],
                "limitations": [
                    "the card records caller-supplied metric values and proposal declarations; no benchmark execution occurs",
                    "a contaminated attempt is evidence of holdout use, not evidence of the candidate's quality"
                ]
            }));
        }

        let (Some(before), Some(after)) = (before_measurement, after_measurement) else {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                "stage": "measurement_completeness",
                "refusal": "both baseline and candidate require clean measurements before an evolution card can be assembled",
                "fail_closed": true,
                "measurement_count": measurement_rows.len(),
                "max_rows": max_rows,
                "measurement_rows": measurement_rows.iter().take(max_rows).collect::<Vec<_>>(),
                "measurement_rows_omitted": measurement_rows.len().saturating_sub(max_rows),
                "guarantees": [
                    "missing before or after evidence is not converted into a zero delta or a partial improvement claim"
                ]
            }));
        };
        let card = match EvolutionCard::measured(
            card_id,
            proposal,
            before,
            after,
            &rollback_handle,
            defeaters,
        ) {
            Ok(card) => card,
            Err(error) => {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "evolution_error_serialization_failed"
                    })
                });
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                    "stage": "card_validation",
                    "refusal": error.to_string(),
                    "error": detail,
                    "fail_closed": true,
                    "measurement_count": measurement_rows.len(),
                    "max_rows": max_rows,
                    "measurement_rows": measurement_rows.iter().take(max_rows).collect::<Vec<_>>(),
                    "measurement_rows_omitted": measurement_rows.len().saturating_sub(max_rows),
                    "guarantees": [
                        "protected surfaces, changed-artifact declarations, defeaters, and metric/surface agreement gate card construction"
                    ]
                }));
            }
        };
        match card.claim_improvement(direction) {
            Ok(claim) => Ok(json!({
                "ok": true,
                "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                "status": "improvement_claimed",
                "claimable": true,
                "card": card,
                "claim": claim,
                "sentence": claim.to_sentence(),
                "measurement_count": measurement_rows.len(),
                "max_rows": max_rows,
                "measurement_rows": measurement_rows.iter().take(max_rows).collect::<Vec<_>>(),
                "measurement_rows_omitted": measurement_rows.len().saturating_sub(max_rows),
                "guarantees": [
                    "the claim is produced only from two clean measurements on one certifying surface",
                    "direction, rollback handle, changed artifacts, and defeaters remain part of the claim"
                ],
                "limitations": [
                    "a point delta is not a confidence interval, causal effect, biological validity result, or release approval",
                    "the caller supplies the metric values and must independently support the stated defeaters"
                ]
            })),
            Err(error) => {
                let detail = serde_json::to_value(&error).unwrap_or_else(|_| {
                    json!({
                        "error": "evolution_error_serialization_failed"
                    })
                });
                Ok(json!({
                    "ok": true,
                    "schema": "bioprism-mcp/lab-evolution-audit/0.1",
                    "status": "claim_refused",
                    "claimable": false,
                    "card": card,
                    "claim_refusal": error.to_string(),
                    "claim_error": detail,
                    "measurement_count": measurement_rows.len(),
                    "max_rows": max_rows,
                    "measurement_rows": measurement_rows.iter().take(max_rows).collect::<Vec<_>>(),
                    "measurement_rows_omitted": measurement_rows.len().saturating_sub(max_rows),
                    "guarantees": [
                        "a clean before/after card that is not an improvement remains a negative result",
                        "the endpoint never upgrades a non-improvement or missing rollback handle into a claim"
                    ],
                    "limitations": [
                        "the claim refusal is a kernel judgment over caller-supplied point measurements, not a statistical test"
                    ]
                }))
            }
        }
    }

    pub(super) fn lab_instrument_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a LaboratoryIntegrationRequest4")?;
        let receipt =
            crate::research_contracts::operate_lab_instrument_interoperability_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lab::LABORATORY_INTEGRATION_FEATURE_ID,
            "contract_version": bioprism_lab::LABORATORY_INTEGRATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "instrument endpoints are ranked deterministically by evidence, freshness, and stable identity",
                "endpoint, capability, interlock, policy, federation, locality, aggregate-only, replay, provenance, health, and adversarial gates fail closed",
                "missing, stale, unknown, speculative, contradictory, omitted, revoked, and unhealthy states remain explicit and no hardware is contacted"
            ],
            "limitations": [
                "the gateway negotiates caller-supplied endpoint attestations and does not connect to instruments or dispatch actions",
                "a qualified receipt is an interoperability preflight artifact, not hardware readiness, experiment execution, scientific validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn lab_semantic_parity(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a LabSemanticParityRequest")?;
        let receipt = crate::research_contracts::evaluate_semantic_parity_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lab::SEMANTIC_PARITY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "institution summaries are compared by canonical semantic and scenario identities",
                "semantic disagreement remains unknown rather than a consensus",
                "the route evaluates typed summaries and performs no instrument or raw-data effect"
            ],
            "limitations": [
                "the route does not rerun protocol simulations or inspect institution-local raw outputs",
                "a passed parity receipt is a benchmark admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn lab_federated_experiment_design_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::run_lab_federated_experiment_design_interoperability_gateway_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lab::LAB_EXPERIMENT_DESIGN_INTEROPERABILITY_FEATURE_ID,
            "contract_version": bioprism_lab::LAB_EXPERIMENT_DESIGN_INTEROPERABILITY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "federated continual experiment-objective and capability manifests are negotiated deterministically with explicit migration-loss receipts",
                "missing modalities/controls, semantic or instrument-profile conflicts, unknown or contradicted evidence, policy, locality, replay, provenance, and approval gaps remain visible",
                "qualified effects are limited to contract negotiation; no protocol, instrument, or raw-data effect is dispatched"
            ],
            "limitations": [
                "the gateway evaluates caller-supplied capability manifests and never contacts instruments or workflow services",
                "an executable-design artifact is a bounded interoperability contract, not scientific validity or a clinical decision"
            ]
        }))
    }
}
