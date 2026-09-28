//! Runtime effect, tape, workflow, and execution assurance handlers.

use super::*;

impl Server {
    pub(super) fn runtime_effect_check(&self, arguments: &Value) -> Result<Value, String> {
        let policy: EffectPolicy = serde_json::from_value(
            arguments
                .get("policy")
                .cloned()
                .ok_or("policy is required and must be a serialized runtime EffectPolicy")?,
        )
        .map_err(|error| format!("invalid effect policy: {error}"))?;
        let request: EffectRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or("request is required and must be a serialized runtime EffectRequest")?,
        )
        .map_err(|error| format!("invalid effect request: {error}"))?;
        let class = request.class();
        let authorization = match policy.authorize(&request) {
            Ok(authorization) => authorization,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "authorization",
                    "request": request,
                    "kind": request.kind(),
                    "class": class,
                    "class_label": class.to_string(),
                    "target_host": request.target_host(),
                    "target_path": request.target_path(),
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "undeclared, disallowed, non-canonical, and network-denied effects never become executable through this inspection tool"
                }));
            }
        };
        let simulated = matches!(authorization, bioprism_runtime::Authorization::Simulate)
            .then(|| EffectPolicy::simulated_outcome(&request));
        Ok(json!({
            "ok": true,
            "request": request,
            "kind": request.kind(),
            "class": class,
            "class_label": class.to_string(),
            "authorization": authorization,
            "target_host": request.target_host(),
            "target_path": request.target_path(),
            "simulated_outcome": simulated,
            "guarantees": [
                "authorization is deny-by-default and checks declaration before path or network details",
                "simulation is explicitly labelled and derived from the request",
                "this tool authorizes or simulates a description only; it performs no effect"
            ],
            "limitations": [
                "a successful authorization is not evidence that an external provider is available",
                "filesystem and network state are not touched by this MCP surface"
            ]
        }))
    }

    pub(super) fn runtime_tape_verify(&self, arguments: &Value) -> Result<Value, String> {
        let tape: WorldTape = serde_json::from_value(
            arguments
                .get("tape")
                .cloned()
                .ok_or("tape is required and must be a serialized runtime WorldTape")?,
        )
        .map_err(|error| format!("invalid or unverifiable world tape: {error}"))?;
        if tape.entries().len() > 100_000 {
            return Err("world tape exceeds the 100000-entry safety bound".into());
        }
        if tape.checkpoints().len() > 10_000 {
            return Err("world tape exceeds the 10000-checkpoint safety bound".into());
        }
        let other = match arguments.get("other_tape") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                serde_json::from_value::<WorldTape>(value.clone())
                    .map_err(|error| format!("invalid comparison world tape: {error}"))?,
            ),
        };
        let checkpoint_results: Vec<Value> = tape
            .checkpoints()
            .iter()
            .map(|checkpoint| match tape.verify_checkpoint(checkpoint) {
                Ok(()) => json!({
                    "id": checkpoint.id,
                    "step": checkpoint.step,
                    "tape_head": checkpoint.tape_head,
                    "provider": checkpoint.provider,
                    "restoration": checkpoint.restoration,
                    "ok": true
                }),
                Err(error) => json!({
                    "id": checkpoint.id,
                    "step": checkpoint.step,
                    "tape_head": checkpoint.tape_head,
                    "provider": checkpoint.provider,
                    "restoration": checkpoint.restoration,
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true
                }),
            })
            .collect();
        let divergence = other
            .as_ref()
            .and_then(|candidate| tape.first_divergence(candidate));
        let artifacts = tape.artifacts();
        let checkpoint_pass_count = checkpoint_results
            .iter()
            .filter(|checkpoint| checkpoint["ok"] == json!(true))
            .count();
        let checkpoint_failure_count = checkpoint_results
            .len()
            .saturating_sub(checkpoint_pass_count);
        let simulated_steps = tape.simulated_steps();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/runtime-tape-verify/0.1",
            "run": tape.run(),
            "lineage": tape.lineage(),
            "entries": tape.len(),
            "head": tape.head(),
            "chain_verified": true,
            "checkpoint_results": checkpoint_results,
            "checkpoint_count": tape.checkpoints().len(),
            "checkpoint_pass_count": checkpoint_pass_count,
            "checkpoint_failure_count": checkpoint_failure_count,
            "artifacts": artifacts,
            "artifact_consumed_count": artifacts.consumed.len(),
            "artifact_created_count": artifacts.created.len(),
            "simulated_steps": simulated_steps,
            "simulated_step_count": simulated_steps.len(),
            "first_divergence": divergence,
            "comparison_supplied": other.is_some(),
            "guarantees": [
                "deserialization verifies the hash chain before a tape is accepted",
                "checkpoint identity, artifact reads/writes, simulated provenance, and suffix divergence remain separate",
                "a comparison reports the earliest digest disagreement rather than an aggregate similarity score"
            ],
            "limitations": [
                "the tool verifies a supplied tape but does not replay it or contact a provider",
                "artifact digests identify recorded content and do not prove external filesystem state"
            ]
        }))
    }

    pub(super) fn runtime_workflow_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized WorkflowExecutionRequest")?;
        let receipt = crate::research_contracts::execute_workflow_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_runtime::WORKFLOW_EXECUTION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "workflow validation, deterministic topological ordering, policy, authority, evidence, and budget preflight happen before effects",
                "dry-run uses the same ordering and gates without appending tape entries",
                "executed nodes are recorded in one replay-bound ExecutionRun with explicit checkpoints"
            ],
            "limitations": [
                "the MCP surface accepts declarative actions and never invokes an external host or instrument",
                "a successful receipt proves admission and replayable recording, not biological validity"
            ]
        }))
    }

    pub(super) fn runtime_interpretation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized EvidenceBackedResult4")?;
        let receipt = crate::research_contracts::runtime_interpretation_assurance_json(request)?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_runtime::INTERPRETATION_ASSURANCE_FEATURE_ID,"receipt":receipt,"guarantees":["candidate partition and ordering are deterministic","comparability, replay, provenance, omissions, uncertainty, negative evidence, policy, locality, protected closure, signed approval, federation, and adversarial gates remain explicit","release effect is always block:unsafe-release"],"limitations":["typed declarations are evaluated without rendering, raw-data access, biological inference, or clinical decisions","aggregate-only output requires independent scientific review"]}),
        )
    }

    pub(super) fn runtime_knowledge_representation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_runtime_knowledge_representation_assurance_json(
                arguments,
            )?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_runtime::RUNTIME_KNOWLEDGE_REPRESENTATION_FEATURE_ID,"contract_version":bioprism_runtime::RUNTIME_KNOWLEDGE_REPRESENTATION_CONTRACT_VERSION,"receipt":receipt,"guarantees":["federated continual typed claims and aggregate-only peer summaries are deterministically partitioned","missing, unknown, unmeasured, contradicted, omitted, negative, replay, provenance, policy, protected-closure, locality, budget, and adversarial states remain visible","the route always emits block:unsafe-release and never exports raw data, executes tools, or makes clinical decisions"],"limitations":["the harness verifies caller-supplied claim and peer attestations without fetching sources or opening federation connections","a blocked receipt is a safety and conformance artifact, not a scientific conclusion"]}),
        )
    }

    pub(super) fn fabric_experiment_design_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_fabric_experiment_design_interoperability_gateway_json(
                arguments,
            )?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_fabric::EXPERIMENT_DESIGN_GATEWAY_FEATURE_ID,"contract_version":bioprism_fabric::EXPERIMENT_DESIGN_GATEWAY_CONTRACT_VERSION,"receipt":receipt,"guarantees":["versioned experiment-objective and capability manifests are negotiated deterministically with explicit migration-loss receipts","missing modalities/controls, semantic or instrument-profile conflicts, unknown or contradicted evidence, policy, locality, replay, provenance, and approval gaps remain visible","qualified effects are limited to contract negotiation; no protocol, instrument, or raw-data effect is dispatched"],"limitations":["the gateway evaluates caller-supplied capability manifests and never contacts instruments or workflow services","an executable-design artifact is a bounded interoperability contract, not scientific validity or a clinical decision"]}),
        )
    }

    pub(super) fn stress_publication_research_object_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_stress_publication_research_object_workbench_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_stress::PUBLICATION_RESEARCH_OBJECT_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_stress::PUBLICATION_RESEARCH_OBJECT_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "validated digest-only research runs are deterministically partitioned into qualified, conditional, blocked, or unknown release states",
                "replay, provenance, standards, protected closure, policy, locality, omissions, uncertainty, and negative results remain explicit",
                "the A1 workbench is read-only and never signs, publishes, uploads, dereferences, or makes clinical decisions"
            ],
            "limitations": [
                "the workbench compiles caller-supplied attestations and does not cryptographically sign or transport payloads",
                "a qualified research-object view is not scientific validity, diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn runtime_execution_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let run = bioprism_ids::RunId::parse(
            arguments
                .get("run")
                .and_then(Value::as_str)
                .unwrap_or("mcp-runtime-run"),
        )
        .map_err(|error| format!("invalid run id: {error}"))?;
        let policy: EffectPolicy = serde_json::from_value(
            arguments
                .get("policy")
                .cloned()
                .ok_or("policy is required and must be a serialized runtime EffectPolicy")?,
        )
        .map_err(|error| format!("invalid effect policy: {error}"))?;
        let raw_requests = arguments
            .get("requests")
            .cloned()
            .ok_or("requests is required and must be an array of EffectRequest values")?;
        let request_values = raw_requests
            .as_array()
            .ok_or("requests must be an array of EffectRequest values")?;
        if request_values.len() > 1_000 {
            return Err("requests exceeds the 1000-effect safety bound".into());
        }
        let request_bytes = serde_json::to_vec(&raw_requests)
            .map_err(|error| format!("cannot measure request envelope: {error}"))?;
        if request_bytes.len() > 2_000_000 {
            return Err("requests exceed the 2000000-byte safety bound".into());
        }
        let requests: Vec<EffectRequest> = request_values
            .iter()
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("invalid effect request: {error}"))
            })
            .collect::<Result<_, _>>()?;
        let budget_plan: Option<BudgetPlan> = arguments
            .get("budget")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid runtime BudgetPlan: {error}"))?;
        let world =
            runtime_world_from_config(arguments.get("world").unwrap_or(&Value::Null), "world")?;
        let mut recording = RecordingHost::new(run.clone(), world, policy.clone());
        if let Some(plan) = &budget_plan {
            recording = recording.with_budget(BudgetController::from_plan(plan));
        }

        let mut live_outcomes = Vec::new();
        let mut execution_error = None;
        for request in &requests {
            match recording.perform(request.clone()) {
                Ok(outcome) => live_outcomes.push(outcome.into_value()),
                Err(error) => {
                    execution_error = Some(error.to_string());
                    break;
                }
            }
        }
        let (tape, world, journal, budget) = recording.into_parts();
        let recorded_requests = tape.entries().len();
        let mut replay_outcomes = Vec::new();
        let mut replay_error = None;
        let mut replay = ReplayHost::new(tape.clone());
        for request in requests.iter().take(recorded_requests) {
            match replay.perform(request.clone()) {
                Ok(outcome) => replay_outcomes.push(outcome.into_value()),
                Err(error) => {
                    replay_error = Some(error.to_string());
                    break;
                }
            }
        }
        let replay_finish = replay.finish();
        if let Err(error) = &replay_finish {
            replay_error = Some(error.to_string());
        }
        let replay_match = replay_error.is_none() && live_outcomes == replay_outcomes;
        let budget_view = budget.as_ref().map(|controller| {
            json!({
                "accounting": controller.accounting(),
                "warnings": controller.warnings(),
                "aborted_on": controller.aborted_on(),
                "fully_consumed_effects": controller.used(RuntimeResource::ToolCalls)
            })
        });

        let fork = match arguments.get("fork") {
            None | Some(Value::Null) => Value::Null,
            Some(raw_fork) => {
                let fork = raw_fork.as_object().ok_or("fork must be an object")?;
                let step = fork
                    .get("step")
                    .and_then(Value::as_u64)
                    .ok_or("fork.step is required and must be a non-negative integer")?;
                let child_run = bioprism_ids::RunId::parse(
                    fork.get("run")
                        .and_then(Value::as_str)
                        .unwrap_or("mcp-runtime-child"),
                )
                .map_err(|error| format!("invalid fork run id: {error}"))?;
                let raw_suffix = fork.get("requests").cloned().unwrap_or_else(|| json!([]));
                let suffix_values = raw_suffix
                    .as_array()
                    .ok_or("fork.requests must be an array")?;
                if suffix_values.len() > 1_000 {
                    return Err("fork.requests exceeds the 1000-effect safety bound".into());
                }
                let suffix: Vec<EffectRequest> = suffix_values
                    .iter()
                    .cloned()
                    .map(|value| {
                        serde_json::from_value(value)
                            .map_err(|error| format!("invalid fork effect request: {error}"))
                    })
                    .collect::<Result<_, _>>()?;
                if step > tape.len() {
                    json!({
                        "ok": false,
                        "stage": "fork",
                        "step": step,
                        "refusal": format!("fork step {step} is beyond tape length {}", tape.len()),
                        "fail_closed": true
                    })
                } else {
                    let observed = observable_state(&tape, step).map_err(|error| {
                        format!("cannot materialize observed fork state: {error}")
                    })?;
                    let fork_world = runtime_world_from_config(
                        fork.get("world").unwrap_or(&Value::Null),
                        "fork.world",
                    )?;
                    let mut suffix_host =
                        open_suffix(&tape, step, child_run, fork_world, policy.clone())
                            .map_err(|error| format!("cannot open fork suffix: {error}"))?;
                    suffix_host.resume_at_fork();
                    let mut suffix_outcomes = Vec::new();
                    let mut suffix_error = None;
                    for request in &suffix {
                        match suffix_host.perform(request.clone()) {
                            Ok(outcome) => suffix_outcomes.push(outcome.into_value()),
                            Err(error) => {
                                suffix_error = Some(error.to_string());
                                break;
                            }
                        }
                    }
                    let child_tape = suffix_host
                        .finish()
                        .map_err(|error| format!("cannot finish fork suffix: {error}"))?;
                    let comparison = compare_suffixes(&tape, &child_tape);
                    json!({
                        "ok": suffix_error.is_none(),
                        "step": step,
                        "inherited_steps": child_tape.inherited_steps(),
                        "observed_state": observed,
                        "suffix_outcomes": suffix_outcomes,
                        "suffix_error": suffix_error,
                        "child_tape": child_tape,
                        "comparison": comparison,
                        "guarantee": "the inherited prefix is read from the parent tape and is never re-performed by the fork host"
                    })
                }
            }
        };

        let replay_complete = replay_error.is_none() && replay_outcomes.len() == recorded_requests;
        let recording_complete = execution_error.is_none() && recorded_requests == requests.len();
        let fork_requested = !fork.is_null();
        let live_outcome_count = live_outcomes.len();
        let policy_journal_count = journal.len();
        let replay_outcome_count = replay_outcomes.len();

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/runtime-execution-simulate/0.1",
            "run": run,
            "request_count": requests.len(),
            "recorded_requests": recorded_requests,
            "recording_complete": recording_complete,
            "partial_recording": !recording_complete,
            "live_outcomes": live_outcomes,
            "live_outcome_count": live_outcome_count,
            "execution_error": execution_error,
            "tape": tape,
            "world": {
                "calls": world.calls(),
                "task_millis": world.task_millis(),
                "state_manifest": world.state_manifest(),
                "file_changes": world.journal()
            },
            "policy_journal": journal,
            "policy_journal_count": policy_journal_count,
            "budget": budget_view,
            "replay": {
                "verified": replay_error.is_none(),
                "matched": replay_match,
                "outcomes": replay_outcomes,
                "outcome_count": replay_outcome_count,
                "complete": replay_complete,
                "error": replay_error
            },
            "replay_outcome_count": replay_outcome_count,
            "replay_complete": replay_complete,
            "fork": fork,
            "fork_requested": fork_requested,
            "guarantees": [
                "the supplied effect program runs only against the deterministic in-process world; no host filesystem, network, process, model, message, or payment endpoint is touched",
                "every authorized effect is hash-chained with request, outcome, and performed-versus-simulated provenance",
                "replay checks request identity and consumes the complete recorded prefix, so changed or truncated programs fail closed",
                "budgets charge before an effect and report soft warnings separately from hard exhaustion",
                "forks expose an observable state and inherit the prefix without re-performing it"
            ],
            "limitations": [
                "the in-process world is not a container, subprocess, durable queue, or real provider",
                "a fork starts its suffix source from the caller-supplied fork.world configuration; restoration of external state is not implied",
                "this endpoint audits a bounded request trace and does not infer agent intent or biological validity"
            ]
        }))
    }
}
