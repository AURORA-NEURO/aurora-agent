//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    /// Execute a typed multimodal computation DAG through the deterministic synthetic worker.
    /// Production containers, GPUs, and schedulers remain caller-owned through the Rust SDK seam.
    pub(super) fn glioma_computation_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request: ComputationExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_computation_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma computation execution request: {error}"))?;
        let mut executor = DryRunGliomaComputationExecutor;
        let execution = execute_glioma_computation(&request, &mut executor)
            .map_err(|error| format!("glioma computation execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "stable topological order, replay-keyed cache, bounded budget, and retries are explicit",
                "typed local artifacts and dependency-blocked work remain visible",
                "the MCP route uses a synthetic worker and performs no external computation or data movement",
                "production containers, GPUs, and schedulers require a caller-owned GliomaComputationExecutor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation execution: {error}"))
    }

    /// Join a typed computation DAG with its local results and expose the smallest
    /// dependency-safe recomputation frontier.
    pub(super) fn glioma_computation_lineage(&self, arguments: &Value) -> Result<Value, String> {
        let request: ComputationLineageRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_computation_lineage requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma computation lineage request: {error}"))?;
        let lineage = join_glioma_computation_lineage(&request)
            .map_err(|error| format!("glioma computation lineage refused: {error}"))?;
        serde_json::to_value(json!({
            "lineage": lineage,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_computation_execute",
                "glioma_computation_placement",
                "glioma_computation_recovery_execute"
            ],
            "guarantees": [
                "task, dependency, result, schema, replay, and local-artifact relationships are explicit",
                "failed, partial, missing, stale, and negative nodes expand to a deterministic recomputation frontier",
                "reusable tasks remain separate from the frontier and can be replayed only under caller policy",
                "the route performs no worker execution, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation lineage: {error}"))
    }

    /// Gate a completed local computation on repeated replay stability. The route consumes only
    /// typed summaries; it never moves matrices, invokes a worker, or promotes a model result.
    pub(super) fn glioma_computation_reproducibility(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationReproducibilityRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_reproducibility requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma computation reproducibility request: {error}")
            })?;
        let runs: Vec<ComputationReproducibilityRun> = serde_json::from_value(
            arguments
                .get("runs")
                .cloned()
                .ok_or_else(|| "glioma_computation_reproducibility requires runs".to_string())?,
        )
        .map_err(|error| format!("invalid glioma computation reproducibility runs: {error}"))?;
        let output = analyze_glioma_computation_reproducibility(&request, &runs)
            .map_err(|error| format!("glioma computation reproducibility refused: {error}"))?;
        serde_json::to_value(json!({
            "reproducibility": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_computation_interpretation_evidence_gate",
                "glioma_robustness_guided_computation",
                "glioma_computation_recovery_execute"
            ],
            "guarantees": [
                "deterministic tasks require byte-identical output digests when the gate is enabled",
                "numerical tasks are gated on explicit effect and runtime drift rather than completion alone",
                "failed, partial, undercovered, drifted, and high-uncertainty replays remain visible",
                "the route consumes local summaries only and performs no external computation, data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation reproducibility: {error}"))
    }

    /// Compile a resource-bounded, dependency-closed multimodal computation portfolio. The
    /// planner only emits an execution-ready order; it never invokes external code or moves raw
    /// data. Institution-local workers can pass the selected tasks to the computation executor.
    pub(super) fn glioma_computation_portfolio_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationPortfolioRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_computation_portfolio_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma computation portfolio request: {error}"))?;
        let candidates: Vec<ComputationCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_computation_portfolio_plan requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid glioma computation portfolio candidates: {error}"))?;
        let plan = plan_glioma_computation_portfolio(&request, &candidates)
            .map_err(|error| format!("glioma computation portfolio refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "prerequisite closure and deterministic task order are explicit",
                "budget, duration, task-count, modality, and deterministic-policy gates are fail-closed",
                "missing dependencies, cycles, deferred work, and negative evidence remain visible",
                "the route does not execute external code, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation portfolio: {error}"))
    }

    /// Place a typed multimodal computation DAG onto institution-local workers without
    /// dispatching it. The schedule accounts for data locality, transfer and compute budgets,
    /// replay-valid cache artifacts, worker windows, critical path, and blocked work; callers
    /// may pass the validated handoff to their own execution adapter.
    pub(super) fn glioma_computation_placement(&self, arguments: &Value) -> Result<Value, String> {
        let request: ComputationPlacementRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_computation_placement requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma computation placement request: {error}"))?;
        let schedule = schedule_glioma_computation_placement(&request)
            .map_err(|error| format!("glioma computation placement refused: {error}"))?;
        serde_json::to_value(json!({
            "schedule": schedule,
            "dispatch": "not_started",
            "preflight_required": true,
            "guarantees": [
                "typed DAG dependencies, completed work, cache reuse, and blocked descendants remain explicit",
                "worker capability, availability, data locality, transfer, compute-budget, and mission-window gates are deterministic",
                "only replay-valid schema-compatible local artifacts are reused",
                "the route never executes code, moves payloads, dispatches a worker, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation placement: {error}"))
    }

    /// Plan and execute a selected computation portfolio through the deterministic sandbox
    /// worker. Institution-local callers can replace the worker through the Rust executor seam.
    pub(super) fn glioma_computation_portfolio_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationPortfolioExecutionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_portfolio_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma computation portfolio execution request: {error}")
            })?;
        let mut executor = DryRunGliomaComputationExecutor;
        let execution = execute_glioma_computation_portfolio(&request, &mut executor)
            .map_err(|error| format!("glioma computation portfolio execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "portfolio selection and execution share one replayable objective and dependency closure",
                "the selected DAG is passed only to the typed local computation executor",
                "partial, failed, blocked, unresolved, deferred, and negative states remain explicit",
                "the MCP route uses a synthetic worker and performs no external computation or data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation portfolio execution: {error}"))
    }

    /// Run a bounded multi-round glioma computation campaign with a deterministic dry-run
    /// planner/worker. Institution-local hosts replace both seams for adaptive analyses.
    pub(super) fn glioma_computation_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaComputationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma computation campaign request: {error}"))?;
        let mut planner = StaticGliomaComputationPlanner;
        let mut executor = DryRunGliomaComputationExecutor;
        let campaign = execute_glioma_computation_campaign(&request, &mut planner, &mut executor)
            .map_err(|error| format!("glioma computation campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "each round replans only from typed local computation outcomes",
                "hard cost, duration, deterministic-task, dependency, retry, cache, and artifact gates remain active",
                "negative, partial, failed, skipped, blocked, and unresolved work is never promoted to completion",
                "the MCP route performs no external computation or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation campaign: {error}"))
    }

    /// Recover failed, partial, or skipped computation tasks without replaying suspect cache
    /// entries. The MCP route uses the deterministic local planner and worker.
    pub(super) fn glioma_computation_recovery_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationRecoveryRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_recovery_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma computation recovery request: {error}"))?;
        let mut planner = StaticGliomaComputationPlanner;
        let mut executor = DryRunGliomaComputationExecutor;
        let campaign = execute_glioma_computation_recovery(&request, &mut planner, &mut executor)
            .map_err(|error| format!("glioma computation recovery refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "failed, partial, and skipped task frontiers are closed over their typed prerequisites before recovery",
                "suspect cache entries are invalidated and recovery receives a fresh replay identity",
                "initial and recovery campaigns remain separately replayable with independent budgets, durations, and outcomes",
                "the MCP route performs no external computation, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation recovery: {error}"))
    }

    /// Reweight a typed computation portfolio from observed robustness debt, then execute the
    /// dependency-closed re-analysis through the deterministic local worker.
    pub(super) fn glioma_robustness_guided_computation_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: RobustnessGuidedComputationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_robustness_guided_computation_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma robustness-guided computation request: {error}")
            })?;
        let mut executor = dry_run_robustness_guided_computation_executor();
        let computation = execute_glioma_robustness_guided_computation(&request, &mut executor)
            .map_err(|error| format!("glioma robustness-guided computation refused: {error}"))?;
        serde_json::to_value(json!({
            "computation": computation,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "leave-out fragility, negative cases, direction reversals, and unresolved omissions become explicit re-analysis priority signals",
                "candidate weights are adjusted without changing task identity, model-system binding, or typed DAG dependencies",
                "the existing portfolio planner enforces prerequisite closure, cost, duration, modality, and deterministic-task gates",
                "stable robustness can hold without dispatch; infeasible coverage remains an explicit no-feasible-plan state",
                "the MCP route emits synthetic local computation artifacts only and never promotes them to biological evidence"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma robustness-guided computation: {error}"))
    }

    /// Compile a high-level glioma computation intent into a closed DAG and run it through the
    /// existing bounded campaign controller. Institution-local callers replace the dry-run
    /// planner/worker while retaining the same compiler and safety gates.
    pub(super) fn glioma_computation_workflow_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaComputationWorkflowRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_workflow_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma computation workflow request: {error}"))?;
        let workflow = compile_glioma_computation_workflow(&request)
            .map_err(|error| format!("glioma computation workflow refused: {error}"))?;
        let campaign_request = workflow
            .campaign_request(&request)
            .map_err(|error| format!("glioma computation workflow campaign refused: {error}"))?;
        let mut planner = StaticGliomaComputationPlanner;
        let mut executor = DryRunGliomaComputationExecutor;
        let campaign =
            execute_glioma_computation_campaign(&campaign_request, &mut planner, &mut executor)
                .map_err(|error| {
                    format!("glioma computation workflow execution refused: {error}")
                })?;
        serde_json::to_value(json!({
            "workflow": workflow,
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "researcher intent expands into a deterministic modality-aware dependency-closed DAG",
                "resource estimates and declared shortfalls remain visible before campaign execution",
                "the compiled DAG reuses P09 cost, duration, deterministic-task, retry, cache, artifact, and negative-result gates",
                "MCP uses a synthetic worker and performs no external computation, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation workflow: {error}"))
    }

    /// Run the complete intent-to-computation operating cycle in the deterministic local
    /// sandbox. Governed-local execution is reserved for an institution-owned worker gateway.
    pub(super) fn glioma_computation_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaComputationOperatingCycleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_operating_cycle requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma computation operating-cycle request: {error}")
            })?;
        if matches!(
            request.execution_mode,
            ComputationExecutionMode::GovernedLocal
        ) {
            return Err(
                "glioma_computation_operating_cycle MCP route is simulation-only; governed_local requires an institution-owned worker"
                    .to_string(),
            );
        }
        let cycle = execute_glioma_computation_operating_cycle_dry_run(&request)
            .map_err(|error| format!("glioma computation operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "typed researcher intent compiles into a dependency-closed multimodal DAG before execution",
                "resource shortfalls hold the cycle before any worker task is dispatched when require_within_resources is true",
                "local execution preserves deterministic replay identity, cache, artifact, retry, negative, and partial-result gates",
                "MCP performs no external computation, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma computation operating cycle: {error}"))
    }

    /// Compile computation campaign outcomes into interpretation, replication, and recovery
    /// actions without promoting any computation result to a biological conclusion.
    pub(super) fn glioma_computation_interpretation_frontier_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationInterpretationFrontierRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_interpretation_frontier_compile requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma computation interpretation frontier request: {error}")
            })?;
        let frontier =
            compile_glioma_computation_interpretation_frontier(&request).map_err(|error| {
                format!("glioma computation interpretation frontier refused: {error}")
            })?;
        serde_json::to_value(json!({
            "frontier": frontier,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "completed and cached computations become typed interpretation candidates",
                "negative computations become replication or falsification candidates",
                "partial, failed, and skipped computations become bounded recovery candidates",
                "replay, baseline, uncertainty, and negative-result evaluation obligations remain explicit",
                "the route performs no raw-data movement, biological conclusion, or clinical decision"
            ]
        }))
        .map_err(|error| {
            format!("cannot encode glioma computation interpretation frontier: {error}")
        })
    }

    /// Execute the computation-derived frontier through the same bounded autonomous mission
    /// controller used by the rest of the research engine. The MCP executor is local-only.
    pub(super) fn glioma_computation_interpretation_frontier_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationInterpretationFrontierRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_interpretation_frontier_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma computation interpretation frontier request: {error}")
            })?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_computation_interpretation_frontier(&request, &mut executor)
            .map_err(|error| {
                format!("glioma computation interpretation frontier refused: {error}")
            })?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "frontier actions remain behind the bounded autonomous mission controller",
                "all effects are local deterministic computation and artifact writes",
                "blocked, negative, failed, partial, and budget-limited outcomes remain typed",
                "interpretation promotion still requires replay, baseline, uncertainty, and negative-result review",
                "the route performs no instrument execution, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| {
            format!("cannot encode glioma computation interpretation frontier run: {error}")
        })
    }

    /// Bind returned computation summaries to their routed actions and run the P10
    /// cross-family interpretation gate in the deterministic local sandbox.
    pub(super) fn glioma_computation_interpretation_evidence_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ComputationInterpretationEvidenceGateRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_computation_interpretation_evidence_gate requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma computation interpretation evidence request: {error}")
            })?;
        let gate =
            execute_glioma_computation_interpretation_evidence_gate(&request).map_err(|error| {
                format!("glioma computation interpretation evidence refused: {error}")
            })?;
        serde_json::to_value(json!({
            "gate": gate,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "only observations for completed or negative frontier actions can enter synthesis",
                "computation evidence is separated from replication evidence by routed stage",
                "missing observations, negative outcomes, replay drift, and uncertainty remain explicit",
                "cross-family interpretation still requires quality, independent-group, and replication floors",
                "the route performs no raw-data movement, instrument effect, or clinical decision"
            ]
        }))
        .map_err(|error| {
            format!("cannot encode glioma computation interpretation evidence gate: {error}")
        })
    }

    /// Synthesize independent preclinical interpretation families into a bounded research
    /// conclusion. The gate exposes disagreement, negative evidence, missing replication, and
    /// leave-one-family-out instability instead of silently collapsing them into confidence.
    pub(super) fn glioma_interpretation_synthesize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InterpretationSynthesisRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_interpretation_synthesize requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma interpretation synthesis request: {error}"))?;
        let synthesis = synthesize_glioma_interpretation(&request)
            .map_err(|error| format!("glioma interpretation synthesis refused: {error}"))?;
        serde_json::to_value(json!({
            "synthesis": synthesis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "cross-family effects are aggregated only from typed local preclinical summaries",
                "contradictory, missing, unresolved, low-quality, and negative evidence remains visible",
                "leave-one-family-out stability and declared replication/family floors gate qualification",
                "MCP executes no assays, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma interpretation synthesis: {error}"))
    }

    /// Compile the interpretation gate and adaptive next-action frontier as one bounded
    /// researcher handoff. It plans only; selected actions remain behind their typed executors.
    pub(super) fn glioma_interpretation_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaInterpretationOperatingCycleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_interpretation_operating_cycle requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma interpretation operating-cycle request: {error}")
            })?;
        let cycle = execute_glioma_interpretation_operating_cycle(&request)
            .map_err(|error| format!("glioma interpretation operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "cross-family interpretation and leave-one-family-out stability gates run before frontier selection",
                "contradiction, missing replication, negative, and unresolved evidence remain explicit",
                "frontier actions are typed bounded handoffs and require their own institution-local executor",
                "MCP executes no assays, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma interpretation operating cycle: {error}"))
    }
}
