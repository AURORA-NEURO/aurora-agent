//! Evaluation coverage, boundary, acquisition, and reference audits.

use super::*;

impl Server {
    pub(super) fn bioeval_mesh_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-mesh-audit/0.1";
        const MAX_EVALUATORS: usize = 1_024;
        const MAX_VERDICTS: usize = 1_024;
        const MAX_ID_BYTES: usize = 256;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_mesh_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "evaluators derived from system artifacts are refused before scoring",
                    "shared-input evaluators are counted by independence class rather than evaluator count",
                    "within-class disagreement remains distinct from across-class disagreement",
                    "abstentions remain explicit and never become failures or dissent pairs",
                ],
                "limitations": [
                    "the route audits declarations and verdicts without authenticating evaluators or checking their artifacts",
                    "the route does not choose a biological truth, majority-vote a split, calibrate a judge, or adjudicate a disagreement",
                    "input independence is verified only when evaluators declare their consumed artifacts",
                ],
            })
        };
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure mesh audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("mesh audit input exceeds the 20000000-byte safety bound".into());
        }
        let parse_strings = |name: &str, raw: Option<&Value>| -> Result<Vec<String>, String> {
            let Some(raw) = raw else {
                return Ok(Vec::new());
            };
            let values = raw
                .as_array()
                .ok_or_else(|| format!("{name} must be an array of strings"))?;
            let mut output = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let item = value
                    .as_str()
                    .ok_or_else(|| format!("{name}[{index}] must be a string"))?;
                if item.trim().is_empty() || item.len() > MAX_ID_BYTES {
                    return Err(format!(
                        "{name}[{index}] must contain 1 to {MAX_ID_BYTES} bytes"
                    ));
                }
                output.push(item.to_string());
            }
            Ok(output)
        };
        let system_artifacts =
            match parse_strings("system_artifacts", arguments.get("system_artifacts")) {
                Ok(values) => {
                    let mut seen = BTreeSet::new();
                    if values.iter().any(|value| !seen.insert(value.clone())) {
                        return Ok(refusal(
                            "mesh_validation",
                            "system_artifacts must be unique".into(),
                        ));
                    }
                    values
                }
                Err(error) => return Ok(refusal("mesh_validation", error)),
            };
        let raw_evaluators = arguments
            .get("evaluators")
            .and_then(Value::as_array)
            .ok_or(
                "evaluators is required and must be an array of serialized EvaluatorDecl values",
            )?;
        if raw_evaluators.is_empty() || raw_evaluators.len() > MAX_EVALUATORS {
            return Err("evaluators must contain 1 to 1024 rows".into());
        }
        let raw_verdicts = arguments
            .get("verdicts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_verdicts.len() > MAX_VERDICTS {
            return Err("verdicts are bounded at 1024 rows".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let expected = match arguments.get("expected") {
            Some(value) => Some(
                value
                    .as_str()
                    .ok_or("expected must be a string when supplied")?,
            ),
            None => None,
        };
        let require_independence = arguments
            .get("require_independence")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let require_ratings = arguments
            .get("require_independent_ratings")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut mesh = BioevalMesh::for_system(system_artifacts.clone());
        let mut evaluator_ids = BTreeSet::new();
        for (index, raw) in raw_evaluators.iter().enumerate() {
            let evaluator: BioevalEvaluatorDecl = match serde_json::from_value(raw.clone()) {
                Ok(evaluator) => evaluator,
                Err(error) => {
                    return Ok(refusal(
                        "evaluator_deserialization",
                        format!("evaluators[{index}] is not a valid EvaluatorDecl: {error}"),
                    ));
                }
            };
            if evaluator.id.trim().is_empty() || evaluator.id.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "evaluator_validation",
                    format!("evaluators[{index}].id must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !evaluator_ids.insert(evaluator.id.clone()) {
                return Ok(refusal(
                    "evaluator_validation",
                    format!("evaluator {:?} appears more than once", evaluator.id),
                ));
            }
            if evaluator
                .inputs
                .iter()
                .any(|input| input.trim().is_empty() || input.len() > MAX_ID_BYTES)
                || evaluator
                    .derived_from
                    .iter()
                    .any(|input| input.trim().is_empty() || input.len() > MAX_ID_BYTES)
            {
                return Ok(refusal(
                    "evaluator_validation",
                    format!(
                        "evaluators[{index}] contains an artifact outside the {MAX_ID_BYTES}-byte bound"
                    ),
                ));
            }
            if let Err(error) = mesh.admit(evaluator) {
                return Ok(refusal("evaluator_admission", error.to_string()));
            }
        }

        let mut verdicts = Vec::with_capacity(raw_verdicts.len());
        let mut verdict_ids = BTreeSet::new();
        for (index, raw) in raw_verdicts.iter().enumerate() {
            let verdict: BioevalEvaluatorVerdict = match serde_json::from_value(raw.clone()) {
                Ok(verdict) => verdict,
                Err(error) => {
                    return Ok(refusal(
                        "verdict_deserialization",
                        format!("verdicts[{index}] is not a valid EvaluatorVerdict: {error}"),
                    ));
                }
            };
            if verdict.evaluator.trim().is_empty() || verdict.evaluator.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "verdict_validation",
                    format!("verdicts[{index}].evaluator must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !verdict_ids.insert(verdict.evaluator.clone()) {
                return Ok(refusal(
                    "verdict_validation",
                    format!(
                        "verdicts contain more than one row for evaluator {:?}",
                        verdict.evaluator
                    ),
                ));
            }
            if !verdict.abstained
                && (verdict.position.trim().is_empty() || verdict.position.len() > MAX_ID_BYTES)
            {
                return Ok(refusal(
                    "verdict_validation",
                    format!(
                        "verdicts[{index}].position must contain 1 to {MAX_ID_BYTES} bytes for a called evaluator"
                    ),
                ));
            }
            verdicts.push(verdict);
        }

        let census = match mesh.census() {
            Ok(census) => census,
            Err(error) => return Ok(refusal("mesh_validation", error.to_string())),
        };
        if require_independence && !census.independence_verified() {
            return Ok(refusal(
                "independence_policy",
                "require_independence was true but one or more evaluators declared no inputs"
                    .into(),
            ));
        }
        let classes = mesh.independence_classes();
        let disagreements = match mesh.disagreements(&verdicts) {
            Ok(disagreements) => disagreements,
            Err(error) => return Ok(refusal("disagreement_analysis", error.to_string())),
        };
        let within_count = disagreements
            .iter()
            .filter(|disagreement| matches!(disagreement, BioevalDisagreement::WithinClass(_)))
            .count();
        let across_count = disagreements
            .iter()
            .filter(|disagreement| matches!(disagreement, BioevalDisagreement::AcrossClasses(_)))
            .count();
        let disagreement_rows = disagreements
            .iter()
            .take(max_items)
            .map(|disagreement| {
                json!({
                    "disagreement": disagreement,
                    "about_case": disagreement.is_about_the_case(),
                    "witness": disagreement.witness(),
                })
            })
            .collect::<Vec<_>>();
        let (rating_status, rating_rows, rating_refusal) = match mesh.independent_ratings(&verdicts)
        {
            Ok(ratings) => (
                "accepted",
                serde_json::to_value(ratings).expect("ratings serialize"),
                Value::Null,
            ),
            Err(error) => ("refused", json!([]), json!(error.to_string())),
        };
        if require_ratings && rating_status != "accepted" {
            return Ok(refusal(
                "rating_projection",
                format!(
                    "require_independent_ratings was true but the class-collapsed rating projection was refused: {}",
                    rating_refusal.as_str().unwrap_or("unknown refusal")
                ),
            ));
        }
        let (contribution_status, contribution_rows, contribution_refusal) = match expected {
            Some(expected) => match mesh.contributions(&verdicts, expected) {
                Ok(contributions) => (
                    "accepted",
                    serde_json::to_value(contributions).expect("contributions serialize"),
                    Value::Null,
                ),
                Err(error) => ("refused", json!([]), json!(error.to_string())),
            },
            None => ("not_requested", json!([]), Value::Null),
        };
        let reported_ids = verdicts
            .iter()
            .map(|verdict| verdict.evaluator.clone())
            .collect::<BTreeSet<_>>();
        let unreported = evaluator_ids
            .difference(&reported_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let abstentions = verdicts
            .iter()
            .filter(|verdict| verdict.abstained)
            .map(|verdict| verdict.evaluator.clone())
            .collect::<BTreeSet<_>>();
        let bounded_ids = |ids: &BTreeSet<String>| {
            json!({
                "ids": ids.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": ids.len(),
                "omitted": ids.len().saturating_sub(max_items),
            })
        };
        let class_rows = classes
            .iter()
            .take(max_items)
            .map(|class| {
                json!({
                    "members": class,
                    "size": class.len(),
                    "inputs_declared": class.iter().all(|id| mesh.evaluators().iter().find(|e| e.id == *id).is_some_and(|e| !e.inputs.is_empty())),
                })
            })
            .collect::<Vec<_>>();
        let evaluator_rows = mesh
            .evaluators()
            .iter()
            .take(max_items)
            .map(|evaluator| {
                json!({
                    "id": evaluator.id,
                    "kind": evaluator.kind,
                    "tier": evaluator.kind.tier(),
                    "inputs": evaluator.inputs,
                    "derived_from": evaluator.derived_from,
                    "model_judge": evaluator.kind.is_model_judge(),
                })
            })
            .collect::<Vec<_>>();
        let verdict_rows = verdicts
            .iter()
            .take(max_items)
            .map(|verdict| {
                json!({
                    "evaluator": verdict.evaluator,
                    "position": verdict.position,
                    "abstained": verdict.abstained,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_mesh_audit",
            "mesh": {
                "system_artifacts": system_artifacts,
                "evaluator_count": census.evaluators,
                "independent_class_count": census.independent_classes,
                "non_model_class_count": census.non_model_classes,
                "independence_verified": census.independence_verified(),
                "kinds_present": census.kinds_present,
                "inputs_undeclared": census.inputs_undeclared,
            },
            "evaluators": {
                "rows": evaluator_rows,
                "returned": census.evaluators.min(max_items),
                "total": census.evaluators,
                "omitted": census.evaluators.saturating_sub(max_items),
            },
            "classes": {
                "rows": class_rows,
                "returned": classes.len().min(max_items),
                "total": classes.len(),
                "omitted": classes.len().saturating_sub(max_items),
            },
            "verdicts": {
                "rows": verdict_rows,
                "returned": verdicts.len().min(max_items),
                "total": verdicts.len(),
                "omitted": verdicts.len().saturating_sub(max_items),
            },
            "disagreements": {
                "rows": disagreement_rows,
                "returned": disagreements.len().min(max_items),
                "total": disagreements.len(),
                "omitted": disagreements.len().saturating_sub(max_items),
                "within_class_count": within_count,
                "across_class_count": across_count,
            },
            "independent_ratings": {
                "status": rating_status,
                "rows": rating_rows,
                "refusal": rating_refusal,
            },
            "contributions": {
                "status": contribution_status,
                "expected": expected,
                "rows": contribution_rows,
                "refusal": contribution_refusal,
            },
            "findings": {
                "inputs_undeclared": bounded_ids(&census.inputs_undeclared.iter().cloned().collect()),
                "unreported_evaluators": bounded_ids(&unreported),
                "abstaining_evaluators": bounded_ids(&abstentions),
                "within_class_disagreement_count": within_count,
                "across_class_disagreement_count": across_count,
                "rating_projection_refused": rating_status == "refused",
            },
            "guarantees": [
                "shared inputs collapse evaluators into transitive independence classes",
                "same-class disagreement is an evaluator defect witness, not a hard-case consensus split",
                "across-class disagreement remains a case-level unresolved finding",
                "abstentions are retained and become unknown contributions only when contribution projection is requested",
                "independent ratings contribute at most one rating per input class and refuse internally split classes",
                "bounded evaluator, class, verdict, disagreement, and finding projections retain total and omitted counts",
            ],
            "limitations": [
                "declared inputs are not independently inspected and an empty input set makes independence unverified",
                "the route does not adjudicate disagreements, calibrate judges, or select a consensus policy",
                "contributions are typed evidence for a downstream ladder and do not themselves choose biological truth",
            ],
        }))
    }

    pub(super) fn bioeval_burden_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-burden-audit/0.1";
        const MAX_RESOURCES: usize = 4_096;
        const MAX_BRANCHES: usize = 4_096;
        const MAX_DRAWS: usize = 16_384;
        const MAX_ID_BYTES: usize = 256;
        const MAX_INPUT_BYTES: usize = 20_000_000;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_burden_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "failed actions retain their resource draws",
                    "units are compared by declared equality with no invented conversion",
                    "nonrenewable fork double-spends remain explicit",
                    "residual quantities are computed from inherited branch history",
                ],
                "limitations": [
                    "the route does not assign utility, optimize a policy, or price a resource",
                    "a declared draw is treated as caller-supplied evidence and is not verified against an external inventory",
                    "branch feasibility is a logical check, not proof that the branch was physically executed",
                ],
            })
        };
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure burden audit input: {error}"))?;
        if encoded.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "burden audit input exceeds the {MAX_INPUT_BYTES}-byte safety bound"
            ));
        }
        let bounded_text = |name: &str, value: Option<&Value>| -> Result<String, String> {
            let value = value
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{name} must be a string"))?;
            if value.trim().is_empty() || value.len() > MAX_ID_BYTES {
                return Err(format!("{name} must contain 1 to {MAX_ID_BYTES} bytes"));
            }
            Ok(value.to_string())
        };
        let root = bounded_text("root", arguments.get("root"))?;
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let policy = |name: &str| -> Result<bool, String> {
            match arguments.get(name) {
                None => Ok(false),
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| format!("{name} must be a boolean")),
            }
        };
        let require_joint = policy("require_joint_feasible")?;
        let require_no_waste = policy("require_no_wasted_nonrenewable")?;
        let raw_resources = arguments
            .get("resources")
            .and_then(Value::as_array)
            .ok_or("resources is required and must be an array")?;
        if raw_resources.is_empty() || raw_resources.len() > MAX_RESOURCES {
            return Err("resources must contain 1 to 4096 rows".into());
        }
        let raw_branches = arguments
            .get("branches")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_branches.len() > MAX_BRANCHES {
            return Err("branches are bounded at 4096 rows".into());
        }
        let raw_draws = arguments
            .get("draws")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_draws.len() > MAX_DRAWS {
            return Err("draws are bounded at 16384 rows".into());
        }

        let mut ledger = BioevalLedger::new(root.clone());
        let mut resource_ids = BTreeSet::new();
        let mut resources = Vec::with_capacity(raw_resources.len());
        for (index, raw) in raw_resources.iter().enumerate() {
            let resource: BioevalResource = match serde_json::from_value(raw.clone()) {
                Ok(resource) => resource,
                Err(error) => {
                    return Ok(refusal(
                        "resource_deserialization",
                        format!("resources[{index}] is not a valid Resource: {error}"),
                    ));
                }
            };
            if resource.id.trim().is_empty()
                || resource.id.len() > MAX_ID_BYTES
                || resource.unit.trim().is_empty()
                || resource.unit.len() > MAX_ID_BYTES
            {
                return Ok(refusal(
                    "resource_validation",
                    format!("resources[{index}] id and unit must be bounded non-empty strings"),
                ));
            }
            if !resource_ids.insert(resource.id.clone()) {
                return Ok(refusal(
                    "resource_validation",
                    format!("resource {:?} appears more than once", resource.id),
                ));
            }
            if let Err(error) = ledger.declare(resource.clone()) {
                return Ok(refusal("resource_declaration", error.to_string()));
            }
            resources.push(resource);
        }

        let mut branch_ids = BTreeSet::from([root.clone()]);
        let mut branch_parents = BTreeMap::from([(root.clone(), None::<String>)]);
        for (index, raw) in raw_branches.iter().enumerate() {
            let Some(object) = raw.as_object() else {
                return Ok(refusal(
                    "branch_deserialization",
                    format!("branches[{index}] must be an object"),
                ));
            };
            let id = match bounded_text(&format!("branches[{index}].id"), object.get("id")) {
                Ok(id) => id,
                Err(error) => return Ok(refusal("branch_validation", error)),
            };
            let parent = match object.get("parent") {
                None | Some(Value::Null) => root.clone(),
                Some(value) => {
                    match bounded_text(&format!("branches[{index}].parent"), Some(value)) {
                        Ok(parent) => parent,
                        Err(error) => return Ok(refusal("branch_validation", error)),
                    }
                }
            };
            if !branch_ids.insert(id.clone()) {
                return Ok(refusal(
                    "branch_validation",
                    format!("branch {:?} appears more than once", id),
                ));
            }
            if !branch_ids.contains(&parent) {
                return Ok(refusal(
                    "branch_validation",
                    format!("branch {:?} names undeclared parent {:?}", id, parent),
                ));
            }
            if let Err(error) = ledger.fork(&parent, id.clone()) {
                return Ok(refusal("branch_declaration", error.to_string()));
            }
            branch_parents.insert(id, Some(parent));
        }

        let mut draw_rows = Vec::with_capacity(raw_draws.len());
        for (index, raw) in raw_draws.iter().enumerate() {
            let Some(object) = raw.as_object() else {
                return Ok(refusal(
                    "draw_deserialization",
                    format!("draws[{index}] must be an object"),
                ));
            };
            let branch = match bounded_text(&format!("draws[{index}].branch"), object.get("branch"))
            {
                Ok(branch) => branch,
                Err(error) => return Ok(refusal("draw_validation", error)),
            };
            let draw: BioevalDraw = match serde_json::from_value(raw.clone()) {
                Ok(draw) => draw,
                Err(error) => {
                    return Ok(refusal(
                        "draw_deserialization",
                        format!("draws[{index}] is not a valid Draw: {error}"),
                    ));
                }
            };
            if draw.action.trim().is_empty()
                || draw.action.len() > MAX_ID_BYTES
                || draw.resource.trim().is_empty()
                || draw.resource.len() > MAX_ID_BYTES
                || draw.unit.trim().is_empty()
                || draw.unit.len() > MAX_ID_BYTES
            {
                return Ok(refusal(
                    "draw_validation",
                    format!("draws[{index}] action, resource, and unit must be bounded strings"),
                ));
            }
            if let Err(error) = ledger.draw(&branch, draw.clone()) {
                return Ok(refusal("draw_admission", error.to_string()));
            }
            draw_rows.push((branch, draw));
        }

        let parse_branch_list = |name: &str| -> Result<Option<Vec<String>>, String> {
            let Some(raw) = arguments.get(name) else {
                return Ok(None);
            };
            let values = raw
                .as_array()
                .ok_or_else(|| format!("{name} must be an array of branch ids"))?;
            let mut ids = Vec::with_capacity(values.len());
            let mut seen = BTreeSet::new();
            for (index, value) in values.iter().enumerate() {
                let id = bounded_text(&format!("{name}[{index}]"), Some(value))?;
                if !seen.insert(id.clone()) {
                    return Err(format!("{name} must contain unique branch ids"));
                }
                if !branch_ids.contains(&id) {
                    return Err(format!("{name} names undeclared branch {id:?}"));
                }
                ids.push(id);
            }
            Ok(Some(ids))
        };
        let inspect_branches = match parse_branch_list("inspect_branches") {
            Ok(Some(ids)) => ids,
            Ok(None) => branch_ids.iter().cloned().collect(),
            Err(error) => return Ok(refusal("branch_selection", error)),
        };
        let joint_branches = match parse_branch_list("joint_branches") {
            Ok(ids) => ids,
            Err(error) => return Ok(refusal("joint_selection", error)),
        };
        if require_joint && joint_branches.is_none() {
            return Ok(refusal(
                "joint_feasibility_policy",
                "require_joint_feasible was true but joint_branches was not supplied".into(),
            ));
        }
        let (joint_status, joint_refusal) = match joint_branches.as_ref() {
            None => ("not_requested", Value::Null),
            Some(branches) => {
                let refs = branches.iter().map(String::as_str).collect::<Vec<_>>();
                match ledger.joint_feasibility(&refs) {
                    Ok(()) => ("accepted", Value::Null),
                    Err(error) => ("refused", json!(error.to_string())),
                }
            }
        };
        if require_joint && joint_status != "accepted" {
            return Ok(refusal(
                "joint_feasibility_policy",
                format!(
                    "the selected branches are not jointly feasible: {}",
                    joint_refusal.as_str().unwrap_or("unknown refusal")
                ),
            ));
        }

        let mut wasted_rows = Vec::new();
        for branch in &inspect_branches {
            let draws = match ledger.wasted_nonrenewable(branch) {
                Ok(draws) => draws,
                Err(error) => return Ok(refusal("waste_analysis", error.to_string())),
            };
            for draw in draws {
                wasted_rows.push(json!({
                    "branch": branch,
                    "action": draw.action,
                    "resource": draw.resource,
                    "amount": draw.amount,
                    "unit": draw.unit,
                    "outcome": draw.outcome,
                    "destructive": draw.destructive,
                }));
            }
        }
        if require_no_waste && !wasted_rows.is_empty() {
            return Ok(refusal(
                "waste_policy",
                format!(
                    "selected branches contain {} wasted destructive nonrenewable draw(s)",
                    wasted_rows.len()
                ),
            ));
        }

        let class_counts =
            resources
                .iter()
                .fold(BTreeMap::<String, usize>::new(), |mut counts, resource| {
                    let class = serde_json::to_value(resource.class)
                        .expect("resource class serializes")
                        .as_str()
                        .expect("resource class is a string")
                        .to_string();
                    *counts.entry(class).or_default() += 1;
                    counts
                });
        let nonrenewable_resources = resources
            .iter()
            .filter(|resource| resource.class.is_nonrenewable())
            .count();
        let bounded_strings = |values: BTreeSet<String>| {
            json!({
                "ids": values.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": values.len(),
                "omitted": values.len().saturating_sub(max_items),
            })
        };
        let branch_rows = inspect_branches
            .iter()
            .take(max_items)
            .map(|branch| {
                let branch_ledger: &BioevalBranchLedger = ledger
                    .branch(branch)
                    .expect("branch selection was validated");
                let residual = ledger.residual(branch).expect("branch residual is valid");
                let consumed = resources
                    .iter()
                    .map(|resource| (resource.id.clone(), branch_ledger.consumed(&resource.id)))
                    .collect::<BTreeMap<_, _>>();
                let wasted = resources
                    .iter()
                    .map(|resource| (resource.id.clone(), branch_ledger.wasted(&resource.id)))
                    .collect::<BTreeMap<_, _>>();
                json!({
                    "id": branch,
                    "parent": branch_parents.get(branch).cloned().flatten(),
                    "draw_count": branch_ledger.draws().len(),
                    "consumed": consumed,
                    "wasted": wasted,
                    "residual": residual,
                })
            })
            .collect::<Vec<_>>();
        let resource_rows = resources
            .iter()
            .take(max_items)
            .map(|resource| {
                json!({
                    "id": resource.id,
                    "class": resource.class,
                    "initial": resource.initial,
                    "unit": resource.unit,
                    "nonrenewable": resource.class.is_nonrenewable(),
                })
            })
            .collect::<Vec<_>>();
        let draw_projection = draw_rows
            .iter()
            .take(max_items)
            .map(|(branch, draw)| {
                json!({
                    "branch": branch,
                    "action": draw.action,
                    "resource": draw.resource,
                    "amount": draw.amount,
                    "unit": draw.unit,
                    "outcome": draw.outcome,
                    "destructive": draw.destructive,
                })
            })
            .collect::<Vec<_>>();
        let wasted_actions = wasted_rows
            .iter()
            .filter_map(|row| row.get("action").and_then(Value::as_str).map(String::from));
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_burden_audit",
            "burden": {
                "root": root,
                "resource_count": resources.len(),
                "branch_count": branch_ids.len(),
                "draw_count": draw_rows.len(),
                "nonrenewable_resource_count": nonrenewable_resources,
                "resource_class_counts": class_counts,
                "inspected_branch_count": inspect_branches.len(),
                "policies": {
                    "require_joint_feasible": require_joint,
                    "require_no_wasted_nonrenewable": require_no_waste,
                },
            },
            "resources": {
                "rows": resource_rows,
                "returned": resources.len().min(max_items),
                "total": resources.len(),
                "omitted": resources.len().saturating_sub(max_items),
            },
            "branches": {
                "rows": branch_rows,
                "returned": inspect_branches.len().min(max_items),
                "total": inspect_branches.len(),
                "omitted": inspect_branches.len().saturating_sub(max_items),
            },
            "draws": {
                "rows": draw_projection,
                "returned": draw_rows.len().min(max_items),
                "total": draw_rows.len(),
                "omitted": draw_rows.len().saturating_sub(max_items),
            },
            "joint_feasibility": {
                "status": joint_status,
                "branches": joint_branches,
                "refusal": joint_refusal,
            },
            "wasted_nonrenewable": {
                "rows": wasted_rows.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "returned": wasted_rows.len().min(max_items),
                "total": wasted_rows.len(),
                "omitted": wasted_rows.len().saturating_sub(max_items),
            },
            "findings": {
                "wasted_nonrenewable_actions": bounded_strings(wasted_actions.collect()),
                "joint_feasibility_refused": joint_status == "refused",
                "failed_draws_still_counted": draw_rows.iter().filter(|(_, draw)| draw.outcome == bioprism_bioevalx::burden::DrawOutcome::Wasted).count(),
            },
            "guarantees": [
                "failed actions retain their resource consumption",
                "branch residuals include inherited ancestor draws",
                "nonrenewable fork double-spends remain refusals rather than plausible totals",
                "unit mismatch is refused without guessing a conversion",
                "wasted destructive nonrenewable draws remain visible",
            ],
            "limitations": [
                "no utility, price, optimal policy, or accuracy trade-off is inferred",
                "declared resources and draws are not independently authenticated",
                "joint feasibility only evaluates the selected branch set",
            ],
        }))
    }

    pub(super) fn bioeval_reveal_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-reveal-audit/0.1";
        const MAX_COMMITMENTS: usize = 4_096;
        const MAX_OUTCOMES: usize = 4_096;
        const MAX_ID_BYTES: usize = 256;
        const MAX_TEXT_BYTES: usize = 4_096;
        const MAX_INPUT_BYTES: usize = 20_000_000;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_reveal_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "commitments are sealed before outcomes are revealed",
                    "a rubric digest mismatch never becomes a score",
                    "uncommitted outcomes and unrevealed commitments remain visible",
                    "a revealed state cannot be revealed a second time",
                ],
                "limitations": [
                    "the route records caller-supplied timestamps but does not attest them",
                    "the route does not sign submissions or search external preprints for leakage",
                    "prediction-versus-outcome scoring is not invented; the route only certifies admissible pairs",
                ],
            })
        };
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure reveal audit input: {error}"))?;
        if encoded.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "reveal audit input exceeds the {MAX_INPUT_BYTES}-byte safety bound"
            ));
        }
        let bounded_text =
            |name: &str, value: Option<&Value>, limit: usize| -> Result<String, String> {
                let value = value
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("{name} must be a string"))?;
                if value.trim().is_empty() || value.len() > limit {
                    return Err(format!("{name} must contain 1 to {limit} bytes"));
                }
                Ok(value.to_string())
            };
        let study = bounded_text("study", arguments.get("study"), MAX_ID_BYTES)?;
        let sealed_at_text = bounded_text("sealed_at", arguments.get("sealed_at"), MAX_TEXT_BYTES)?;
        let sealed_at = match bioprism_scope::Timestamp::parse(&sealed_at_text) {
            Ok(timestamp) => timestamp,
            Err(error) => {
                return Ok(refusal(
                    "timestamp_validation",
                    format!("sealed_at is not a valid timestamp: {error}"),
                ));
            }
        };
        let rubric = arguments
            .get("rubric")
            .cloned()
            .ok_or("rubric is required and may be any JSON value")?;
        let raw_commitments = arguments
            .get("commitments")
            .and_then(Value::as_array)
            .ok_or("commitments is required and must be an array")?;
        if raw_commitments.is_empty() || raw_commitments.len() > MAX_COMMITMENTS {
            return Err("commitments must contain 1 to 4096 rows".into());
        }
        let raw_outcomes = arguments
            .get("outcomes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_outcomes.len() > MAX_OUTCOMES {
            return Err("outcomes are bounded at 4096 rows".into());
        }
        let score_rubric = arguments.get("score_rubric").cloned();
        let policy = |name: &str| -> Result<bool, String> {
            match arguments.get(name) {
                None => Ok(false),
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| format!("{name} must be a boolean")),
            }
        };
        let require_scoring = policy("require_scoring")?;
        let require_rubric_match = policy("require_rubric_match")?;
        let require_complete = policy("require_complete")?;
        if require_scoring && score_rubric.is_none() {
            return Ok(refusal(
                "scoring_policy",
                "require_scoring was true but score_rubric was not supplied".into(),
            ));
        }

        let mut registration = BioevalRegistration::open(study.clone());
        let mut commitment_ids = BTreeSet::new();
        let mut commitment_rows = Vec::with_capacity(raw_commitments.len());
        for (index, raw) in raw_commitments.iter().enumerate() {
            let commitment: BioevalCommitment = match serde_json::from_value(raw.clone()) {
                Ok(commitment) => commitment,
                Err(error) => {
                    return Ok(refusal(
                        "commitment_deserialization",
                        format!("commitments[{index}] is not a valid Commitment: {error}"),
                    ));
                }
            };
            if commitment.target.trim().is_empty()
                || commitment.target.len() > MAX_ID_BYTES
                || commitment.analysis_plan.trim().is_empty()
                || commitment.analysis_plan.len() > MAX_TEXT_BYTES
            {
                return Ok(refusal(
                    "commitment_validation",
                    format!(
                        "commitments[{index}] target and analysis_plan exceed their declared bounds"
                    ),
                ));
            }
            if !commitment_ids.insert(commitment.target.clone()) {
                return Ok(refusal(
                    "commitment_admission",
                    format!("commitment {:?} appears more than once", commitment.target),
                ));
            }
            if let Err(error) = registration.commit(commitment.clone()) {
                return Ok(refusal("commitment_admission", error.to_string()));
            }
            commitment_rows.push(commitment);
        }
        let sealed = match registration.seal(&rubric, sealed_at) {
            Ok(sealed) => sealed,
            Err(error) => return Ok(refusal("seal", error.to_string())),
        };

        let mut outcome_ids = BTreeSet::new();
        let mut outcomes = Vec::with_capacity(raw_outcomes.len());
        for (index, raw) in raw_outcomes.iter().enumerate() {
            let outcome: BioevalOutcome = match serde_json::from_value(raw.clone()) {
                Ok(outcome) => outcome,
                Err(error) => {
                    return Ok(refusal(
                        "outcome_deserialization",
                        format!("outcomes[{index}] is not a valid Outcome: {error}"),
                    ));
                }
            };
            if outcome.target.trim().is_empty() || outcome.target.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "outcome_validation",
                    format!("outcomes[{index}].target must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !outcome_ids.insert(outcome.target.clone()) {
                return Ok(refusal(
                    "outcome_validation",
                    format!("outcome {:?} appears more than once", outcome.target),
                ));
            }
            outcomes.push(outcome);
        }
        let seal_lock = match sealed.commit(BioevalCommitment::new(
            "__post-seal-probe__",
            Value::Null,
            "__post-seal-probe__",
        )) {
            Ok(()) => json!({ "status": "accepted", "refusal": Value::Null }),
            Err(error) => json!({ "status": "refused", "refusal": error.to_string() }),
        };
        let sealed_at_value = json!(sealed.sealed_at());
        let rubric_digest = sealed.rubric_digest().to_string();
        let commitment_digest = sealed.commitment_digest().to_string();
        let revealed = match sealed.reveal(outcomes.clone()) {
            Ok(revealed) => revealed,
            Err(error) => return Ok(refusal("reveal", error.to_string())),
        };
        let reveal_lock = match revealed.reveal(Vec::new()) {
            Ok(()) => json!({ "status": "accepted", "refusal": Value::Null }),
            Err(error) => json!({ "status": "refused", "refusal": error.to_string() }),
        };
        let (scoring_status, scoring_value, scoring_refusal, scoring_complete, unrevealed_targets) =
            match score_rubric.as_ref() {
                None => (
                    "not_requested",
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Vec::new(),
                ),
                Some(score_rubric) => match revealed.score_under(score_rubric) {
                    Ok(scoring) => {
                        let complete = scoring.complete();
                        let unrevealed = scoring.unrevealed.clone();
                        (
                            "accepted",
                            serde_json::to_value(scoring).expect("scoring serializes"),
                            Value::Null,
                            json!(complete),
                            unrevealed,
                        )
                    }
                    Err(error) => (
                        "refused",
                        Value::Null,
                        json!(error.to_string()),
                        Value::Null,
                        Vec::new(),
                    ),
                },
            };
        if require_rubric_match && scoring_status != "accepted" {
            return Ok(refusal(
                "rubric_integrity_policy",
                format!(
                    "require_rubric_match was true but scoring was not admitted: {}",
                    scoring_refusal
                        .as_str()
                        .unwrap_or("scoring was not requested")
                ),
            ));
        }
        if require_complete && (scoring_status != "accepted" || scoring_complete != json!(true)) {
            return Ok(refusal(
                "completeness_policy",
                "require_complete was true but the admitted scoring projection is not complete"
                    .into(),
            ));
        }
        let bounded_ids = |values: BTreeSet<String>| {
            json!({
                "ids": values.iter().take(1000).cloned().collect::<Vec<_>>(),
                "total": values.len(),
                "omitted": values.len().saturating_sub(1000),
            })
        };
        let commitment_projection = commitment_rows
            .iter()
            .take(1000)
            .map(|commitment| {
                json!({
                    "target": commitment.target,
                    "prediction": commitment.prediction,
                    "analysis_plan": commitment.analysis_plan,
                })
            })
            .collect::<Vec<_>>();
        let outcome_projection = outcomes
            .iter()
            .take(1000)
            .map(|outcome| json!({ "target": outcome.target, "observed": outcome.observed }))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_reveal_audit",
            "study": study,
            "sealed_at": sealed_at_value,
            "digests": {
                "rubric": rubric_digest,
                "commitments": commitment_digest,
            },
            "commitments": {
                "rows": commitment_projection,
                "returned": commitment_rows.len().min(1000),
                "total": commitment_rows.len(),
                "omitted": commitment_rows.len().saturating_sub(1000),
            },
            "outcomes": {
                "rows": outcome_projection,
                "returned": outcomes.len().min(1000),
                "total": outcomes.len(),
                "omitted": outcomes.len().saturating_sub(1000),
            },
            "seal_lock": seal_lock,
            "reveal_lock": reveal_lock,
            "scoring": {
                "status": scoring_status,
                "value": scoring_value,
                "refusal": scoring_refusal,
                "complete": scoring_complete,
            },
            "findings": {
                "unrevealed_commitments": bounded_ids(unrevealed_targets.into_iter().collect()),
                "selective_publication": scoring_status == "accepted" && scoring_complete == json!(false),
                "rubric_match_refused": scoring_status == "refused",
                "uncommitted_outcome_refused": scoring_refusal.as_str().is_some_and(|text| text.contains("no commitment")),
                "seal_lock_refused": seal_lock["status"] == json!("refused"),
                "reveal_lock_refused": reveal_lock["status"] == json!("refused"),
            },
            "guarantees": [
                "the rubric and commitment set are content-addressed at seal time",
                "outcomes are revealed only after the commitment set is frozen",
                "a changed rubric cannot produce an admitted scoring projection",
                "unrevealed commitments remain visible instead of disappearing from the denominator",
                "second reveal and post-seal commitment probes are refused by the state machine",
            ],
            "limitations": [
                "sealed_at is a caller assertion and not an external timestamp attestation",
                "the route does not sign commitments or search public artifacts for prior leakage",
                "prediction-versus-outcome correctness is left to a separate scoring contract",
            ],
        }))
    }

    pub(super) fn bioeval_boundary_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-boundary-audit/0.1";
        const MAX_POLICIES: usize = 4_096;
        const MAX_FLOWS: usize = 8_192;
        const MAX_TEXT_BYTES: usize = 4_096;
        const MAX_INPUT_BYTES: usize = 20_000_000;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_boundary_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "authorized, compliant, violating, veto, and bypass verdicts remain distinct",
                    "materialized forbidden irreversible flows remain vetoes",
                    "channel exposure is reported without a combined utility-safety score",
                    "missing transmission principles are instrumentation refusals rather than default violations",
                ],
                "limitations": [
                    "flows and policies are caller-labeled and no payload detector is executed",
                    "the route does not infer necessity, minimize disclosure, or choose a Pareto point",
                    "a permitted flow is authorization evidence, not proof that a transfer occurred",
                ],
            })
        };
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure boundary audit input: {error}"))?;
        if encoded.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "boundary audit input exceeds the {MAX_INPUT_BYTES}-byte safety bound"
            ));
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let policy = |name: &str| -> Result<bool, String> {
            match arguments.get(name) {
                None => Ok(false),
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| format!("{name} must be a boolean")),
            }
        };
        let require_clean = policy("require_no_violations")?;
        let require_no_veto = policy("require_no_vetoes")?;
        let utility = match arguments.get("utility") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let utility = value.as_f64().ok_or("utility must be a finite number")?;
                if !utility.is_finite() {
                    return Err("utility must be a finite number".into());
                }
                Some(utility)
            }
        };
        let raw_policies = arguments
            .get("policies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_policies.len() > MAX_POLICIES {
            return Err("policies are bounded at 4096 rows".into());
        }
        let raw_flows = arguments
            .get("flows")
            .and_then(Value::as_array)
            .ok_or("flows is required and must be an array")?;
        if raw_flows.is_empty() || raw_flows.len() > MAX_FLOWS {
            return Err("flows must contain 1 to 8192 rows".into());
        }
        let mut assessment = BioevalBoundaryAssessment::new();
        let mut policy_ids = BTreeSet::new();
        let mut policies = Vec::with_capacity(raw_policies.len());
        for (index, raw) in raw_policies.iter().enumerate() {
            let policy: BioevalBoundaryPolicy = match serde_json::from_value(raw.clone()) {
                Ok(policy) => policy,
                Err(error) => {
                    return Ok(refusal(
                        "policy_deserialization",
                        format!("policies[{index}] is not a valid Policy: {error}"),
                    ));
                }
            };
            if policy.id.trim().is_empty()
                || policy.id.len() > MAX_TEXT_BYTES
                || policy.transmission_principle.trim().is_empty()
                || policy.transmission_principle.len() > MAX_TEXT_BYTES
            {
                return Ok(refusal(
                    "policy_validation",
                    format!(
                        "policies[{index}] id and transmission_principle must be bounded strings"
                    ),
                ));
            }
            if !policy_ids.insert(policy.id.clone()) {
                return Ok(refusal(
                    "policy_admission",
                    format!("policy {:?} appears more than once", policy.id),
                ));
            }
            if let Err(error) = assessment.allow(policy.clone()) {
                return Ok(refusal("policy_admission", error.to_string()));
            }
            policies.push(policy);
        }
        let mut flow_ids = BTreeSet::new();
        let mut flows = Vec::with_capacity(raw_flows.len());
        for (index, raw) in raw_flows.iter().enumerate() {
            let flow: BioevalFlow = match serde_json::from_value(raw.clone()) {
                Ok(flow) => flow,
                Err(error) => {
                    return Ok(refusal(
                        "flow_deserialization",
                        format!("flows[{index}] is not a valid Flow: {error}"),
                    ));
                }
            };
            let fields = [
                ("id", flow.id.as_str()),
                ("sender", flow.sender.as_str()),
                ("subject", flow.subject.as_str()),
                ("recipient", flow.recipient.as_str()),
                ("information_type", flow.information_type.as_str()),
                ("purpose", flow.purpose.as_str()),
                (
                    "transmission_principle",
                    flow.transmission_principle.as_str(),
                ),
            ];
            if fields.iter().any(|(_, value)| value.len() > MAX_TEXT_BYTES) {
                return Ok(refusal(
                    "flow_validation",
                    format!("flows[{index}] contains text over the {MAX_TEXT_BYTES}-byte bound"),
                ));
            }
            if flow.id.trim().is_empty() {
                return Ok(refusal(
                    "flow_validation",
                    format!("flows[{index}].id must be non-empty"),
                ));
            }
            if !flow_ids.insert(flow.id.clone()) {
                return Ok(refusal(
                    "flow_validation",
                    format!("flow {:?} appears more than once", flow.id),
                ));
            }
            if let Err(error) = assessment.assess(&flow) {
                return Ok(refusal("flow_assessment", error.to_string()));
            }
            flows.push(flow);
        }
        let violations = assessment.violations();
        let vetoes = assessment.vetoes();
        if require_clean && !violations.is_empty() {
            return Ok(refusal(
                "violation_policy",
                format!(
                    "require_no_violations was true but {} violating flow(s) remain",
                    violations.len()
                ),
            ));
        }
        if require_no_veto && !vetoes.is_empty() {
            return Ok(refusal(
                "veto_policy",
                format!(
                    "require_no_vetoes was true but {} veto or bypass flow(s) remain",
                    vetoes.len()
                ),
            ));
        }
        let verdict_by_id = assessment
            .verdicts()
            .iter()
            .map(|(id, verdict)| (id.as_str(), verdict))
            .collect::<BTreeMap<_, _>>();
        let flow_rows = flows
            .iter()
            .take(max_items)
            .map(|flow| {
                let verdict = verdict_by_id
                    .get(flow.id.as_str())
                    .expect("every flow was assessed");
                json!({
                    "id": flow.id,
                    "sender": flow.sender,
                    "subject": flow.subject,
                    "recipient": flow.recipient,
                    "information_type": flow.information_type,
                    "purpose": flow.purpose,
                    "transmission_principle": flow.transmission_principle,
                    "channel": flow.channel,
                    "external": flow.channel.is_external(),
                    "effect": flow.effect,
                    "irreversible": flow.irreversible,
                    "verdict": verdict,
                    "violation": verdict.is_violation(),
                    "veto": verdict.is_veto(),
                })
            })
            .collect::<Vec<_>>();
        let policy_rows = policies
            .iter()
            .take(max_items)
            .map(|policy| serde_json::to_value(policy).expect("policy serializes"))
            .collect::<Vec<_>>();
        let bounded_ids = |values: BTreeSet<String>| {
            json!({
                "ids": values.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": values.len(),
                "omitted": values.len().saturating_sub(max_items),
            })
        };
        let violation_ids = violations
            .iter()
            .map(|(id, _)| (*id).to_string())
            .collect::<BTreeSet<_>>();
        let veto_ids = vetoes
            .iter()
            .map(|(id, _)| (*id).to_string())
            .collect::<BTreeSet<_>>();
        let compliant_ids = assessment
            .compliant_proposals()
            .into_iter()
            .map(String::from)
            .collect::<BTreeSet<_>>();
        let channel_counts = assessment
            .violations_by_channel(&flows)
            .into_iter()
            .map(|(channel, count)| {
                (
                    serde_json::to_value(channel)
                        .expect("channel serializes")
                        .as_str()
                        .expect("channel is a string")
                        .to_string(),
                    count,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let (composite_status, composite_value, composite_refusal) = match utility {
            None => ("not_requested", Value::Null, Value::Null),
            Some(utility) => match assessment.composite_with_utility(utility) {
                Ok(value) => ("accepted", json!(value), Value::Null),
                Err(error) => ("refused", Value::Null, json!(error.to_string())),
            },
        };
        let pareto = utility.map(|utility| {
            json!({
                "utility": utility,
                "violations": violations.len(),
            })
        });
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_boundary_audit",
            "boundary": {
                "policy_count": policies.len(),
                "flow_count": flows.len(),
                "authorised_count": assessment.verdicts().iter().filter(|(_, verdict)| matches!(verdict, bioprism_bioevalx::boundary::FlowVerdict::Authorised { .. })).count(),
                "compliant_count": compliant_ids.len(),
                "violation_count": violations.len(),
                "veto_count": vetoes.len(),
                "external_flow_count": flows.iter().filter(|flow| flow.channel.is_external()).count(),
                "policies": {
                    "require_no_violations": require_clean,
                    "require_no_vetoes": require_no_veto,
                },
            },
            "policies": {
                "rows": policy_rows,
                "returned": policies.len().min(max_items),
                "total": policies.len(),
                "omitted": policies.len().saturating_sub(max_items),
            },
            "flows": {
                "rows": flow_rows,
                "returned": flows.len().min(max_items),
                "total": flows.len(),
                "omitted": flows.len().saturating_sub(max_items),
            },
            "violations_by_channel": channel_counts,
            "pareto": pareto,
            "composite": {
                "status": composite_status,
                "value": composite_value,
                "refusal": composite_refusal,
            },
            "findings": {
                "violating_flows": bounded_ids(violation_ids),
                "veto_flows": bounded_ids(veto_ids),
                "compliant_proposals": bounded_ids(compliant_ids),
                "composite_refused": composite_status == "refused",
                "bypass_is_veto": vetoes.iter().any(|(_, verdict)| matches!(verdict, bioprism_bioevalx::boundary::FlowVerdict::Bypass { .. })),
            },
            "guarantees": [
                "policy-authorized flows remain distinct from proposals whose denial was respected",
                "materialized unauthorized irreversible flows are vetoes",
                "bypass attempts remain findings even when they did not succeed",
                "channel exposure is retained rather than collapsed into a privacy percentage",
                "utility and safety remain a Pareto pair; composite_with_utility refuses while violations stand",
            ],
            "limitations": [
                "the route audits declared flows and does not inspect payloads or detect hidden transfers",
                "no necessity or minimum-disclosure counterfactual is inferred",
                "a clean labeled assessment is not an attestation that no uninstrumented flow occurred",
            ],
        }))
    }

    pub(super) fn bioeval_plane_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-plane-audit/0.1";
        const MAX_DIMENSIONS: usize = 4_096;
        const MAX_ID_BYTES: usize = 256;

        let raw_plane = arguments
            .get("plane")
            .ok_or("plane is required and must be a serialized ScorePlane")?;
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure plane audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("plane audit input exceeds the 20000000-byte safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_plane_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "a missing score remains unscored rather than becoming zero",
                    "inapplicable dimensions remain outside the fold denominator",
                    "a fold is accepted only after the real ScorePlane policy accepts it",
                ],
                "limitations": [
                    "the route does not measure scores, select dimensions, or compare systems",
                    "weights and cells are caller-supplied serialized evidence",
                ],
            })
        };

        let plane: BioevalScorePlane = match serde_json::from_value(raw_plane.clone()) {
            Ok(plane) => plane,
            Err(error) => {
                return Ok(refusal(
                    "plane_deserialization",
                    format!("plane is not a valid ScorePlane: {error}"),
                ));
            }
        };
        if plane.system.trim().is_empty() || plane.system.len() > MAX_ID_BYTES {
            return Ok(refusal(
                "plane_validation",
                format!("plane.system must contain 1 to {MAX_ID_BYTES} bytes"),
            ));
        }
        if plane.dimensions().len() > MAX_DIMENSIONS {
            return Ok(refusal(
                "plane_validation",
                format!("plane dimensions are bounded at {MAX_DIMENSIONS} rows"),
            ));
        }

        let serialized = serde_json::to_value(&plane).expect("ScorePlane serializes");
        let serialized_cells = serialized
            .get("cells")
            .and_then(Value::as_object)
            .ok_or("serialized ScorePlane did not contain its cells map")?;
        let declared_ids = plane
            .dimensions()
            .iter()
            .map(|dimension| dimension.id.clone())
            .collect::<BTreeSet<_>>();
        if serialized_cells.keys().any(|id| !declared_ids.contains(id)) {
            return Ok(refusal(
                "plane_validation",
                "cells contains an undeclared dimension".into(),
            ));
        }

        let mut dimension_ids = BTreeSet::new();
        let mut cells = Vec::with_capacity(plane.dimensions().len());
        let mut scored_count = 0usize;
        let mut unscored_count = 0usize;
        let mut inapplicable_count = 0usize;
        let mut unscored_ids = Vec::new();
        let mut inapplicable_ids = Vec::new();
        for dimension in plane.dimensions() {
            if dimension.id.trim().is_empty() || dimension.id.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "plane_validation",
                    format!("dimension ids must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !dimension_ids.insert(dimension.id.clone()) {
                return Ok(refusal(
                    "plane_validation",
                    format!("dimension {:?} appears more than once", dimension.id),
                ));
            }
            if !dimension.weight.is_finite() || dimension.weight <= 0.0 {
                return Ok(refusal(
                    "plane_validation",
                    format!(
                        "dimension {:?} has a non-positive or non-finite weight",
                        dimension.id
                    ),
                ));
            }
            let cell = plane
                .cell(&dimension.id)
                .ok_or_else(|| format!("dimension {:?} has no corresponding cell", dimension.id))?;
            let cell_value = serde_json::to_value(cell).expect("Cell serializes");
            let parsed_cell: BioevalCell =
                serde_json::from_value(cell_value.clone()).map_err(|error| {
                    format!("dimension {:?} has an invalid Cell: {error}", dimension.id)
                })?;
            match &parsed_cell {
                BioevalCell::Scored { .. } => {
                    if !plane.tier.admits(dimension.required) {
                        return Ok(refusal(
                            "plane_validation",
                            format!(
                                "dimension {:?} is scored despite being out of tier",
                                dimension.id
                            ),
                        ));
                    }
                    scored_count += 1;
                }
                BioevalCell::Unscored { .. } => {
                    unscored_count += 1;
                    unscored_ids.push(dimension.id.clone());
                }
                BioevalCell::Inapplicable { required, declared } => {
                    if *required != dimension.required || *declared != plane.tier {
                        return Ok(refusal(
                            "plane_validation",
                            format!(
                                "dimension {:?} carries an inconsistent inapplicable tier",
                                dimension.id
                            ),
                        ));
                    }
                    inapplicable_count += 1;
                    inapplicable_ids.push(dimension.id.clone());
                }
            }
            cells.push(json!({
                "id": dimension.id,
                "required": dimension.required,
                "weight": dimension.weight,
                "cell": cell_value,
                "measured": parsed_cell.is_measured(),
                "blocks_fold": parsed_cell.blocks_fold(),
            }));
        }

        let fold = plane.fold(BioevalFoldPolicy::ExcludeInapplicable);
        let fold_projection = match fold {
            Ok(fold) => json!({
                "folded": true,
                "policy": fold.policy,
                "value": fold.value,
                "included": fold.included,
                "excluded": fold.excluded,
                "refusal": Value::Null,
            }),
            Err(error) => json!({
                "folded": false,
                "policy": BioevalFoldPolicy::ExcludeInapplicable,
                "value": Value::Null,
                "included": [],
                "excluded": [],
                "refusal": error.to_string(),
            }),
        };
        if arguments
            .get("require_fold")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !fold_projection["folded"].as_bool().unwrap_or(false)
        {
            return Ok(refusal(
                "fold_policy",
                fold_projection["refusal"]
                    .as_str()
                    .unwrap_or("ScorePlane fold was refused")
                    .to_string(),
            ));
        }
        let bounded = |values: &[String]| {
            json!({
                "ids": values.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": values.len(),
                "omitted": values.len().saturating_sub(max_items),
            })
        };
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_plane_audit",
            "plane": {
                "system": plane.system,
                "tier": plane.tier,
                "dimension_count": plane.dimensions().len(),
                "scored_count": scored_count,
                "unscored_count": unscored_count,
                "inapplicable_count": inapplicable_count,
            },
            "dimensions": {
                "rows": cells.into_iter().take(max_items).collect::<Vec<_>>(),
                "returned": plane.dimensions().len().min(max_items),
                "total": plane.dimensions().len(),
                "omitted": plane.dimensions().len().saturating_sub(max_items),
            },
            "findings": {
                "unscored_dimensions": bounded(&unscored_ids),
                "inapplicable_dimensions": bounded(&inapplicable_ids),
                "fold_blocked": !fold_projection["folded"].as_bool().unwrap_or(false),
                "fold_refusal": fold_projection["refusal"].clone(),
            },
            "fold": fold_projection,
            "guarantees": [
                "unscored dimensions retain their reason and never become zero",
                "inapplicable dimensions are excluded only by the named ExcludeInapplicable policy",
                "fold value, included dimensions, and excluded dimensions remain bound together",
                "dimension rows and identifier findings retain total and omitted counts",
            ],
            "limitations": [
                "the route audits caller-supplied scores and does not run an evaluator",
                "the route does not impute, reweight by preference, rank systems, or compare incompatible bases",
                "a folded value is a descriptive aggregation, not biological truth, causal effect, or release approval",
            ],
        }))
    }

    pub(super) fn bioeval_acquisition_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-acquisition-audit/0.1";
        let raw_obligations = arguments
            .get("obligations")
            .and_then(Value::as_array)
            .ok_or("obligations is required and must be an array")?;
        let raw_actions = arguments
            .get("actions")
            .and_then(Value::as_array)
            .ok_or("actions is required and must be an array")?;
        if raw_obligations.len() > 512 || raw_actions.len() > 512 {
            return Err("obligations and actions are each bounded at 512 rows".into());
        }
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure acquisition audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("acquisition audit input exceeds the 20000000-byte safety bound".into());
        }

        let mut obligations = Vec::with_capacity(raw_obligations.len());
        let mut obligation_ids = BTreeSet::new();
        for (index, raw) in raw_obligations.iter().enumerate() {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("obligations[{index}] requires an id"))?;
            if id.trim().is_empty() || id.len() > 256 {
                return Err(format!(
                    "obligations[{index}] ids must contain 1 to 256 bytes"
                ));
            }
            if !obligation_ids.insert(id.to_string()) {
                return Err(format!("obligations[{index}] duplicates id {id:?}"));
            }
            let required = raw
                .get("required")
                .and_then(Value::as_bool)
                .ok_or_else(|| format!("obligations[{index}] requires boolean required"))?;
            obligations.push(if required {
                AcquisitionObligation::required(id)
            } else {
                AcquisitionObligation::optional(id)
            });
        }

        let mut trace = AcquisitionTrace::against(obligations)
            .map_err(|error| format!("cannot initialize acquisition trace: {error}"))?;
        for (index, raw) in raw_actions.iter().enumerate() {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("actions[{index}] requires an id"))?;
            if id.trim().is_empty() || id.len() > 256 {
                return Err(format!("actions[{index}] ids must contain 1 to 256 bytes"));
            }
            let kind: AcquisitionKind = serde_json::from_value(
                raw.get("kind")
                    .cloned()
                    .ok_or_else(|| format!("actions[{index}] requires kind"))?,
            )
            .map_err(|error| {
                format!("actions[{index}] has an invalid acquisition kind: {error}")
            })?;
            let cost = raw
                .get("cost")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("actions[{index}] requires a non-negative integer cost"))?;
            let raw_closes: &[Value] = raw
                .get("closes")
                .map(|value| {
                    value
                        .as_array()
                        .ok_or_else(|| format!("actions[{index}].closes must be an array"))
                })
                .transpose()?
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if raw_closes.len() > 512 {
                return Err(format!(
                    "actions[{index}].closes is bounded at 512 obligations"
                ));
            }
            let mut closes = BTreeSet::new();
            let mut action = AcquisitionTraceAction::new(id, kind, cost);
            for (close_index, close) in raw_closes.iter().enumerate() {
                let obligation = close.as_str().ok_or_else(|| {
                    format!("actions[{index}].closes[{close_index}] must be a string")
                })?;
                if !closes.insert(obligation.to_string()) {
                    return Err(format!(
                        "actions[{index}].closes duplicates obligation {obligation:?}"
                    ));
                }
                action = action.closing(obligation);
            }
            if let Err(error) = trace.perform(action) {
                return Ok(json!({
                    "ok": false,
                    "schema": SCHEMA,
                    "stage": "trace_validation",
                    "action_index": index,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "duplicate action identifiers and unopened obligations cannot be credited as successful acquisition work",
                        "a domain-invalid trace is not projected as an admissible or clean run",
                    ],
                }));
            }
        }

        let stopped_after = arguments
            .get("stopped_after")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if stopped_after {
            trace = trace.stopping();
        }

        let reference = match arguments.get("reference_policy") {
            None | Some(Value::Null) => None,
            Some(raw) => {
                let name = raw
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("reference_policy requires name")?;
                if name.trim().is_empty() || name.len() > 256 {
                    return Err("reference_policy name must contain 1 to 256 bytes".into());
                }
                let cost = raw
                    .get("cost")
                    .and_then(Value::as_u64)
                    .ok_or("reference_policy requires a non-negative integer cost")?;
                let admissible = raw
                    .get("admissible")
                    .and_then(Value::as_bool)
                    .ok_or("reference_policy requires boolean admissible")?;
                Some(AcquisitionReferencePolicy::new(name, cost, admissible))
            }
        };
        if arguments
            .get("require_reference")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && reference.is_none()
        {
            return Ok(json!({
                "ok": false,
                "schema": SCHEMA,
                "stage": "reference_policy",
                "refusal": "a named reference_policy is required before regret can be computed",
                "fail_closed": true,
                "guarantees": [
                    "regret is never measured against an unnamed or hidden acquisition baseline"
                ],
            }));
        }

        let open = trace.open();
        let open_ids: BTreeSet<&str> = open
            .iter()
            .map(|obligation| obligation.id.as_str())
            .collect();
        let redundant: BTreeSet<&str> = trace
            .redundant()
            .iter()
            .map(|action| action.id.as_str())
            .collect();
        let unnecessary: BTreeSet<&str> = trace
            .unnecessary()
            .iter()
            .map(|action| action.id.as_str())
            .collect();
        let kind_name = |kind: AcquisitionKind| match kind {
            AcquisitionKind::Retrieval => "retrieval",
            AcquisitionKind::Assay => "assay",
            AcquisitionKind::Metadata => "metadata",
            AcquisitionKind::Expert => "expert",
            AcquisitionKind::Analysis => "analysis",
        };
        let obligations_projection = trace
            .obligations()
            .iter()
            .map(|obligation| {
                json!({
                    "id": obligation.id,
                    "required": obligation.required,
                    "closed": !open_ids.contains(obligation.id.as_str()),
                    "open": open_ids.contains(obligation.id.as_str()),
                })
            })
            .collect::<Vec<_>>();
        let action_projection = trace
            .actions()
            .iter()
            .enumerate()
            .map(|(index, action)| {
                json!({
                    "index": index,
                    "id": action.id,
                    "kind": kind_name(action.kind),
                    "cost": action.cost,
                    "closes": action.closes,
                    "redundant": redundant.contains(action.id.as_str()),
                    "unnecessary": unnecessary.contains(action.id.as_str()),
                })
            })
            .collect::<Vec<_>>();
        let cost_by_kind = AcquisitionKind::ALL
            .iter()
            .map(|kind| {
                json!({
                    "kind": kind_name(*kind),
                    "cost": trace.cost_by_kind().get(kind).copied().unwrap_or(0),
                })
            })
            .collect::<Vec<_>>();
        let regret = reference
            .as_ref()
            .map(|policy| {
                trace
                    .regret_against(Some(policy))
                    .map_err(|error| error.to_string())
            })
            .transpose()?;
        let reference_projection = reference.as_ref().map(|policy| {
            json!({
                "name": policy.name,
                "cost": policy.cost,
                "admissible": policy.admissible,
            })
        });
        let status = if trace.admissible() {
            "admissible"
        } else if trace.stopped_after {
            "stopped_inadmissible"
        } else {
            "open"
        };
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_acquisition_audit",
            "status": status,
            "stopped_after": trace.stopped_after,
            "admissible": trace.admissible(),
            "obligations": obligations_projection,
            "open_obligations": open.iter().map(|obligation| json!({"id": obligation.id, "required": obligation.required})).collect::<Vec<_>>(),
            "required_open_count": open.iter().filter(|obligation| obligation.required).count(),
            "optional_open_count": open.iter().filter(|obligation| !obligation.required).count(),
            "actions": action_projection,
            "action_count": trace.actions().len(),
            "cost": trace.cost(),
            "cost_by_kind": cost_by_kind,
            "findings": {
                "redundant_action_ids": redundant.into_iter().collect::<Vec<_>>(),
                "unnecessary_action_ids": unnecessary.into_iter().collect::<Vec<_>>(),
                "deferred_decisive_cost": trace.deferred_decisive(),
            },
            "reference_policy": reference_projection,
            "regret": regret.as_ref().map(|value| json!({
                "policy": value.policy,
                "cost_difference": value.cost_difference,
                "this_admissible": value.this_admissible,
                "reference_admissible": value.reference_admissible,
                "like_for_like": value.like_for_like(),
            })),
            "guarantees": [
                "required and optional obligations remain separate; optional closure cannot make a required decision admissible",
                "redundant, unnecessary, and deferred-decisive findings are derived from action order and declared closures",
                "regret is a named cost comparison and like_for_like remains visible when either policy is inadmissible",
            ],
            "limitations": [
                "the route audits a caller-supplied trace and does not execute retrieval, assays, analyses, tools, or expert consultations",
                "information gain, downstream decision improvement, adaptive acquisition optimality, and biological truth are not estimated",
                "costs remain in the caller's unit and are not converted across acquisition kinds",
            ],
        }))
    }

    pub(super) fn bioeval_reference_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw = arguments
            .get("reference")
            .cloned()
            .ok_or("reference is required and must be a serialized ReferenceStandard")?;
        let reference = match raw.get("standard").and_then(Value::as_str) {
            Some("distribution") => {
                let mass = serde_json::from_value::<std::collections::BTreeMap<String, f64>>(
                    raw.get("mass")
                        .cloned()
                        .ok_or("distribution reference is missing mass")?,
                )
                .map_err(|error| format!("invalid reference mass map: {error}"))?;
                let dispersion: Dispersion = serde_json::from_value(
                    raw.get("dispersion")
                        .cloned()
                        .ok_or("distribution reference is missing dispersion")?,
                )
                .map_err(|error| format!("invalid reference dispersion: {error}"))?;
                ReferenceStandard::Distribution(
                    ReferenceDistribution::new(mass, dispersion)
                        .map_err(|error| format!("invalid reference distribution: {error}"))?,
                )
            }
            _ => serde_json::from_value(raw)
                .map_err(|error| format!("invalid reference standard: {error}"))?,
        };
        let state = arguments
            .get("state")
            .and_then(Value::as_str)
            .map(str::to_string);
        let (modal_state, modal_mass, entropy_bits, dispersion, state_mass) =
            match reference.distribution() {
                Some(distribution) => {
                    let (state_name, mass) = distribution.mode();
                    (
                        Some(state_name.to_string()),
                        Some(mass),
                        Some(distribution.entropy_bits()),
                        Some(distribution.dispersion().as_str()),
                        state.as_deref().and_then(|name| distribution.mass_on(name)),
                    )
                }
                None => (None, None, None, None, None),
            };
        let reference_kind = reference.as_str();
        let can_certify_clean_pass = reference.can_certify_a_clean_pass();
        let resolution = reference
            .distribution()
            .map(|distribution| distribution.resolution());
        let modal_confidence = reference
            .distribution()
            .map(ReferenceDistribution::modal_confidence);
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/bioeval-reference-audit/0.1",
            "reference": reference,
            "reference_kind": reference_kind,
            "can_certify_clean_pass": can_certify_clean_pass,
            "resolution": resolution,
            "modal_state": modal_state,
            "modal_mass": modal_mass,
            "modal_confidence": modal_confidence,
            "entropy_bits": entropy_bits,
            "dispersion": dispersion,
            "queried_state": state,
            "queried_state_mass": state_mass,
            "guarantees": [
                "distributed reference truth remains a distribution rather than being collapsed to a label",
                "aleatoric, annotation, mixed, and unattributed dispersion remain distinct",
                "unresolved and not-evaluable references are not treated as hidden negatives",
                "mass normalization is validated through ReferenceDistribution::new before metrics are reported"
            ],
            "limitations": [
                "this audits a reference standard and does not score a prediction or adjudicate an oracle",
                "flat state distributions do not provide taxonomic or causal graph distance"
            ]
        }))
    }
}
