//! MCP Glioma research program and experiment-planning handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    /// Rehearse a complete glioma research program locally. The dry-run intentionally executes
    /// no provider, instrument, network, or federation effect; it is a bounded way to inspect the
    /// compiled stage graph and replay/checkpoint behavior before wiring real local executors.
    pub(super) fn glioma_research_dry_run(&self, arguments: &Value) -> Result<Value, String> {
        let intent_value = arguments
            .get("intent")
            .cloned()
            .ok_or_else(|| "glioma_research_dry_run requires intent".to_string())?;
        let intent: GliomaResearchIntent = serde_json::from_value(intent_value)
            .map_err(|error| format!("invalid glioma research intent: {error}"))?;
        let receipt = dry_run_glioma_research(&intent)
            .map_err(|error| format!("glioma dry-run refused: {error}"))?;
        serde_json::to_value(receipt)
            .map_err(|error| format!("cannot encode glioma execution receipt: {error}"))
    }

    /// Plan the next adaptive, checkpoint-aware glioma workflow batch. This is a pure planning
    /// surface: it never dispatches a provider, moves raw data, calls an instrument, or asserts a
    /// biological result. A local host can submit the returned `next_ready_batch` to its own
    /// caller-owned executor after applying its institution's approvals.
    pub(super) fn glioma_workflow_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: GliomaWorkflowRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_workflow_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma workflow request: {error}"))?;
        let plan = plan_glioma_workflow(&request)
            .map_err(|error| format!("glioma workflow planning refused: {error}"))?;
        let next_ready_batch = plan.next_ready_batch();
        serde_json::to_value(json!({
            "plan": plan,
            "next_ready_batch": next_ready_batch,
            "dispatch": "not_started",
            "guarantees": [
                "mode scope closes over upstream evidence, QC, and policy dependencies",
                "unknown, contradictory, underpowered, and non-local inputs hold or abstain",
                "the returned plan is deterministic and digest-bound",
                "physical execution remains behind a caller-owned local executor and approval gate"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma workflow plan: {error}"))
    }

    /// Simulate a caller-declared glioma protocol with deterministic resource-constrained list
    /// scheduling. This computes timing, parallelism, critical path, utilization, and explicit
    /// capacity/risk/approval stops; it never dispatches a provider or claims an assay result.
    pub(super) fn glioma_protocol_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let request: ProtocolSimulationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_simulate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol simulation request: {error}"))?;
        let output = simulate_glioma_protocol(&request)
            .map_err(|error| format!("glioma protocol simulation refused: {error}"))?;
        serde_json::to_value(json!({
            "simulation": output,
            "dispatch": "not_started",
            "guarantees": [
                "critical-path-first scheduling is deterministic from typed tasks and resources",
                "capacity, horizon, risk, and instrument approvals fail closed",
                "unscheduled work remains explicit and is never treated as completed",
                "the output is a scheduling model, not biological evidence"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol simulation: {error}"))
    }

    /// Stress a typed glioma protocol across bounded local resource and approval scenarios. This
    /// is a simulation-only route: failed scenarios hold autonomous dispatch and never become
    /// biological evidence or an instrument command.
    pub(super) fn glioma_protocol_scenario_ensemble(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProtocolScenarioEnsembleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_scenario_ensemble requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol scenario ensemble request: {error}"))?;
        let output = simulate_glioma_protocol_scenario_ensemble(&request)
            .map_err(|error| format!("glioma protocol scenario ensemble refused: {error}"))?;
        serde_json::to_value(json!({
            "ensemble": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_protocol_branch_optimize", "glioma_protocol_autonomous_execute"],
            "guarantees": [
                "each declared scenario is simulated with deterministic timing, capacity, risk, and approval perturbations",
                "weighted and worst-case schedule coverage remain distinct from biological efficacy",
                "failed scenarios and bottleneck tasks remain explicit negative evidence",
                "the route performs no provider, instrument, specimen, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol scenario ensemble: {error}"))
    }

    /// Select the best typed protocol branch through deterministic beam search over the local
    /// resource-constrained simulator. This route plans only and never dispatches a branch.
    pub(super) fn glioma_protocol_branch_optimize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProtocolBranchOptimizationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_branch_optimize requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol branch optimization request: {error}"))?;
        let plan = optimize_glioma_protocol_branches(&request)
            .map_err(|error| format!("glioma protocol branch optimization refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_protocol_execute",
            "guarantees": [
                "each retained branch is evaluated by the typed resource, horizon, risk, and approval simulator",
                "beam and budget pruning remain explicit as unresolved alternatives",
                "branch replacements preserve the original model, output schema, and dependency topology",
                "the route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol branch optimization: {error}"))
    }

    /// Run a bounded protocol-level autonomous loop through the deterministic synthetic worker.
    /// Production effects remain behind the caller-owned GliomaProtocolExecutor seam.
    pub(super) fn glioma_protocol_autonomous_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AutonomousProtocolControllerRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_protocol_autonomous_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma autonomous protocol request: {error}"))?;
        let mut executor = DryRunGliomaProtocolExecutor;
        let run = execute_glioma_autonomous_protocol(&request, &mut executor)
            .map_err(|error| format!("glioma autonomous protocol refused: {error}"))?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_protocol_execute",
            "guarantees": [
                "branch planning, local execution, negative evidence, and compensation remain in one bounded typed loop",
                "only an untried branch returned by the planner can be selected for execution",
                "round, retry, beam, branch, and budget limits are explicit and fail closed",
                "the MCP route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma autonomous protocol run: {error}"))
    }

    /// Compile quality- and uncertainty-aware endpoint evidence from a local protocol result.
    /// This is a scientific handoff only; it does not infer a clinical conclusion or dispatch work.
    pub(super) fn glioma_protocol_evidence_surface(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProtocolEvidenceSurfaceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_evidence_surface requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol evidence surface request: {error}"))?;
        let surface = compile_glioma_protocol_evidence_surface(&request)
            .map_err(|error| format!("glioma protocol evidence surface refused: {error}"))?;
        serde_json::to_value(json!({
            "surface": surface,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_mechanism_explore", "glioma_analysis_run"],
            "guarantees": [
                "robust endpoint summaries retain quality, uncertainty, replicate, negative, and contradictory states",
                "missing, failed, skipped, or partial task outputs cannot be promoted to qualified evidence",
                "the route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol evidence surface: {error}"))
    }

    /// Fuse study-local protocol evidence surfaces while retaining transport boundaries.
    /// Raw experimental data never leaves the institution-local surface boundary.
    pub(super) fn glioma_protocol_multistudy_fusion(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProtocolEvidenceFusionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_multistudy_fusion requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol multistudy fusion request: {error}"))?;
        let fusion = fuse_glioma_protocol_evidence(&request)
            .map_err(|error| format!("glioma protocol multistudy fusion refused: {error}"))?;
        serde_json::to_value(json!({
            "fusion": fusion,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_mechanism_explore", "glioma_analysis_run"],
            "guarantees": [
                "raw experimental data remains local and only typed surface summaries are fused",
                "heterogeneity, contradiction, negative, and under-supported endpoints remain explicit",
                "the route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol multistudy fusion: {error}"))
    }

    /// Decide which fused preclinical endpoints can cross a target-model workflow boundary.
    /// The gate preserves negative and blocked findings and never performs a biological effect.
    pub(super) fn glioma_protocol_transport_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProtocolTransportGateRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_transport_gate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol transport gate request: {error}"))?;
        let gate = gate_glioma_protocol_transport(&request)
            .map_err(|error| format!("glioma protocol transport gate refused: {error}"))?;
        serde_json::to_value(json!({
            "gate": gate,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_mechanism_explore", "glioma_experiment_design"],
            "guarantees": [
                "target-model, independent-study, site-diversity, information, and heterogeneity gates are explicit",
                "negative, contradictory, heterogeneous, unresolved, and under-supported endpoints cannot be promoted",
                "the route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol transport gate: {error}"))
    }

    /// Execute a feasible protocol through the deterministic synthetic executor. Production
    /// instrument or worker effects remain behind the Rust `GliomaProtocolExecutor` seam.
    pub(super) fn glioma_protocol_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request: ProtocolExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol execution request: {error}"))?;
        let mut executor = DryRunGliomaProtocolExecutor;
        let execution = execute_glioma_protocol(&request, &mut executor)
            .map_err(|error| format!("glioma protocol execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "execution requires a feasible deterministic protocol simulation",
                "dependency order, typed output schemas, retries, partial results, and skipped work remain explicit",
                "the MCP route uses synthetic local artifacts and performs no biological or instrument effect",
                "production effects require a caller-owned local GliomaProtocolExecutor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol execution: {error}"))
    }

    /// Compile a bounded recovery portfolio for failed, partial, or skipped protocol tasks.
    /// Candidates must preserve the original typed task contract and completed/negative evidence;
    /// this route never retries, dispatches, or fabricates an assay result.
    pub(super) fn glioma_protocol_compensation(&self, arguments: &Value) -> Result<Value, String> {
        let request: ProtocolCompensationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_protocol_compensation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma protocol compensation request: {error}"))?;
        let plan = plan_glioma_protocol_compensation(&request)
            .map_err(|error| format!("glioma protocol compensation refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_protocol_execute",
            "guarantees": [
                "failed, partial, and skipped tasks remain explicit in the blocked frontier",
                "replacement candidates must match output schema, model, resource, and dependency contracts",
                "completed and negative evidence are preserved; no skipped task is promoted",
                "the route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma protocol compensation: {error}"))
    }

    /// Execute the beam-selected action portfolio through the deterministic synthetic executor.
    /// Production assays, analysis workers, and instruments remain behind the Rust
    /// `GliomaActionExecutor` seam.
    pub(super) fn glioma_action_portfolio_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ActionPortfolioExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_action_portfolio_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma action portfolio execution request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let execution = execute_glioma_action_portfolio(&request, &mut executor)
            .map_err(|error| format!("glioma action portfolio execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "the beam-selected portfolio is executed in dependency order through a caller-owned seam",
                "bounded transient retries, missing artifacts, failures, partial results, and skipped dependents remain explicit",
                "the MCP route uses synthetic local artifacts and performs no biological, instrument, or external effect",
                "production workers and gateways require a caller-owned GliomaActionExecutor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma action portfolio execution: {error}"))
    }

    /// Run a bounded observation-driven autonomous glioma campaign through deterministic local
    /// planner/worker seams. Production planners and workers remain caller-owned.
    pub(super) fn glioma_autonomous_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaAutonomousCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_autonomous_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma autonomous campaign request: {error}"))?;
        let mut planner = StaticGliomaActionPlanner;
        let mut executor = DryRunGliomaActionExecutor;
        let campaign = execute_glioma_autonomous_campaign(&request, &mut planner, &mut executor)
            .map_err(|error| format!("glioma autonomous campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "the campaign replans only from typed outcomes returned by the local executor",
                "the action registry, budget, rounds, dependencies, and failure boundaries are bounded",
                "negative, partial, failed, skipped, and no-candidate stops remain explicit",
                "the MCP route performs no biological, instrument, or external effect",
                "production planners and workers require caller-owned GliomaActionPlanner and GliomaActionExecutor implementations"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma autonomous campaign: {error}"))
    }

    /// Close one evidence-to-action-to-execution cycle with the deterministic sandbox worker.
    /// Institution-local callers can replace the worker through the Rust SDK seam.
    pub(super) fn glioma_research_autopilot_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaResearchAutopilotRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_research_autopilot_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma research autopilot request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_research_autopilot(&request, &mut executor)
            .map_err(|error| format!("glioma research autopilot refused: {error}"))?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "P04 compiled context is selected and executed through the same dependency-safe P07 portfolio path",
                "completed actions, budget, policy, artifacts, negative results, and failures remain explicit",
                "a no-runnable-action hold has no synthetic execution result",
                "the MCP route uses a synthetic local worker; production assay or analysis effects require a caller-owned GliomaActionExecutor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma research autopilot: {error}"))
    }

    /// Execute the pending P01 evidence-priority queue through typed local adapters. The MCP
    /// adapter uses a synthetic worker, while institution-local Rust callers can provide a real
    /// assay, analysis, or instrument gateway behind the executor trait.
    pub(super) fn glioma_evidence_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaEvidenceCampaignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_campaign_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence campaign request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let execution = execute_glioma_evidence_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma evidence campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "only selected evidence-priority actions and their declared dependency closure are admitted",
                "missing adapters, approval blocks, retries, partial effects, failures, and negative results remain explicit",
                "the queue digest is carried into the execution result and cannot be silently replaced",
                "the MCP route uses a synthetic local worker; production effects require a caller-owned GliomaActionExecutor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence campaign: {error}"))
    }

    /// Refresh P01 evidence deltas through a bounded, institution-local executor and replan
    /// surveillance after each round. The MCP adapter remains a synthetic sandbox worker.
    pub(super) fn glioma_evidence_refresh_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceRefreshCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_refresh_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma evidence refresh campaign request: {error}")
            })?;
        let mut executor = DryRunEvidenceRefreshCampaignExecutor;
        let campaign = execute_glioma_evidence_refresh_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma evidence refresh campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "surveillance deltas are recomputed after every returned local evidence record",
                "only typed de-identified local evidence metadata crosses the MCP boundary",
                "stale, unknown, contradictory, negative, and missing coverage remain explicit",
                "retry, budget, no-progress, executor failure, and malformed-record stops remain explicit",
                "the route performs no internet fetch, raw data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence refresh campaign: {error}"))
    }

    /// Compile, resolve, and recompile the P02 claim frontier through a bounded local worker.
    /// The MCP adapter is synthetic; production adapters remain institution-local.
    pub(super) fn glioma_knowledge_resolution_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeResolutionCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_knowledge_resolution_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma knowledge resolution campaign request: {error}")
            })?;
        let mut executor = DryRunKnowledgeResolutionCampaignExecutor;
        let campaign = execute_glioma_knowledge_resolution_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma knowledge resolution campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "typed knowledge and frontier are recomputed from returned evidence after every round",
                "contradictory, negative, unknown, missing-coverage, retry, budget, and no-progress states remain explicit",
                "only local de-identified evidence metadata crosses this MCP boundary",
                "the route performs no internet fetch, raw data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge resolution campaign: {error}"))
    }

    /// Execute bounded metadata-only multimodal ingestion/QC rounds through a local adapter.
    pub(super) fn glioma_multimodal_ingestion_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalIngestionCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_ingestion_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma multimodal ingestion campaign request: {error}")
            })?;
        let mut executor = DryRunMultimodalIngestionCampaignExecutor;
        let campaign = execute_glioma_multimodal_ingestion_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma multimodal ingestion campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "QC is recomputed from every returned observation after each bounded round",
                "missing modalities, model coverage, excluded observations, defects, retries, budget, and no-progress remain explicit",
                "raw payloads remain institution-local and only de-identified metadata crosses the MCP boundary",
                "the route performs no clinical decision or external instrument effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal ingestion campaign: {error}"))
    }

    /// Admit downstream preclinical research surfaces from the final multimodal QC state. MCP
    /// uses a deterministic metadata-only dry run; production callers provide the local adapter.
    pub(super) fn glioma_multimodal_readiness_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalReadinessRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_readiness_gate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal readiness request: {error}"))?;
        let mut executor = DryRunMultimodalIngestionCampaignExecutor;
        let readiness = execute_glioma_multimodal_readiness_gate(&request, &mut executor)
            .map_err(|error| format!("glioma multimodal readiness gate refused: {error}"))?;
        serde_json::to_value(json!({
            "readiness": readiness,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "modality/model coverage and comparable-observation quality are quantified from the executed QC campaign",
                "analysis, mechanism, experiment-design, replication, and publication surfaces are admitted independently",
                "partial, blocked, unresolved, missing-modality, defect, and negative-result actions remain explicit",
                "MCP performs no raw-data movement, external instrument effect, clinical decision, or treatment recommendation"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal readiness: {error}"))
    }

    /// Run the complete P03 ingestion-to-readiness operating cycle in the deterministic metadata
    /// sandbox. Governed-local execution remains behind an institution-owned adapter.
    pub(super) fn glioma_multimodal_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaMultimodalOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal operating-cycle request: {error}"))?;
        if matches!(
            request.execution_mode,
            MultimodalExecutionMode::GovernedLocal
        ) {
            return Err(
                "glioma_multimodal_operating_cycle MCP route is simulation-only; governed_local requires an institution-owned metadata adapter"
                    .to_string(),
            );
        }
        let cycle = execute_glioma_multimodal_operating_cycle_dry_run(&request)
            .map_err(|error| format!("glioma multimodal operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "metadata-only ingestion and QC run before downstream surface admission",
                "missing modalities, quality defects, conditional surfaces, and remediation actions remain explicit",
                "admitted surfaces are typed handoffs and never become biological conclusions by themselves",
                "MCP moves no raw payloads, executes no instruments, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal operating cycle: {error}"))
    }

    /// Compile and dispatch a bounded P04 question-to-action campaign through a local adapter.
    pub(super) fn glioma_decision_context_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionContextCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_decision_context_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma decision-context campaign request: {error}")
            })?;
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let campaign = execute_glioma_decision_context_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma decision-context campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "typed knowledge, decision context, and selected action portfolio are recomputed after every returned evidence row",
                "the action executor receives only a typed claim-scoped contract and cannot promote a plan into evidence",
                "negative, contradictory, unresolved, omission, retry, budget, and no-progress states remain explicit",
                "raw payloads remain institution-local and the route performs no clinical decision or external instrument effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-context campaign: {error}"))
    }

    /// Execute the selected robust P04 branch through MCP's deterministic local executor and
    /// fail over to the next Pareto branch when returned evidence drifts from the forecast.
    pub(super) fn glioma_decision_branch_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionBranchCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_decision_branch_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma decision-branch campaign request: {error}"))?;
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let campaign = execute_glioma_decision_branch_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma decision-branch campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the selected robust branch executes only through the typed local executor and every action remains bound to the immutable decision context",
                "observed evidence is scored against branch forecasts and drift, failed actions, budget blocks, and fallback branches remain explicit",
                "negative, contradictory, unknown, stale, and unmeasured evidence is retained and never promoted into a confident conclusion",
                "the MCP route moves no raw payloads and performs no external instrument, federation, clinical, or treatment effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-branch campaign: {error}"))
    }

    /// Recompile P04 knowledge and decision context after each returned branch-evidence batch.
    pub(super) fn glioma_adaptive_decision_branch_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveDecisionBranchCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_decision_branch_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive decision-branch request: {error}"))?;
        let campaign = execute_glioma_adaptive_decision_branch_campaign_dry_run(&request).map_err(
            |error| format!("glioma adaptive decision-branch campaign refused: {error}"),
        )?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "knowledge, decision context, and robust branch projections are recompiled only from returned typed local evidence",
                "scenario forecasts remain distinct from observations and are never promoted by the controller",
                "qualified, partial, budget-blocked, failed, unresolved, and no-progress states remain explicit",
                "the MCP route performs no raw-data movement, external instrument effect, federation, diagnosis, treatment, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive decision-branch campaign: {error}"))
    }

    /// Run the full P04 evidence-to-decision operating cycle through MCP's deterministic local
    /// executor: knowledge, context, dependency graph, robust branches, then campaign execution.
    pub(super) fn glioma_decision_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision operating-cycle request: {error}"))?;
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let cycle = execute_glioma_decision_operating_cycle(&request, &mut executor)
            .map_err(|error| format!("glioma decision operating-cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "typed knowledge is compiled into a dependency-closed decision context and action graph",
                "scenario branches preserve expected value, worst-case value, uncertainty, and failure risk separately",
                "the selected decision campaign executes only through MCP's synthetic local adapter",
                "contradictions, negative evidence, omissions, retries, budget limits, and no-progress states remain explicit",
                "the route performs no raw-data movement, external instrument effect, clinical decision, or treatment recommendation"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision operating-cycle: {error}"))
    }
}
