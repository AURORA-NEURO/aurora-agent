//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    pub(super) fn glioma_experiment_design(&self, arguments: &Value) -> Result<Value, String> {
        let request: ExperimentRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_experiment_design requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma experiment request: {error}"))?;
        let arms: Vec<ExperimentArm> = serde_json::from_value(
            arguments
                .get("arms")
                .cloned()
                .ok_or_else(|| "glioma_experiment_design requires arms".to_string())?,
        )
        .map_err(|error| format!("invalid glioma experiment arms: {error}"))?;
        let output = design_preclinical_experiment(&request, &arms)
            .map_err(|error| format!("glioma experiment design refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma experiment design: {error}"))
    }

    /// Compile a balanced, multi-factor preclinical glioma contrast panel. This is a design-only
    /// route: it never randomizes live material, executes an assay, or reports formal power.
    pub(super) fn glioma_contrast_panel_design(&self, arguments: &Value) -> Result<Value, String> {
        let request: ContrastDesignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_contrast_panel_design requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma contrast design request: {error}"))?;
        let design = design_glioma_contrast_panel(&request)
            .map_err(|error| format!("glioma contrast panel design refused: {error}"))?;
        serde_json::to_value(json!({
            "design": design,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "factorial conditions and main-effect estimands are deterministic and balanced when admitted",
                "required interaction coverage, replicate floors, budget blocks, and design limitations remain explicit",
                "design adequacy is a bounded balance/replicate proxy and is not a formal power calculation",
                "the route does not randomize material, execute an assay, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma contrast panel design: {error}"))
    }

    pub(super) fn glioma_analysis_run(&self, arguments: &Value) -> Result<Value, String> {
        let request: AnalysisRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_analysis_run requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma analysis request: {error}"))?;
        let dataset: AnalysisDataset = serde_json::from_value(
            arguments
                .get("dataset")
                .cloned()
                .ok_or_else(|| "glioma_analysis_run requires dataset".to_string())?,
        )
        .map_err(|error| format!("invalid glioma analysis dataset: {error}"))?;
        let output = analyze_preclinical_outcomes(&request, &dataset)
            .map_err(|error| format!("glioma analysis refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma analysis result: {error}"))
    }

    pub(super) fn glioma_replication_assess(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReplicationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_replication_assess requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication request: {error}"))?;
        let studies: Vec<ReplicationStudy> = serde_json::from_value(
            arguments
                .get("studies")
                .cloned()
                .ok_or_else(|| "glioma_replication_assess requires studies".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication studies: {error}"))?;
        let output = assess_replication(&request, &studies)
            .map_err(|error| format!("glioma replication assessment refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma replication assessment: {error}"))
    }

    /// Pool independent preclinical glioma studies with fixed-point inverse-uncertainty and
    /// random-effects weights.
    /// Heterogeneity, contradictory directions, and influential sites remain visible and never
    /// become a clinical or treatment recommendation.
    pub(super) fn glioma_replication_meta_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MetaAnalysisRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_replication_meta_analyze requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication meta-analysis request: {error}"))?;
        let studies: Vec<ReplicationStudy> = serde_json::from_value(
            arguments
                .get("studies")
                .cloned()
                .ok_or_else(|| "glioma_replication_meta_analyze requires studies".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication meta-analysis studies: {error}"))?;
        let output = analyze_replication_meta_analysis(&request, &studies)
            .map_err(|error| format!("glioma replication meta-analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "fixed-effect and deterministic random-effects inverse-uncertainty pools are reported",
                "between-study variance, Cochran heterogeneity, I2, and leave-one-study-out influence are reported",
                "underpowered, contradictory, heterogeneous, and signal-poor results remain explicit",
                "the output is preclinical replication evidence, not a clinical recommendation"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication meta-analysis: {error}"))
    }

    /// Run the bounded P10 replication/interpretation loop with the simulation-only local
    /// executor. Institution hosts can call the same Rust contract with a real local executor;
    /// this MCP route never moves raw data or promotes synthetic observations into evidence.
    pub(super) fn glioma_replication_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaReplicationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma replication campaign request: {error}"))?;
        let mut executor = DryRunGliomaReplicationCampaignExecutor::default();
        let campaign = execute_glioma_replication_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma replication campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "replication, meta-analysis, and transportability are recomputed from typed local study objects each round",
                "heterogeneity, influential studies, missing coverage, model distance, negative evidence, and unresolved states remain explicit",
                "hard action, retry, budget, target-model, and no-progress gates remain active",
                "simulation observations are never admitted as biological evidence",
                "the MCP route performs no external computation, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication campaign: {error}"))
    }

    /// Run the science-aware cross-program glioma mission with the deterministic local executor.
    /// Real institutions can call the same research-crate function with an approved local
    /// executor; this MCP route never touches instruments, moves raw data, or promotes synthetic
    /// outcomes into biological evidence.
    pub(super) fn glioma_autonomous_research_mission_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaMissionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_autonomous_research_mission_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma autonomous mission request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let mission = execute_glioma_autonomous_research_mission(&request, &mut executor)
            .map_err(|error| format!("glioma autonomous mission refused: {error}"))?;
        serde_json::to_value(json!({
            "mission": mission,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "stage, model-system, modality, information-gain, and uncertainty gates are explicit",
                "each round is replanned only from typed local action outcomes",
                "negative results remain first-class and do not satisfy positive qualification gates",
                "failed, partial, blocked, budget, and no-progress states stop the mission honestly",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma autonomous research mission: {error}"))
    }

    /// Recover a failed or partial autonomous glioma mission by closing the failed dependency
    /// cone and running a separate bounded mission over alternate typed actions. MCP remains a
    /// dry-run rehearsal; institutions provide the production-local executor.
    pub(super) fn glioma_autonomous_research_mission_recover(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaMissionRecoveryRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_autonomous_research_mission_recover requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mission recovery request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let campaign = execute_glioma_mission_recovery(&request, &mut executor)
            .map_err(|error| format!("glioma mission recovery refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "failed, partial, and skipped action frontiers are closed through their dependency cones before recovery",
                "initial and recovery missions remain separately addressable and replayable",
                "alternate candidates are selected under a fresh bounded budget and never silently replay invalidated actions",
                "negative evidence and uncertainty from both attempts remain first-class",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mission recovery: {error}"))
    }

    /// Compile a research intent into a complete typed glioma mission and run the bounded local
    /// controller. Missing inputs and approvals are returned as held plans without dispatch.
    pub(super) fn glioma_intent_mission_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request: GliomaIntentMissionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_intent_mission_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma intent mission request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let campaign = execute_glioma_intent_mission(&request, &mut executor)
            .map_err(|error| format!("glioma intent mission refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the typed intent is compiled into a dependency-safe action graph across evidence, multimodal QC, mechanism, experiment, computation, interpretation, replication, and release stages",
                "representative modality and preclinical model-system bindings are deterministic and remain visible in every action candidate",
                "missing inputs, approval requirements, locality violations, and disabled optional stages hold or block the plan before dispatch",
                "initial execution and failed-frontier recovery use separate bounded budgets and retain negative evidence",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma intent mission: {error}"))
    }

    /// Expand an admitted intent into a bounded multimodal/model-system action portfolio and run
    /// it through the local mission controller. The MCP worker remains simulation-only.
    pub(super) fn glioma_multimodal_mission_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaMultimodalMissionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_mission_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal mission request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let campaign = execute_glioma_multimodal_mission(&request, &mut executor)
            .map_err(|error| format!("glioma multimodal mission refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "each admitted research stage expands into deterministic modality/model-system variants under a bounded portfolio limit",
                "every variant retains canonical dependency handoffs and cannot bypass upstream stage qualification",
                "mission selection can trade information gain against cross-modal coverage, model diversity, cost, and reproducibility",
                "missing inputs, approvals, locality violations, negative evidence, and recovery frontiers remain explicit",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal mission: {error}"))
    }

    /// Run the P06 closed-loop multi-fidelity optimizer with a deterministic local worker. The
    /// MCP adapter is rehearsal-only; institutions provide the production assay or analysis
    /// executor through the research crate's explicit trait.
    pub(super) fn glioma_multi_fidelity_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiFidelityCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multi_fidelity_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multi-fidelity campaign request: {error}"))?;
        let mut executor = DryRunMultiFidelityCampaignExecutor;
        let campaign = execute_glioma_multi_fidelity_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma multi-fidelity campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "screening, mechanistic, and validation candidates retain explicit fidelity and parent-support gates",
                "transfer, neighbourhood, prior, and observed estimates are never conflated",
                "each round replans only from returned typed local observations",
                "duplicate replicate keys, missing artifacts, retries, worker failures, and budget stops remain explicit",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-fidelity campaign: {error}"))
    }

    /// Compare aggregate benchmark outcomes from independent preclinical sites. Raw traces stay
    /// at the institutions; this route computes robust pooled effects and blocks unstable
    /// federation conclusions.
    pub(super) fn glioma_federated_benchmark_consensus(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBenchmarkRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_benchmark_consensus requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma federated benchmark request: {error}"))?;
        let sites: Vec<FederatedBenchmarkSite> =
            serde_json::from_value(arguments.get("sites").cloned().ok_or_else(|| {
                "glioma_federated_benchmark_consensus requires sites".to_string()
            })?)
            .map_err(|error| format!("invalid glioma federated benchmark sites: {error}"))?;
        let output = analyze_federated_benchmark(&request, &sites)
            .map_err(|error| format!("glioma federated benchmark consensus refused: {error}"))?;
        serde_json::to_value(json!({
            "consensus": output,
            "dispatch": "not_started",
            "guarantees": [
                "only aggregate site scores, baselines, uncertainty, and replicate counts cross the federation boundary",
                "fixed-point inverse-uncertainty pooling, weighted median, heterogeneity, and leave-one-site-out influence are deterministic",
                "underpowered, contradictory, heterogeneous, negative, and site-sensitive outcomes remain explicit",
                "the output is a preclinical benchmark comparison, not a clinical recommendation or instrument command"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated benchmark consensus: {error}"))
    }

    /// Estimate whether aggregate-only site summaries have enough information for a benchmark
    /// interpretation. This is a sufficiency gate, not a clinical or operational decision.
    pub(super) fn glioma_federated_benchmark_power(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBenchmarkPowerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_benchmark_power requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated benchmark power request: {error}"))?;
        let sites: Vec<FederatedBenchmarkSite> = serde_json::from_value(
            arguments
                .get("sites")
                .cloned()
                .ok_or_else(|| "glioma_federated_benchmark_power requires sites".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated benchmark power sites: {error}"))?;
        let output = analyze_federated_benchmark_power(&request, &sites)
            .map_err(|error| format!("glioma federated benchmark power refused: {error}"))?;
        serde_json::to_value(json!({
            "power": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_federated_benchmark_site_plan",
                "glioma_federated_benchmark_consensus",
                "glioma_federated_benchmark_campaign_execute"
            ],
            "guarantees": [
                "only aggregate site scores, uncertainty, replicate counts, and artifact boundaries cross this route",
                "inverse-uncertainty information, conservative signal-to-noise, power proxy, heterogeneity, and leave-one-site-out influence are deterministic",
                "underpowered, heterogeneous, influence-dominated, and binding-mismatched sites remain explicit evidence gaps",
                "the route performs no site dispatch, raw-data movement, instrument action, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated benchmark power: {error}"))
    }

    /// Join a local P10 closure interpretation with aggregate-only P12 consortium consensus.
    /// Alignment is required for qualification; heterogeneity, negative evidence, and model
    /// disagreement remain explicit and no raw data or physical effect crosses MCP.
    pub(super) fn glioma_federated_interpretation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedInterpretationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_interpretation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated interpretation request: {error}"))?;
        let output = interpret_glioma_federated_closure(&request)
            .map_err(|error| format!("glioma federated interpretation refused: {error}"))?;
        serde_json::to_value(json!({
            "interpretation": output,
            "dispatch": "not_started",
            "simulation_only": false,
            "physical_dispatch": false,
            "next_routes": [
                "glioma_federated_benchmark_site_plan",
                "glioma_federated_benchmark_campaign_execute",
                "glioma_replication_closure_frontier",
                "glioma_research_object_prepare"
            ],
            "guarantees": [
                "only aggregate benchmark summaries and a validated local interpretation enter the gate",
                "qualification requires both local interpretation and cross-site consensus alignment",
                "heterogeneous, negative, underpowered, and unresolved outcomes remain explicit",
                "the route never moves raw observations, executes an instrument, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated interpretation: {error}"))
    }

    /// Plan an aggregate-only consortium expansion without dispatching a site or moving raw data.
    pub(super) fn glioma_federated_benchmark_site_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBenchmarkSitePlannerRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_benchmark_site_plan requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated benchmark site-plan request: {error}")
            })?;
        let plan = plan_federated_benchmark_sites(&request)
            .map_err(|error| format!("glioma federated benchmark site plan refused: {error}"))?;
        serde_json::to_value(json!({
            "site_plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "candidate site projections are conservative scenarios, never observations",
                "the planner reuses robust consensus, heterogeneity, and leave-one-site-out influence gates",
                "budget, privacy-risk, replicate, independent-study, and candidate bounds are explicit",
                "only aggregate scores and local artifact references are modeled; no raw data, assay, or clinical decision is moved"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated benchmark site plan: {error}"))
    }

    /// Analyze aggregate mechanism effects across federated preclinical model systems. This is a
    /// value-only scientific transport analysis; no raw observation, assay, or clinical action is
    /// dispatched by MCP.
    pub(super) fn glioma_federated_mechanism_transport(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedMechanismTransportRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_mechanism_transport requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated mechanism transport request: {error}")
            })?;
        let sites: Vec<FederatedMechanismSite> =
            serde_json::from_value(arguments.get("sites").cloned().ok_or_else(|| {
                "glioma_federated_mechanism_transport requires sites".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated mechanism transport sites: {error}")
            })?;
        let analysis = analyze_federated_mechanism_transport(&request, &sites)
            .map_err(|error| format!("glioma federated mechanism transport refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "only aggregate mechanism effects, quality, replicate counts, signatures, and local artifact references cross the federation boundary",
                "model-system coverage, similarity weighting, direction conflict, heterogeneity, and leave-one-site-out fragility remain explicit",
                "underpowered, distant, low-quality, negative, partial, and unresolved sites are never imputed or silently excluded",
                "the output is a preclinical transport analysis, not a clinical recommendation, treatment claim, or instrument command"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated mechanism transport: {error}"))
    }

    /// Run a bounded adaptive aggregate-only mechanism transport campaign. The MCP route uses
    /// the deterministic local sandbox; institution-owned executors remain outside the server.
    pub(super) fn glioma_federated_mechanism_transport_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedMechanismTransportCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_mechanism_transport_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated mechanism transport campaign request: {error}")
            })?;
        let campaign =
            execute_federated_mechanism_transport_campaign_dry_run(&request).map_err(|error| {
                format!("glioma federated mechanism transport campaign refused: {error}")
            })?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "only local, aggregate-only federated mechanism summaries are accepted",
                "transport analysis is recomputed after every successful follow-up site",
                "heterogeneous, negative, underpowered, failed, retry, budget, and no-progress outcomes remain explicit",
                "the MCP route moves no raw traces, human data, instrument effect, or clinical decision"
            ]
        }))
        .map_err(|error| {
            format!(
                "cannot encode glioma federated mechanism transport campaign: {error}"
            )
        })
    }

    /// Run a bounded autonomous federated benchmark campaign. Only typed aggregate sites are
    /// returned by the synthetic worker; institutions retain raw traces and execution authority.
    pub(super) fn glioma_federated_benchmark_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBenchmarkCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_benchmark_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated benchmark campaign request: {error}")
            })?;
        let mut executor = DryRunFederatedBenchmarkCampaignExecutor;
        let campaign = execute_federated_benchmark_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma federated benchmark campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the controller replans from deterministic aggregate consensus after each returned site",
                "only local artifact references, scores, uncertainty, and replicate counts cross the federation boundary",
                "heterogeneous, negative, underpowered, budget-blocked, failed, and unresolved states remain explicit",
                "duplicate identities, unbound benchmark results, retries, and no-progress stops are rejected",
                "the MCP route performs no raw-data movement, clinical decision, or instrument execution"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated benchmark campaign: {error}"))
    }

    /// Run the aggregate-only federated benchmark operating cycle in the deterministic local
    /// sandbox. Governed-local execution remains behind an institution-owned executor.
    pub(super) fn glioma_federated_benchmark_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBenchmarkOperatingCycleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_benchmark_operating_cycle requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated benchmark operating-cycle request: {error}")
            })?;
        if matches!(
            request.execution_mode,
            FederatedBenchmarkExecutionMode::GovernedLocal
        ) {
            return Err(
                "glioma_federated_benchmark_operating_cycle MCP route is simulation-only; governed_local requires an institution-owned aggregate executor"
                    .to_string(),
            );
        }
        let cycle =
            execute_federated_benchmark_operating_cycle_dry_run(&request).map_err(|error| {
                format!("glioma federated benchmark operating cycle refused: {error}")
            })?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "aggregate-only boundary and site identity checks run before the campaign",
                "cross-site consensus is recomputed after each bounded follow-up action",
                "qualified, negative, heterogeneous, partial, blocked, and unresolved states remain explicit",
                "MCP moves no raw traces, executes no instruments, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated benchmark operating cycle: {error}"))
    }

    /// Compile a conservative federated site projection directly into an aggregate-only
    /// benchmark campaign. The MCP worker remains synthetic; institutions own execution.
    pub(super) fn glioma_federated_adaptive_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBenchmarkAdaptiveCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_adaptive_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated adaptive campaign request: {error}")
            })?;
        let campaign = execute_federated_benchmark_adaptive_campaign_dry_run(&request)
            .map_err(|error| format!("glioma federated adaptive campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "planner projections remain separate from observed aggregate campaign results",
                "only local-only non-human non-identifying aggregates cross the federation boundary",
                "the selected site portfolio is executed through a caller-owned aggregate-only seam",
                "heterogeneous, negative, partial, blocked, and no-admissible-plan states remain explicit",
                "the MCP route performs no raw-data movement, instrument effect, network effect, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated adaptive campaign: {error}"))
    }

    /// Execute a local reproducibility replay campaign for a preclinical glioma release
    /// candidate. The MCP worker is synthetic; production hosts provide the replay executor.
    pub(super) fn glioma_replay_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplayCampaignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_replay_campaign_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replay campaign request: {error}"))?;
        let mut executor = DryRunReplayCampaignExecutor;
        let campaign = execute_glioma_replay_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma replay campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "replay tasks execute only after declared dependencies match",
                "content-hash mismatches, unavailable tasks, budget stops, and blocked dependants remain explicit",
                "non-deterministic tasks are never fabricated as reproducible by the synthetic worker",
                "raw experimental data and production compute credentials remain institution-local",
                "the output is release-readiness evidence, not an unsigned publication or clinical conclusion"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replay campaign: {error}"))
    }

    /// Reconcile validated archival replay reports across sites and epochs without rerunning work.
    pub(super) fn glioma_replay_history_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplayHistoryRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_replay_history_reconcile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replay history request: {error}"))?;
        let report = reconcile_glioma_replay_history(&request)
            .map_err(|error| format!("glioma replay history reconciliation refused: {error}"))?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "aggregate_only_archival_reconciliation",
            "simulation_only": false,
            "guarantees": [
                "only already validated campaign summaries are reconciled",
                "the output contains digests, opaque site commitments, counts, and explicit evidence states",
                "missing sites, divergent outputs, and unresolved campaigns are never promoted to reproducible",
                "this route does not retrieve archives, execute code, authenticate sites, move raw data, or make clinical decisions"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replay history report: {error}"))
    }

    /// Run the local single-study release workflow with the MCP-owned dry-run replay executor.
    pub(super) fn glioma_local_release_workflow_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaLocalReleaseWorkflowRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_local_release_workflow_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma local release workflow request: {error}"))?;
        let workflow = execute_glioma_local_release_workflow_dry_run(&request)
            .map_err(|error| format!("glioma local release workflow refused: {error}"))?;
        serde_json::to_value(json!({
            "workflow": workflow,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "multimodal packaging and dependency closure run before synthetic replay",
                "the replay executor is never invoked when packaging or dependency closure is partial or blocked",
                "release-gate output preserves unresolved, divergent, and human-review requirements",
                "the route never signs, publishes, uploads, moves raw data, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma local release workflow: {error}"))
    }

    /// Reconcile per-study release workflows while keeping their packages and evidence separate.
    pub(super) fn glioma_multistudy_release_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiStudyReleaseRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multistudy_release_reconcile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multi-study release request: {error}"))?;
        let report = reconcile_glioma_multistudy_release(&request).map_err(|error| {
            format!("glioma multi-study release reconciliation refused: {error}")
        })?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "aggregate_only_multi_study_reconciliation",
            "simulation_only": false,
            "guarantees": [
                "every expected study remains separately content-addressed",
                "missing, partial, unresolved, blocked, and non-reproducible study states remain explicit",
                "portfolio readiness requires every expected study to be ready for its own signing review",
                "the route does not merge artifacts, authenticate study or reviewer identities, sign, publish, or move raw data"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study release report: {error}"))
    }

    /// Execute a bounded release queue with the MCP-owned deterministic replay worker.
    pub(super) fn glioma_release_batch_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReleaseBatchRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_batch_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release batch request: {error}"))?;
        let mut executor = DryRunReplayCampaignExecutor;
        let report = execute_glioma_release_batch(&request, &mut executor)
            .map_err(|error| format!("glioma release batch refused: {error}"))?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "bounded_priority_queue_dry_run",
            "simulation_only": true,
            "guarantees": [
                "every item is preflighted before the first synthetic replay call",
                "queue order and the shared batch budget are deterministic and bounded",
                "budget-deferred workflows remain distinct from blocked package/dependency workflows",
                "an execution failure with unknown resource use stops later dispatch",
                "the route performs no network, instrument, signing, publication, or raw-data effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma release batch report: {error}"))
    }

    /// Reconcile portfolio snapshots and change-review evidence across federation epochs.
    pub(super) fn glioma_federated_continual_release_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedContinualReleaseRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_continual_release_reconcile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid federated continual release request: {error}"))?;
        let report = reconcile_glioma_federated_continual_release(&request).map_err(|error| {
            format!("federated continual release reconciliation refused: {error}")
        })?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "aggregate_only_federated_continual_release_reconciliation",
            "simulation_only": false,
            "guarantees": [
                "successive snapshots must retain one ordered expected cohort and research identity",
                "manifest changes are bound to explicit per-study review evidence and an independent approval quorum",
                "missing epochs, studies, continuity, and change approvals remain visible and block readiness",
                "the report explicitly marks site and reviewer commitments unauthenticated and signing/publication as not performed",
                "site and reviewer commitments are opaque caller inputs that are not authenticated by this route",
                "the route does not sign, publish, upload, move raw data, contact sites, or make clinical decisions"
            ]
        }))
        .map_err(|error| format!("cannot encode federated continual release report: {error}"))
    }

    /// Compile a local stewardship checklist without authorizing a release or signing action.
    pub(super) fn glioma_local_release_review_packet_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LocalReleaseReviewPacketRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_local_release_review_packet_compile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid local release review packet request: {error}"))?;
        let packet = compile_glioma_local_release_review_packet(&request)
            .map_err(|error| format!("local release review packet refused: {error}"))?;
        serde_json::to_value(json!({
            "packet": packet,
            "dispatch": "digest_bound_local_human_review_checklist",
            "simulation_only": false,
            "guarantees": [
                "the checklist is fixed, ordered, and bound to one validated P11-F13 workflow digest",
                "omitted evidence stays unavailable and omitted decisions stay pending",
                "responses cannot acknowledge evidence that is unavailable",
                "reviewer identity and independence are caller assertions and are not authenticated",
                "checklist completion does not satisfy P11-F20 approval, authorize release, sign, or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode local release review packet: {error}"))
    }

    /// Reconcile independent local review packets against one exact expected study cohort.
    pub(super) fn glioma_portfolio_review_workbench_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: PortfolioReviewWorkbenchRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_portfolio_review_workbench_reconcile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid portfolio review workbench request: {error}"))?;
        let report = reconcile_glioma_portfolio_review_workbench(&request)
            .map_err(|error| format!("portfolio review workbench refused: {error}"))?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "aggregate_only_multi_study_review_workbench",
            "simulation_only": false,
            "guarantees": [
                "each P11-F17 packet is bound to its exact P11-F14 workflow digest and disposition",
                "the full expected cohort remains in the report, including missing studies and packets",
                "one study's completed checklist cannot promote another study's release state",
                "study and reviewer commitments are unauthenticated caller inputs",
                "portfolio checklist completion does not satisfy signing quorum, authorize release, sign, or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode portfolio review workbench report: {error}"))
    }

    /// Attach local review packets to a deterministic prospective release queue.
    pub(super) fn glioma_release_batch_review_workbench_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseBatchReviewWorkbenchRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_release_batch_review_workbench_reconcile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid release batch review request: {error}"))?;
        let report = reconcile_glioma_release_batch_review_workbench(&request)
            .map_err(|error| format!("release batch review workbench refused: {error}"))?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "bounded_prospective_human_review_queue",
            "simulation_only": false,
            "guarantees": [
                "the P11-F15 queue order and every deferred or failed job are preserved",
                "each P11-F17 packet must match one produced job's exact workflow digest and disposition",
                "missing packets remain pending and cannot be promoted from another job's review",
                "reviewer commitments are unauthenticated and review completion does not authorize release",
                "the route does not sign, publish, upload, move raw data, or contact sites"
            ]
        }))
        .map_err(|error| format!("cannot encode release batch review report: {error}"))
    }

    /// Prepare canonical bytes for an institution-controlled local release signature.
    pub(super) fn glioma_local_release_signature_payload_prepare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let context: LocalReleaseSignatureContextRequest =
            serde_json::from_value(arguments.get("context").cloned().ok_or_else(|| {
                "glioma_local_release_signature_payload_prepare requires context".to_string()
            })?)
            .map_err(|error| format!("invalid local release signature context: {error}"))?;
        let prepared = prepare_glioma_local_release_signature_payload(&context)
            .map_err(|error| format!("local release signing payload refused: {error}"))?;
        serde_json::to_value(json!({
            "prepared": prepared,
            "dispatch": "canonical_ed25519_signing_payload_preparation",
            "private_key_received": false,
            "guarantees": [
                "payload bytes bind the ready workflow, completed checklist, release gate, signer key ID, public-key digest, and caller challenge",
                "only a ready local workflow with a completed human checklist can enter payload preparation",
                "the route does not create a signature, authenticate signer identity or key authority, prevent replay, authorize release, or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode prepared release signing payload: {error}"))
    }

    /// Verify an externally produced Ed25519 signature without claiming trust in its public key.
    pub(super) fn glioma_local_release_signature_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LocalReleaseSignatureVerificationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_local_release_signature_verify requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid local release signature verification request: {error}")
            })?;
        let verification = verify_glioma_local_release_signature(&request)
            .map_err(|error| format!("local release signature verification refused: {error}"))?;
        serde_json::to_value(json!({
            "verification": verification,
            "dispatch": "detached_ed25519_signature_verification",
            "guarantees": [
                "the detached signature is verified over the canonical P11-F21 payload",
                "workflow, review packet, gate, manifest, key ID, public key, and challenge are digest-bound",
                "a valid signature proves mathematical key possession for these bytes only; key authority and signer identity are not authenticated",
                "the verifier does not check challenge reuse, authorize release, receive private keys, sign, or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode local release signature verification: {error}"))
    }

    /// Prepare exact signed trust-policy bytes for an institution-controlled root signer.
    pub(super) fn glioma_release_trust_policy_payload_prepare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let policy: ReleaseTrustPolicy =
            serde_json::from_value(arguments.get("policy").cloned().ok_or_else(|| {
                "glioma_release_trust_policy_payload_prepare requires policy".to_string()
            })?)
            .map_err(|error| format!("invalid release trust policy: {error}"))?;
        let prepared = prepare_glioma_release_trust_policy_payload(&policy)
            .map_err(|error| format!("release trust policy refused: {error}"))?;
        serde_json::to_value(json!({
            "prepared": prepared,
            "dispatch": "institutional_release_trust_policy_payload_preparation",
            "private_key_received": false,
            "guarantees": [
                "the exact bounded policy bytes bind ordered signer-key grants, research scope, approval purpose, validity interval, and revocation time",
                "the policy authority must sign these bytes outside this service",
                "the route does not establish root trust or authorize release"
            ]
        }))
        .map_err(|error| format!("cannot encode prepared trust policy payload: {error}"))
    }

    /// Verify the local trust-root pin, signed key policy, and project-scoped signer grant.
    pub(super) fn glioma_release_trust_policy_evaluate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseTrustPolicyEvaluationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_release_trust_policy_evaluate requires request".to_string()
            })?)
            .map_err(|error| format!("invalid release trust policy evaluation request: {error}"))?;
        let decision = evaluate_glioma_release_trust_policy(&request)
            .map_err(|error| format!("release trust policy evaluation refused: {error}"))?;
        serde_json::to_value(json!({
            "decision": decision,
            "dispatch": "pinned_root_signed_release_key_policy_evaluation",
            "guarantees": [
                "the F21 signature record is revalidated before trust evaluation",
                "the policy signature is verified against the caller-configured root key and expected key digest",
                "only a current exact-key, exact-research, exact-purpose grant is trusted for release-approval review",
                "the trust pin remains operator-supplied; this route does not check challenge replay, authorize release, or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode release signer trust decision: {error}"))
    }

    pub(super) fn glioma_research_object_prepare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ResearchObjectRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_research_object_prepare requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma research-object request: {error}"))?;
        let output = build_research_object_manifest(&request)
            .map_err(|error| format!("glioma research-object preparation refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma research-object manifest: {error}"))
    }

    /// Compile a digest-bound inventory of every negative result and limitation in a
    /// validated local release manifest. The route makes no claim about scientific truth or
    /// release authorization and does not repeat disclosure text in its output.
    pub(super) fn glioma_release_disclosure_register_build(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let manifest: ResearchObjectManifest =
            serde_json::from_value(arguments.get("manifest").cloned().ok_or_else(|| {
                "glioma_release_disclosure_register_build requires manifest".to_string()
            })?)
            .map_err(|error| format!("invalid research-object manifest: {error}"))?;
        let register = compile_glioma_release_disclosure_register(&manifest)
            .map_err(|error| format!("release disclosure register refused: {error}"))?;
        serde_json::to_value(json!({
            "register": register,
            "dispatch": "not_started",
            "guarantees": [
                "every manifest negative-result and limitation statement has a category-bound digest commitment",
                "disclosure text is not copied into this register",
                "statement digests are unkeyed and do not provide confidentiality",
                "the register does not verify scientific truth, authorize release, sign, or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode release disclosure register: {error}"))
    }

    /// Reconcile validated study-level disclosure registers against an exact expected cohort.
    /// Missing studies stay visible, and no cross-study disclosure counts are pooled.
    pub(super) fn glioma_release_disclosure_panel_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseDisclosurePanelRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_release_disclosure_panel_reconcile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid release disclosure panel request: {error}"))?;
        let studies: Vec<ReleaseDisclosureStudyInput> =
            serde_json::from_value(arguments.get("studies").cloned().ok_or_else(|| {
                "glioma_release_disclosure_panel_reconcile requires studies".to_string()
            })?)
            .map_err(|error| format!("invalid release disclosure study inputs: {error}"))?;
        let panel = reconcile_glioma_release_disclosure_panel(&request, &studies)
            .map_err(|error| format!("release disclosure panel refused: {error}"))?;
        serde_json::to_value(json!({
            "panel": panel,
            "dispatch": "not_started",
            "guarantees": [
                "every observed F01 manifest and F02 register is revalidated and matched to one exact expected commitment",
                "missing expected studies remain explicit and each study retains its own disclosure counts",
                "disclosure counts are not pooled, statement text is not returned, and independence declarations remain unauthenticated",
                "the panel verifies neither scientific truth nor release authorization and does not sign or publish"
            ]
        }))
        .map_err(|error| format!("cannot encode release disclosure panel: {error}"))
    }

    /// Reconcile a predeclared, priority-ordered queue of P11-F03 disclosure panels.
    pub(super) fn glioma_release_disclosure_batch_reconcile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseDisclosureBatchRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_release_disclosure_batch_reconcile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid release disclosure batch request: {error}"))?;
        let panels: Vec<ReleaseDisclosureBatchInput> =
            serde_json::from_value(arguments.get("panels").cloned().ok_or_else(|| {
                "glioma_release_disclosure_batch_reconcile requires panels".to_string()
            })?)
            .map_err(|error| format!("invalid release disclosure batch panels: {error}"))?;
        let batch = reconcile_glioma_release_disclosure_batch(&request, &panels)
            .map_err(|error| format!("release disclosure batch refused: {error}"))?;
        serde_json::to_value(json!({
            "batch": batch,
            "dispatch": "not_started",
            "guarantees": [
                "every submitted panel is revalidated against one predeclared item, research identity, and request digest",
                "priority ordering and awaiting submissions are explicit; per-study disclosure counts are not pooled",
                "the batch starts no execution, includes no disclosure text, and establishes no independence, scientific truth, or release authorization"
            ]
        }))
        .map_err(|error| format!("cannot encode release disclosure batch: {error}"))
    }

    /// Compile a modality-complete, provenance-closed research object before accountable signing.
    pub(super) fn glioma_multimodal_research_object_prepare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalResearchObjectRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_research_object_prepare requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma multimodal research-object request: {error}")
            })?;
        let bundle = compile_glioma_multimodal_research_object(&request).map_err(|error| {
            format!("glioma multimodal research-object preparation refused: {error}")
        })?;
        serde_json::to_value(json!({
            "bundle": bundle,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_replay_campaign_execute",
                "glioma_research_object_release_gate",
                "glioma_release_operating_cycle"
            ],
            "guarantees": [
                "required modality coverage, semantic-loss budgets, provenance closure, and local-only constraints are explicit",
                "cross-modal alignment requires shared provenance and remains blocked when unproven",
                "negative evidence and omissions remain part of the release object",
                "the route performs no upload, signing, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal research-object bundle: {error}"))
    }

    /// Check a multimodal research object against a target schema edition without mutating or
    /// rewriting artifacts.
    pub(super) fn glioma_research_object_migration_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ResearchObjectMigrationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_migration_plan requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma research-object migration request: {error}")
            })?;
        let plan = plan_glioma_research_object_migration(&request)
            .map_err(|error| format!("glioma research-object migration refused: {error}"))?;
        serde_json::to_value(json!({
            "migration": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_multimodal_research_object_prepare",
                "glioma_replay_campaign_execute",
                "glioma_research_object_release_gate"
            ],
            "guarantees": [
                "lossless metadata rewrites, explicit recomputation, and blocked migrations remain distinct",
                "content hashes and provenance are preserved whenever a decision claims preservation",
                "semantic-loss budgets and required-modality omissions fail closed",
                "the route performs no artifact rewrite, upload, signing, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma research-object migration plan: {error}"))
    }

    /// Analyze transitive research-object artifact and program closure without fetching or
    /// rewriting any artifact.
    pub(super) fn glioma_research_object_dependency_closure(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DependencyClosureRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_dependency_closure requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma dependency-closure request: {error}"))?;
        let closure = analyze_glioma_research_object_dependency_closure(&request)
            .map_err(|error| format!("glioma dependency closure refused: {error}"))?;
        serde_json::to_value(json!({
            "closure": closure,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_research_object_migration_plan",
                "glioma_research_object_dependency_closure",
                "glioma_replay_campaign_execute",
                "glioma_research_object_release_gate"
            ],
            "guarantees": [
                "transitive artifact and program closure remains explicit",
                "cycles, missing upstreams, orphaned artifacts, depth limits, and coverage omissions fail closed",
                "the route performs no artifact fetch, raw-data movement, upload, signing, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma dependency closure: {error}"))
    }

    /// Evaluate replay and review evidence before a preclinical research object enters signing.
    /// The route emits a deterministic gate only; it never signs, publishes, moves raw data, or
    /// makes a clinical decision.
    pub(super) fn glioma_research_object_release_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseGateRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_release_gate requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma release-gate request: {error}"))?;
        let campaign: ReplayCampaign =
            serde_json::from_value(arguments.get("campaign").cloned().ok_or_else(|| {
                "glioma_research_object_release_gate requires campaign".to_string()
            })?)
            .map_err(|error| format!("invalid glioma replay campaign: {error}"))?;
        let gate = evaluate_glioma_release_gate(&request, &campaign)
            .map_err(|error| format!("glioma research-object release gate refused: {error}"))?;
        serde_json::to_value(json!({
            "gate": gate,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "replay coverage, exact hashes, manifest status, uncertainty, and review independence remain explicit",
                "negative evidence is preserved as evidence and never converted into a publication pass",
                "publishable means ready for accountable signing review, not signed or publicly released",
                "MCP moves no raw data, creates no signature, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma research-object release gate: {error}"))
    }

    /// Run the complete replay-to-release handoff in the deterministic local sandbox. A
    /// publishable result is only ready for accountable signing; MCP never signs, uploads, or
    /// moves raw data.
    pub(super) fn glioma_release_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaReleaseOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release operating-cycle request: {error}"))?;
        if matches!(request.execution_mode, ReleaseExecutionMode::GovernedLocal) {
            return Err(
                "glioma_release_operating_cycle MCP route is simulation-only; governed_local requires an institution-owned replay executor"
                    .to_string(),
            );
        }
        let cycle = execute_glioma_release_operating_cycle_dry_run(&request)
            .map_err(|error| format!("glioma release operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the research-object manifest is built before dependency-aware replay",
                "exact hashes, coverage, negative evidence, uncertainty, and independent review gates remain explicit",
                "publishable means ready for accountable signing and never claims a signature or upload",
                "MCP moves no raw data, executes no instruments, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma release operating cycle: {error}"))
    }
}
