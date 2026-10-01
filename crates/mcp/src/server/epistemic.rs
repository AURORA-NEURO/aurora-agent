//! MCP Epistemic selection and decision-quotient handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn epistemic_selection_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/epistemic-selection-audit/0.1";
        let raw_problem = arguments
            .get("problem")
            .cloned()
            .ok_or("problem is required and must be a serialized epistemic DecisionProblem")?;
        let raw_belief = arguments
            .get("belief")
            .cloned()
            .ok_or("belief is required and must be a serialized epistemic Belief")?;
        let raw_pool = arguments
            .get("evidence_pool")
            .cloned()
            .ok_or("evidence_pool is required and must be a serialized epistemic EvidencePool")?;
        let raw_constraint = arguments
            .get("constraint")
            .and_then(Value::as_object)
            .ok_or("constraint is required and must be an object")?;
        let encoded = serde_json::to_vec(&json!({
            "problem": raw_problem.clone(),
            "belief": raw_belief.clone(),
            "evidence_pool": raw_pool.clone(),
            "constraint": raw_constraint,
            "protected": arguments.get("protected").cloned().unwrap_or_else(|| json!([])),
        }))
        .map_err(|error| format!("cannot measure epistemic selection envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("epistemic selection input exceeds the 20000000-byte safety bound".into());
        }

        let parsed_problem: EpistemicDecisionProblem = serde_json::from_value(raw_problem)
            .map_err(|error| format!("invalid decision problem: {error}"))?;
        if parsed_problem.action_count() > 1_000 || parsed_problem.model_count() > 1_000 {
            return Err("decision problems are bounded at 1000 actions and 1000 models".into());
        }
        let action_count = parsed_problem.action_count();
        let model_count = parsed_problem.model_count();
        let problem_ref = &parsed_problem;
        let loss = (0..action_count)
            .flat_map(|action| (0..model_count).map(move |model| problem_ref.loss(action, model)))
            .collect::<Vec<_>>();
        let problem = EpistemicDecisionProblem::new(
            parsed_problem.actions().to_vec(),
            parsed_problem.models().to_vec(),
            loss,
        )
        .map_err(|error| format!("decision problem invariant failed: {error}"))?;

        let parsed_belief: EpistemicBelief = serde_json::from_value(raw_belief)
            .map_err(|error| format!("invalid belief: {error}"))?;
        if parsed_belief.len() > 1_000 {
            return Err("belief is bounded at 1000 models".into());
        }
        let belief = EpistemicBelief::new(parsed_belief.masses().to_vec())
            .map_err(|error| format!("belief invariant failed: {error}"))?;

        let parsed_pool: EpistemicEvidencePool = serde_json::from_value(raw_pool)
            .map_err(|error| format!("invalid evidence pool: {error}"))?;
        if !(1..=64).contains(&parsed_pool.len()) {
            return Err("evidence_pool must contain between 1 and 64 observed items".into());
        }
        let pool = EpistemicEvidencePool::new(parsed_pool.items().to_vec())
            .map_err(|error| format!("evidence pool invariant failed: {error}"))?;
        pool.check_against(&problem).map_err(|error| {
            format!("evidence pool is incompatible with the decision problem: {error}")
        })?;

        let parse_optional_cardinality = |key: &str| -> Result<Option<usize>, String> {
            raw_constraint
                .get(key)
                .map(|value| {
                    value
                        .as_u64()
                        .map(|number| number as usize)
                        .ok_or_else(|| format!("constraint {key} must be a non-negative integer"))
                })
                .transpose()
        };
        let cardinality = parse_optional_cardinality("cardinality")?;
        if cardinality.is_some_and(|value| value > 64) {
            return Err("constraint cardinality is bounded at 64".into());
        }
        let budget = raw_constraint
            .get("budget")
            .map(|value| {
                value
                    .as_f64()
                    .ok_or("constraint budget must be a finite number".to_string())
            })
            .transpose()?;
        let costs = match raw_constraint.get("costs") {
            Some(value) => value
                .as_array()
                .ok_or("constraint costs must be an array".to_string())?
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    value
                        .as_f64()
                        .ok_or_else(|| format!("constraint costs[{index}] must be a finite number"))
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => pool.items().iter().map(|item| item.cost).collect(),
        };
        if costs.len() != pool.len() {
            return Err(format!(
                "constraint costs must contain exactly {} entries",
                pool.len()
            ));
        }
        let constraint = EpistemicConstraint::bounded(cardinality, budget, costs)
            .map_err(|error| format!("constraint invariant failed: {error}"))?;

        let mut protected = BTreeSet::new();
        let raw_protected = arguments
            .get("protected")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_protected = raw_protected
            .as_array()
            .ok_or("protected must be an array of evidence indexes")?;
        for value in raw_protected {
            let index = value
                .as_u64()
                .map(|number| number as usize)
                .ok_or("protected indexes must be non-negative integers")?;
            if index >= pool.len() {
                return Err(format!(
                    "protected evidence index {index} is outside the pool of {} items",
                    pool.len()
                ));
            }
            if !protected.insert(index) {
                return Err(format!("protected evidence index {index} is duplicated"));
            }
        }

        let check_submodularity = arguments
            .get("check_submodularity")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let include_lazy = arguments
            .get("include_lazy")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let compare_optimum = arguments
            .get("compare_optimum")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let tolerance = arguments
            .get("tolerance")
            .map(|value| {
                value
                    .as_f64()
                    .ok_or("tolerance must be a finite non-negative number".to_string())
            })
            .transpose()?
            .unwrap_or(1e-9);
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err("tolerance must be a finite non-negative number".into());
        }

        let objective = match EpistemicRegretReduction::new(&problem, &belief, &pool) {
            Ok(objective) => objective,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": SCHEMA,
                    "stage": "objective",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "selection never proceeds when the full observed context contradicts every supplied model",
                        "regret reduction remains decision-relative and does not become a causal or clinical claim",
                    ],
                }));
            }
        };

        let submodularity_report = if check_submodularity && pool.len() <= 12 {
            match epistemic_submodularity_check(&objective, tolerance) {
                Ok(report) => Some(report),
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": SCHEMA,
                        "stage": "submodularity",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "guarantees": [
                            "the exhaustive check never degrades to sampling",
                            "no approximation factor is attached when the objective cannot be tabulated safely",
                        ],
                    }));
                }
            }
        } else {
            None
        };
        let submodularity = if !check_submodularity {
            json!({
                "status": "not_requested",
                "tolerance": tolerance,
            })
        } else if pool.len() > 12 {
            json!({
                "status": "not_run",
                "reason": "exhaustive_cap",
                "ground": pool.len(),
                "cap": 12,
                "tolerance": tolerance,
            })
        } else {
            json!({
                "status": "evaluated",
                "report": submodularity_report.as_ref(),
            })
        };

        let selection = match epistemic_greedy(
            &objective,
            &constraint,
            &protected,
            submodularity_report.as_ref(),
        ) {
            Ok(selection) => selection,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": SCHEMA,
                    "stage": "greedy_selection",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "submodularity": submodularity,
                    "guarantees": [
                        "protected closure is validated before any marginal is evaluated",
                        "non-positive marginal steps are not forced merely to fill a quota",
                    ],
                }));
            }
        };

        let item_rows: Vec<Value> = pool
            .items()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                json!({
                    "index": index,
                    "id": item.id,
                    "cost": item.cost,
                })
            })
            .collect();
        let project_index = |index: usize| {
            item_rows
                .get(index)
                .cloned()
                .unwrap_or_else(|| json!({"index": index, "refused": "unknown evidence index"}))
        };
        let project_selection = |value: &bioprism_epistemic::Selection| {
            json!({
                "protected": value.protected.iter().map(|index| project_index(*index)).collect::<Vec<_>>(),
                "chosen": value.chosen.iter().map(|index| project_index(*index)).collect::<Vec<_>>(),
                "value": value.value,
                "cost": value.cost,
                "steps": value.steps.iter().map(|step| json!({
                    "evidence": project_index(step.element),
                    "marginal": step.marginal,
                    "cost": step.cost,
                    "reevaluations": step.reevaluations,
                })).collect::<Vec<_>>(),
                "guarantee": value.guarantee,
                "evaluations": value.evaluations,
            })
        };

        let lazy_selection = if include_lazy {
            match epistemic_lazy_greedy(
                &objective,
                &constraint,
                &protected,
                submodularity_report.as_ref(),
            ) {
                Ok(value) => Some(value),
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": SCHEMA,
                        "stage": "lazy_selection",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "submodularity": submodularity,
                        "greedy": project_selection(&selection),
                    }));
                }
            }
        } else {
            None
        };

        let exact_optimum = if !compare_optimum {
            json!({
                "status": "not_requested",
                "cap": 20,
            })
        } else if pool.len() > 20 {
            json!({
                "status": "not_run",
                "reason": "exhaustive_cap",
                "ground": pool.len(),
                "cap": 20,
            })
        } else {
            let (chosen, value) =
                match epistemic_brute_force_optimum(&objective, &constraint, &protected) {
                    Ok(result) => result,
                    Err(error) => {
                        return Ok(json!({
                            "ok": false,
                            "schema": SCHEMA,
                            "stage": "exact_optimum",
                            "refusal": error.to_string(),
                            "fail_closed": true,
                            "submodularity": submodularity,
                            "greedy": project_selection(&selection),
                            "lazy": lazy_selection.as_ref().map(project_selection),
                        }));
                    }
                };
            let selected_value = selection.value;
            let (ratio, ratio_status) = if value.abs() <= 1e-12 {
                (Value::Null, "undefined_zero_optimum")
            } else {
                (json!(selected_value / value), "computed")
            };
            json!({
                "status": "evaluated",
                "chosen": chosen.iter().map(|index| project_index(*index)).collect::<Vec<_>>(),
                "value": value,
                "cost": constraint.cost_of(&chosen),
                "ratio": ratio,
                "ratio_status": ratio_status,
                "greedy_gap": value - selected_value,
            })
        };

        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "objective": "regret_reduction",
            "problem": {
                "actions": problem.actions(),
                "models": problem.models(),
                "action_count": problem.action_count(),
                "model_count": problem.model_count(),
            },
            "belief": { "mass": belief.masses() },
            "evidence_pool": {
                "count": pool.len(),
                "items": item_rows,
                "total_cost": pool.rate(&pool.everything()).map_err(|error| error.to_string())?,
            },
            "constraint": {
                "cardinality": constraint.cardinality,
                "budget": constraint.budget,
                "costs": constraint.costs,
            },
            "protected": protected.iter().map(|index| project_index(*index)).collect::<Vec<_>>(),
            "baseline": {
                "full_context_regret": objective.baseline(),
                "empty_context_value": objective.value(&BTreeSet::new()).map_err(|error| error.to_string())?,
            },
            "submodularity": submodularity,
            "greedy": project_selection(&selection),
            "lazy": lazy_selection.as_ref().map(project_selection),
            "comparisons": {
                "greedy_lazy_agree": lazy_selection.as_ref().map(|value| value.chosen == selection.chosen),
                "exact_optimum": exact_optimum,
            },
            "guarantees": [
                "the 1 - 1/e factor is serialized only when exhaustive monotonicity and submodularity checks pass under a cardinality-only constraint",
                "protected evidence is charged and validated before relevance selection",
                "exact comparison is exhaustive only within the stated 20-item cap; above it the route reports not_run rather than sampling",
                "regret reduction is a decision-relative observed-context objective, not causal identification, adaptive acquisition, clinical advice, or execution",
            ],
            "limitations": [
                "selection cost is a caller-supplied scalarization; token, compute, latency, privacy, specimen, and expert burden vectors are not inferred",
                "lazy-greedy agreement is diagnostic and does not establish submodularity without the exhaustive report",
                "adaptive or sequential acquisition policies remain outside this non-adaptive observed-context planner",
            ],
        }))
    }

    pub(super) fn epistemic_context_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_problem = arguments
            .get("problem")
            .cloned()
            .ok_or("problem is required and must be a serialized epistemic DecisionProblem")?;
        let raw_belief = arguments
            .get("belief")
            .cloned()
            .ok_or("belief is required and must be a serialized epistemic Belief")?;
        let raw_pool = arguments
            .get("evidence_pool")
            .cloned()
            .ok_or("evidence_pool is required and must be a serialized epistemic EvidencePool")?;
        let criterion: EpistemicDistortionCriterion = serde_json::from_value(
            arguments
                .get("criterion")
                .cloned()
                .ok_or("criterion is required and must be bayes_regret or minimax_regret")?,
        )
        .map_err(|error| format!("invalid context distortion criterion: {error}"))?;
        let tolerance = arguments
            .get("tolerance")
            .and_then(Value::as_f64)
            .ok_or("tolerance is required and must be a finite non-negative number")?;
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err("tolerance must be finite and non-negative".into());
        }
        let compatibility_floor = arguments
            .get("compatibility_floor")
            .and_then(Value::as_f64)
            .ok_or("compatibility_floor is required and must be between 0 and 1")?;
        if !compatibility_floor.is_finite() || !(0.0..=1.0).contains(&compatibility_floor) {
            return Err("compatibility_floor must be finite and between 0 and 1".into());
        }
        let include_frontier = arguments
            .get("include_frontier")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let max_rows = arguments
            .get("max_rows")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_rows == 0 || max_rows > 1_000 {
            return Err("max_rows must be between 1 and 1000".into());
        }
        let raw_subsets = arguments
            .get("subsets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_subsets.len() > 256 {
            return Err("subsets must contain at most 256 context index sets".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "problem": raw_problem.clone(),
            "belief": raw_belief.clone(),
            "evidence_pool": raw_pool.clone(),
            "criterion": criterion,
            "tolerance": tolerance,
            "compatibility_floor": compatibility_floor,
            "subsets": raw_subsets.clone(),
        }))
        .map_err(|error| format!("cannot measure epistemic context envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("epistemic context input exceeds the 20000000-byte safety bound".into());
        }

        let parsed_problem: EpistemicDecisionProblem = serde_json::from_value(raw_problem)
            .map_err(|error| format!("invalid decision problem: {error}"))?;
        if parsed_problem.action_count() > 1_000 || parsed_problem.model_count() > 1_000 {
            return Err("decision problems are bounded at 1000 actions and 1000 models".into());
        }
        let action_count = parsed_problem.action_count();
        let model_count = parsed_problem.model_count();
        let problem_ref = &parsed_problem;
        let problem = EpistemicDecisionProblem::new(
            parsed_problem.actions().to_vec(),
            parsed_problem.models().to_vec(),
            (0..action_count)
                .flat_map(|action| {
                    (0..model_count).map(move |model| problem_ref.loss(action, model))
                })
                .collect(),
        )
        .map_err(|error| format!("decision problem invariant failed: {error}"))?;

        let parsed_belief: EpistemicBelief = serde_json::from_value(raw_belief)
            .map_err(|error| format!("invalid belief: {error}"))?;
        if parsed_belief.len() > 1_000 {
            return Err("belief is bounded at 1000 models".into());
        }
        let belief = EpistemicBelief::new(parsed_belief.masses().to_vec())
            .map_err(|error| format!("belief invariant failed: {error}"))?;

        let parsed_pool: EpistemicEvidencePool = serde_json::from_value(raw_pool)
            .map_err(|error| format!("invalid evidence pool: {error}"))?;
        if parsed_pool.len() > 16 {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/epistemic-context-audit/0.1",
                "stage": "enumeration_bound",
                "refusal": "evidence_pool has more than 16 items; exhaustive frontier and minimal-context claims would exceed the kernel cap",
                "fail_closed": true,
                "pool_size": parsed_pool.len(),
                "cap": 16,
                "guarantees": [
                    "a frontier is never downgraded to a heuristic sample while retaining a minimum claim"
                ]
            }));
        }
        let pool = EpistemicEvidencePool::new(parsed_pool.items().to_vec())
            .map_err(|error| format!("evidence pool invariant failed: {error}"))?;
        pool.check_against(&problem)
            .map_err(|error| format!("evidence pool does not match decision problem: {error}"))?;

        let identification = match epistemic_identification(
            &problem,
            &belief,
            &pool,
            tolerance,
            compatibility_floor,
        ) {
            Ok(report) => report,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/epistemic-context-audit/0.1",
                    "stage": "identification",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "decision identification is not inferred from a compressed context when full evidence is invalid"
                    ]
                }));
            }
        };
        let sufficiency = match epistemic_minimal_sufficient_context(
            &problem,
            &belief,
            &pool,
            criterion,
            tolerance,
            compatibility_floor,
        ) {
            Ok(report) => report,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/epistemic-context-audit/0.1",
                    "stage": "minimal_context",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "insufficient or contradictory evidence is not converted into a cheapest context"
                    ]
                }));
            }
        };
        let frontier = if include_frontier {
            match epistemic_frontier(&problem, &belief, &pool, criterion, compatibility_floor) {
                Ok(report) => json!(report),
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/epistemic-context-audit/0.1",
                        "stage": "frontier",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "guarantees": [
                            "frontier output is exhaustive or withheld; it is never a sampled approximation wearing a minimum label"
                        ]
                    }));
                }
            }
        } else {
            Value::Null
        };

        let mut subset_rows = Vec::with_capacity(raw_subsets.len());
        let mut subset_refusal_count = 0usize;
        for (index, raw_subset) in raw_subsets.iter().enumerate() {
            let Some(values) = raw_subset.as_array() else {
                subset_refusal_count += 1;
                subset_rows.push(json!({
                    "index": index,
                    "result": "refused",
                    "refusal": "each subset must be an array of evidence indexes",
                    "fail_closed": true,
                }));
                continue;
            };
            let mut subset = BTreeSet::new();
            let mut refusal = None;
            for (position, value) in values.iter().enumerate() {
                let Some(raw_index) = value.as_u64() else {
                    refusal = Some(format!(
                        "subsets[{index}][{position}] must be a non-negative integer"
                    ));
                    break;
                };
                let evidence_index = usize::try_from(raw_index).map_err(|_| {
                    format!("subsets[{index}][{position}] is outside the supported range")
                })?;
                if evidence_index >= pool.len() {
                    refusal = Some(format!(
                        "subsets[{index}] names evidence index {evidence_index}, outside pool size {}",
                        pool.len()
                    ));
                    break;
                }
                if !subset.insert(evidence_index) {
                    refusal = Some(format!(
                        "subsets[{index}] repeats evidence index {evidence_index}"
                    ));
                    break;
                }
            }
            if let Some(refusal) = refusal {
                subset_refusal_count += 1;
                subset_rows.push(json!({
                    "index": index,
                    "subset": values,
                    "result": "refused",
                    "refusal": refusal,
                    "fail_closed": true,
                }));
                continue;
            }
            match epistemic_evaluate_context(
                &problem,
                &belief,
                &pool,
                &subset,
                criterion,
                compatibility_floor,
            ) {
                Ok(evaluation) => subset_rows.push(json!({
                    "index": index,
                    "subset": subset,
                    "result": "evaluated",
                    "evaluation": evaluation,
                })),
                Err(error) => {
                    subset_refusal_count += 1;
                    subset_rows.push(json!({
                        "index": index,
                        "subset": subset,
                        "result": "refused",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                    }));
                }
            }
        }
        let output = json!({
            "ok": true,
            "schema": "bioprism-mcp/epistemic-context-audit/0.1",
            "criterion": criterion,
            "tolerance": tolerance,
            "compatibility_floor": compatibility_floor,
            "problem": {
                "actions": problem.actions(),
                "models": problem.models(),
                "action_count": problem.action_count(),
                "model_count": problem.model_count(),
            },
            "evidence_pool": {
                "item_count": pool.len(),
                "items": pool.items().iter().map(|item| json!({
                    "id": item.id,
                    "cost": item.cost,
                    "likelihoods": item.likelihoods(),
                })).collect::<Vec<_>>(),
                "full_rate": pool.rate(&pool.everything()).map_err(|error| error.to_string())?,
            },
            "identification": identification,
            "sufficiency": sufficiency,
            "frontier": frontier,
            "include_frontier": include_frontier,
            "subset_rows": subset_rows.iter().take(max_rows as usize).collect::<Vec<_>>(),
            "subset_count": subset_rows.len(),
            "subset_refusal_count": subset_refusal_count,
            "subset_rows_omitted": subset_rows.len().saturating_sub(max_rows as usize),
            "max_rows": max_rows,
            "guarantees": [
                "rate is summed evidence cost and distortion is decision regret, not embedding similarity",
                "identification, minimal sufficiency, exhaustive frontier, and requested subset evaluations remain separate evidence layers",
                "minimax non-identification abstains rather than selecting a context that cannot support the requested tolerance"
            ],
            "limitations": [
                "the prior, loss matrix, likelihoods, and scalar evidence costs are caller-declared",
                "the endpoint evaluates already-available evidence and does not run an adaptive acquisition policy",
                "decision identification is not causal identification, and a sufficient context is not a biological or clinical conclusion"
            ]
        });
        let output_bytes = serde_json::to_vec(&output)
            .map_err(|error| format!("cannot measure epistemic context result: {error}"))?;
        if output_bytes.len() > 20_000_000 {
            return Err("epistemic context result exceeds the 20000000-byte safety bound".into());
        }
        Ok(output)
    }

    pub(super) fn epistemic_decision_quotient(&self, arguments: &Value) -> Result<Value, String> {
        let raw_problem = arguments
            .get("problem")
            .cloned()
            .ok_or("problem is required and must be a serialized epistemic DecisionProblem")?;
        let raw_actions = arguments
            .get("permitted_actions")
            .and_then(Value::as_array)
            .ok_or("permitted_actions is required and must be an array of action names")?;
        if raw_actions.is_empty() || raw_actions.len() > 1_000 {
            return Err("permitted_actions must contain between 1 and 1000 actions".into());
        }
        let permitted_actions = raw_actions
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let action = value
                    .as_str()
                    .ok_or_else(|| format!("permitted_actions[{index}] must be a string"))?;
                if action.trim().is_empty() || action.len() > 256 {
                    return Err(format!(
                        "permitted_actions[{index}] must contain between 1 and 256 bytes"
                    ));
                }
                Ok(action.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let encoded = serde_json::to_vec(&json!({
            "problem": raw_problem.clone(),
            "permitted_actions": permitted_actions.clone(),
        }))
        .map_err(|error| format!("cannot measure decision quotient envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("decision quotient input exceeds the 20000000-byte safety bound".into());
        }

        let parsed_problem: EpistemicDecisionProblem = serde_json::from_value(raw_problem)
            .map_err(|error| format!("invalid decision problem: {error}"))?;
        parsed_problem
            .validate()
            .map_err(|error| format!("decision problem invariant failed: {error}"))?;
        if parsed_problem.action_count() > 1_000 || parsed_problem.model_count() > 1_000 {
            return Err("decision problems are bounded at 1000 actions and 1000 models".into());
        }
        let action_count = parsed_problem.action_count();
        let model_count = parsed_problem.model_count();
        let problem_ref = &parsed_problem;
        let loss = (0..action_count)
            .flat_map(|action| (0..model_count).map(move |model| problem_ref.loss(action, model)))
            .collect::<Vec<_>>();
        let problem = EpistemicDecisionProblem::new(
            parsed_problem.actions().to_vec(),
            parsed_problem.models().to_vec(),
            loss,
        )
        .map_err(|error| format!("decision problem invariant failed: {error}"))?;

        let quotient = match epistemic_decision_equivalence_quotient(&problem, &permitted_actions) {
            Ok(quotient) => quotient,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/epistemic-decision-quotient/0.1",
                    "stage": "decision_quotient",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "an empty, duplicate, or unknown permitted-action set is refused rather than treated as all actions",
                        "malformed or non-finite decision losses remain refusals",
                    ],
                    "limitations": [
                        "the quotient is decision-relative to the supplied loss table and permitted action names",
                    ],
                }));
            }
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/epistemic-decision-quotient/0.1",
            "quotient": quotient,
            "summary": {
                "original_model_count": problem.model_count(),
                "quotient_model_count": quotient.quotient_model_count,
                "merged_model_count": quotient.merged_model_count,
                "compressed": quotient.compressed(),
                "compression_fraction": quotient.compression_fraction(),
            },
            "guarantees": [
                "models merge only when their permitted-action loss-difference profiles are exactly equal",
                "model identities, class membership, preferred-action ties, and the permitted action boundary remain visible",
                "the relation is transitive and deterministic under model or action input reordering",
                "the quotient preserves permitted-action ordering and regret profiles, not absolute loss baselines",
            ],
            "limitations": [
                "this is not causal, biological, clinical, predictive, or likelihood equivalence",
                "forbidden actions and distinctions outside the supplied loss table are intentionally not preserved",
                "fiber-query/0.2 still cannot invoke this pass because it carries neither permitted_actions nor decision_loss",
            ],
        }))
    }

    pub(super) fn epistemic_voi(&self, arguments: &Value) -> Result<Value, String> {
        let raw_problem = arguments
            .get("problem")
            .cloned()
            .ok_or("problem is required and must be a serialized epistemic DecisionProblem")?;
        let raw_belief = arguments
            .get("belief")
            .cloned()
            .ok_or("belief is required and must be a serialized epistemic Belief")?;
        let raw_acquisitions = if let Some(raw) = arguments.get("acquisition") {
            vec![raw.clone()]
        } else {
            arguments
                .get("acquisitions")
                .and_then(Value::as_array)
                .cloned()
                .ok_or("provide acquisition or acquisitions")?
        };
        if raw_acquisitions.is_empty() || raw_acquisitions.len() > 64 {
            return Err("acquisitions must contain between 1 and 64 actions".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "problem": raw_problem.clone(),
            "belief": raw_belief.clone(),
            "acquisitions": raw_acquisitions.clone(),
        }))
        .map_err(|error| format!("cannot measure epistemic envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("epistemic input exceeds the 20000000-byte safety bound".into());
        }

        let parsed_problem: EpistemicDecisionProblem = serde_json::from_value(raw_problem)
            .map_err(|error| format!("invalid decision problem: {error}"))?;
        if parsed_problem.action_count() > 1_000 || parsed_problem.model_count() > 1_000 {
            return Err("decision problems are bounded at 1000 actions and 1000 models".into());
        }
        let action_count = parsed_problem.action_count();
        let model_count = parsed_problem.model_count();
        let problem_ref = &parsed_problem;
        let loss = (0..action_count)
            .flat_map(|action| (0..model_count).map(move |model| problem_ref.loss(action, model)))
            .collect::<Vec<_>>();
        let problem = EpistemicDecisionProblem::new(
            parsed_problem.actions().to_vec(),
            parsed_problem.models().to_vec(),
            loss,
        )
        .map_err(|error| format!("decision problem invariant failed: {error}"))?;

        let parsed_belief: EpistemicBelief = serde_json::from_value(raw_belief)
            .map_err(|error| format!("invalid belief: {error}"))?;
        if parsed_belief.len() > 1_000 {
            return Err("belief is bounded at 1000 models".into());
        }
        let belief = EpistemicBelief::new(parsed_belief.masses().to_vec())
            .map_err(|error| format!("belief invariant failed: {error}"))?;

        let parse_acquisition = |raw: &Value| -> Result<EpistemicAcquisition, String> {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .ok_or("each acquisition requires an id")?;
            if id.trim().is_empty() || id.len() > 256 {
                return Err("acquisition ids must contain between 1 and 256 bytes".into());
            }
            let cost = raw
                .get("cost")
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("acquisition {id:?} requires a finite numeric cost"))?;
            let raw_outcomes = raw
                .get("outcomes")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("acquisition {id:?} requires outcomes"))?;
            if raw_outcomes.is_empty() || raw_outcomes.len() > 1_000 {
                return Err(format!(
                    "acquisition {id:?} must contain 1 to 1000 outcomes"
                ));
            }
            let mut outcomes = Vec::with_capacity(raw_outcomes.len());
            for raw_outcome in raw_outcomes {
                let label = raw_outcome
                    .get("label")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("acquisition {id:?} outcomes require labels"))?;
                let likelihood = raw_outcome
                    .get("likelihood")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("acquisition {id:?}/{label:?} requires likelihood"))?
                    .iter()
                    .map(|value| {
                        value.as_f64().ok_or_else(|| {
                            format!("acquisition {id:?}/{label:?} has a non-numeric likelihood")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                outcomes.push(EpistemicOutcome::new(label, likelihood));
            }
            EpistemicAcquisition::new(id, cost, outcomes, problem.model_count())
                .map_err(|error| format!("acquisition {id:?} invariant failed: {error}"))
        };

        let acquisitions = raw_acquisitions
            .iter()
            .map(parse_acquisition)
            .collect::<Result<Vec<_>, _>>()?;
        let value = if acquisitions.len() == 1 {
            epistemic_value_of_information(&problem, &belief, &acquisitions[0])
        } else {
            epistemic_joint_value(&problem, &belief, &acquisitions)
        };
        let value = match value {
            Ok(value) => value,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "value_of_information",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "improper likelihood partitions, non-finite values, contradictory beliefs, and exhaustive outcome explosions remain refusals",
                        "gross value is not reported as net value; declared acquisition cost remains separate",
                    ],
                }));
            }
        };
        let action_after = value
            .action_after
            .iter()
            .map(|index| problem.actions()[*index].clone())
            .collect::<Vec<_>>();
        let action_without = problem.actions()[value.action_without].clone();
        let complementarity = if acquisitions.len() > 1 {
            match epistemic_complementarity(&problem, &belief, &acquisitions) {
                Ok(report) => json!(report),
                Err(error) => json!({
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                }),
            }
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "mode": if acquisitions.len() == 1 { "single" } else { "non_adaptive_joint_bundle" },
            "value": value,
            "actions": {
                "without": action_without,
                "after": action_after,
            },
            "complementarity": complementarity,
            "guarantees": [
                "gross risk reduction and declared acquisition cost remain separate",
                "action changes are reported by action identity rather than rounded numeric value",
                "joint bundles are explicitly non-adaptive and enumerate bounded outcome products",
                "no causal identification, adaptive policy, execution, or hidden prior is invented",
            ],
        }))
    }

    pub(super) fn epistemic_adaptive_acquisition(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/epistemic-adaptive-acquisition/0.1";
        const MAX_ACQUISITIONS: usize = 16;
        const MAX_STEPS: usize = 16;
        let raw_problem = arguments
            .get("problem")
            .cloned()
            .ok_or("problem is required and must be a serialized epistemic DecisionProblem")?;
        let raw_belief = arguments
            .get("belief")
            .cloned()
            .ok_or("belief is required and must be a serialized epistemic Belief")?;
        let raw_acquisitions = arguments
            .get("acquisitions")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("acquisitions is required and must be an array")?;
        if raw_acquisitions.is_empty() || raw_acquisitions.len() > MAX_ACQUISITIONS {
            return Err(format!(
                "acquisitions must contain between 1 and {MAX_ACQUISITIONS} actions"
            ));
        }
        let budget = arguments
            .get("budget")
            .and_then(Value::as_f64)
            .ok_or("budget is required and must be a finite non-negative number")?;
        if !budget.is_finite() || budget < 0.0 {
            return Err("budget must be finite and non-negative".into());
        }
        let max_steps = arguments
            .get("max_steps")
            .and_then(Value::as_u64)
            .ok_or("max_steps is required and must be an integer")?
            as usize;
        if max_steps > MAX_STEPS {
            return Err(format!(
                "max_steps must not exceed the exact horizon cap of {MAX_STEPS}"
            ));
        }
        let encoded = serde_json::to_vec(&json!({
            "problem": raw_problem.clone(),
            "belief": raw_belief.clone(),
            "acquisitions": raw_acquisitions.clone(),
            "budget": budget,
            "max_steps": max_steps,
        }))
        .map_err(|error| format!("cannot measure adaptive epistemic envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("adaptive epistemic input exceeds the 20000000-byte safety bound".into());
        }

        let problem: EpistemicDecisionProblem = serde_json::from_value(raw_problem)
            .map_err(|error| format!("invalid decision problem: {error}"))?;
        problem
            .validate()
            .map_err(|error| format!("decision problem invariant failed: {error}"))?;
        if problem.action_count() > 1_000 || problem.model_count() > 1_000 {
            return Err("decision problems are bounded at 1000 actions and 1000 models".into());
        }
        let parsed_belief: EpistemicBelief = serde_json::from_value(raw_belief)
            .map_err(|error| format!("invalid belief: {error}"))?;
        if parsed_belief.len() > 1_000 {
            return Err("belief is bounded at 1000 models".into());
        }
        let belief = EpistemicBelief::new(parsed_belief.masses().to_vec())
            .map_err(|error| format!("belief invariant failed: {error}"))?;

        let parse_acquisition = |raw: &Value| -> Result<EpistemicAcquisition, String> {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .ok_or("each acquisition requires an id")?;
            if id.trim().is_empty() || id.len() > 256 {
                return Err("acquisition ids must contain between 1 and 256 bytes".into());
            }
            let cost = raw
                .get("cost")
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("acquisition {id:?} requires a finite numeric cost"))?;
            let raw_outcomes = raw
                .get("outcomes")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("acquisition {id:?} requires outcomes"))?;
            if raw_outcomes.is_empty() || raw_outcomes.len() > 1_000 {
                return Err(format!(
                    "acquisition {id:?} must contain 1 to 1000 outcomes"
                ));
            }
            let mut outcomes = Vec::with_capacity(raw_outcomes.len());
            for raw_outcome in raw_outcomes {
                let label = raw_outcome
                    .get("label")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("acquisition {id:?} outcomes require labels"))?;
                if label.trim().is_empty() || label.len() > 256 {
                    return Err(format!(
                        "acquisition {id:?} outcome labels must contain between 1 and 256 bytes"
                    ));
                }
                let likelihood = raw_outcome
                    .get("likelihood")
                    .and_then(Value::as_array)
                    .ok_or_else(|| format!("acquisition {id:?}/{label:?} requires likelihood"))?
                    .iter()
                    .map(|value| {
                        value.as_f64().ok_or_else(|| {
                            format!("acquisition {id:?}/{label:?} has a non-numeric likelihood")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                outcomes.push(EpistemicOutcome::new(label, likelihood));
            }
            EpistemicAcquisition::new(id, cost, outcomes, problem.model_count())
                .map_err(|error| format!("acquisition {id:?} invariant failed: {error}"))
        };
        let acquisitions = raw_acquisitions
            .iter()
            .map(parse_acquisition)
            .collect::<Result<Vec<_>, _>>()?;
        let policy = match epistemic_adaptive_policy(
            &problem,
            &belief,
            &acquisitions,
            budget,
            max_steps,
        ) {
            Ok(policy) => policy,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": SCHEMA,
                    "stage": "adaptive_policy",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "the endpoint never executes an acquisition or claims that an observation was collected",
                        "budget, horizon, unique-acquisition, likelihood-partition, and exact-state caps are refusal boundaries",
                        "a refusal is never converted into a sampled or approximate policy",
                    ],
                    "limitations": [
                        "outcomes are conditionally independent given the supplied models",
                        "declared costs are caller-supplied scalarizations rather than inferred resource vectors",
                        "the result is decision-relative and is not causal, clinical, biological, or predictive truth",
                    ],
                }));
            }
        };

        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "budget": budget,
            "max_steps": max_steps,
            "problem": {
                "actions": problem.actions(),
                "models": problem.models(),
                "action_count": problem.action_count(),
                "model_count": problem.model_count(),
            },
            "acquisitions": acquisitions.iter().map(|acquisition| json!({
                "id": acquisition.id,
                "cost": acquisition.cost,
                "outcomes": acquisition.outcomes().iter().map(|outcome| json!({
                    "label": outcome.label,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "policy": {
                "expected_total": policy.expected_total,
                "expected_terminal_risk": policy.expected_terminal_risk,
                "expected_acquisition_cost": policy.expected_acquisition_cost,
                "nodes_evaluated": policy.nodes_evaluated,
                "selected_depth": policy.selected_depth,
                "root": project_adaptive_node(&policy.root, &problem),
            },
            "guarantees": [
                "the returned policy is exact under the stated 16-acquisition, 16-step, and 65536-state enumeration caps",
                "each acquisition is used at most once and branch-dependent next choices are explicit",
                "expected terminal risk and expected declared cost remain separate from their scalarized total",
                "posterior model masses and outcome probabilities are serialized at every branch",
            ],
            "limitations": [
                "conditional independence given the caller-supplied models is assumed",
                "the endpoint plans but does not execute, authenticate, schedule, or observe acquisitions",
                "the decision-relative policy is not causal, clinical, biological, or predictive truth",
                "costs are a caller-supplied scalarization of burden, latency, privacy, compute, or money",
            ],
        }))
    }

    pub(super) fn epistemic_adaptive_costed(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/epistemic-adaptive-costed/0.1";
        let raw_problem = arguments
            .get("problem")
            .cloned()
            .ok_or("problem is required")?;
        let raw_belief = arguments
            .get("belief")
            .cloned()
            .ok_or("belief is required")?;
        let raw_acquisitions = arguments
            .get("acquisitions")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("acquisitions is required and must be an array")?;
        if raw_acquisitions.is_empty() || raw_acquisitions.len() > 16 {
            return Err("acquisitions must contain between 1 and 16 actions".into());
        }
        let raw_budget = arguments
            .get("budget")
            .cloned()
            .ok_or("budget is required and must be a seven-dimensional cost vector")?;
        let raw_weights = arguments
            .get("weights")
            .cloned()
            .ok_or("weights is required and must be a seven-dimensional cost vector")?;
        let budget: EpistemicCostVector = serde_json::from_value(raw_budget)
            .map_err(|error| format!("invalid vector budget: {error}"))?;
        let weights: EpistemicCostWeights = serde_json::from_value(raw_weights)
            .map_err(|error| format!("invalid vector weights: {error}"))?;
        let max_steps = arguments
            .get("max_steps")
            .and_then(Value::as_u64)
            .ok_or("max_steps is required and must be an integer")?
            as usize;
        if max_steps > 16 {
            return Err("max_steps must not exceed the exact horizon cap of 16".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "problem": raw_problem.clone(),
            "belief": raw_belief.clone(),
            "acquisitions": raw_acquisitions.clone(),
            "budget": budget,
            "weights": weights,
            "max_steps": max_steps,
        }))
        .map_err(|error| format!("cannot measure vector-cost envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("vector-cost adaptive input exceeds the 20000000-byte safety bound".into());
        }
        let problem: EpistemicDecisionProblem = serde_json::from_value(raw_problem)
            .map_err(|error| format!("invalid decision problem: {error}"))?;
        problem
            .validate()
            .map_err(|error| format!("decision problem invariant failed: {error}"))?;
        if problem.action_count() > 1_000 || problem.model_count() > 1_000 {
            return Err("decision problems are bounded at 1000 actions and 1000 models".into());
        }
        let belief: EpistemicBelief = serde_json::from_value(raw_belief)
            .map_err(|error| format!("invalid belief: {error}"))?;
        if belief.len() > 1_000 {
            return Err("belief is bounded at 1000 models".into());
        }
        belief
            .check_against(&problem)
            .map_err(|error| format!("belief invariant failed: {error}"))?;
        let acquisitions: Vec<EpistemicCostedAcquisition> =
            serde_json::from_value(Value::Array(raw_acquisitions))
                .map_err(|error| format!("invalid vector-cost acquisitions: {error}"))?;
        let policy = match epistemic_vector_adaptive_policy(
            &problem,
            &belief,
            &acquisitions,
            budget,
            weights,
            max_steps,
        ) {
            Ok(policy) => policy,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": SCHEMA,
                    "stage": "adaptive_vector_policy",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "cost_dimensions": bioprism_epistemic::COST_DIMENSIONS,
                    "guarantees": [
                        "every acquisition path is checked component-wise against the remaining vector budget",
                        "scalar weights compare only policies that already satisfy every component budget",
                        "no scalar cost is substituted for an omitted vector dimension",
                    ],
                }));
            }
        };
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "cost_dimensions": bioprism_epistemic::COST_DIMENSIONS,
            "budget": budget,
            "weights": weights,
            "max_steps": max_steps,
            "problem": {
                "actions": problem.actions(),
                "models": problem.models(),
                "action_count": problem.action_count(),
                "model_count": problem.model_count(),
            },
            "acquisitions": acquisitions,
            "policy": policy,
            "guarantees": [
                "the exact finite-horizon policy is feasible in every declared cost dimension",
                "expected acquisition cost is returned as a vector and as its explicit scalarization",
                "branch-dependent next acquisitions and posterior masses remain serialized",
            ],
            "limitations": [
                "the planner assumes conditional independence given caller-supplied models",
                "cost vectors and weights are caller declarations, not provider telemetry",
                "the result plans but does not execute acquisitions or establish causal, clinical, biological, or predictive truth",
            ],
        }))
    }

    pub(super) fn epistemic_adaptive_execute(&self, arguments: &Value) -> Result<Value, String> {
        let mode = arguments
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("simulate");
        if mode != "simulate" && mode != "replay" {
            return Err("mode must be \"simulate\" or \"replay\"".into());
        }
        let problem: EpistemicDecisionProblem = serde_json::from_value(
            arguments
                .get("problem")
                .cloned()
                .ok_or("problem is required")?,
        )
        .map_err(|error| format!("invalid decision problem: {error}"))?;
        let belief: EpistemicBelief = serde_json::from_value(
            arguments
                .get("belief")
                .cloned()
                .ok_or("belief is required")?,
        )
        .map_err(|error| format!("invalid belief: {error}"))?;
        let acquisitions: Vec<EpistemicAcquisition> = serde_json::from_value(
            arguments
                .get("acquisitions")
                .cloned()
                .ok_or("acquisitions is required")?,
        )
        .map_err(|error| format!("invalid acquisitions: {error}"))?;
        let budget = arguments
            .get("budget")
            .and_then(Value::as_f64)
            .ok_or("budget is required and must be a finite non-negative number")?;
        let max_steps = arguments
            .get("max_steps")
            .and_then(Value::as_u64)
            .ok_or("max_steps is required and must be an integer")?
            as usize;
        let plan = EpistemicAdaptivePlan::new(problem, belief, acquisitions, budget, max_steps)
            .map_err(|error| format!("adaptive plan refused: {error}"))?;
        let plan_digest = plan
            .digest()
            .map_err(|error| format!("cannot digest adaptive plan: {error}"))?;
        let provider = arguments
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("mcp-simulated")
            .to_string();

        let receipt = if mode == "replay" {
            let prior: EpistemicAdaptiveExecutionReceipt = serde_json::from_value(
                arguments
                    .get("receipt")
                    .cloned()
                    .ok_or("receipt is required in replay mode")?,
            )
            .map_err(|error| format!("invalid adaptive execution receipt: {error}"))?;
            plan.replay(&prior)
                .map_err(|error| format!("adaptive replay refused: {error}"))?
        } else {
            let grant = match arguments.get("authorization") {
                None => None,
                Some(raw) => {
                    let object = raw.as_object().ok_or("authorization must be an object")?;
                    let grant_id = object
                        .get("grant_id")
                        .and_then(Value::as_str)
                        .ok_or("authorization.grant_id is required")?;
                    let authorized_provider = object
                        .get("provider")
                        .and_then(Value::as_str)
                        .ok_or("authorization.provider is required")?;
                    Some(
                        EpistemicExecutionGrant::issue(
                            grant_id,
                            plan_digest.clone(),
                            authorized_provider,
                        )
                        .map_err(|error| format!("authorization refused: {error}"))?,
                    )
                }
            };
            let raw_observations = arguments
                .get("observations")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if raw_observations.len() > 16 {
                return Err("observations cannot exceed the exact 16-step bound".into());
            }
            let script = raw_observations
                .iter()
                .enumerate()
                .map(|(index, raw)| {
                    let object = raw
                        .as_object()
                        .ok_or_else(|| format!("observations[{index}] must be an object"))?;
                    let acquisition_id = object
                        .get("acquisition_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("observations[{index}].acquisition_id is required")
                        })?;
                    let outcome_label = object
                        .get("outcome_label")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            format!("observations[{index}].outcome_label is required")
                        })?;
                    Ok((acquisition_id.to_string(), outcome_label.to_string()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let mut executor = EpistemicScriptedExecutor::simulated(provider, script);
            plan.execute(grant.as_ref(), &mut executor)
                .map_err(|error| format!("adaptive execution refused: {error}"))?
        };
        let (observed, simulated, replayed) = receipt.provenance_counts();
        Ok(json!({
            "ok": true,
            "schema": ADAPTIVE_EXECUTION_SCHEMA,
            "mode": mode,
            "plan_digest": plan_digest,
            "completed": receipt.is_completed(),
            "receipt": receipt,
            "provenance_counts": {
                "observed": observed,
                "simulated": simulated,
                "replayed": replayed,
            },
            "guarantees": [
                "no provider call occurs without an explicit plan-scoped grant",
                "provider identity, acquisition identity, declared outcome labels, evidence digests, path budget, and policy order are checked",
                "partial and refused runs retain their validated prefix rather than being upgraded to completion",
                "replay uses a receipt-only executor with no live-source fallback",
            ],
            "limitations": [
                "the MCP surface provides a simulated adapter for local contract testing; external providers must implement the Rust acquisition seam",
                "an observed provenance label is a provider declaration, not authentication, consent, chain of custody, or clinical/release authority",
                "the decision policy remains conditional-independence, model-relative planning rather than causal or predictive truth",
            ],
        }))
    }

    pub(super) fn epistemic_retrieval_synthesis_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_epistemic_retrieval_synthesis_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_FEATURE_ID,
            "contract_version": RETRIEVAL_SYNTHESIS_FEDERATED_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "bounded candidate ranking is deterministic, replay-bound, and source-quorum aware",
                "unknown, unmeasured, contradicted, omitted, and negative evidence remain explicit",
                "raw data stays institution-local and policy, approval, federation, provenance, and locality gates fail closed"
            ],
            "limitations": [
                "the route consumes caller-supplied typed summaries and does not fetch documents or export raw text",
                "a qualified synthesis posture is not biological validity or clinical advice"
            ]
        }))
    }

    pub(super) fn epistemic_experiment_design_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_epistemic_experiment_design_research_workbench_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_epistemic::EXPERIMENT_DESIGN_RESEARCH_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_epistemic::EXPERIMENT_DESIGN_RESEARCH_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "power-aware candidates are ranked deterministically with explicit executable, unresolved, blocked, and missing partitions",
                "factor closure, baseline/replay/provenance, evidence, comparability, policy, locality, budget, omission, uncertainty, and negative-result states remain visible",
                "the A1 workbench emits only a bounded design view and never schedules animals, consumes material, controls instruments, or makes clinical decisions"
            ],
            "limitations": [
                "the route evaluates caller-supplied digest-only design summaries and does not run power simulations or laboratory actions",
                "a qualified executable-design contract is a planning artifact, not a biological conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn oracle_combine(&self, arguments: &Value) -> Result<Value, String> {
        let subject = arguments
            .get("subject")
            .and_then(Value::as_str)
            .ok_or("subject is required")?;
        if subject.trim().is_empty() {
            return Err("subject must not be empty".into());
        }
        let at = arguments
            .get("at")
            .and_then(Value::as_str)
            .ok_or("at is required in YYYY-MM-DDTHH:MM:SSZ form")?;
        let at = UtcTimestamp::parse(at).map_err(|error| error.to_string())?;
        let values = arguments
            .get("judgements")
            .and_then(Value::as_array)
            .ok_or("judgements is required and must be an array")?;
        if values.is_empty() || values.len() > 1_000 {
            return Err("judgements must contain between 1 and 1000 entries".into());
        }
        let judgements: Vec<Judgement> = values
            .iter()
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("invalid oracle judgement: {error}"))
            })
            .collect::<Result<_, _>>()?;
        let minimum = arguments
            .get("minimum_deciding_tier")
            .and_then(Value::as_str)
            .map(|value| {
                serde_json::from_value::<OracleEvidenceTier>(json!(value))
                    .map_err(|error| format!("invalid minimum_deciding_tier: {error}"))
            })
            .transpose()?
            .unwrap_or(OracleEvidenceTier::Judge);
        let policy = MeshPolicy::new(minimum);
        let verdict = policy.combine(subject, &at, judgements);
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/oracle-combine/0.1",
            "subject": verdict.subject,
            "at": verdict.at,
            "status": verdict.status(),
            "underdetermined": verdict.is_underdetermined(),
            "deciding_tier": verdict.deciding_tier(),
            "judge_only": verdict.is_judge_only(),
            "suppressed_override": verdict.has_suppressed_override(),
            "acceptable": verdict.acceptable,
            "basis": verdict.basis,
            "confidence": verdict.confidence,
            "establishes": verdict.establishes(),
            "does_not_establish": verdict.does_not_establish(),
            "contributing": verdict.contributing.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_contributing": verdict.contributing.len().saturating_sub(max_items),
            "withheld": verdict.withheld.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_withheld": verdict.withheld.len().saturating_sub(max_items),
            "inadmissible": verdict.inadmissible.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_inadmissible": verdict.inadmissible.len().saturating_sub(max_items),
            "suppressed": verdict.suppressed.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_suppressed": verdict.suppressed.len().saturating_sub(max_items),
            "disagreements": verdict.disagreements.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_disagreements": verdict.disagreements.len().saturating_sub(max_items),
            "guarantees": [
                "evidence tier, not confidence, determines which judgements may decide",
                "same-tier disagreement remains set-valued and underdetermined rather than becoming a majority vote",
                "expired, superseded, out-of-scope, and weaker judgements remain visible in separate ledgers",
            ],
            "limitations": [
                "judgements are caller-supplied and this tool does not run an oracle, sandbox an evaluator, or authenticate provenance",
                "the mesh combines positions; it does not infer biological truth, adjudicate a disputed result, or manufacture missing evidence",
            ]
        }))
    }

    pub(super) fn policy_screen(&self, arguments: &Value) -> Result<Value, String> {
        let relative = arguments
            .get("world")
            .and_then(Value::as_str)
            .ok_or("world is required (a JSON world document relative to the server root)")?;
        let path = self.resolve(relative)?;
        if path.is_dir() {
            return Err(
                "policy_screen requires a JSON world document, not an indexed store".into(),
            );
        }
        let world = World::from_json(self.read_json(&path)?).map_err(|error| error.to_string())?;
        let request_value = arguments
            .get("request")
            .cloned()
            .ok_or("request is required and must bind principal, purpose, channel and at")?;
        let request: PolicyRequest = serde_json::from_value(request_value)
            .map_err(|error| format!("invalid policy request: {error}"))?;

        let mut lattice = PolicyLattice::new();
        if let Some(rules) = arguments.get("rules") {
            let rules = rules
                .as_array()
                .ok_or("rules must be an array of policy rule objects")?;
            for (index, raw_rule) in rules.iter().enumerate() {
                let rule: PolicyRule = serde_json::from_value(raw_rule.clone())
                    .map_err(|error| format!("invalid policy rule at index {index}: {error}"))?;
                lattice.register(rule).map_err(|error| {
                    format!("cannot register policy rule at index {index}: {error}")
                })?;
            }
        }
        if let Some(tags) = arguments.get("tags") {
            let tags = tags
                .as_object()
                .ok_or("tags must be an object mapping tag names to policy labels")?;
            for (tag, raw_label) in tags {
                let label: PolicyLabel = serde_json::from_value(raw_label.clone())
                    .map_err(|error| format!("invalid policy label for tag {tag:?}: {error}"))?;
                lattice.register_tag(tag.clone(), label);
            }
        }

        let selected: Vec<&bioprism_world::Fact> = match arguments.get("facts") {
            None => world.facts.iter().collect(),
            Some(raw_facts) => {
                let ids = raw_facts
                    .as_array()
                    .ok_or("facts must be an array of fact ids")?;
                let mut selected = Vec::with_capacity(ids.len());
                let mut seen = BTreeSet::new();
                for raw_id in ids {
                    let id = raw_id
                        .as_str()
                        .ok_or("facts must be an array of fact ids")?;
                    if !seen.insert(id) {
                        return Err(format!("facts must not contain duplicate id {id:?}"));
                    }
                    selected.push(
                        world
                            .fact(id)
                            .ok_or_else(|| format!("fact {id:?} is not present in the world"))?,
                    );
                }
                selected
            }
        };
        let selected_ids = selected
            .iter()
            .map(|fact| fact.id.as_str().to_string())
            .collect::<Vec<_>>();
        let unregistered_tags = selected
            .iter()
            .map(|fact| {
                json!({
                    "fact_id": fact.id.as_str(),
                    "tags": lattice.unregistered_tags(fact),
                })
            })
            .filter(|item| item["tags"].as_array().is_some_and(|tags| !tags.is_empty()))
            .collect::<Vec<_>>();

        let screening = lattice.screen(selected.iter().copied(), &request);
        let admitted = screening
            .admitted
            .iter()
            .map(|item| {
                json!({
                    "fact_id": item.fact.id.as_str(),
                    "admission": item.admission,
                })
            })
            .collect::<Vec<_>>();
        let refused = screening
            .refused
            .iter()
            .map(|item| {
                json!({
                    "fact_id": item.fact_id,
                    "refusal": item.refusal,
                    "constraint": item.refusal.constraint(),
                    "escalatable": item.refusal.is_escalatable(),
                })
            })
            .collect::<Vec<_>>();
        let complete = screening.is_complete();
        let refusal_count = screening.refused.len();
        Ok(json!({
            "ok": true,
            "world": relative,
            "world_id": world.world_id,
            "world_sha256": world.content_hash(),
            "policy_version": lattice.version(),
            "request": request,
            "selected_facts": selected_ids,
            "candidate_count": selected.len(),
            "admitted_count": admitted.len(),
            "refused_count": refusal_count,
            "admitted": admitted,
            "refused": refused,
            "complete": complete,
            "derived_label": screening.derived_label(),
            "obligations": screening.obligations(),
            "trace": screening.trace.to_json(),
            "unregistered_tags": unregistered_tags,
            "guarantees": {
                "screened_before_selection": true,
                "unknown_policy_denies_by_default": true,
                "refusals_are_retained": true,
                "obligations_are_declared_not_discharged": true,
            },
        }))
    }
}
