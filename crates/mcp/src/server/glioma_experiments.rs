//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    /// Size a privacy-aware multi-site benchmark across heterogeneous sites and study systems.
    pub(super) fn glioma_heterogeneity_adaptive_benchmark_power(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: HeterogeneityAdaptivePowerRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_heterogeneity_adaptive_benchmark_power requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive benchmark power request: {error}"))?;
        let plan =
            plan_glioma_heterogeneity_adaptive_benchmark_power(&request).map_err(|error| {
                format!("glioma adaptive benchmark power planning refused: {error}")
            })?;
        Ok(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "site identity, data quality, modality coverage, attrition, privacy noise, heterogeneity, effect target, power, and total budget are evaluated together",
                "adaptive sample-size recommendations are bounded by replicate multiplier and surface-point limits",
                "the route performs aggregate planning only and does not dispatch benchmark jobs"
            ]
        }))
    }

    /// Select a diverse, heterogeneity-aware experiment portfolio with a reserved replication budget.
    pub(super) fn glioma_heterogeneity_aware_experiment_portfolio(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: HeterogeneityAwareExperimentPortfolioRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_heterogeneity_aware_experiment_portfolio requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma heterogeneity-aware experiment portfolio request: {error}")
            })?;
        let portfolio =
            plan_glioma_heterogeneity_aware_experiment_portfolio(&request).map_err(|error| {
                format!("glioma heterogeneity-aware experiment portfolio refused: {error}")
            })?;
        Ok(json!({
            "portfolio": portfolio,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "required model-system strata, power floor, risk ceiling, per-replicate costs, candidate availability, and portfolio size are enforced",
                "replication reserve is protected before candidate selection and diversity remains explicit",
                "unsafe, infeasible, and deferred candidates are returned without executing experiments"
            ]
        }))
    }

    /// Bind selected portfolio candidates to typed research actions for downstream scheduling.
    pub(super) fn glioma_heterogeneity_portfolio_mission(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: HeterogeneityPortfolioMissionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_heterogeneity_portfolio_mission requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma heterogeneity portfolio mission request: {error}")
            })?;
        let plan = plan_glioma_heterogeneity_portfolio_mission(&request)
            .map_err(|error| format!("glioma heterogeneity portfolio mission refused: {error}"))?;
        Ok(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "portfolio candidate bindings, action DAG, completed work, observations, budget, beam, risk, autonomy, and instrument/federation approvals are checked",
                "the mission returns typed scheduling actions but does not execute experiments or dispatch instruments",
                "unapproved physical and federated effects remain unavailable"
            ]
        }))
    }

    /// Select a bounded local glioma assay batch by expected reduction in mechanism uncertainty.
    /// The route is an information-design planner only: it does not execute biology, dispatch
    /// instruments, or turn a model declaration into a clinical conclusion.
    pub(super) fn glioma_information_design(&self, arguments: &Value) -> Result<Value, String> {
        let request: InformationDesignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_information_design requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma information-design request: {error}"))?;
        let mechanisms: Vec<DesignMechanism> = serde_json::from_value(
            arguments
                .get("mechanisms")
                .cloned()
                .ok_or_else(|| "glioma_information_design requires mechanisms".to_string())?,
        )
        .map_err(|error| format!("invalid glioma information-design mechanisms: {error}"))?;
        let actions: Vec<DesignAction> = serde_json::from_value(
            arguments
                .get("actions")
                .cloned()
                .ok_or_else(|| "glioma_information_design requires actions".to_string())?,
        )
        .map_err(|error| format!("invalid glioma information-design actions: {error}"))?;
        let acquisition_objective: InformationAcquisitionObjective = serde_json::from_value(
            arguments
                .get("acquisition_objective")
                .cloned()
                .unwrap_or_else(|| json!(InformationAcquisitionObjective::MechanismGini)),
        )
        .map_err(|error| {
            format!("invalid glioma information-design acquisition objective: {error}")
        })?;
        let output = plan_glioma_information_design_with_objective(
            &request,
            acquisition_objective,
            &mechanisms,
            &actions,
        )
        .map_err(|error| format!("glioma information design refused: {error}"))?;
        serde_json::to_value(json!({
            "design": output,
            "dispatch": "not_started",
            "guarantees": [
                "information gain is integer-only expected Gini reduction over caller-declared outcome distributions",
                "selection is bounded by feasibility, risk, cost, budget, and replicate limits",
                "the plan keeps unresolved and risk-blocked assays explicit for researcher review",
                "the route prioritizes local preclinical assays but never executes biology or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma information design: {error}"))
    }

    /// Compile a multi-assay glioma panel with information, diversity, safety, feasibility, and
    /// budget gates. Candidate outcome distributions are planning declarations only.
    pub(super) fn glioma_adaptive_panel(&self, arguments: &Value) -> Result<Value, String> {
        let request: AdaptivePanelRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_panel requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive panel request: {error}"))?;
        let panel = plan_glioma_adaptive_panel(&request)
            .map_err(|error| format!("glioma adaptive panel refused: {error}"))?;
        serde_json::to_value(json!({
            "panel": panel,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_protocol_simulate", "glioma_protocol_transport_gate", "glioma_adaptive_information_campaign"],
            "guarantees": [
                "panel selection uses deterministic expected Gini information gain over declared outcome distributions",
                "correlated independence groups receive explicit diversity treatment rather than silent double-counting",
                "risk, feasibility, cost, budget, deferred actions, and blocked actions remain visible",
                "planned outcomes remain distinct from observations and the route never executes an assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive panel: {error}"))
    }

    /// Plan a multi-site replication topology from paired local arm summaries; this is design
    /// only and never dispatches an assay or infers a clinical effect.
    pub(super) fn glioma_replication_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReplicationPlanRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_replication_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication plan request: {error}"))?;
        let observations: Vec<ReplicationObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_replication_plan requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication observations: {error}"))?;
        let plan = plan_glioma_replication(&request, &observations)
            .map_err(|error| format!("glioma replication plan refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_protocol_simulate", "glioma_protocol_transport_gate", "glioma_power_reestimate"],
            "guarantees": [
                "site contrasts, pooled effect, heterogeneity, power proxy, and leave-one-site-out sensitivity remain explicit",
                "replicate allocation is bounded by site diversity, cost, risk, budget, and total-replicate gates",
                "heterogeneous and risk-blocked sites are never silently pooled or promoted",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication plan: {error}"))
    }

    /// Replan the next bounded replication wave from returned local observations. This route
    /// preserves low-quality, heterogeneous, negative, risk-blocked, and budget-blocked states;
    /// it never dispatches an assay or promotes a planning proxy into a clinical conclusion.
    pub(super) fn glioma_replication_continuation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplicationContinuationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_replication_continuation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma replication continuation request: {error}"))?;
        let plan = plan_glioma_replication_continuation(&request)
            .map_err(|error| format!("glioma replication continuation refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_protocol_scenario_ensemble", "glioma_protocol_autonomous_execute", "glioma_replication_campaign_execute"],
            "guarantees": [
                "each next-wave action is derived from validated local observations and the P06 topology planner",
                "quality, round, power, effect-direction, heterogeneity, risk, and budget stopping gates remain explicit",
                "negative or null replication evidence is preserved and can terminate additional spending",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication continuation: {error}"))
    }

    /// Compile a continuation decision into a typed local protocol and run its deterministic
    /// preflight. The returned protocol is an artifact for a caller-owned executor, never a
    /// hardware command or a biological conclusion.
    pub(super) fn glioma_replication_protocol_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplicationProtocolCompileRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_protocol_compile requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma replication protocol compile request: {error}")
            })?;
        let compilation = compile_glioma_replication_protocol(&request)
            .map_err(|error| format!("glioma replication protocol compilation refused: {error}"))?;
        serde_json::to_value(json!({
            "compilation": compilation,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_protocol_scenario_ensemble", "glioma_protocol_autonomous_execute"],
            "guarantees": [
                "continuation site actions compile into deterministic setup, arm, and optional QC tasks",
                "the generated request is preflighted against local resources, horizon, risk, and approval gates",
                "held or failed sites remain explicit and are never silently turned into protocol work",
                "the route performs no provider, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication protocol compilation: {error}"))
    }

    /// Allocate a bounded glioma experiment batch against the lower tail of declared scenarios.
    /// This is a design product only; it never dispatches an assay or instrument.
    pub(super) fn glioma_robust_experiment_design(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: RobustExperimentDesignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_robust_experiment_design requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma robust experiment design request: {error}"))?;
        let design = design_glioma_robust_experiment(&request)
            .map_err(|error| format!("glioma robust experiment design refused: {error}"))?;
        serde_json::to_value(json!({
            "design": design,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_protocol_simulate", "glioma_protocol_transport_gate"],
            "guarantees": [
                "allocation maximizes declared lower-tail scenario utility with integer diminishing marginal gains",
                "budget, throughput, risk, feasibility, and minimum robust-information gates remain explicit",
                "scenario uncertainty and weak candidates remain visible rather than being converted into confident claims",
                "the route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma robust experiment design: {error}"))
    }

    /// Compile a confounding-aware blocked randomization matrix for a preclinical glioma study.
    /// The planner balances declared nuisance strata and spends residual capacity on the highest
    /// variance-aware information gain; it never randomizes specimens or dispatches a protocol.
    pub(super) fn glioma_blocked_randomization_design(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: BlockedRandomizationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_blocked_randomization_design requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma blocked-randomization request: {error}"))?;
        let design = plan_glioma_blocked_randomization(&request)
            .map_err(|error| format!("glioma blocked randomization refused: {error}"))?;
        serde_json::to_value(json!({
            "design": design,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_protocol_simulate",
                "glioma_power_reestimate",
                "glioma_replication_plan"
            ],
            "guarantees": [
                "declared nuisance strata are balanced explicitly rather than hidden in a post-hoc adjustment",
                "integer allocation uses variance-aware marginal information gain per unit cost",
                "risk, feasibility, capacity, budget, and excluded-arm evidence remain visible",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma blocked randomization design: {error}"))
    }

    /// Replan a local adaptive glioma assay campaign from categorical outcomes already returned
    /// by an institution-local executor. This is a planning endpoint; physical execution remains
    /// behind the typed Rust executor seam and is never performed by MCP.
    pub(super) fn glioma_adaptive_information_campaign(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveInformationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_information_campaign requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive campaign request: {error}"))?;
        let mechanisms: Vec<DesignMechanism> =
            serde_json::from_value(arguments.get("mechanisms").cloned().ok_or_else(|| {
                "glioma_adaptive_information_campaign requires mechanisms".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive campaign mechanisms: {error}"))?;
        let actions: Vec<DesignAction> =
            serde_json::from_value(arguments.get("actions").cloned().ok_or_else(|| {
                "glioma_adaptive_information_campaign requires actions".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive campaign actions: {error}"))?;
        let observations: Vec<AdaptiveInformationObservation> = arguments
            .get("observations")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid glioma adaptive campaign observations: {error}"))?
            .unwrap_or_default();
        let output = plan_glioma_adaptive_information_campaign(
            &request,
            &mechanisms,
            &actions,
            &observations,
        )
        .map_err(|error| format!("glioma adaptive information campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": output,
            "dispatch": "not_started",
            "guarantees": [
                "each returned categorical outcome is applied with integer Bayes normalization and explicit zero-likelihood refusal",
                "every replan respects declared replicate, risk, feasibility, cost, and budget ceilings",
                "posterior concentration, exhausted assays, unresolved information, and negative evidence remain visible",
                "MCP only compiles the next local preclinical batch; an institution-owned executor must authorize and run any assay"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive information campaign: {error}"))
    }

    /// Execute a bounded adaptive replicate-allocation campaign through the deterministic local
    /// sandbox. Each accepted aggregate batch updates the Beta posterior before the next arm is
    /// selected; MCP never contacts hardware or makes a clinical decision.
    pub(super) fn glioma_adaptive_allocation_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveAllocationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_allocation_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma adaptive-allocation campaign request: {error}")
            })?;
        let mut executor = DryRunAdaptiveAllocationCampaignExecutor;
        let campaign = execute_glioma_adaptive_allocation_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma adaptive-allocation campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "each arm batch is selected from the posterior-aware allocator and replanned after the returned successes/failures",
                "aggregate observations are arm-bound, local-only, artifact-validated, and require exact replicate accounting",
                "underpowered, negative, risk-blocked, budget, retry, no-progress, and executor-failure states remain explicit",
                "MCP never contacts hardware, moves raw biology, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive-allocation campaign: {error}"))
    }

    /// Select the next local preclinical glioma assay with an uncertainty-aware kernel surrogate.
    /// The optimizer is deterministic and bounded; it only compiles a next batch and never
    /// dispatches biology or turns a surrogate estimate into a clinical recommendation.
    pub(super) fn glioma_active_learning(&self, arguments: &Value) -> Result<Value, String> {
        let request: ActiveLearningRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_active_learning requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma active-learning request: {error}"))?;
        let candidates: Vec<ActiveLearningCandidate> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or_else(|| "glioma_active_learning requires candidates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma active-learning candidates: {error}"))?;
        let observations: Vec<ActiveLearningObservation> = arguments
            .get("observations")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid glioma active-learning observations: {error}"))?
            .unwrap_or_default();
        let plan = plan_glioma_active_learning(&request, &candidates, &observations)
            .map_err(|error| format!("glioma active-learning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "same-candidate and nearby-candidate observations are combined with an integer inverse-distance kernel",
                "contradictory or sparse support widens uncertainty instead of being averaged into confidence",
                "selection respects cost, risk, replicate, budget, and redundancy-group limits with explicit blocked/deferred states",
                "MCP compiles a local next batch only; an institution-owned executor must authorize and run any assay"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma active-learning plan: {error}"))
    }

    /// Plan a batch against caller-supplied posterior draws and predictive likelihoods.
    /// This route is planner-only: it does not train the model or dispatch any assay.
    pub(super) fn glioma_posterior_batch(&self, arguments: &Value) -> Result<Value, String> {
        let request: PosteriorBatchRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_posterior_batch requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma posterior-batch request: {error}"))?;
        let candidates: Vec<PosteriorBatchCandidate> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or_else(|| "glioma_posterior_batch requires candidates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma posterior-batch candidates: {error}"))?;
        let plan = plan_glioma_posterior_batch(&request, &candidates)
            .map_err(|error| format!("glioma posterior-batch planner refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "the institution supplies posterior draws and calibrated candidate outcome probabilities; this route does not fit a model",
                "posterior mass, candidate likelihood coverage, target vectors, and probability normalization are validated before selection",
                "selection is a deterministic cost-adjusted surrogate over conditional posterior disagreement with explicit risk, replicate, budget, and unresolved outcomes",
                "the input digest binds the plan to normalized request, candidate, and posterior inputs but does not authenticate or conceal them",
                "MCP only returns a local preclinical plan; no assay is dispatched and no clinical conclusion is made"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma posterior-batch plan: {error}"))
    }

    /// Execute a bounded active-learning campaign through the deterministic local sandbox
    /// executor. A deployment may replace that executor with an institution-owned gateway;
    /// this MCP route never contacts instruments or moves raw biology.
    pub(super) fn glioma_active_learning_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ActiveLearningCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_active_learning_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma active-learning campaign request: {error}"))?;
        let mut executor = DryRunActiveLearningCampaignExecutor;
        let campaign = execute_glioma_active_learning_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma active-learning campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "each selected assay is executed only by the caller-owned deterministic sandbox executor",
                "returned observations are candidate-bound, content-addressed, and used for the next replan",
                "retry, budget, replicate, risk, redundancy, unresolved, and executor-failure gates are explicit",
                "MCP never contacts hardware, moves raw data, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma active-learning campaign: {error}"))
    }

    /// Score the next local assay across competing mechanistic surrogates. This route compiles a
    /// conservative batch only; it never dispatches biology or converts model disagreement into a
    /// clinical conclusion.
    pub(super) fn glioma_robust_active_learning(&self, arguments: &Value) -> Result<Value, String> {
        let request: RobustActiveLearningRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_robust_active_learning requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma robust active-learning request: {error}"))?;
        let candidates: Vec<RobustActiveLearningCandidate> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or_else(|| "glioma_robust_active_learning requires candidates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma robust active-learning candidates: {error}"))?;
        let observations: Vec<RobustActiveLearningObservation> = arguments
            .get("observations")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| {
                format!("invalid glioma robust active-learning observations: {error}")
            })?
            .unwrap_or_default();
        let plan = plan_glioma_robust_active_learning(&request, &candidates, &observations)
            .map_err(|error| format!("glioma robust active-learning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "competing mechanistic surrogates are reliability- and prior-weighted without collapsing disagreement",
                "lower-tail acquisition retains residual uncertainty and contradiction as explicit gates",
                "selection respects risk, replicate, budget, model-support, and redundancy limits",
                "MCP compiles a local next batch only; an institution-owned executor must authorize and run any assay"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma robust active-learning plan: {error}"))
    }

    /// Execute repeated robust ensemble-guided assay rounds in the deterministic local sandbox.
    pub(super) fn glioma_robust_active_learning_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: RobustActiveLearningCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_robust_active_learning_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid robust active-learning campaign request: {error}"))?;
        let mut executor = DryRunRobustActiveLearningCampaignExecutor::default();
        let campaign = execute_glioma_robust_active_learning_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma robust active-learning campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "each round uses the reliability- and prior-weighted lower-tail ensemble planner",
                "returned observations are candidate-bound and become inputs to the next replan",
                "budget, replicate, risk, disagreement, retry, unresolved, and executor-failure states remain explicit",
                "MCP never contacts hardware, moves raw data, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode robust active-learning campaign: {error}"))
    }

    /// Choose a bounded next batch across screening, mechanistic, and validation model systems.
    /// This is a local planner: it never dispatches an assay or converts a surrogate estimate
    /// into a clinical recommendation.
    pub(super) fn glioma_multi_fidelity_optimize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiFidelityOptimizationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multi_fidelity_optimize requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multi-fidelity request: {error}"))?;
        let candidates: Vec<FidelityCandidate> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or_else(|| "glioma_multi_fidelity_optimize requires candidates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multi-fidelity candidates: {error}"))?;
        let observations: Vec<FidelityObservation> = arguments
            .get("observations")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid glioma multi-fidelity observations: {error}"))?
            .unwrap_or_default();
        let plan = plan_glioma_multi_fidelity_optimization(&request, &candidates, &observations)
            .map_err(|error| format!("glioma multi-fidelity optimization refused: {error}"))?;
        serde_json::to_value(json!({
            "optimization": plan,
            "dispatch": "not_started",
            "guarantees": [
                "paired cross-fidelity calibration is estimated from shared designs and carries residual uncertainty",
                "unobserved candidates use transferred or neighborhood support instead of fabricated certainty",
                "selection is bounded by cost, risk, replicate, fidelity-support, and portfolio-diversity gates",
                "the route only plans local preclinical work and never executes biology or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-fidelity optimization: {error}"))
    }

    /// Detect instrument-control drift before a preclinical run is admitted. The calibration
    /// computation is local and deterministic; this endpoint never touches hardware.
    pub(super) fn glioma_instrument_calibration(&self, arguments: &Value) -> Result<Value, String> {
        let request: CalibrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_calibration requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument calibration request: {error}"))?;
        let runs: Vec<CalibrationRun> = serde_json::from_value(
            arguments
                .get("runs")
                .cloned()
                .ok_or_else(|| "glioma_instrument_calibration requires runs".to_string())?,
        )
        .map_err(|error| format!("invalid glioma calibration runs: {error}"))?;
        let output = analyze_instrument_calibration(&request, &runs)
            .map_err(|error| format!("glioma instrument calibration refused: {error}"))?;
        serde_json::to_value(json!({
            "calibration": output,
            "dispatch": "not_started",
            "guarantees": [
                "reference controls use a robust median/MAD rather than a single run",
                "drift uses a deterministic Theil-Sen slope and explicit final/max deviations",
                "noisy, drifting, or insufficient calibration history blocks admission explicitly",
                "the route only plans a preclinical instrument gate and never executes hardware"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument calibration: {error}"))
    }

    /// Extract quality- and drift-gated signal endpoints from value-only local instrument points.
    /// This route never touches hardware or promotes a signal to scientific evidence by itself.
    pub(super) fn glioma_instrument_signal_extract(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: SignalExtractionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_signal_extract requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma signal-extraction request: {error}"))?;
        let points: Vec<InstrumentSignalPoint> = serde_json::from_value(
            arguments
                .get("points")
                .cloned()
                .ok_or_else(|| "glioma_instrument_signal_extract requires points".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument signal points: {error}"))?;
        let output = extract_glioma_instrument_signal(&request, &points)
            .map_err(|error| format!("glioma instrument signal extraction refused: {error}"))?;
        serde_json::to_value(json!({
            "signal_extraction": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_instrument_assay_adjudicate",
                "glioma_instrument_science_loop_execute",
                "glioma_mechanism_discriminate"
            ],
            "guarantees": [
                "local median baselines and robust residual noise remain replay-stable",
                "quality, spacing, drift, and signal gates preserve blocked channels and negative evidence",
                "raw traces remain behind the institution-local gateway and only bounded summaries cross this route",
                "the route performs no hardware, biological, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument signal extraction: {error}"))
    }

    /// Evaluate prospective cross-run endpoint stability from local extraction summaries. This
    /// route never touches hardware and never promotes an endpoint without stability gates.
    pub(super) fn glioma_instrument_batch_stability(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: SignalBatchStabilityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_batch_stability requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma batch-stability request: {error}"))?;
        let runs: Vec<InstrumentSignalRun> = serde_json::from_value(
            arguments
                .get("runs")
                .cloned()
                .ok_or_else(|| "glioma_instrument_batch_stability requires runs".to_string())?,
        )
        .map_err(|error| format!("invalid glioma signal runs: {error}"))?;
        let output = analyze_glioma_instrument_batch_stability(&request, &runs)
            .map_err(|error| format!("glioma instrument batch stability refused: {error}"))?;
        serde_json::to_value(json!({
            "batch_stability": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_instrument_assay_adjudicate",
                "glioma_instrument_science_loop_execute",
                "glioma_replication_campaign_execute"
            ],
            "guarantees": [
                "channel coverage, robust dispersion, noise, peak-score, and run-drift gates are explicit",
                "underpowered or unstable endpoints remain blocked rather than promoted",
                "only value-only extraction summaries cross the route and raw traces remain local",
                "the route performs no hardware, biological, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument batch stability: {error}"))
    }

    /// Align local instrument channels by bounded integer lag and gate cross-channel concordance.
    /// The route emits summaries only; it never dispatches hardware or promotes biology.
    pub(super) fn glioma_instrument_multichannel_concordance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultichannelConcordanceRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_multichannel_concordance requires request".to_string()
            })?)
            .map_err(|error| format!("invalid multichannel concordance request: {error}"))?;
        let channels: Vec<MultichannelInput> =
            serde_json::from_value(arguments.get("channels").cloned().ok_or_else(|| {
                "glioma_instrument_multichannel_concordance requires channels".to_string()
            })?)
            .map_err(|error| format!("invalid multichannel concordance channels: {error}"))?;
        let output = analyze_glioma_instrument_multichannel_concordance(&request, &channels)
            .map_err(|error| format!("multichannel concordance refused: {error}"))?;
        serde_json::to_value(json!({
            "concordance": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_instrument_signal_extract",
                "glioma_instrument_batch_stability",
                "glioma_instrument_assay_adjudicate"
            ],
            "guarantees": [
                "integer-lag search, fixed-point correlation, overlap, quality, and residual gates are deterministic",
                "weak, inverse, undercovered, and high-residual channels remain blocked or unresolved",
                "raw traces remain institution-local and only bounded concordance summaries cross this route",
                "the route performs no hardware, biological, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode multichannel concordance: {error}"))
    }

    /// Compute aggregate-only cross-site endpoint consensus with privacy and heterogeneity gates.
    /// Raw traces and instrument effects remain institution-local.
    pub(super) fn glioma_federated_instrument_consensus(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedInstrumentConsensusRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_instrument_consensus requires request".to_string()
            })?)
            .map_err(|error| format!("invalid federated instrument consensus request: {error}"))?;
        let sites: Vec<FederatedInstrumentSite> =
            serde_json::from_value(arguments.get("sites").cloned().ok_or_else(|| {
                "glioma_federated_instrument_consensus requires sites".to_string()
            })?)
            .map_err(|error| format!("invalid federated instrument sites: {error}"))?;
        let output = analyze_glioma_federated_instrument_consensus(&request, &sites)
            .map_err(|error| format!("federated instrument consensus refused: {error}"))?;
        serde_json::to_value(json!({
            "consensus": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_federated_benchmark_consensus",
                "glioma_instrument_batch_stability",
                "glioma_replication_federated_transport_execute"
            ],
            "guarantees": [
                "only privacy-gated aggregate endpoint summaries participate",
                "heterogeneity, leave-one-site-out sensitivity, and maximum site influence remain explicit",
                "raw traces, samples, human data, and hardware effects never cross this route",
                "the route performs no federation write, hardware, biological, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode federated instrument consensus: {error}"))
    }

    /// Compile an interlocked, authorization-bound instrument plan. The route never dispatches
    /// hardware; an institution-local gateway must verify the approval digest again before use.
    pub(super) fn glioma_instrument_preflight(&self, arguments: &Value) -> Result<Value, String> {
        let request: InstrumentPreflightRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_preflight requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument preflight request: {error}"))?;
        let plan = preflight_glioma_instrument(&request)
            .map_err(|error| format!("glioma instrument preflight refused: {error}"))?;
        serde_json::to_value(json!({
            "preflight": plan,
            "dispatch": "not_started",
            "guarantees": [
                "qualified calibration, emergency stop, guard, deck, temperature, waste, and authorization gates are explicit",
                "actions are serialized, typed, parameter-bounded, risk-budgeted, and digest-bound",
                "blocked and unmeasured actions remain visible with compensation order and reasons",
                "the route never contacts an instrument or consumes biological material"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument preflight: {error}"))
    }

    /// Schedule a bounded fleet of compatible preclinical instruments before per-instrument
    /// preflight. Scheduling never admits or dispatches hardware and never consumes material.
    pub(super) fn glioma_instrument_fleet_schedule(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentFleetScheduleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_fleet_schedule requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument fleet request: {error}"))?;
        let schedule = schedule_glioma_instrument_fleet(&request)
            .map_err(|error| format!("glioma instrument fleet scheduling refused: {error}"))?;
        serde_json::to_value(json!({
            "schedule": schedule,
            "dispatch": "not_started",
            "preflight_required": true,
            "guarantees": [
                "dependency-safe assignment respects instrument capability, availability, calibration, operator, deadline, and shared-risk gates",
                "critical path, fleet utilization, blocked tasks, and negative scheduling evidence remain explicit",
                "dispatch_permitted is always false until each assignment passes the existing instrument preflight and gateway authorization",
                "the route does not contact hardware or consume biological material"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument fleet schedule: {error}"))
    }

    /// Execute a validated fleet schedule through the deterministic local gateway. Every run
    /// remains bound to its scheduled instrument and admitted preflight plan; MCP uses a dry-run
    /// executor while institution-local callers can supply a hardware gateway through Rust.
    pub(super) fn glioma_instrument_fleet_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentFleetExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_fleet_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument fleet execution request: {error}"))?;
        let interlocks = request
            .runs
            .first()
            .map(|run| run.execution.live_interlocks.clone())
            .unwrap_or(InstrumentInterlockSnapshot {
                observed_tick: 0,
                emergency_stop_clear: true,
                guard_closed: true,
                deck_clear: true,
                consumables_available: true,
                waste_capacity_milli: 0,
                temperature_milli: None,
                minimum_temperature_milli: None,
                maximum_temperature_milli: None,
                calibration_valid_until_tick: 0,
                calibration_sequence_index: 0,
            });
        let mut executor = DryRunInstrumentExecutor {
            interlocks,
            emergency_stop_called: false,
        };
        let execution = execute_glioma_instrument_fleet(&request, &mut executor)
            .map_err(|error| format!("glioma instrument fleet execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "every admitted schedule task is bound to its assigned instrument and preflight plan before gateway entry",
                "dependency-blocked, schedule-blocked, negative, partial, unresolved, failed, retry, and emergency-stop outcomes remain explicit",
                "the existing guarded executor rechecks authorization and live interlocks before every operation",
                "MCP emits synthetic local artifacts only; no hardware, raw-data, or clinical effect occurs"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument fleet execution: {error}"))
    }

    /// Execute an admitted preclinical instrument plan through the deterministic local gateway
    /// seam. MCP uses a dry-run executor; institution-owned callers can supply a gateway that
    /// performs hardware authentication, interlock reads, and emergency-stop wiring.
    pub(super) fn glioma_instrument_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request: InstrumentExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument execution request: {error}"))?;
        let mut executor = DryRunInstrumentExecutor {
            interlocks: request.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let execution = execute_glioma_instrument_plan(&request, &mut executor)
            .map_err(|error| format!("glioma instrument execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "the admitted P08 preflight digest, authorization scope, and action order are checked before dispatch",
                "live interlocks and gateway authorization are rechecked before every operation",
                "retries are bounded and partial, failed, blocked, negative, and emergency-stop outcomes remain explicit",
                "MCP emits local synthetic artifacts only; production hardware effects require an institution-owned InstrumentExecutor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument execution: {error}"))
    }

    /// Plan bounded recovery for a completed local instrument execution record. The recovery
    /// planner never reconnects to hardware or turns an execution outcome into evidence.
    pub(super) fn glioma_instrument_recovery_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentRecoveryRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_recovery_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument recovery request: {error}"))?;
        let plan = plan_glioma_instrument_recovery(&request)
            .map_err(|error| format!("glioma instrument recovery refused: {error}"))?;
        serde_json::to_value(json!({
            "recovery": plan,
            "dispatch": "not_started",
            "next_routes": ["glioma_instrument_preflight", "glioma_instrument_execute"],
            "guarantees": [
                "partial, failed, blocked, unresolved, and negative run outcomes remain explicit",
                "retries, resumes, recalibration, compensation, and human review are bounded and typed",
                "negative results are never converted into automatic retries or successful evidence",
                "the route performs no hardware connection, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument recovery: {error}"))
    }

    /// Execute a bounded sequence of admitted instrument runs through the deterministic local
    /// gateway. Any partial, unresolved, blocked, or failed run halts the remaining queue.
    pub(super) fn glioma_instrument_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma instrument campaign request: {error}"))?;
        let interlocks = request
            .runs
            .first()
            .map(|run| run.execution.live_interlocks.clone())
            .ok_or_else(|| "glioma instrument campaign requires at least one run".to_string())?;
        let mut executor = DryRunInstrumentExecutor {
            interlocks,
            emergency_stop_called: false,
        };
        let campaign = execute_glioma_instrument_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma instrument campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "admitted plans execute only in declared order through the guarded P08 executor",
                "partial, unresolved, blocked, failed, negative, retry, and emergency-stop states halt or partition the remaining queue explicitly",
                "each run preserves its preflight digest, authorization, live interlock checks, local artifacts, and replay digest",
                "MCP emits no hardware effect, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument campaign: {error}"))
    }

    /// Select and execute a dependency-closed, information-aware instrument subset. MCP uses a
    /// deterministic dry-run gateway; an institution-local caller can reuse the research
    /// algorithm with its own guarded InstrumentExecutor.
    pub(super) fn glioma_adaptive_instrument_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveInstrumentCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_instrument_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma adaptive instrument campaign request: {error}")
            })?;
        let mut executor = dry_run_adaptive_instrument_executor(&request)
            .map_err(|error| format!("glioma adaptive instrument campaign refused: {error}"))?;
        let campaign = execute_glioma_adaptive_instrument_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma adaptive instrument campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "integer scoring combines expected information, frontier novelty, reproducibility, instrument cost, and physical risk",
                "selected work is dependency-closed, endpoint-diverse, and bounded by explicit time and risk budgets",
                "only selected admitted plans enter the existing guarded instrument campaign executor",
                "information and endpoint floors remain explicit no-feasible-plan holds rather than being fabricated",
                "MCP emits synthetic local artifacts only; no hardware, raw-data, or clinical effect occurs"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive instrument campaign: {error}"))
    }

    /// Apply the all-runs preflight barrier before dispatching a local instrument campaign.
    pub(super) fn glioma_instrument_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument operating-cycle request: {error}"))?;
        if matches!(
            request.execution_mode,
            InstrumentExecutionMode::GovernedLocal
        ) {
            return Err(
                "glioma_instrument_operating_cycle MCP route is simulation-only; governed_local requires an institution-owned gateway"
                    .to_string(),
            );
        }
        let mut executor = dry_run_instrument_executor_from_request(&request)
            .map_err(|error| format!("glioma instrument operating cycle refused: {error}"))?;
        let cycle = execute_glioma_instrument_operating_cycle(&request, &mut executor)
            .map_err(|error| format!("glioma instrument operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "every run is preflight-validated before any dispatch",
                "any blocked or unresolved run holds the whole queue when require_all_admitted is true",
                "authorization, interlocks, retries, emergency stop, artifact, and negative states remain explicit",
                "MCP emits no hardware effect, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument operating cycle: {error}"))
    }

    /// Adjudicate typed local assay summaries after an instrument run. Hardware completion is
    /// intentionally insufficient: QC, uncertainty, replicate, effect, and negative-control
    /// gates must clear before any result becomes eligible for downstream science.
    pub(super) fn glioma_instrument_assay_adjudicate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AssayEvidenceRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_assay_adjudicate requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma assay evidence request: {error}"))?;
        let execution: InstrumentExecutionRun =
            serde_json::from_value(arguments.get("execution").cloned().ok_or_else(|| {
                "glioma_instrument_assay_adjudicate requires execution".to_string()
            })?)
            .map_err(|error| format!("invalid glioma instrument execution: {error}"))?;
        let observations: Vec<AssayEvidenceObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_instrument_assay_adjudicate requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma assay observations: {error}"))?;
        let assessment = adjudicate_glioma_assay_evidence(&request, &execution, &observations)
            .map_err(|error| format!("glioma assay adjudication refused: {error}"))?;
        serde_json::to_value(json!({
            "assessment": assessment,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "hardware completion never becomes biological evidence without typed assay and QC adjudication",
                "QC, uncertainty, replicate, effect, and negative-control gates remain explicit",
                "missing, negative, partial, and unresolved results produce observable next actions",
                "the route never executes hardware, moves raw data, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma assay evidence assessment: {error}"))
    }

    /// Run the complete local-simulation instrument-to-science loop: preflight, guarded campaign
    /// execution, assay evidence adjudication, and typed next research action selection.
    pub(super) fn glioma_instrument_science_loop_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentScienceLoopRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_science_loop_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma instrument science-loop request: {error}"))?;
        if matches!(
            request.operating_cycle.execution_mode,
            InstrumentExecutionMode::GovernedLocal
        ) {
            return Err(
                "glioma_instrument_science_loop_execute MCP route is simulation-only; governed_local requires an institution-owned gateway"
                    .to_string(),
            );
        }
        let observations: Vec<InstrumentAssayEvidenceRunObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_instrument_science_loop_execute requires observations".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma instrument science-loop observations: {error}")
            })?;
        let mut executor = dry_run_instrument_executor_from_request(&request.operating_cycle)
            .map_err(|error| format!("glioma instrument science loop refused: {error}"))?;
        let output = execute_glioma_instrument_science_loop(&request, &observations, &mut executor)
            .map_err(|error| format!("glioma instrument science loop refused: {error}"))?;
        serde_json::to_value(json!({
            "loop": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "preflight and guarded execution complete before assay evidence is adjudicated",
                "hardware completion never promotes a result without QC, replicate, uncertainty, effect, and negative-control gates",
                "missing summaries, negative assays, partial campaigns, and safety blocks become typed next research actions",
                "MCP emits only local synthetic artifacts; production hardware requires an institution-owned executor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument science loop: {error}"))
    }

    /// Route adjudicated instrument outcomes into the autonomous research mission controller.
    /// The MCP worker is deterministic and local-only; physical instrument effects remain behind
    /// the institution-owned executor used by the upstream science loop.
    pub(super) fn glioma_instrument_research_frontier_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: InstrumentResearchFrontierRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_research_frontier_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma instrument research frontier request: {error}")
            })?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_instrument_research_frontier(&request, &mut executor)
            .map_err(|error| format!("glioma instrument research frontier refused: {error}"))?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "qualified assay records route to computation, unresolved records to replication/QC, and negative records to falsification",
                "the frontier is scored by information, QC, replicate support, uncertainty, cost, and scientific status",
                "instrument completion is never promoted to evidence by the handoff",
                "autonomous mission outcomes preserve negative, blocked, failed, budget, and no-progress states",
                "the MCP route performs no hardware execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument research frontier: {error}"))
    }
}
