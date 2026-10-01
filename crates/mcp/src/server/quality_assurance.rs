//! Manufacturing, stewardship, and reusable quality gate handlers.

use super::*;

impl Server {
    pub(super) fn megafactory_twin_audit(&self, arguments: &Value) -> Result<Value, String> {
        let reference = parse_mechanistic_model(
            arguments
                .get("reference")
                .ok_or("reference is required and must be a MechanisticModel")?,
            "reference",
        )?;
        let raw_alternatives = arguments
            .get("alternatives")
            .ok_or("alternatives is required and must be an array of MechanisticModel values")?;
        let values = raw_alternatives
            .as_array()
            .ok_or("alternatives must be an array")?;
        if values.is_empty() || values.len() > 64 {
            return Err("alternatives must contain between 1 and 64 models".into());
        }
        let alternatives = values
            .iter()
            .enumerate()
            .map(|(index, value)| parse_mechanistic_model(value, &format!("alternatives[{index}]")))
            .collect::<Result<Vec<_>, _>>()?;
        if reference.compartments.len() > 64 {
            return Err("models may contain at most 64 compartments".into());
        }

        let initial_value = arguments
            .get("initial")
            .ok_or("initial is required and must be an array of finite numbers")?;
        let initial = initial_value
            .as_array()
            .ok_or("initial must be an array of finite numbers")?
            .iter()
            .map(|value| {
                let number = value
                    .as_f64()
                    .ok_or("initial must contain only finite numbers")?;
                if !number.is_finite() {
                    return Err("initial must contain only finite numbers".to_string());
                }
                Ok(number)
            })
            .collect::<Result<Vec<_>, String>>()?;
        if initial.len() > 64 {
            return Err("initial may contain at most 64 state values".into());
        }
        let steps = arguments
            .get("steps")
            .and_then(Value::as_u64)
            .ok_or("steps is required and must be a non-negative integer")?;
        if steps > 10_000 {
            return Err("steps exceeds the 10000-step safety bound".into());
        }
        let intervention: bioprism_megafactory::Intervention = serde_json::from_value(
            arguments
                .get("intervention")
                .cloned()
                .ok_or("intervention is required and must be an Intervention")?,
        )
        .map_err(|error| format!("invalid intervention: {error}"))?;
        let outcome_compartment = arguments
            .get("outcome_compartment")
            .and_then(Value::as_str)
            .ok_or("outcome_compartment is required and must be a string")?;

        let baseline = match reference.run(&initial, steps as usize, None) {
            Ok(value) => value,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "reference_simulation",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "a malformed state space or initial state never becomes a simulated oracle"
                }));
            }
        };
        let intervened = match reference.run(&initial, steps as usize, Some(&intervention)) {
            Ok(value) => value,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "intervention_simulation",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "an invalid intervention never produces a partial counterfactual"
                }));
            }
        };
        let probe = match DiscrepancyProbe::run(
            &reference,
            &alternatives,
            &initial,
            steps as usize,
            &intervention,
            outcome_compartment,
        ) {
            Ok(value) => value,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "discrepancy_probe",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "a counterfactual is not usable until every plausible model shares the state space and the probe runs"
                }));
            }
        };

        Ok(json!({
            "ok": true,
            "reference": reference,
            "alternatives": alternatives,
            "initial": initial,
            "steps": steps,
            "intervention": intervention,
            "outcome_compartment": outcome_compartment,
            "reference_baseline": baseline,
            "reference_intervened": intervened,
            "probe": probe,
            "oracle_eligible": probe.is_usable_as_an_oracle(),
            "headline": probe.headline(),
            "guarantees": [
                "every model is reconstructed through MechanisticModel::new so shape and misspecification validation cannot be bypassed by JSON",
                "the reference effect remains explicitly under-model and carries its known misspecification",
                "sign stability is required for oracle eligibility; magnitude range and disagreement models remain visible",
                "the simulation is deterministic arithmetic over caller-supplied rates and does not claim biological truth"
            ],
            "limitations": [
                "no stochastic calibration, observed-data fit, posterior, or real assay forward model is performed",
                "the optional model set is caller-supplied and is not a guarantee that all plausible mechanisms were enumerated",
                "this tool qualifies a simulated oracle; it does not publish a benchmark or execute a distributed factory"
            ]
        }))
    }

    pub(super) fn megafactory_placement_audit(&self, arguments: &Value) -> Result<Value, String> {
        let job: FactoryJob = serde_json::from_value(
            arguments
                .get("job")
                .cloned()
                .ok_or("job is required and must be a serialized factory Job")?,
        )
        .map_err(|error| format!("invalid job: {error}"))?;
        let request: WorkRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or("request is required and must be a serialized WorkRequest")?,
        )
        .map_err(|error| format!("invalid placement request: {error}"))?;
        let worker: WorkerProfile = serde_json::from_value(
            arguments
                .get("worker")
                .cloned()
                .ok_or("worker is required and must be a serialized WorkerProfile")?,
        )
        .map_err(|error| format!("invalid worker profile: {error}"))?;
        let commit_count = arguments
            .get("commit_count")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        if commit_count == 0 || commit_count > 100 {
            return Err("commit_count must be between 1 and 100".into());
        }
        let item = arguments
            .get("item")
            .and_then(Value::as_str)
            .unwrap_or(&job.id);
        if item.is_empty() || item.chars().any(char::is_control) {
            return Err("item must be non-empty and contain no control characters".into());
        }
        let placement = match megafactory_place(&job, &request, &worker) {
            Ok(value) => value,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "placement",
                    "job": job,
                    "request": request,
                    "worker": worker,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "capability, attestation, oracle independence, and enclave transfer checks run before any commit ledger is touched"
                }));
            }
        };

        let supersede = arguments
            .get("supersede_fence")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut registry = FenceRegistry::new();
        let first = registry.issue(&job.id);
        let current = if supersede {
            registry.issue(&job.id)
        } else {
            first
        };
        let stale_admission = supersede.then(|| registry.admit(&job.id, first));
        let current_admission = registry.admit(&job.id, current);
        let mut ledger = ExecutionLedger::new();
        let mut commit_errors = Vec::new();
        for _ in 0..commit_count {
            if let Err(error) = ledger.commit(&registry, &job.id, item, current, job.idempotency) {
                commit_errors.push(error.to_string());
                break;
            }
        }
        let duplicates = ledger.duplicates();
        Ok(json!({
            "ok": true,
            "job": job,
            "request": request,
            "worker": worker,
            "placement": placement,
            "fencing": {
                "first": first.get(),
                "current": current.get(),
                "superseded": supersede,
                "stale_admission": stale_admission.map(|result| match result {
                    Ok(()) => json!({ "ok": true }),
                    Err(error) => json!({ "ok": false, "refusal": error.to_string(), "fail_closed": true }),
                }),
                "current_admission": match current_admission {
                    Ok(()) => json!({ "ok": true }),
                    Err(error) => json!({ "ok": false, "refusal": error.to_string(), "fail_closed": true }),
                }
            },
            "ledger": {
                "commits": ledger.commits(),
                "executed_items": ledger.executed_items(),
                "commit_errors": commit_errors,
                "duplicates": duplicates,
                "has_incidents": duplicates.has_incidents()
            },
            "guarantees": [
                "placement is a predicate over declared capability, data locality, access tier, attestation, and oracle independence",
                "non-local transfer is reported as bytes rather than hidden inside an accepted placement",
                "fences are monotone and stale writes are refused before they reach the execution ledger",
                "idempotent waste, compensable repeats, and non-idempotent incidents remain separate"
            ],
            "limitations": [
                "the workflow is in-memory and does not schedule workers, move data, or provide a durable compare-and-set",
                "commit_count is a deterministic duplicate simulation, not evidence that an external side effect happened",
                "throughput, fairness, backpressure, and real attestation are outside this local predicate surface"
            ]
        }))
    }

    pub(super) fn stewardship_review_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_review = arguments
            .get("review")
            .cloned()
            .ok_or("review is required and must be a serialized ReviewRecord")?;
        let encoded = serde_json::to_vec(&raw_review)
            .map_err(|error| format!("cannot measure review envelope: {error}"))?;
        if encoded.len() > 5_000_000 {
            return Err("review exceeds the 5000000-byte safety bound".into());
        }
        let review: ReviewRecord = serde_json::from_value(raw_review)
            .map_err(|error| format!("invalid stewardship review: {error}"))?;
        let nondeterministic = review.revision.nondeterministic;
        let mandatory =
            bioprism_stewardship::review::ReviewDimension::mandatory_for(nondeterministic);
        let evaluator = review.revision.evaluator.clone();
        match review.conclude() {
            Ok(approval) => {
                let covered = approval
                    .covered()
                    .into_iter()
                    .map(|dimension| dimension.as_str())
                    .collect::<Vec<_>>();
                let unreviewed = approval
                    .unreviewed()
                    .into_iter()
                    .map(|(dimension, why)| json!({ "dimension": dimension.as_str(), "why": why }))
                    .collect::<Vec<_>>();
                let approval_json = serde_json::to_value(&approval)
                    .map_err(|error| format!("cannot serialize scoped approval: {error}"))?;
                Ok(json!({
                    "ok": true,
                    "decision": "issued",
                    "evaluator": evaluator,
                    "nondeterministic": nondeterministic,
                    "mandatory_dimensions": mandatory.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
                    "covered_dimensions": covered,
                    "unreviewed_dimensions": unreviewed,
                    "approval": approval_json,
                    "guarantees": [
                        "approval is dimension-scoped rather than a universal evaluator endorsement",
                        "self-review, non-independent review, empty corpus, failed dimensions, and unsupported passes block issuance",
                        "unreviewed calibration dimensions remain visible in the issued approval",
                        "the transport does not authenticate actor identities or create an expiry clock",
                    ],
                }))
            }
            Err(error) => Ok(json!({
                "ok": false,
                "decision": "refused",
                "evaluator": evaluator,
                "nondeterministic": nondeterministic,
                "mandatory_dimensions": mandatory.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
                "refusal": error.to_string(),
                "fail_closed": true,
                "guarantee": "a review never becomes an approval when any mandatory governance condition is absent"
            })),
        }
    }

    pub(super) fn quality_gate_run(&self, arguments: &Value) -> Result<Value, String> {
        let raw_dataset = arguments
            .get("dataset")
            .cloned()
            .ok_or("dataset is required and must be a serialized Dataset")?;
        let raw_gate = arguments
            .get("gate")
            .cloned()
            .ok_or("gate is required and must be a serialized Gate")?;
        let dataset: QualityDataset = serde_json::from_value(raw_dataset)
            .map_err(|error| format!("invalid quality dataset: {error}"))?;
        let gate: QualityGate = serde_json::from_value(raw_gate)
            .map_err(|error| format!("invalid quality gate: {error}"))?;
        if dataset.rows() > 100_000 {
            return Err("dataset may contain at most 100000 rows".into());
        }
        if dataset.column_names().len() > 1_000 {
            return Err("dataset may contain at most 1000 columns".into());
        }
        if gate.is_empty() {
            return Err("gate must contain at least one named check".into());
        }
        if gate.len() > 1_000 {
            return Err("gate may contain at most 1000 checks".into());
        }
        let references: ReferenceSets = arguments
            .get("references")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid quality reference sets: {error}"))?
            .unwrap_or_else(ReferenceSets::new);
        let report = gate.run(&dataset, &references);
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/quality-gate/0.1",
            "verdict": report.verdict.name(),
            "passed": report.verdict.is_passed(),
            "dataset": report.dataset,
            "rows": report.rows,
            "check_count": report.outcomes.len(),
            "report": report,
            "guarantees": [
                "pass requires every named check to run and hold",
                "failed checks carry a concrete row and expected value witness",
                "missing columns, null-only columns, wrong types, and missing references remain not_runnable",
                "the gate never repairs data, infers a schema, samples rows, or converts indeterminate into pass",
            ],
        }))
    }
}
