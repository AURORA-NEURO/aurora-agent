//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    pub(super) fn glioma_mechanism_explore(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_explore requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism request: {error}"))?;
        let candidates: Vec<MechanismCandidate> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_explore requires candidates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism candidates: {error}"))?;
        let output = explore_mechanisms(&request, &candidates)
            .map_err(|error| format!("glioma mechanism exploration refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma mechanism portfolio: {error}"))
    }

    /// Simulate signed delayed mechanism feedback for a local preclinical glioma model. The
    /// result is a bounded scientific trajectory and intervention-sensitivity plan; it never
    /// treats the model as observed biology or selects a clinical treatment.
    pub(super) fn glioma_mechanism_dynamics(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismDynamicsRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_dynamics requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism-dynamics request: {error}"))?;
        let nodes: Vec<MechanismDynamicsNode> = serde_json::from_value(
            arguments
                .get("nodes")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_dynamics requires nodes".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism-dynamics nodes: {error}"))?;
        let edges: Vec<MechanismDynamicsEdge> = serde_json::from_value(
            arguments
                .get("edges")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_dynamics requires edges".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism-dynamics edges: {error}"))?;
        let interventions: Vec<MechanismDynamicsIntervention> = serde_json::from_value(
            arguments
                .get("interventions")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid glioma mechanism-dynamics interventions: {error}"))?;
        let plan = simulate_glioma_mechanism_dynamics(&request, &nodes, &edges, &interventions)
            .map_err(|error| format!("glioma mechanism dynamics refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "signed delayed feedback is evaluated with bounded fixed-point arithmetic",
                "stability, oscillation, divergence, risk, and budget states remain explicit",
                "intervention sensitivity is leave-one-out model sensitivity, not observed efficacy",
                "the route never dispatches an assay or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism-dynamics plan: {error}"))
    }

    /// Score competing mechanistic predictions against local observations and rank the next
    /// assay by expected information gain. This is a bounded planning computation: it does not
    /// execute an assay or infer a clinical action.
    pub(super) fn glioma_mechanism_discriminate(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismDiscriminationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_discriminate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism discrimination request: {error}"))?;
        let hypotheses: Vec<MechanismHypothesis> = serde_json::from_value(
            arguments
                .get("hypotheses")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_discriminate requires hypotheses".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism hypotheses: {error}"))?;
        let observations: Vec<MechanismFeatureObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_mechanism_discriminate requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism observations: {error}"))?;
        let actions: Vec<MechanismDiscriminatorAction> = serde_json::from_value(
            arguments
                .get("actions")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_discriminate requires actions".to_string())?,
        )
        .map_err(|error| format!("invalid glioma discriminator actions: {error}"))?;
        let output = discriminate_mechanisms(&request, &hypotheses, &observations, &actions)
            .map_err(|error| format!("glioma mechanism discrimination refused: {error}"))?;
        serde_json::to_value(json!({
            "discrimination": output,
            "dispatch": "not_started",
            "guarantees": [
                "mechanism fit uses bounded residual likelihood with explicit feature coverage",
                "action ranking uses posterior-weighted pairwise prediction separation, feasibility, and cost",
                "missing observations, diffuse posteriors, and information-poor actions remain explicit",
                "the route plans a preclinical assay and never executes instruments or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism discrimination: {error}"))
    }

    /// Compute which declared mechanism pairs remain indistinguishable and select affordable
    /// discriminating features. This is local analysis only; it never executes an assay.
    pub(super) fn glioma_mechanism_identifiability(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismIdentifiabilityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_identifiability requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism-identifiability request: {error}"))?;
        let mechanisms: Vec<IdentifiabilityMechanism> =
            serde_json::from_value(arguments.get("mechanisms").cloned().ok_or_else(|| {
                "glioma_mechanism_identifiability requires mechanisms".to_string()
            })?)
            .map_err(|error| format!("invalid glioma identifiability mechanisms: {error}"))?;
        let features: Vec<IdentifiabilityFeature> = serde_json::from_value(
            arguments
                .get("features")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_identifiability requires features".to_string())?,
        )
        .map_err(|error| format!("invalid glioma identifiability features: {error}"))?;
        let mut request = request;
        request.mechanisms = mechanisms;
        request.features = features;
        let output = analyze_glioma_mechanism_identifiability(&request)
            .map_err(|error| format!("glioma mechanism identifiability refused: {error}"))?;
        serde_json::to_value(json!({
            "identifiability": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_mechanism_discriminate",
                "glioma_information_design",
                "glioma_mechanism_dynamics"
            ],
            "guarantees": [
                "pairwise mechanism separation is computed from typed local predictions and feature quality",
                "observationally indistinguishable pairs remain unresolved rather than being over-ranked",
                "feature selection is bounded by risk, quality, cost, budget, and explicit pair coverage",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism identifiability: {error}"))
    }

    /// Score cross-model mechanistic invariance and select a bounded signature panel. This is
    /// transportability planning only; it never executes an assay or makes a clinical decision.
    pub(super) fn glioma_mechanism_invariance(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismInvarianceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_invariance requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism-invariance request: {error}"))?;
        let contexts: Vec<MechanismInvarianceContext> = serde_json::from_value(
            arguments
                .get("contexts")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_invariance requires contexts".to_string())?,
        )
        .map_err(|error| format!("invalid glioma invariance contexts: {error}"))?;
        let mechanisms: Vec<InvarianceMechanism> = serde_json::from_value(
            arguments
                .get("mechanisms")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_invariance requires mechanisms".to_string())?,
        )
        .map_err(|error| format!("invalid glioma invariance mechanisms: {error}"))?;
        let signatures: Vec<MechanismSignature> = serde_json::from_value(
            arguments
                .get("signatures")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_invariance requires signatures".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism signatures: {error}"))?;
        let mut request = request;
        request.contexts = contexts;
        request.mechanisms = mechanisms;
        request.signatures = signatures;
        let output = analyze_glioma_mechanism_invariance(&request)
            .map_err(|error| format!("glioma mechanism invariance refused: {error}"))?;
        serde_json::to_value(json!({
            "invariance": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_mechanism_identifiability",
                "glioma_mechanism_discriminate",
                "plan_glioma_replication"
            ],
            "guarantees": [
                "directional consistency and mechanism separation are weighted across declared preclinical contexts",
                "context-reversing signatures remain below the invariance floor rather than being promoted",
                "panel selection is bounded by quality, risk, cost, budget, and pair coverage",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism invariance: {error}"))
    }

    /// Rank mechanistic perturbation assays by posterior-weighted separation and select a
    /// bounded, non-redundant research portfolio. This route never dispatches biology.
    pub(super) fn glioma_mechanism_intervention_value(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismInterventionValueRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_intervention_value requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma intervention-value request: {error}"))?;
        let candidates: Vec<MechanismInterventionCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_mechanism_intervention_value requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid glioma intervention-value candidates: {error}"))?;
        let output = analyze_glioma_mechanism_intervention_value(&request, &candidates)
            .map_err(|error| format!("glioma mechanism intervention value refused: {error}"))?;
        serde_json::to_value(json!({
            "intervention_value": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_discriminate",
                "glioma_mechanism_validation_plan",
                "glioma_information_design"
            ],
            "guarantees": [
                "posterior-weighted pairwise separation is discounted by declared prediction uncertainty",
                "selection is bounded by feasibility, risk, cost, budget, and redundancy-group gates",
                "low-information and high-uncertainty perturbations remain explicit deferred or negative evidence",
                "the route only prioritizes preclinical assays and never executes biology or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma intervention value: {error}"))
    }

    /// Update competing mechanism posteriors from typed local preclinical observations. This
    /// is a bounded scientific computation; it never dispatches an assay or makes a clinical
    /// decision.
    pub(super) fn glioma_mechanism_bayesian_update(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: BayesianMechanismUpdateRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_bayesian_update requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma Bayesian mechanism request: {error}"))?;
        let hypotheses: Vec<BayesianMechanismHypothesis> =
            serde_json::from_value(arguments.get("hypotheses").cloned().ok_or_else(|| {
                "glioma_mechanism_bayesian_update requires hypotheses".to_string()
            })?)
            .map_err(|error| format!("invalid glioma Bayesian mechanism hypotheses: {error}"))?;
        let observations: Vec<MechanismFeatureObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_mechanism_bayesian_update requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma Bayesian mechanism observations: {error}"))?;
        let update = update_glioma_mechanism_posterior(&request, &hypotheses, &observations)
            .map_err(|error| format!("glioma Bayesian mechanism update refused: {error}"))?;
        serde_json::to_value(json!({
            "update": update,
            "dispatch": "not_started",
            "next_route": "glioma_adaptive_mechanism_policy",
            "guarantees": [
                "posterior mass is integer-normalized and replay-stable",
                "missing shared features remain unresolved rather than being imputed",
                "low-posterior mechanisms and contradictory fits remain explicit negative evidence",
                "the route performs no assay, instrument, federation, raw-data, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma Bayesian mechanism update: {error}"))
    }

    /// Assimilate digest-bound Bayesian mechanism snapshots across study epochs without
    /// converting posterior state into an observed causal result.
    pub(super) fn glioma_mechanism_evidence_assimilation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismEvidenceAssimilationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_evidence_assimilation requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism assimilation request: {error}"))?;
        let assimilation = assimilate_glioma_mechanism_evidence(&request)
            .map_err(|error| format!("glioma mechanism evidence assimilation refused: {error}"))?;
        serde_json::to_value(json!({
            "assimilation": assimilation,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_discriminate",
                "glioma_mechanism_action_plan",
                "glioma_mechanism_operating_cycle"
            ],
            "guarantees": [
                "recency weighting is integer deterministic and snapshot digests remain bound",
                "mechanisms absent from a later snapshot remain explicit omissions",
                "negative, contradicted, unresolved, and posterior-trend states remain separate",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism evidence assimilation: {error}"))
    }

    /// Convert assimilated mechanism evidence into a bounded, uncertainty-aware next-action
    /// frontier. This route plans only; it never executes an assay or instrument action.
    pub(super) fn glioma_mechanism_closed_loop(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismClosedLoopRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_closed_loop requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism closed-loop request: {error}"))?;
        let plan = plan_glioma_mechanism_closed_loop(&request)
            .map_err(|error| format!("glioma mechanism closed-loop refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_discriminate",
                "glioma_mechanism_action_plan",
                "glioma_mechanism_operating_cycle"
            ],
            "guarantees": [
                "contradicted and unresolved mechanisms remain explicit and route to local information-gain actions",
                "budget, risk, and action-count bounds are enforced before any downstream dispatch",
                "negative evidence and uncertainty are preserved in the content-addressed plan",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism closed-loop plan: {error}"))
    }

    /// Select bounded model-system upgrades that reduce mechanism transport debt before
    /// downstream experiment or protocol dispatch.
    pub(super) fn glioma_mechanism_multi_fidelity_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiFidelityControlRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_multi_fidelity_control requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multi-fidelity control request: {error}"))?;
        let plan = plan_glioma_mechanism_multi_fidelity_control(&request)
            .map_err(|error| format!("glioma multi-fidelity control refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_closed_loop",
                "glioma_mechanism_action_plan",
                "glioma_experiment_frontier_controller"
            ],
            "guarantees": [
                "model-system escalation is strict, typed, bounded, and preclinical-only",
                "transport debt, missing target coverage, contradiction, and approval requirements remain explicit",
                "budget, risk, information-gain, and transport-gain gates run before downstream dispatch",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-fidelity control plan: {error}"))
    }

    /// Replan a bounded mechanism frontier from typed local outcomes while retaining
    /// contradictions, uncertainty, and missing feedback as first-class workflow state.
    pub(super) fn glioma_mechanism_feedback_replan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismFeedbackReplanRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_feedback_replan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism feedback replan request: {error}"))?;
        let replan = replan_glioma_mechanism_feedback(&request)
            .map_err(|error| format!("glioma mechanism feedback replan refused: {error}"))?;
        serde_json::to_value(json!({
            "replan": replan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_action_plan",
                "glioma_mechanism_operating_cycle",
                "glioma_mechanism_closed_loop"
            ],
            "guarantees": [
                "contradictions, unresolved outcomes, supported outcomes, and missing feedback remain explicit",
                "only validated local artifact references can influence the deterministic replan",
                "budget, risk, action-count, and approval gates run before downstream dispatch",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism feedback replan: {error}"))
    }

    /// Compile the returned mechanism frontier into a dependency-closed local workflow. The
    /// compiler only emits a plan; institution-local policy and executors own every physical or
    /// external effect.
    pub(super) fn glioma_mechanism_workflow_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismWorkflowRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_workflow_compile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism workflow request: {error}"))?;
        let plan = compile_glioma_mechanism_workflow(&request)
            .map_err(|error| format!("glioma mechanism workflow refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_workflow_assure",
                "glioma_mechanism_operating_cycle",
                "glioma_protocol_simulation",
                "glioma_instrument_preflight"
            ],
            "guarantees": [
                "prerequisite closure and deterministic topological execution waves are explicit",
                "cycle, dependency, risk, approval, provenance, action-count, and budget gates run before dispatch",
                "deferred and blocked actions remain visible rather than being silently dropped",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism workflow: {error}"))
    }

    /// Verify a local single-study mechanism workflow before handing its admitted actions to an
    /// institution-local executor. The assurance plan is a deterministic safety gate, not an
    /// execution receipt or a substitute for researcher authorization.
    pub(super) fn glioma_mechanism_workflow_assure(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismWorkflowAssuranceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_workflow_assure requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma workflow assurance request: {error}"))?;
        let plan = assure_glioma_mechanism_workflow(&request)
            .map_err(|error| format!("glioma mechanism workflow assurance refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_operating_cycle",
                "glioma_mechanism_feedback_replan",
                "glioma_mechanism_workflow_compile"
            ],
            "guarantees": [
                "dependency closure is replayed in topological order and failed prerequisites propagate downstream",
                "stale, missing, unresolved, contradicted, low-quality, and exhausted evidence remains explicit",
                "risk, approval, compensation, local-artifact, route, retry, action-count, and budget gates run before dispatch",
                "the route performs no assay, instrument execution, raw-data movement, federation side effect, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma workflow assurance: {error}"))
    }

    /// Join site-local mechanism workflows into an aggregate-only multimodal, multi-study
    /// portfolio. Lane executors and policy owners retain every raw-data and physical effect.
    pub(super) fn glioma_mechanism_multi_study_workflow_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismMultiStudyWorkflowRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_multi_study_workflow_compile requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multi-study workflow request: {error}"))?;
        let plan = compile_glioma_multi_study_mechanism_workflow(&request)
            .map_err(|error| format!("glioma multi-study mechanism workflow refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_workflow_compile",
                "glioma_mechanism_operating_cycle",
                "glioma_federated_benchmark"
            ],
            "guarantees": [
                "study, site, model-system, modality, replication, and transport coverage remain explicit",
                "only aggregate workflow metadata crosses the multi-study boundary; raw data stay local to lanes",
                "lane-local prerequisites, cycles, budget, wave, and task limits are enforced before dispatch",
                "the route performs no assay, instrument execution, raw-data movement, federation side effect, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study mechanism workflow: {error}"))
    }

    /// Control one prospective high-throughput mechanism epoch from typed local outcomes.
    /// Admission, retries, drift, queue pressure, resources, and budget are resolved before any
    /// downstream executor receives work; this route never executes biology or moves raw data.
    pub(super) fn glioma_mechanism_prospective_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismProspectiveControllerRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_prospective_control requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma prospective controller request: {error}"))?;
        let plan = control_glioma_mechanism_prospective_batch(&request)
            .map_err(|error| format!("glioma prospective controller refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_feedback_replan",
                "glioma_mechanism_workflow_assure",
                "glioma_mechanism_operating_cycle",
                "glioma_mechanism_multi_study_workflow_compile"
            ],
            "guarantees": [
                "rolling epochs retain missing, unresolved, contradicted, failed, stale, low-quality, and drifted outcomes explicitly",
                "resource, queue, inflight, retry, approval, and budget gates run before any downstream dispatch",
                "local artifacts remain de-identified and site-local; the controller never executes an assay or instrument",
                "the route performs no raw-data movement, federation side effect, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma prospective controller: {error}"))
    }

    /// Bridge mechanism prediction/observation residuals across model systems and expose the
    /// lowest-transport mechanisms for the next discriminating workflow.
    pub(super) fn glioma_mechanism_fidelity_bridge(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismFidelityBridgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_fidelity_bridge requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism fidelity request: {error}"))?;
        let bridge = bridge_glioma_mechanism_fidelity(&request)
            .map_err(|error| format!("glioma mechanism fidelity bridge refused: {error}"))?;
        serde_json::to_value(json!({
            "bridge": bridge,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_discriminate",
                "glioma_mechanism_action_plan",
                "glioma_mechanism_operating_cycle"
            ],
            "guarantees": [
                "cross-model transport is computed from typed prediction/observation residuals",
                "target-model gaps and low-transport mechanisms remain explicit negative or uncertain states",
                "all artifacts remain local, de-identified, and content-addressed",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism fidelity bridge: {error}"))
    }

    /// Stress mechanism ranking under bounded perturbations and explicit omission scenarios
    /// before admitting the next autonomous research action.
    pub(super) fn glioma_mechanism_robustness_stress(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismRobustnessStressRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_robustness_stress requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism robustness request: {error}"))?;
        let stress = stress_glioma_mechanism_robustness(&request)
            .map_err(|error| format!("glioma mechanism robustness stress refused: {error}"))?;
        serde_json::to_value(json!({
            "stress": stress,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_mechanism_discriminate",
                "glioma_mechanism_action_plan",
                "glioma_mechanism_operating_cycle"
            ],
            "guarantees": [
                "bounded perturbation scenarios and weights are explicit and replayable",
                "omitted mechanism-scenario pairs carry no fabricated score or rank",
                "rank reversals and measured-scenario worst/best scores and stability remain visible",
                "aggregate scores and stability remain absent when no scenario measured a mechanism",
                "fragile mechanisms are routed for more evidence rather than promoted automatically",
                "the route performs no assay, instrument execution, raw-data movement, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism robustness stress: {error}"))
    }

    /// Filter a transition-aware longitudinal mechanism posterior from local preclinical
    /// observations; this is inference only and never dispatches an assay.
    pub(super) fn glioma_mechanism_state_filter(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismStateFilterRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_state_filter requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism state filter request: {error}"))?;
        let filter = filter_glioma_mechanism_states(&request)
            .map_err(|error| format!("glioma mechanism state filter refused: {error}"))?;
        serde_json::to_value(json!({
            "filter": filter,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_mechanism_discriminate", "glioma_mechanism_dynamics", "glioma_mechanism_action_plan"],
            "guarantees": [
                "transition priors and local residual compatibility remain deterministic and integer-normalized",
                "measurement/process uncertainty, coverage gaps, feature-level negative evidence, and change points remain explicit",
                "filtered posteriors remain planning state and are never emitted as clinical or biological conclusions",
                "route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism state filter: {error}"))
    }

    /// Smooth a completed longitudinal mechanism trajectory with future evidence; this is
    /// retrospective inference only and never dispatches an assay or emits a clinical decision.
    pub(super) fn glioma_mechanism_state_smoother(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismStateSmootherRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_state_smoother requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism state smoother request: {error}"))?;
        let smoother = smooth_glioma_mechanism_states(&request)
            .map_err(|error| format!("glioma mechanism state smoother refused: {error}"))?;
        serde_json::to_value(json!({
            "smoother": smoother,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_mechanism_state_filter",
                "glioma_mechanism_discriminate",
                "glioma_mechanism_dynamics",
                "glioma_mechanism_action_plan"
            ],
            "guarantees": [
                "forward-backward smoothing is deterministic and integer-normalized",
                "future evidence can revise earlier latent-state support without rewriting observations",
                "transition support, coverage gaps, entropy, negative features, and change points remain explicit",
                "smoothed mechanism states remain planning evidence and are never emitted as clinical or biological conclusions",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism state smoother: {error}"))
    }

    /// Reconcile independent typed mechanism evidence streams while preserving source conflict
    /// and leave-one-source-out sensitivity; this route never executes an assay or clinical action.
    pub(super) fn glioma_mechanism_consensus(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismConsensusRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_consensus requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism consensus request: {error}"))?;
        let evidence = serde_json::from_value(
            arguments
                .get("evidence")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_consensus requires evidence".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism consensus evidence: {error}"))?;
        let request = MechanismConsensusRequest {
            evidence,
            ..request
        };
        let consensus = compile_glioma_mechanism_consensus(&request)
            .map_err(|error| format!("glioma mechanism consensus refused: {error}"))?;
        serde_json::to_value(json!({
            "consensus": consensus,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_mechanism_state_filter",
                "glioma_mechanism_state_smoother",
                "glioma_mechanism_discriminate",
                "glioma_mechanism_action_plan"
            ],
            "guarantees": [
                "evidence is aggregated by source rather than double-counted by record",
                "source reliability, coverage, conflict pairs, negative evidence, and leave-one-source-out sensitivity remain explicit",
                "consensus posteriors are planning evidence and are never emitted as causal or clinical conclusions",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism consensus: {error}"))
    }

    /// Calibrate mechanism probabilities against future local observations using deterministic
    /// reliability bins and a prequential holdout. This route evaluates value-only artifacts; it
    /// never refits a model, executes an assay, or turns calibration into a causal claim.
    pub(super) fn glioma_mechanism_calibrate(&self, arguments: &Value) -> Result<Value, String> {
        let request: MechanismCalibrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_calibrate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism calibration request: {error}"))?;
        let observations: Vec<MechanismCalibrationObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_calibrate requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism calibration observations: {error}"))?;
        let calibration: MechanismCalibration =
            calibrate_glioma_mechanisms(&request, &observations)
                .map_err(|error| format!("glioma mechanism calibration refused: {error}"))?;
        serde_json::to_value(json!({
            "calibration": calibration,
            "dispatch": "not_started",
            "guarantees": [
                "scores are computed from typed local value-only observations with fixed reliability bins",
                "the final observed round is reported as a prequential holdout and is never imputed",
                "calibration error, Brier loss, negative discordance, and underpowered mechanisms remain explicit",
                "the route does not refit a model, execute an assay, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism calibration: {error}"))
    }

    /// Compile discriminator information gain into executable local action candidates for the
    /// autonomous campaign controller. This route plans only; it never dispatches an assay.
    pub(super) fn glioma_mechanism_action_plan(&self, arguments: &Value) -> Result<Value, String> {
        let discrimination: MechanismDiscrimination =
            serde_json::from_value(arguments.get("discrimination").cloned().ok_or_else(|| {
                "glioma_mechanism_action_plan requires discrimination".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism discrimination: {error}"))?;
        let config: MechanismActionPlannerConfig = serde_json::from_value(
            arguments
                .get("config")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_action_plan requires config".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism action planner config: {error}"))?;
        let plan = compile_mechanism_action_plan(&discrimination, &config)
            .map_err(|error| format!("glioma mechanism action planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "residual-likelihood information gain is compiled into typed A1 local candidates",
                "ranking accounts for information per cost, feasibility, measurement uncertainty, and mechanism-unlock value",
                "source uncertainty and negative evidence remain attached to the plan",
                "the route does not invent observations, execute assays, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism action plan: {error}"))
    }

    /// Compile a finite-horizon, model-uncertainty-aware mechanism policy. This is a planner;
    /// it does not treat posterior mass as causal truth or dispatch an assay.
    pub(super) fn glioma_adaptive_mechanism_policy(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveMechanismPolicyRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_mechanism_policy requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive mechanism policy request: {error}"))?;
        let policy = plan_glioma_adaptive_mechanism_policy(&request)
            .map_err(|error| format!("glioma adaptive mechanism policy refused: {error}"))?;
        serde_json::to_value(json!({
            "policy": policy,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "posterior updates use bounded integer likelihoods from returned local observations",
                "action scores combine Gini information gain, expected effect, lower-tail robustness, feasibility, risk, cost, and redundancy",
                "beam selection is finite-horizon and budget bounded; no inferred estimate is emitted as an observation",
                "the route plans preclinical assays only and never makes a causal, clinical, or treatment decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive mechanism policy: {error}"))
    }

    /// Execute the adaptive mechanism policy through a deterministic sandbox worker. Production
    /// institutions call the research-crate executor seam with their own local assay gateway.
    pub(super) fn glioma_adaptive_mechanism_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveMechanismCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_mechanism_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma adaptive mechanism campaign request: {error}")
            })?;
        let mut executor = DryRunAdaptiveMechanismPolicyExecutor;
        let campaign = execute_glioma_adaptive_mechanism_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma adaptive mechanism campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "each round replans from the returned typed observation and never promotes synthetic output to biological evidence",
                "finite-horizon posterior, information, robustness, feasibility, retry, budget, and convergence gates remain explicit",
                "duplicate action observations, invalid artifacts, worker failure, and no-progress states fail closed",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive mechanism campaign: {error}"))
    }

    /// Execute calibration-aware adaptive mechanism selection through a deterministic sandbox.
    pub(super) fn glioma_calibrated_mechanism_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: CalibratedMechanismCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_calibrated_mechanism_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma calibrated mechanism campaign request: {error}")
            })?;
        let campaign = execute_glioma_calibrated_mechanism_campaign_dry_run(&request)
            .map_err(|error| format!("glioma calibrated mechanism campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "model-specific calibration trust discounts expected effects before adaptive action ranking",
                "calibration debt can earn an explicit bounded exploration bonus without qualifying an uncalibrated posterior",
                "held-out coverage, calibration error, Brier loss, posterior entropy, retry, budget, and no-progress gates remain explicit",
                "the route performs no instrument effect, raw-data movement, federation, diagnosis, treatment, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma calibrated mechanism campaign: {error}"))
    }

    /// Execute a bounded mechanism-discrimination loop through a local feature-measurement adapter.
    pub(super) fn glioma_mechanism_discrimination_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismDiscriminationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_discrimination_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism discrimination campaign request: {error}")
            })?;
        let mut executor = DryRunMechanismDiscriminationCampaignExecutor;
        let campaign = execute_glioma_mechanism_discrimination_campaign(&request, &mut executor)
            .map_err(|error| {
                format!("glioma mechanism discrimination campaign refused: {error}")
            })?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the next feature is selected from posterior-weighted information gain and recomputed after every returned observation",
                "feature observations are claim-scoped, local-only, artifact-validated, and never synthesized into clinical or causal conclusions",
                "diffuse mechanisms, missing features, negative evidence, retries, budget, no-progress, and executor failure remain explicit",
                "the route performs no instrument effect or raw-data movement; production adapters remain institution-local"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism discrimination campaign: {error}"))
    }

    /// Run the complete P05 mechanism loop in the deterministic MCP sandbox: discriminate,
    /// execute a bounded local observation campaign, and compile the next typed assay package.
    pub(super) fn glioma_mechanism_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism operating-cycle request: {error}"))?;
        let mut executor = DryRunMechanismDiscriminationCampaignExecutor;
        let cycle = execute_glioma_mechanism_operating_cycle(&request, &mut executor)
            .map_err(|error| format!("glioma mechanism operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "competing mechanism discrimination is recomputed after every returned local observation",
                "the final information-gain ranking is compiled into a typed A1 assay work package",
                "negative evidence, uncertainty, retries, budget exhaustion, no-progress, and diffuse mechanisms remain explicit",
                "the sandbox emits metadata-only observations and performs no instrument effect, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism operating cycle: {error}"))
    }

    /// Propagate signed mechanistic support over a local preclinical glioma evidence graph. This
    /// is a bounded fixed-point calculation only; it never executes an assay or makes a clinical
    /// decision.
    pub(super) fn glioma_mechanism_graph_propagate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismGraphRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_graph_propagate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism graph request: {error}"))?;
        let nodes: Vec<MechanismGraphNode> = serde_json::from_value(
            arguments
                .get("nodes")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_graph_propagate requires nodes".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism graph nodes: {error}"))?;
        let edges: Vec<MechanismGraphEdge> = serde_json::from_value(
            arguments
                .get("edges")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_graph_propagate requires edges".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism graph edges: {error}"))?;
        let output = propagate_glioma_mechanism_graph(&request, &nodes, &edges)
            .map_err(|error| format!("glioma mechanism graph propagation refused: {error}"))?;
        serde_json::to_value(json!({
            "propagation": output,
            "dispatch": "not_started",
            "guarantees": [
                "activating and inhibiting edges propagate signed support with bounded fixed-point arithmetic",
                "low-confidence edges, disconnected nodes, contradiction, and non-convergence remain visible",
                "ranking is deterministic and top-k bounded without turning scores into claims of truth",
                "the route produces local preclinical mechanism priorities only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism graph propagation: {error}"))
    }

    /// Compare a mathematical mechanism-network baseline with signed node perturbations. The
    /// result prioritizes assays and never represents a causal estimate or clinical advice.
    pub(super) fn glioma_mechanism_counterfactual(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: CounterfactualRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_counterfactual requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism counterfactual request: {error}"))?;
        let nodes: Vec<MechanismGraphNode> = serde_json::from_value(
            arguments
                .get("nodes")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_counterfactual requires nodes".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism counterfactual nodes: {error}"))?;
        let edges: Vec<MechanismGraphEdge> = serde_json::from_value(
            arguments
                .get("edges")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_counterfactual requires edges".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism counterfactual edges: {error}"))?;
        let interventions: Vec<CounterfactualIntervention> =
            serde_json::from_value(arguments.get("interventions").cloned().ok_or_else(|| {
                "glioma_mechanism_counterfactual requires interventions".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism counterfactual interventions: {error}")
            })?;
        let output = simulate_glioma_counterfactual(&request, &nodes, &edges, &interventions)
            .map_err(|error| format!("glioma mechanism counterfactual refused: {error}"))?;
        serde_json::to_value(json!({
            "counterfactual": output,
            "dispatch": "not_started",
            "guarantees": [
                "baseline and intervention fixed points use bounded signed propagation with convergence gates",
                "activating and inhibiting effects, low-confidence edges, and unresolved non-convergence remain explicit",
                "target ranking is a mathematical assay-prioritization artifact and not a causal or clinical conclusion",
                "the route never executes a perturbation or moves raw preclinical data"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism counterfactual: {error}"))
    }

    /// Run the same signed mechanism perturbation across multiple local models and expose only
    /// targets that survive an explicit model-agreement gate. This route never executes biology.
    pub(super) fn glioma_mechanism_ensemble_counterfactual(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: CounterfactualEnsembleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_ensemble_counterfactual requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism ensemble request: {error}"))?;
        let models: Vec<CounterfactualModel> =
            serde_json::from_value(arguments.get("models").cloned().ok_or_else(|| {
                "glioma_mechanism_ensemble_counterfactual requires models".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism ensemble models: {error}"))?;
        let interventions: Vec<CounterfactualIntervention> =
            serde_json::from_value(arguments.get("interventions").cloned().ok_or_else(|| {
                "glioma_mechanism_ensemble_counterfactual requires interventions".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism ensemble interventions: {error}"))?;
        let output = simulate_glioma_counterfactual_ensemble(&request, &models, &interventions)
            .map_err(|error| {
                format!("glioma mechanism ensemble counterfactual refused: {error}")
            })?;
        serde_json::to_value(json!({
            "ensemble": output,
            "dispatch": "not_started",
            "guarantees": [
                "effects are model-prior weighted and each model simulation remains inspectable",
                "target direction is unresolved below the declared model-agreement floor",
                "model averaging is an assay-prioritization artifact and not a causal or clinical conclusion",
                "the route never executes a perturbation or moves raw preclinical data"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism ensemble counterfactual: {error}"))
    }

    /// Compile a robust, risk-aware intervention portfolio across local mechanistic models. The
    /// route performs no biological dispatch; an institution-local executor must separately
    /// authorize and run any selected assay.
    pub(super) fn glioma_robust_intervention_portfolio(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: RobustInterventionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_robust_intervention_portfolio requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma robust portfolio request: {error}"))?;
        let models: Vec<CounterfactualModel> =
            serde_json::from_value(arguments.get("models").cloned().ok_or_else(|| {
                "glioma_robust_intervention_portfolio requires models".to_string()
            })?)
            .map_err(|error| format!("invalid glioma robust portfolio models: {error}"))?;
        let candidates: Vec<RobustInterventionCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_robust_intervention_portfolio requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid glioma robust portfolio candidates: {error}"))?;
        let output = plan_glioma_robust_intervention_portfolio(&request, &models, &candidates)
            .map_err(|error| format!("glioma robust intervention portfolio refused: {error}"))?;
        serde_json::to_value(json!({
            "portfolio": output,
            "dispatch": "not_started",
            "guarantees": [
                "each candidate is simulated independently across the declared local model ensemble",
                "selection uses prior-weighted expected, lower-tail, worst-case, feasibility, risk, cost, and redundancy gates",
                "model disagreement, risk blocks, budget deferrals, and unresolved effects remain explicit",
                "the route only compiles a preclinical assay portfolio and never executes biology or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma robust intervention portfolio: {error}"))
    }

    /// Bridge a robust mechanistic intervention portfolio into a power-aware validation queue.
    /// This route is still planning-only: it composes P05 model uncertainty with P06 interim
    /// boundaries and returns the exact missing bindings or next local batch for a researcher.
    pub(super) fn glioma_mechanism_validation_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismValidationPlanRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_mechanism_validation_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma mechanism-validation request: {error}"))?;
        let output = plan_glioma_mechanism_validation(&request)
            .map_err(|error| format!("glioma mechanism validation refused: {error}"))?;
        serde_json::to_value(json!({
            "validation": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_replication_plan",
                "glioma_replication_protocol_compile",
                "glioma_analysis_run"
            ],
            "guarantees": [
                "only robustly selected mechanism candidates can enter the validation queue",
                "power, interim efficacy, futility, risk, budget, and replicate gates remain typed and explicit",
                "missing local observations never become fabricated null results or confidence",
                "negative and underpowered validation outcomes remain first-class research evidence",
                "the route never dispatches an assay, instrument, federation, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism validation plan: {error}"))
    }

    /// Pool measured arm summaries from a completed validation batch and advance the bounded P06
    /// interim controller. Task artifacts alone are never treated as measurements.
    pub(super) fn glioma_validation_batch_assess(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ValidationBatchAssessmentRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_validation_batch_assess requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma validation-batch assessment request: {error}"))?;
        let assessment = assess_glioma_validation_batch(&request)
            .map_err(|error| format!("glioma validation batch assessment refused: {error}"))?;
        serde_json::to_value(json!({
            "assessment": assessment,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_mechanism_validation_protocol_compile",
                "glioma_replication_plan",
                "glioma_mechanism_validation_plan"
            ],
            "guarantees": [
                "new summaries must bind to scheduled and non-withheld validation arms",
                "prior and new arm summaries are pooled with bounded integer arithmetic before re-estimation",
                "partial execution and withheld arms cannot silently become scientific observations",
                "efficacy, futility, risk, budget, and underpowered decisions remain explicit",
                "the route never makes a clinical decision or dispatches a physical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma validation batch assessment: {error}"))
    }

    /// Run the bounded P05-to-P07 validation loop with the deterministic local protocol worker.
    /// Measured batches are caller-supplied and remain distinct from synthetic task artifacts.
    pub(super) fn glioma_validation_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ValidationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_validation_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma validation campaign request: {error}"))?;
        let mut executor = DryRunGliomaProtocolExecutor;
        let run = execute_glioma_validation_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma validation campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": run,
            "execution_mode": "dry_run_local_worker",
            "physical_dispatch": false,
            "simulation_only": true,
            "next_routes": [
                "glioma_validation_campaign_execute",
                "glioma_mechanism_validation_protocol_compile",
                "glioma_replication_plan"
            ],
            "guarantees": [
                "each round replans from the prior typed validation state before compiling a protocol",
                "synthetic task artifacts never become measured biological observations",
                "only caller-supplied measured summaries can advance an interim look",
                "stopping, risk, budget, underpowered, incomplete, and unresolved outcomes remain explicit",
                "the dry-run worker cannot touch specimens, instruments, federation, or clinical decisions"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma validation campaign: {error}"))
    }

    /// Admit a completed local validation result to independent-site replication only when the
    /// typed validation controller stopped for efficacy. The route plans the cross-site topology,
    /// continuation wave, and deterministic local preflight; it never treats a local signal as a
    /// preclinical conclusion or dispatches a physical effect.
    pub(super) fn glioma_validation_replication_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ValidationReplicationGateRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_validation_replication_gate requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma validation-replication gate request: {error}")
            })?;
        let gate = plan_glioma_validation_replication_gate(&request)
            .map_err(|error| format!("glioma validation-replication gate refused: {error}"))?;
        serde_json::to_value(json!({
            "gate": gate,
            "dispatch": "not_started",
            "simulation_only": true,
            "physical_dispatch": false,
            "next_routes": [
                "glioma_replication_continuation",
                "glioma_replication_protocol_compile",
                "glioma_replication_campaign_execute",
                "glioma_replication_meta_analyze"
            ],
            "guarantees": [
                "only a typed local efficacy stop can enter the independent-site replication workflow",
                "origin-site observations are rejected as independent evidence",
                "site heterogeneity, quality, leave-one-site-out influence, risk, budget, and protocol holds remain explicit",
                "negative or null replication evidence remains first-class and cannot be promoted",
                "the route never dispatches an assay, instrument, federation, raw data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma validation-replication gate: {error}"))
    }

    /// Execute the guarded P10 multi-site replication/interpretation campaign after validation
    /// admission. MCP uses the deterministic synthetic worker; institution-owned executors remain
    /// the only path to local compute, instruments, or biological effects.
    pub(super) fn glioma_validation_replication_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ValidationReplicationCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_validation_replication_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma validation-replication campaign request: {error}")
            })?;
        let mut executor = DryRunGliomaReplicationCampaignExecutor::default();
        let run = execute_glioma_validation_replication_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma validation-replication campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": run,
            "execution_mode": "dry_run_local_worker",
            "physical_dispatch": false,
            "simulation_only": true,
            "next_routes": [
                "glioma_validation_replication_campaign_execute",
                "glioma_replication_meta_analyze",
                "glioma_federated_mechanism_transport",
                "glioma_research_object_prepare"
            ],
            "guarantees": [
                "only an efficacy-qualified validation gate with non-origin sites can invoke the P10 campaign",
                "site-level replication, meta-analysis, transportability, negative results, and bounded next actions remain typed",
                "the dry-run worker never promotes synthetic observations to biological evidence",
                "failed, partial, heterogeneous, underpowered, blocked, and negative states remain explicit",
                "the route never dispatches an instrument, moves raw data, exports federation payloads, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma validation-replication campaign: {error}"))
    }

    /// Continue an eligible independent-site replication result into the aggregate-only P12
    /// mechanism transport campaign. MCP uses the deterministic federation worker; institution
    /// gateways retain raw observations and all physical execution authority.
    pub(super) fn glioma_replication_federated_transport_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ValidationReplicationTransportRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_federated_transport_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma replication-federation transport request: {error}")
            })?;
        let mut executor = DryRunFederatedMechanismTransportExecutor;
        let run = execute_validation_replication_transport(&request, &mut executor)
            .map_err(|error| format!("glioma replication-federation transport refused: {error}"))?;
        serde_json::to_value(json!({
            "transport": run,
            "execution_mode": "dry_run_aggregate_worker",
            "physical_dispatch": false,
            "simulation_only": true,
            "next_routes": [
                "glioma_replication_federated_transport_execute",
                "glioma_federated_mechanism_transport_campaign_execute",
                "glioma_research_object_prepare"
            ],
            "guarantees": [
                "only validated non-origin aggregate mechanism summaries enter the federation campaign",
                "blocked, held, negative, heterogeneous, unresolved, and budget-limited replication states remain explicit",
                "raw traces, specimen data, human data, instrument effects, and direct identifiers remain local",
                "the synthetic worker cannot become biological evidence or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication-federation transport: {error}"))
    }

    /// Rank the next bounded scientific closure actions after an independent-site replication
    /// result. This is a planning controller: it never dispatches an assay, instrument,
    /// federation export, or clinical decision.
    pub(super) fn glioma_replication_closure_frontier(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplicationClosureFrontierRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_closure_frontier requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma replication-closure request: {error}"))?;
        let frontier = plan_glioma_replication_closure_frontier(&request)
            .map_err(|error| format!("glioma replication-closure frontier refused: {error}"))?;
        serde_json::to_value(json!({
            "frontier": frontier,
            "dispatch": "not_started",
            "simulation_only": true,
            "physical_dispatch": false,
            "next_routes": [
                "glioma_validation_replication_campaign_execute",
                "glioma_replication_federated_transport_execute",
                "glioma_research_object_prepare"
            ],
            "guarantees": [
                "qualified and negative replication outcomes remain explicit holds",
                "candidate actions are ranked only after cost, risk, feasibility, and upstream-state gates",
                "the frontier never invents observations or converts a research result into a clinical decision",
                "selected routes remain institution-owned and require their own typed execution gates"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication-closure frontier: {error}"))
    }

    /// Execute a selected closure-frontier action through the bounded replication campaign.
    /// The MCP worker is deterministic and local; institution-owned executors remain the only
    /// path to physical, protected-data, or federated effects.
    pub(super) fn glioma_replication_closure_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplicationClosureExecutionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_closure_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma replication-closure execution request: {error}")
            })?;
        let mut executor = DryRunGliomaReplicationCampaignExecutor::default();
        let run = execute_glioma_replication_closure(&request, &mut executor)
            .map_err(|error| format!("glioma replication-closure execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": run,
            "execution_mode": "dry_run_local_worker",
            "physical_dispatch": false,
            "simulation_only": true,
            "next_routes": [
                "glioma_replication_closure_frontier",
                "glioma_validation_replication_campaign_execute",
                "glioma_replication_federated_transport_execute",
                "glioma_research_object_prepare"
            ],
            "guarantees": [
                "only explicitly selected closure actions naming the bounded replication route can dispatch",
                "blocked, held, negative, partial, unresolved, and qualified outcomes remain typed",
                "the dry-run worker cannot touch specimens, instruments, protected raw data, federation, or clinical decisions",
                "frontier and campaign digests remain available for deterministic replay and audit"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication-closure execution: {error}"))
    }

    /// Execute a bounded sequence of closure frontiers through the guarded P10 execution seam.
    /// Each frontier is caller-declared, but the campaign owns global budget reservation,
    /// duplicate-action protection, terminal stopping, and replayable round accounting.
    pub(super) fn glioma_replication_closure_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReplicationClosureCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_closure_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma replication-closure campaign request: {error}")
            })?;
        let mut executor = DryRunGliomaReplicationCampaignExecutor::default();
        let run = execute_glioma_replication_closure_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma replication-closure campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": run,
            "execution_mode": "dry_run_local_worker",
            "physical_dispatch": false,
            "simulation_only": true,
            "next_routes": [
                "glioma_replication_closure_frontier",
                "glioma_replication_closure_execute",
                "glioma_replication_federated_transport_execute",
                "glioma_research_object_prepare"
            ],
            "guarantees": [
                "each round is admitted through the guarded closure executor",
                "global budget, duplicate actions, terminal stops, held frontiers, and unresolved evidence remain explicit",
                "the dry-run worker cannot touch specimens, instruments, protected raw data, federation, or clinical decisions",
                "round and campaign digests support deterministic replay and independent review"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma replication-closure campaign: {error}"))
    }

    /// Add observed closure-campaign replication summaries to the cross-family P10 interpreter.
    /// The route is analysis-only: unresolved and negative campaign states stay unresolved or
    /// negative, and no clinical, physical, federation, or publication action is dispatched.
    pub(super) fn glioma_replication_closure_interpret(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ClosureInterpretationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_replication_closure_interpret requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma closure-interpretation request: {error}"))?;
        let interpretation = interpret_glioma_replication_closure(&request)
            .map_err(|error| format!("glioma closure interpretation refused: {error}"))?;
        serde_json::to_value(json!({
            "interpretation": interpretation,
            "execution_mode": "local_analysis",
            "physical_dispatch": false,
            "simulation_only": false,
            "next_routes": [
                "glioma_replication_closure_frontier",
                "glioma_replication_closure_campaign_execute",
                "glioma_research_object_prepare",
                "glioma_release_operating_cycle"
            ],
            "guarantees": [
                "only observed executable closure campaigns become replication-family evidence",
                "held, unresolved, negative, partial, and contradictory evidence remains explicit",
                "the synthesis is bounded by declared model, objective, uncertainty, and replication-family gates",
                "the route never makes a clinical decision or moves raw protected data"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma closure interpretation: {error}"))
    }

    /// Compile the still-open mechanism validation decisions into a deterministic local P07
    /// protocol and preflight it against caller-declared culture/compute resources. This route
    /// never executes an assay, instrument, federation, or clinical decision.
    pub(super) fn glioma_mechanism_validation_protocol_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismValidationProtocolCompileRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_validation_protocol_compile requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism-validation protocol request: {error}")
            })?;
        let output = compile_glioma_mechanism_validation_protocol(&request)
            .map_err(|error| format!("glioma mechanism validation protocol refused: {error}"))?;
        serde_json::to_value(json!({
            "compilation": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_protocol_autonomous_execute",
                "glioma_instrument_preflight",
                "glioma_replication_plan"
            ],
            "guarantees": [
                "only continue and underpowered validation arms are materialized as protocol tasks",
                "efficacy, futility, risk, and budget stops remain withheld with typed evidence",
                "protocol scheduling and preflight are deterministic and resource-bounded",
                "the caller-owned executor remains responsible for approvals, interlocks, and every physical effect",
                "the route never performs a clinical decision or moves raw protected data"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism validation protocol: {error}"))
    }

    /// Execute a compiled mechanism-validation protocol through the deterministic local worker.
    /// Real institution-owned effects remain behind the Rust executor seam; this MCP route only
    /// performs a synthetic run and returns typed artifacts for workflow integration tests.
    pub(super) fn glioma_mechanism_validation_protocol_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MechanismValidationExecutionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_validation_protocol_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism-validation execution request: {error}")
            })?;
        let mut executor = DryRunGliomaProtocolExecutor;
        let execution = execute_glioma_mechanism_validation_protocol(&request, &mut executor)
            .map_err(|error| format!("glioma mechanism validation execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": execution,
            "execution_mode": "dry_run_local_worker",
            "physical_dispatch": false,
            "simulation_only": true,
            "next_routes": [
                "glioma_mechanism_validation_plan",
                "glioma_validation_batch_assess",
                "glioma_validation_campaign_execute",
                "glioma_validation_replication_gate",
                "glioma_validation_replication_campaign_execute",
                "glioma_mechanism_validation_protocol_compile",
                "glioma_protocol_execute"
            ],
            "guarantees": [
                "held or unresolved validation compilations are blocked before task execution",
                "compiled protocols are re-simulated and dependency-checked before each run",
                "partial, failed, skipped, and negative task outcomes remain explicit",
                "the synthetic worker creates local artifacts only and cannot touch instruments or specimens",
                "measured observations must be attached by an institution-owned workflow before scientific re-planning"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism validation execution: {error}"))
    }
}
