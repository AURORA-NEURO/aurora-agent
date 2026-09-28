//! Glioma research handlers grouped by workflow domain.

use super::*;

fn dry_run_stage_workers(
    profiles: &[GliomaStageWorkerProfile],
) -> BTreeMap<String, Box<dyn GliomaStageExecutor>> {
    profiles
        .iter()
        .filter(|profile| profile.available && profile.local_only && profile.deterministic)
        .map(|profile| {
            (
                profile.worker_id.clone(),
                Box::new(DryRunGliomaStageWorker) as Box<dyn GliomaStageExecutor>,
            )
        })
        .collect()
}

impl Server {
    /// Evaluate the autonomous engine's selection policy against held-out utility without
    /// dispatching a provider, instrument, federation, or biological operation.
    pub(super) fn glioma_autonomous_research_engine_evaluate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaAutonomousResearchEngineRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| {
                    "glioma_autonomous_research_engine_evaluate requires request".to_string()
                })?,
        )
        .map_err(|error| format!("invalid glioma autonomous engine evaluation request: {error}"))?;
        let held_out: BTreeMap<String, i64> = serde_json::from_value(
            arguments
                .get("held_out_utility_milli")
                .cloned()
                .ok_or_else(|| {
                    "glioma_autonomous_research_engine_evaluate requires held_out_utility_milli"
                        .to_string()
                })?,
        )
        .map_err(|error| format!("invalid held-out utility map: {error}"))?;
        let evaluation = evaluate_glioma_autonomous_research_engine(&request, &held_out)
            .map_err(|error| format!("glioma autonomous engine evaluation refused: {error}"))?;
        Ok(json!({
            "evaluation": evaluation,
            "evaluation_only": true,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "held-out utility is evaluated after planning and never enters the planner",
                "policy comparisons use a bounded dependency-aware oracle and deterministic baselines",
                "no provider, assay, instrument, federation, raw-data transfer, or clinical operation is invoked"
            ]
        }))
    }

    /// Stress the autonomous engine policy over a bounded map of held-out utility worlds.
    pub(super) fn glioma_autonomous_research_engine_stress_evaluate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaAutonomousResearchEngineRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| {
                    "glioma_autonomous_research_engine_stress_evaluate requires request"
                        .to_string()
                })?,
        )
        .map_err(|error| format!("invalid glioma autonomous engine stress request: {error}"))?;
        let scenarios: BTreeMap<String, BTreeMap<String, i64>> = serde_json::from_value(
            arguments
                .get("held_out_scenarios")
                .cloned()
                .ok_or_else(|| {
                    "glioma_autonomous_research_engine_stress_evaluate requires held_out_scenarios"
                        .to_string()
                })?,
        )
        .map_err(|error| format!("invalid held-out scenario map: {error}"))?;
        let evaluation = evaluate_glioma_autonomous_research_engine_scenarios(&request, &scenarios)
            .map_err(|error| format!("glioma autonomous engine stress evaluation refused: {error}"))?;
        Ok(json!({
            "evaluation": evaluation,
            "evaluation_only": true,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "the number of named held-out scenarios is bounded by the domain evaluator",
                "stress utilities are not exposed to planning and do not become biological evidence",
                "no provider, assay, instrument, federation, raw-data transfer, or clinical operation is invoked"
            ]
        }))
    }

    /// Replay the autonomous engine against named synthetic outcome traces without dispatch.
    pub(super) fn glioma_autonomous_research_engine_trace_evaluate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaAutonomousResearchEngineRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| {
                    "glioma_autonomous_research_engine_trace_evaluate requires request"
                        .to_string()
                })?,
        )
        .map_err(|error| format!("invalid glioma autonomous engine trace request: {error}"))?;
        let traces: BTreeMap<String, BTreeMap<String, GliomaEngineTraceOutcome>> =
            serde_json::from_value(
                arguments
                    .get("outcome_traces")
                    .cloned()
                    .ok_or_else(|| {
                        "glioma_autonomous_research_engine_trace_evaluate requires outcome_traces"
                            .to_string()
                    })?,
            )
            .map_err(|error| format!("invalid synthetic outcome traces: {error}"))?;
        let evaluation = evaluate_glioma_autonomous_research_engine_traces(&request, &traces)
            .map_err(|error| format!("glioma autonomous engine trace evaluation refused: {error}"))?;
        Ok(json!({
            "evaluation": evaluation,
            "evaluation_only": true,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "trace outcomes are synthetic evaluator inputs, never biological observations",
                "missing action outcomes fail closed in every policy replay",
                "no provider, assay, instrument, federation, raw-data transfer, or clinical operation is invoked"
            ]
        }))
    }

    /// Compile typed local worker declarations against the research intent's closed stage graph.
    pub(super) fn glioma_stage_worker_routes_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaStageWorkerRouteRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_stage_worker_routes_compile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma stage-worker route request: {error}"))?;
        let route_plan = compile_glioma_stage_worker_routes(&request)
            .map_err(|error| format!("glioma stage-worker route compilation refused: {error}"))?;
        Ok(json!({
            "route_plan": route_plan,
            "evaluation_only": true,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "each route is bound to the compiled typed intent and one declared worker contract",
                "uncovered, unavailable, incompatible, non-local, or non-deterministic stages remain explicitly blocked",
                "route compilation opens no connection and executes no worker"
            ]
        }))
    }

    /// Rehearse the autonomous stage engine through synthetic workers for declared available,
    /// local, deterministic profiles. Real workers remain caller-owned and are never invoked by
    /// this MCP surface.
    pub(super) fn glioma_autonomous_research_engine_stage_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .cloned()
            .ok_or_else(|| {
                "glioma_autonomous_research_engine_stage_execute requires request".to_string()
            })?;
        let engine_request: GliomaAutonomousResearchEngineRequest =
            serde_json::from_value(request_value.clone())
                .map_err(|error| format!("invalid stage-engine request: {error}"))?;
        let workers: Vec<GliomaStageWorkerProfile> = serde_json::from_value(
            arguments
                .get("workers")
                .cloned()
                .ok_or_else(|| {
                    "glioma_autonomous_research_engine_stage_execute requires workers".to_string()
                })?,
        )
        .map_err(|error| format!("invalid stage-worker profiles: {error}"))?;
        let route_request = GliomaStageWorkerRouteRequest {
            intent: engine_request.intent.clone(),
            workers: workers.clone(),
            require_deterministic: arguments
                .get("require_deterministic")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            require_all_ready: arguments
                .get("require_all_ready")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        };
        let route_plan = compile_glioma_stage_worker_routes(&route_request)
            .map_err(|error| format!("stage-worker route compilation refused: {error}"))?;
        let execution = execute_glioma_autonomous_research_engine_with_stage_workers(
            &engine_request,
            &route_plan,
            dry_run_stage_workers(&workers),
        )
        .map_err(|error| format!("stage-worker engine rehearsal refused: {error}"))?;
        Ok(json!({
            "execution": execution,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "only declared available, local, deterministic workers receive synthetic rehearsal outputs",
                "synthetic stage artifacts are not biological evidence or institution-worker receipts",
                "no real worker, provider, assay, instrument, federation, or clinical operation is invoked"
            ]
        }))
    }

    /// Hold or rehearse an autonomous engine only after the caller-supplied evidence and local
    /// worker gates have been checked by the typed research contract.
    pub(super) fn glioma_evidence_gated_stage_engine_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaEvidenceGatedStageExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| {
                    "glioma_evidence_gated_stage_engine_execute requires request".to_string()
                })?,
        )
        .map_err(|error| format!("invalid evidence-gated stage request: {error}"))?;
        let execution = execute_glioma_evidence_gated_stage_engine(
            &request,
            dry_run_stage_workers(&request.workers),
        )
        .map_err(|error| format!("evidence-gated stage engine refused: {error}"))?;
        let dispatch = if execution.execution_started {
            "dry_run"
        } else {
            "not_started"
        };
        Ok(json!({
            "execution": execution,
            "dispatch": dispatch,
            "simulation_only": true,
            "guarantees": [
                "unqualified or unresolved triangulation returns a typed hold before worker routing",
                "worker rehearsal accepts only available, local, deterministic profiles",
                "synthetic outputs remain explicit and no external or clinical operation is invoked"
            ]
        }))
    }

    /// Reconcile temporal multimodal mechanistic predictions with declared local preclinical
    /// observations and emit a bounded next-measurement frontier.
    pub(super) fn glioma_temporal_multimodal_mechanism_fusion(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: TemporalMultimodalMechanismFusionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| {
                    "glioma_temporal_multimodal_mechanism_fusion requires request".to_string()
                })?,
        )
        .map_err(|error| format!("invalid temporal multimodal mechanism-fusion request: {error}"))?;
        let fusion = execute_glioma_temporal_multimodal_mechanism_fusion(&request)
            .map_err(|error| format!("temporal multimodal mechanism fusion refused: {error}"))?;
        Ok(json!({
            "fusion": fusion,
            "dispatch": "analysis_only",
            "simulation_only": true,
            "guarantees": [
                "mechanism predictions are compared only with caller-supplied local observations",
                "missing evidence and contradictory predictions remain visible in the assessment",
                "the next-measurement frontier is a plan and does not execute assays or make clinical decisions"
            ]
        }))
    }

    /// Infer longitudinal multimodal state transitions from caller-supplied local preclinical
    /// summaries. Missing timepoints, modalities, and non-comparable features remain explicit;
    /// this route does not fetch data, execute an assay, or make a clinical decision.
    pub(super) fn glioma_temporal_multimodal_fusion(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: TemporalFusionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_temporal_multimodal_fusion requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma temporal fusion request: {error}"))?;
        let observations: Vec<TemporalObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_temporal_multimodal_fusion requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma temporal fusion observations: {error}"))?;
        let analysis = analyze_glioma_temporal_multimodal_fusion(&request, &observations)
            .map_err(|error| format!("glioma temporal fusion refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "longitudinal states are joined only by declared sample, timepoint, modality, and local artifact identity",
                "transition priority is based on comparable-feature distance and support, not an unbounded confidence score",
                "missing timepoints, modalities, and non-comparable transitions remain partial or unresolved",
                "stable transitions are published as negative evidence; MCP executes no assay and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma temporal fusion: {error}"))
    }

    /// Align a declared temporal multimodal state to registered spatial cells and a bounded
    /// propagation simulation. The route emits per-sample gaps and typed follow-up actions; it
    /// never infers an unobserved timepoint, executes an assay, moves raw data, or makes a
    /// clinical decision.
    pub(super) fn glioma_temporal_spatial_alignment(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: TemporalSpatialAlignmentRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_temporal_spatial_alignment requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma temporal-spatial alignment request: {error}"))?;
        let observations: Vec<TemporalObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_temporal_spatial_alignment requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma temporal-spatial observations: {error}"))?;
        let cells: Vec<SpatialRegistrationCell> = serde_json::from_value(
            arguments
                .get("cells")
                .cloned()
                .ok_or_else(|| "glioma_temporal_spatial_alignment requires cells".to_string())?,
        )
        .map_err(|error| format!("invalid glioma temporal-spatial cells: {error}"))?;
        let output = analyze_glioma_temporal_spatial_alignment(&request, &observations, &cells)
            .map_err(|error| format!("glioma temporal-spatial alignment refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "local_analysis",
            "simulation_only": false,
            "guarantees": [
                "sample-to-timepoint mappings are caller-declared and never inferred",
                "registration, propagation, and temporal evidence retain separate gates and negative evidence",
                "state gaps, spatial coverage shortfalls, missing timepoints, and non-convergent trajectories become typed follow-up actions",
                "raw artifacts remain institution-local; the route performs preclinical research analysis only and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma temporal-spatial alignment: {error}"))
    }

    /// Infer a bounded marker-aware clonal-evolution graph from caller-supplied local
    /// preclinical profiles. Parentage is a reproducible proposal with explicit ambiguity;
    /// this route never claims a clinical phylogeny or executes a biological intervention.
    pub(super) fn glioma_clonal_evolution(&self, arguments: &Value) -> Result<Value, String> {
        let request: ClonalEvolutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_clonal_evolution requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clonal evolution request: {error}"))?;
        let profiles: Vec<CloneProfile> = serde_json::from_value(
            arguments
                .get("profiles")
                .cloned()
                .ok_or_else(|| "glioma_clonal_evolution requires profiles".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clonal evolution profiles: {error}"))?;
        let analysis = analyze_glioma_clonal_evolution(&request, &profiles)
            .map_err(|error| format!("glioma clonal evolution refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "parent-child edges are bounded to the declared preclinical sample lineage and time window",
                "marker gains, losses, unmeasured states, weak support, parallel branches, and unparented nodes remain explicit",
                "the output is a mechanism-discovery graph proposal and not a clinical phylogeny or diagnosis",
                "MCP executes no assay, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma clonal evolution: {error}"))
    }

    /// Compile a bounded, clone-aware preclinical perturbation/readout panel from a local
    /// clonal-evolution graph. Selection is deterministic set cover per declared cost; no live
    /// perturbation, specimen edit, raw-data movement, or clinical decision occurs here.
    pub(super) fn glioma_clone_perturbation_panel(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ClonePerturbationPanelRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_clone_perturbation_panel requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone panel request: {error}"))?;
        let graph: ClonalEvolutionGraph = serde_json::from_value(
            arguments
                .get("graph")
                .cloned()
                .ok_or_else(|| "glioma_clone_perturbation_panel requires graph".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone panel graph: {error}"))?;
        let candidates: Vec<ClonePerturbationCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_clone_perturbation_panel requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid glioma clone panel candidates: {error}"))?;
        let panel = plan_glioma_clone_perturbation_panel(&request, &graph, &candidates)
            .map_err(|error| format!("glioma clone perturbation panel refused: {error}"))?;
        serde_json::to_value(json!({
            "panel": panel,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "selection is bounded by declared budget, candidate count, and branch-coverage policy",
                "unmeasured targets, uncovered branches, budget shortfalls, and negative evidence remain explicit",
                "the output is a preclinical design artifact and not treatment, resistance, diagnosis, or clinical advice",
                "MCP executes no perturbation, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma clone perturbation panel: {error}"))
    }

    /// Reconcile local replicate outcomes against a selected clone-aware preclinical panel.
    /// Missing and contradictory cells remain unresolved; this route produces interpretation
    /// evidence and next-measurement priorities but never executes biology or makes a clinical
    /// decision.
    pub(super) fn glioma_clone_panel_outcomes(&self, arguments: &Value) -> Result<Value, String> {
        let request: ClonePanelOutcomeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_clone_panel_outcomes requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone panel outcome request: {error}"))?;
        let panel: ClonePerturbationPanel = serde_json::from_value(
            arguments
                .get("panel")
                .cloned()
                .ok_or_else(|| "glioma_clone_panel_outcomes requires panel".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone panel: {error}"))?;
        let observations: Vec<ClonePanelObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_clone_panel_outcomes requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone panel observations: {error}"))?;
        let analysis = analyze_glioma_clone_panel_outcomes(&request, &panel, &observations)
            .map_err(|error| format!("glioma clone panel outcome analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "replicate floors, uncertainty bounds, null effects, contradictions, and missing cells remain explicit",
                "a qualified design panel is never promoted to evidence without local observations",
                "next actions are measurement or retest priorities, not treatment or resistance claims",
                "MCP executes no assay, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma clone panel outcome analysis: {error}"))
    }

    /// Compile a dependency-closed next-action frontier from unresolved or contradictory clone
    /// outcomes. The route plans only; institution-local executors own approvals, instruments,
    /// and any permitted federation effects.
    pub(super) fn glioma_clone_continuation(&self, arguments: &Value) -> Result<Value, String> {
        let request: CloneContinuationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_clone_continuation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone continuation request: {error}"))?;
        let outcome: ClonePanelOutcomeAnalysis = serde_json::from_value(
            arguments
                .get("outcome")
                .cloned()
                .ok_or_else(|| "glioma_clone_continuation requires outcome".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone panel outcome: {error}"))?;
        let candidates: Vec<CloneContinuationCandidate> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or_else(|| "glioma_clone_continuation requires candidates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma clone continuation candidates: {error}"))?;
        let plan = plan_glioma_clone_continuation(&request, &outcome, &candidates)
            .map_err(|error| format!("glioma clone continuation refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "dependency closure, budget, risk, instrument, federation, and approval gates are explicit",
                "unresolved targets, deferred actions, blocked actions, uncertainty, and negative evidence remain visible",
                "the plan is a preclinical research continuation artifact and not treatment, resistance, diagnosis, or clinical advice",
                "MCP executes no assay, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma clone continuation plan: {error}"))
    }

    /// Execute the bounded evolution-aware clone campaign in a deterministic local sandbox.
    /// Production hosts replace the synthetic panel worker with an institution-owned executor.
    pub(super) fn glioma_adaptive_clone_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveCloneCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_clone_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive clone campaign request: {error}"))?;
        let campaign = execute_glioma_adaptive_clone_campaign_dry_run(&request)
            .map_err(|error| format!("glioma adaptive clone campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "clonal graph inference, branch-covering panel design, replicate adjudication, and continuation planning run as one bounded workflow",
                "only caller-returned typed local observations can change the outcome; missing or contradictory cells are never imputed",
                "qualified, negative, partial, unresolved, budget-blocked, and executor-failed states remain explicit",
                "MCP executes no assay, moves no raw data, and makes no diagnosis, treatment, triage, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive clone campaign: {error}"))
    }

    /// Direct one bounded, dependency-closed glioma research batch from a high-level intent.
    /// MCP supplies only a deterministic synthetic action worker; institution-local callers
    /// replace that worker with their approved analysis, simulator, or gateway adapter.
    pub(super) fn glioma_research_director_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaResearchDirectorRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_research_director_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma research director request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_research_director(&request, &mut executor)
            .map_err(|error| format!("glioma research director refused: {error}"))?;
        serde_json::to_value(json!({
            "director": run,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the director compiles the closed glioma stage graph before selecting any action",
                "dependency closure, bounded beam selection, checkpoint locality, approval, instrument, and federation gates remain active",
                "dry-run artifacts are synthetic and never count as biological evidence",
                "production execution remains caller-owned and no clinical decision is produced"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma research director run: {error}"))
    }

    /// Schedule multiple independent preclinical glioma research intents as one bounded local
    /// program. The MCP adapter deliberately uses the deterministic synthetic worker; governed
    /// deployments provide their own institution-local executor to the research crate.
    pub(super) fn glioma_program_scheduler_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaProgramSchedulerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_program_scheduler_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma program scheduler request: {error}"))?;
        let schedule = execute_glioma_program_scheduler_dry_run(&request)
            .map_err(|error| format!("glioma program scheduler refused: {error}"))?;
        serde_json::to_value(json!({
            "schedule": schedule,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "multiple independent preclinical glioma intents are ranked by frontier utility, fairness debt, feasibility, and cost",
                "model and modality capacity admission prevents conflicting local batches and preserves explicit holds",
                "each job retains its own checkpoints and is replanned from returned typed local artifacts",
                "failed jobs, negative outcomes, budget limits, no-progress states, and operator holds remain explicit",
                "MCP produces no raw data, human data, instrument effect, clinical decision, or federation export"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma program scheduler run: {error}"))
    }

    /// Run the multi-objective P06 experiment frontier in a deterministic local sandbox. The
    /// controller combines mechanism information gain, power, clone/modality coverage, fidelity
    /// escalation, risk, cost, prerequisites, and bounded local observation/replanning.
    pub(super) fn glioma_experiment_frontier_controller_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaExperimentFrontierRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_experiment_frontier_controller_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma experiment frontier request: {error}"))?;
        let mut executor = DryRunGliomaExperimentFrontierExecutor;
        let run = execute_glioma_experiment_frontier_controller(&request, &mut executor)
            .map_err(|error| format!("glioma experiment frontier controller refused: {error}"))?;
        serde_json::to_value(json!({
            "frontier": run,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "declared mechanism likelihoods are recalculated after every local observation",
                "power, risk, budget, prerequisites, fidelity, clone, modality, and replication gates remain explicit",
                "null, negative, failed, retryable, unresolved, and budget outcomes remain visible",
                "MCP performs no real assay, instrument effect, raw-data movement, federation export, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma experiment frontier run: {error}"))
    }

    /// Adjudicate a preclinical glioma causal claim by composing causal contrast, confounding
    /// sensitivity, independent-site replication, and meta-analysis gates. The route returns
    /// evidence-driven next actions and never emits a clinical conclusion.
    pub(super) fn glioma_causal_claim_adjudication_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaCausalClaimAdjudicationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_causal_claim_adjudication_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma causal claim request: {error}"))?;
        let adjudication = execute_glioma_causal_claim_adjudication(&request)
            .map_err(|error| format!("glioma causal claim adjudication refused: {error}"))?;
        serde_json::to_value(json!({
            "adjudication": adjudication,
            "dispatch": "local_analysis",
            "simulation_only": false,
            "guarantees": [
                "causal, confounding, independent-replication, and meta-analysis gates remain separate",
                "heterogeneity, null effects, underpowered units, and negative evidence remain visible",
                "next actions are derived from failed gates rather than invented confidence",
                "the result is preclinical research interpretation only and never a diagnosis, treatment, triage, or enrollment decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma causal claim adjudication: {error}"))
    }

    /// Run the mechanism-specific autonomous vertical in a deterministic local sandbox. The
    /// route composes multimodal fusion, pathway activity, feedback dynamics, robust ensemble
    /// intervention ranking, and gated local assay execution; it never performs a real assay or
    /// treats simulation output as biological evidence.
    pub(super) fn glioma_mechanism_discovery_engine_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaMechanismDiscoveryRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_discovery_engine_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism discovery request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_mechanism_discovery_engine(&request, &mut executor)
            .map_err(|error| format!("glioma mechanism discovery engine refused: {error}"))?;
        serde_json::to_value(json!({
            "discovery": run,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "multimodal graph fusion and pathway activity remain separate evidence gates",
                "signed feedback dynamics and model-ensemble lower-tail intervention ranking are recomputed before each action batch",
                "only a qualified, dependency-safe local action portfolio can dispatch",
                "completed, negative, failed, partial, unresolved, budget, and no-progress outcomes remain explicit for replanning",
                "MCP performs no real assay, instrument effect, raw-data movement, federation export, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism discovery run: {error}"))
    }

    /// Admit the director only after local P01 evidence triangulation clears the caller's gate.
    /// MCP uses the same deterministic dry-run action worker as the director route; no biological
    /// effect or clinical decision is performed here.
    pub(super) fn glioma_evidence_gated_research_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaEvidenceGatedResearchRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_gated_research_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma evidence-gated research request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_evidence_gated_research(&request, &mut executor)
            .map_err(|error| format!("glioma evidence-gated research refused: {error}"))?;
        let dispatch = if run.execution_started {
            "dry_run"
        } else {
            "not_started"
        };
        serde_json::to_value(json!({
            "gated_research": run,
            "dispatch": dispatch,
            "simulation_only": true,
            "guarantees": [
                "partial, negative, contradictory, incomplete, and source-dominant evidence holds the workflow before director execution",
                "only independently triangulated qualified claims can admit the bounded director",
                "director dependency, budget, approval, locality, instrument, and federation gates remain active",
                "the route performs no real assay, instrument effect, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence-gated research run: {error}"))
    }

    /// Run the end-to-end autonomous glioma engine. The MCP adapter uses a deterministic local
    /// worker for rehearsal; an institution-owned caller can provide an approved executor to the
    /// same research-crate function for real preclinical computation or instrument gateways.
    pub(super) fn glioma_autonomous_research_engine_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaAutonomousResearchEngineRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_autonomous_research_engine_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma autonomous research engine request: {error}")
            })?;
        let institution_local = self.glioma_action_executor.is_some();
        let engine = if let Some(executor) = &self.glioma_action_executor {
            let mut executor = executor
                .lock()
                .map_err(|_| "configured glioma action executor lock is poisoned".to_string())?;
            execute_glioma_autonomous_research_engine(&request, &mut **executor)
        } else {
            let mut executor = DryRunGliomaActionExecutor;
            execute_glioma_autonomous_research_engine(&request, &mut executor)
        }
        .map_err(|error| format!("glioma autonomous research engine refused: {error}"))?;
        let execution_started = engine
            .cycles
            .iter()
            .any(|cycle| cycle.director.execution.is_some());
        let dispatch = if !execution_started {
            "not_started"
        } else if institution_local {
            "institution_local"
        } else {
            "dry_run"
        };
        let simulation_only = !institution_local || !execution_started;
        serde_json::to_value(json!({
            "engine": engine,
            "dispatch": dispatch,
            "simulation_only": simulation_only,
            "guarantees": [
                "the engine compiles the high-level glioma intent into the closed dependency graph before each cycle",
                "only returned typed local artifacts become downstream checkpoints; stale or missing artifacts cannot unlock work",
                "each cycle preserves selected actions, execution outcomes, negative evidence, uncertainty, budget, and policy holds",
                "bounded retries, instrument and federation permissions, and preclinical data-locality constraints remain active",
                if institution_local {
                    "a configured institution worker receives only typed admitted actions; its authority remains caller-owned"
                } else {
                    "the default MCP route uses synthetic local artifacts and performs no biological, instrument, or external effect"
                }
            ]
        }))
        .map_err(|error| format!("cannot encode glioma autonomous research engine run: {error}"))
    }

    /// Run the autonomous engine and return a stage-gated program handoff for a researcher or
    /// operator. The sandbox executor remains metadata-only and local.
    pub(super) fn glioma_autonomous_program_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AutonomousProgramCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_autonomous_program_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma autonomous program-cycle request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let cycle = execute_glioma_autonomous_program_cycle(&request, &mut executor)
            .map_err(|error| format!("glioma autonomous program cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "all fourteen preclinical stages receive explicit cleared, active, held, approval, blocked, or pending gates",
                "active work is derived from the latest engine frontier rather than historical actions",
                "progress, negative evidence, uncertainty, budget, authority, locality, and operator handoff remain typed and replayable",
                "the route performs no real assay, instrument effect, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma autonomous program cycle: {error}"))
    }

    /// Plan the next bounded autonomous glioma workflow batch from typed prior outcomes. The
    /// scheduler is planning-only: production callers own execution through the existing action
    /// executor and instrument/computation gates.
    pub(super) fn glioma_adaptive_workflow(&self, arguments: &Value) -> Result<Value, String> {
        let request: GliomaAdaptiveWorkflowSchedulerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_workflow requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive workflow request: {error}"))?;
        let plan = plan_glioma_adaptive_workflow(&request)
            .map_err(|error| format!("glioma adaptive workflow refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "qualified, negative, inconclusive, failed, and blocked observations update utility without erasing negative evidence",
                "selected actions are dependency-closed in executable order under cost, risk, authority, instrument, federation, and action-count budgets",
                "blocked and deferred actions carry explicit reasons for later replanning",
                "the route performs no assay, instrument effect, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive workflow plan: {error}"))
    }

    /// Convert a typed cross-family interpretation into a bounded next-action frontier.  The
    /// route performs no assay or instrument effect; an institution-local caller owns execution
    /// after reviewing the selected actions and any unresolved holds.
    pub(super) fn glioma_adaptive_research_frontier(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveFrontierRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_research_frontier requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive frontier request: {error}"))?;
        let frontier = plan_glioma_adaptive_research_frontier(&request)
            .map_err(|error| format!("glioma adaptive frontier refused: {error}"))?;
        serde_json::to_value(json!({
            "frontier": frontier,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "typed interpretation outcomes determine which uncertainty, contradiction, replication, and negative-result debts receive next-action capacity",
                "candidate ranking reuses the bounded, deterministic glioma action selector with explicit budget and authority gates",
                "unresolved and negative synthesis states remain holds or partial outcomes rather than being promoted",
                "MCP executes no assay, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive frontier: {error}"))
    }

    /// Compile and execute the selected P10 adaptive frontier through the deterministic local
    /// action worker. Unresolved synthesis remains held unless the caller explicitly permits
    /// bounded local dispatch; approval, instrument, federation, dependency, retry, and artifact
    /// gates remain enforced by the action portfolio executor.
    pub(super) fn glioma_adaptive_frontier_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveFrontierExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_frontier_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive frontier execution request: {error}"))?;
        let mut executor = dry_run_glioma_adaptive_frontier_executor();
        let output = execute_glioma_adaptive_frontier(&request, &mut executor)
            .map_err(|error| format!("glioma adaptive frontier execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": output,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the typed interpretation frontier is recompiled before execution and selection cannot drift",
                "unresolved synthesis is held unless the caller explicitly permits bounded local dispatch",
                "dependency order, approval, effect, retry, artifact, negative, partial, and failure states remain explicit",
                "the MCP route uses synthetic local artifacts and performs no biological, instrument, network, or clinical effect",
                "institution-local workers can replace the dry-run executor through the Rust SDK seam"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive frontier execution: {error}"))
    }

    /// Run bounded synthesis -> adaptive frontier -> local execution rounds. The MCP worker and
    /// planner are deliberately synthetic; institution-local deployments replace both seams to
    /// return validated preclinical artifacts for the next synthesis round.
    pub(super) fn glioma_adaptive_interpretation_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveInterpretationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_interpretation_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma adaptive interpretation campaign request: {error}")
            })?;
        let campaign = execute_glioma_adaptive_interpretation_campaign_dry_run(&request)
            .map_err(|error| format!("glioma adaptive interpretation campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "each round resynthesizes only the typed evidence request supplied by the caller",
                "the adaptive frontier is recompiled and selection-bound before every local action batch",
                "dry-run artifacts are retained as limitations and cannot become biological evidence",
                "budget, approval, effect, artifact, negative, partial, failure, hold, and no-progress stops remain explicit",
                "institution-local planner and executor seams can feed validated aggregate artifacts into later rounds"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive interpretation campaign: {error}"))
    }

    /// Stress-test a local two-arm glioma analysis under deterministic batch and row omissions.
    /// The suite never promotes an unresolved omission to support and never treats a null result
    /// as a failed computation; it is a bounded scientific robustness report, not execution.
    pub(super) fn glioma_robustness_suite(&self, arguments: &Value) -> Result<Value, String> {
        let request: RobustnessRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_robustness_suite requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma robustness request: {error}"))?;
        let dataset: AnalysisDataset = serde_json::from_value(
            arguments
                .get("dataset")
                .cloned()
                .ok_or_else(|| "glioma_robustness_suite requires dataset".to_string())?,
        )
        .map_err(|error| format!("invalid glioma robustness dataset: {error}"))?;
        let output = assess_glioma_robustness(&request, &dataset)
            .map_err(|error| format!("glioma robustness assessment refused: {error}"))?;
        serde_json::to_value(json!({
            "suite": output,
            "dispatch": "not_started",
            "guarantees": [
                "leave-one-batch and optional leave-one-row analyses use the declared estimand",
                "unresolved omissions never count as supporting evidence",
                "null and negative outcomes remain first-class scientific results",
                "the bounded battery is deterministic and digest-bound",
                "raw data remains local and no clinical decision is produced"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma robustness suite: {error}"))
    }
}
