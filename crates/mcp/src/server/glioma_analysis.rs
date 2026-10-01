//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    /// Fit a lineage-resolved finite-interval propagation operator from local preclinical snapshots.
    pub(super) fn glioma_lineage_propagation_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LineagePropagationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_lineage_propagation_analyze requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma lineage propagation request: {error}"))?;
        let snapshots: Vec<LineagePropagationSnapshot> =
            serde_json::from_value(arguments.get("snapshots").cloned().ok_or_else(|| {
                "glioma_lineage_propagation_analyze requires snapshots".to_string()
            })?)
            .map_err(|error| format!("invalid glioma lineage propagation snapshots: {error}"))?;
        let analysis = analyze_glioma_lineage_propagation(&request, &snapshots)
            .map_err(|error| format!("glioma lineage propagation analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "lineage transitions describe effective interval propagation and descendant yield, not individual-cell switching probabilities",
                "independent experimental units, assay-batch sensitivity, bootstrap uncertainty, and held-out prediction remain explicit",
                "missing lineages and unresolved design ranks are not silently imputed"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma lineage propagation analysis: {error}"))
    }

    /// Decompose lineage propagation contrasts into net-yield and state-composition components.
    pub(super) fn glioma_lineage_response_decompose(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LineageResponseDecompositionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_lineage_response_decompose requires request".to_string())?,
        )
        .map_err(|error| {
            format!("invalid glioma lineage response decomposition request: {error}")
        })?;
        let analysis: LineagePropagationAnalysis =
            serde_json::from_value(arguments.get("analysis").cloned().ok_or_else(|| {
                "glioma_lineage_response_decompose requires analysis".to_string()
            })?)
            .map_err(|error| format!("invalid glioma lineage propagation analysis: {error}"))?;
        let decomposition = analyze_glioma_lineage_response_decomposition(&request, &analysis)
            .map_err(|error| format!("glioma lineage response decomposition refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": decomposition,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "control and treatment propagation are standardized to the investigator-declared baseline state mixture",
                "net descendant yield and destination composition are separated with reconstruction residuals and paired bootstrap uncertainty",
                "yield combines proliferation and death, and composition does not identify individual-cell switching"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma lineage response decomposition: {error}"))
    }

    /// Audit lineage-propagation contrasts across explicitly represented preclinical model systems.
    pub(super) fn glioma_lineage_transport_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LineageTransportRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_lineage_transport_analyze requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma lineage transport request: {error}"))?;
        let studies: Vec<LineageTransportStudy> = serde_json::from_value(
            arguments
                .get("studies")
                .cloned()
                .ok_or_else(|| "glioma_lineage_transport_analyze requires studies".to_string())?,
        )
        .map_err(|error| format!("invalid glioma lineage transport studies: {error}"))?;
        let analysis = analyze_glioma_lineage_transport(&request, &studies)
            .map_err(|error| format!("glioma lineage transport analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "studies are equally weighted within represented model systems, which are equally weighted in the transport summary",
                "excluded studies, system-specific effects, heterogeneity, and leave-one-system-out shifts remain visible",
                "transport describes observed preclinical systems and does not imply generalization to unobserved systems or clinical populations"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma lineage transport analysis: {error}"))
    }

    /// Analyze local longitudinal preclinical glioma observations with deterministic per-unit
    /// slopes. This remains a value-only computation: it never fetches data, dispatches an assay,
    /// or turns a trajectory into a clinical conclusion.
    pub(super) fn glioma_trajectory_analyze(&self, arguments: &Value) -> Result<Value, String> {
        let request: TrajectoryRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_trajectory_analyze requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma trajectory request: {error}"))?;
        let observations: Vec<TrajectoryObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_trajectory_analyze requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma trajectory observations: {error}"))?;
        let output = analyze_glioma_trajectories(&request, &observations)
            .map_err(|error| format!("glioma trajectory analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "per-unit slopes use deterministic integer least squares",
                "missing, duplicate, noisy, and non-monotone trajectories remain explicit",
                "time-grid and unit-floor failures are unresolved rather than imputed",
                "the output is preclinical research analysis, not clinical decision support"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma trajectory analysis: {error}"))
    }

    /// Estimate longitudinal discrete-state transition matrices for local preclinical glioma
    /// observations. This is descriptive research analysis only: it never executes an assay,
    /// moves raw data, or produces a clinical decision.
    pub(super) fn glioma_state_transition_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: StateTransitionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_state_transition_analyze requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma state-transition request: {error}"))?;
        let observations: Vec<StateTransitionObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_state_transition_analyze requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma state-transition observations: {error}"))?;
        let output = analyze_glioma_state_transitions(&request, &observations)
            .map_err(|error| format!("glioma state-transition analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "transition probabilities use deterministic consecutive within-unit observations",
                "irregular windows, absent transitions, sparse arms, null contrasts, and negative evidence remain explicit",
                "state ordering is investigator-declared and is never interpreted as a clinical severity scale",
                "the output is preclinical research analysis, not clinical decision support"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma state-transition analysis: {error}"))
    }

    /// Estimate whether a preclinical glioma effect transports to a declared target model system.
    /// Similarity, heterogeneity, and leave-one-study-out sensitivity stay visible; this route
    /// never turns transportability into a clinical or treatment recommendation.
    pub(super) fn glioma_transportability_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: TransportabilityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_transportability_analyze requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma transportability request: {error}"))?;
        let studies: Vec<TransportStudy> = serde_json::from_value(
            arguments
                .get("studies")
                .cloned()
                .ok_or_else(|| "glioma_transportability_analyze requires studies".to_string())?,
        )
        .map_err(|error| format!("invalid glioma transportability studies: {error}"))?;
        let analysis = analyze_glioma_transportability(&request, &studies)
            .map_err(|error| format!("glioma transportability analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "guarantees": [
                "source effects are weighted by declared signature similarity, quality, replicates, and uncertainty",
                "heterogeneity, target-model distance, excluded studies, and leave-one-out shifts remain explicit",
                "insufficient, negative, distant, and unstable evidence cannot be promoted into portability",
                "the route performs preclinical interpretation only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma transportability analysis: {error}"))
    }

    /// Analyze repeated independent-study effects on one predeclared time grid.
    pub(super) fn glioma_longitudinal_transport_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LongitudinalTransportRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_longitudinal_transport_analyze requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma longitudinal-transport request: {error}"))?;
        let observations: Vec<LongitudinalTransportObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_longitudinal_transport_analyze requires observations".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma longitudinal-transport observations: {error}")
            })?;
        let analysis = analyze_glioma_longitudinal_transport(&request, &observations)
            .map_err(|error| format!("glioma longitudinal transport refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "guarantees": [
                "every observation binds to the declared estimand, effect unit, timepoint, and local artifact",
                "one common eligible study set is used at every required timepoint; incomplete trajectories are excluded, never imputed",
                "quorum, signature and categorical model-system gaps, heterogeneity, leave-one-study-out influence, and trend-direction gates remain explicit",
                "stable sub-threshold trends remain negative evidence; no raw data moves and no clinical decision is produced"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma longitudinal-transport analysis: {error}"))
    }

    /// Compare estimand-aligned study effect intervals without pooling or crossing model strata.
    pub(super) fn glioma_multistudy_concordance_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiStudyConcordanceRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multistudy_concordance_analyze requires request".to_string()
            })?)
            .map_err(|error| format!("invalid multi-study concordance request: {error}"))?;
        let studies: Vec<MultiStudyEffect> =
            serde_json::from_value(arguments.get("studies").cloned().ok_or_else(|| {
                "glioma_multistudy_concordance_analyze requires studies".to_string()
            })?)
            .map_err(|error| format!("invalid multi-study concordance study rows: {error}"))?;
        let analysis = analyze_glioma_multistudy_concordance(&request, &studies)
            .map_err(|error| format!("multi-study concordance analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "guarantees": [
                "every row binds to the exact declared estimand, effect unit, source report, independent group, and local de-identified artifact",
                "same-model study pairs preserve full interval gap and confident opposite-direction states",
                "cross-model pairs are retained as incomparable and are never presented as transport evidence",
                "low-quality, null, and unresolved rows remain visible; unresolved effects are never imputed",
                "the output reports pairwise concordance only; it does not pool study effects or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode multi-study concordance analysis: {error}"))
    }

    /// Audit registry plans against reviewed result-report completeness metadata.
    pub(super) fn glioma_registered_outcome_reporting_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: OutcomeReportingAuditRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_registered_outcome_reporting_audit requires request".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome reporting request: {error}"))?;
        let studies: Vec<RegisteredOutcomeStudy> =
            serde_json::from_value(arguments.get("studies").cloned().ok_or_else(|| {
                "glioma_registered_outcome_reporting_audit requires studies".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome study rows: {error}"))?;
        let audit = audit_glioma_registered_outcome_reporting(&request, &studies)
            .map_err(|error| format!("registered-outcome reporting audit refused: {error}"))?;
        serde_json::to_value(json!({
            "audit": audit,
            "dispatch": "not_started",
            "guarantees": [
                "every registry and result report is digest-bound to local de-identified artifacts",
                "overdue primary outcomes are distinguished from not-yet-due and unknown outcomes",
                "reported outcomes absent from a registry and retrospective registration remain review signals, not misconduct findings",
                "the audit accepts no effect values, infers no publication bias, and dispatches no research action"
            ]
        }))
        .map_err(|error| format!("cannot encode registered-outcome reporting audit: {error}"))
    }

    /// Validate and seal one study-level registered-outcome evidence record.
    pub(super) fn glioma_registered_outcome_record_build(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let study: OutcomeSensitivityStudy =
            serde_json::from_value(arguments.get("study").cloned().ok_or_else(|| {
                "glioma_registered_outcome_record_build requires study".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome study record: {error}"))?;
        let record = build_glioma_registered_outcome_record(&study)
            .map_err(|error| format!("registered-outcome record refused: {error}"))?;
        serde_json::to_value(json!({
            "record": record,
            "dispatch": "not_started",
            "guarantees": [
                "one study binds one exact outcome, estimand, unit, report set, and local de-identified artifacts",
                "availability and status-specific effect fields are validated without cross-study inference",
                "the record carries a deterministic digest and performs no research dispatch"
            ]
        }))
        .map_err(|error| format!("cannot encode registered-outcome evidence record: {error}"))
    }

    /// Build a canonical typed panel from digest-validated independent study records.
    pub(super) fn glioma_registered_outcome_evidence_panel(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: RegisteredOutcomeEvidencePanelRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_registered_outcome_evidence_panel requires request".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome panel request: {error}"))?;
        let records: Vec<RegisteredOutcomeRecord> =
            serde_json::from_value(arguments.get("records").cloned().ok_or_else(|| {
                "glioma_registered_outcome_evidence_panel requires records".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome panel records: {error}"))?;
        let panel = build_glioma_registered_outcome_evidence_panel(&request, &records)
            .map_err(|error| format!("registered-outcome evidence panel refused: {error}"))?;
        serde_json::to_value(json!({
            "panel": panel,
            "dispatch": "not_started",
            "guarantees": [
                "every input is a replay-validated single-study record for the exact panel outcome/estimand/unit",
                "independent groups and exact canonical source-report digest coverage are enforced",
                "availability states remain separate and the panel performs no pooling or directional inference",
                "the panel is a local typed data primitive and dispatches no research action"
            ]
        }))
        .map_err(|error| format!("cannot encode registered-outcome evidence panel: {error}"))
    }

    /// Bound the impact of caller-declared missing-result values on registered-outcome support.
    /// Scenarios remain separate from observed estimates and never dispatch research actions.
    pub(super) fn glioma_registered_outcome_sensitivity_analyze(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: OutcomeSensitivityRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_registered_outcome_sensitivity_analyze requires request".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome sensitivity request: {error}"))?;
        let studies: Vec<OutcomeSensitivityStudy> =
            serde_json::from_value(arguments.get("studies").cloned().ok_or_else(|| {
                "glioma_registered_outcome_sensitivity_analyze requires studies".to_string()
            })?)
            .map_err(|error| format!("invalid registered-outcome sensitivity rows: {error}"))?;
        let sensitivity = analyze_glioma_outcome_missingness_sensitivity(&request, &studies)
            .map_err(|error| format!("registered-outcome sensitivity analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "sensitivity": sensitivity,
            "dispatch": "not_started",
            "guarantees": [
                "observed study estimates remain separate from caller-declared missing-result scenarios",
                "independent groups, exact outcome/estimand/unit, report digests, and local artifacts are validated",
                "missing effects are never imputed, pooled, or interpreted as publication bias",
                "the sensitivity analysis does not dispatch research actions or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode registered-outcome sensitivity analysis: {error}"))
    }

    /// Compile a replayable, bounded resolver portfolio for independently supported rival claims.
    pub(super) fn glioma_prospective_contradiction_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProspectiveContradictionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_prospective_contradiction_plan requires request".to_string()
            })?)
            .map_err(|error| format!("invalid prospective contradiction request: {error}"))?;
        let evidence: Vec<ProspectiveContradictionEvidence> =
            serde_json::from_value(arguments.get("evidence").cloned().ok_or_else(|| {
                "glioma_prospective_contradiction_plan requires evidence".to_string()
            })?)
            .map_err(|error| format!("invalid prospective contradiction evidence: {error}"))?;
        let candidates: Vec<ProspectiveResolverCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_prospective_contradiction_plan requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid prospective resolver candidates: {error}"))?;
        let plan = analyze_glioma_prospective_contradiction(&request, &evidence, &candidates)
            .map_err(|error| format!("prospective contradiction plan refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "rival claims, estimand, effect unit, evidence reports, design reports, and local de-identified artifacts remain bound",
                "evidence is counted by independent group; shared support across rivals cannot establish a contradiction",
                "only feasible, bounded, risk-gated resolver predictions with separated intervals can cover rival pairs",
                "portfolio selection is deterministic, family-exclusive, budget-bounded, and replayable",
                "a ready result is a prospective plan only; no experiment or claim resolution is dispatched"
            ]
        }))
        .map_err(|error| format!("cannot encode prospective contradiction plan: {error}"))
    }

    /// Estimate a pre/post treatment contrast for a preclinical glioma study. This is analysis
    /// only: it does not select a clinical dose, dispatch an assay, or move raw observations.
    pub(super) fn glioma_causal_contrast(&self, arguments: &Value) -> Result<Value, String> {
        let request: CausalContrastRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_causal_contrast requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma causal-contrast request: {error}"))?;
        let observations: Vec<TrajectoryObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_causal_contrast requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma causal-contrast observations: {error}"))?;
        let output = analyze_glioma_causal_contrast(&request, &observations)
            .map_err(|error| format!("glioma causal contrast refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "pre and post windows are declared and computed per tracked unit",
                "the effect is a treatment-minus-control difference in differences",
                "exact label permutations and leave-one-unit bounds remain deterministic",
                "underpowered, capped, null, or non-significant results remain explicit"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma causal contrast: {error}"))
    }

    /// Decompose a preclinical treatment contrast into mediator, direct, and indirect effects.
    /// This is an interpretation computation only; it never recommends an intervention.
    pub(super) fn glioma_causal_mediation(&self, arguments: &Value) -> Result<Value, String> {
        let request: MediationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_causal_mediation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma causal mediation request: {error}"))?;
        let observations: Vec<MediationObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_causal_mediation requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma causal mediation observations: {error}"))?;
        let analysis = analyze_glioma_mediation(&request, &observations)
            .map_err(|error| format!("glioma causal mediation refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "guarantees": [
                "mediator, total, direct, and indirect effects use deterministic integer covariance",
                "measurement uncertainty and leave-one-unit-out influence are explicit",
                "underpowered, null, zero-variance, and fragile decompositions are not promoted",
                "the route performs no biological or clinical decision effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma causal mediation: {error}"))
    }

    /// Estimate a stratified, overlap-checked preclinical glioma contrast. This is interpretation
    /// only: it does not infer patient benefit, select a dose, or dispatch an assay.
    pub(super) fn glioma_stratified_causal_adjustment(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: StratifiedCausalRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_stratified_causal_adjustment requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma stratified-causal request: {error}"))?;
        let observations: Vec<StratifiedObservation> =
            serde_json::from_value(arguments.get("observations").cloned().ok_or_else(|| {
                "glioma_stratified_causal_adjustment requires observations".to_string()
            })?)
            .map_err(|error| format!("invalid glioma stratified-causal observations: {error}"))?;
        let output = analyze_stratified_causal_adjustment(&request, &observations)
            .map_err(|error| format!("glioma stratified causal adjustment refused: {error}"))?;
        serde_json::to_value(json!({
            "adjustment": output,
            "dispatch": "not_started",
            "guarantees": [
                "repeated observations collapse to de-identified unit means before adjustment",
                "only strata with declared arm overlap and unit floors enter the weighted contrast",
                "leave-one-stratum influence, overlap imbalance, excluded strata, and missing coverage remain explicit",
                "the route produces a preclinical estimand only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma stratified causal adjustment: {error}"))
    }

    /// Evaluate competing finite-horizon preclinical glioma experiment policies from local
    /// longitudinal trajectories. This ranks workflow policies only; it never dispatches an
    /// assay, recommends a treatment, or treats an off-policy estimate as clinical evidence.
    pub(super) fn glioma_dynamic_policy_evaluate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DynamicPolicyRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_dynamic_policy_evaluate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma dynamic-policy request: {error}"))?;
        let policies: Vec<DynamicPolicyCandidate> = serde_json::from_value(
            arguments
                .get("policies")
                .cloned()
                .ok_or_else(|| "glioma_dynamic_policy_evaluate requires policies".to_string())?,
        )
        .map_err(|error| format!("invalid glioma dynamic-policy policies: {error}"))?;
        let trajectories: Vec<DynamicPolicyTrajectory> =
            serde_json::from_value(arguments.get("trajectories").cloned().ok_or_else(|| {
                "glioma_dynamic_policy_evaluate requires trajectories".to_string()
            })?)
            .map_err(|error| format!("invalid glioma dynamic-policy trajectories: {error}"))?;
        let evaluation = evaluate_glioma_dynamic_policies(&request, &policies, &trajectories)
            .map_err(|error| format!("glioma dynamic-policy evaluation refused: {error}"))?;
        serde_json::to_value(json!({
            "evaluation": evaluation,
            "dispatch": "not_started",
            "policy_frontier": "preclinical_workflow_only",
            "guarantees": [
                "finite-horizon policies are evaluated with self-normalized inverse-propensity weighting",
                "positivity violations, support coverage, weight clipping, effective sample size, and leave-one-trajectory-out instability remain explicit",
                "selected policy is a bounded workflow frontier and not an observed efficacy or clinical treatment recommendation",
                "raw trajectories remain institution-local and the route performs no external effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma dynamic-policy evaluation: {error}"))
    }

    /// Fit a bounded monotone preclinical glioma dose-response curve. This is analysis only: it
    /// does not choose a clinical dose, dispatch an assay, or move raw observations.
    pub(super) fn glioma_dose_response(&self, arguments: &Value) -> Result<Value, String> {
        let request: DoseResponseRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_dose_response requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma dose-response request: {error}"))?;
        let observations: Vec<DoseResponseObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_dose_response requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma dose-response observations: {error}"))?;
        let output = analyze_glioma_dose_response(&request, &observations)
            .map_err(|error| format!("glioma dose-response analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "the monotone fit uses weighted pool-adjacent-violators arithmetic",
                "raw means, fitted means, residuals, and monotonicity violations remain visible",
                "missing dose levels and replicate floors remain unresolved",
                "half-maximal dose is interpolated only on the declared preclinical grid"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma dose-response analysis: {error}"))
    }

    /// Select a bounded next replicate batch for a preclinical glioma campaign. This is a
    /// conservative statistical allocation only: it never chooses a clinical dose or dispatches
    /// an instrument/protocol.
    pub(super) fn glioma_adaptive_allocation(&self, arguments: &Value) -> Result<Value, String> {
        let request: AdaptiveAllocationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_allocation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive-allocation request: {error}"))?;
        let arms: Vec<AdaptiveArmObservation> = serde_json::from_value(
            arguments
                .get("arms")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_allocation requires arms".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive-allocation arms: {error}"))?;
        let output = allocate_glioma_assays(&request, &arms)
            .map_err(|error| format!("glioma adaptive allocation refused: {error}"))?;
        serde_json::to_value(json!({
            "allocation": output,
            "dispatch": "not_started",
            "guarantees": [
                "Beta posteriors and Cantelli lower bounds are computed with deterministic integer arithmetic",
                "control contrasts, exploration, replicate floors, risk ceilings, and budget limits remain explicit",
                "underpowered, negative, risk-blocked, and budget-capped arms are never silently promoted",
                "the route proposes a next batch only and never executes an assay or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive allocation: {error}"))
    }

    /// Make a bounded sequential interim decision for a local preclinical glioma campaign.
    /// This is the stopping/allocation layer: it can stop for a declared success or futility
    /// gate, hold underpowered arms, or compile one next replicate round. It never dispatches
    /// an assay, chooses a clinical dose, or turns a posterior into a clinical decision.
    pub(super) fn glioma_sequential_design(&self, arguments: &Value) -> Result<Value, String> {
        let request: SequentialDesignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_sequential_design requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma sequential-design request: {error}"))?;
        let observations: Vec<SequentialArmObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_sequential_design requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma sequential-design observations: {error}"))?;
        let plan = plan_glioma_sequential_design(&request, &observations)
            .map_err(|error| format!("glioma sequential design refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "Beta posteriors and integer Cantelli-style gates are deterministic",
                "replicate floors prevent premature success or futility promotion",
                "risk, budget, negative evidence, and uncertainty remain explicit",
                "the route compiles a local next round only and never dispatches assays or makes clinical decisions"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma sequential-design plan: {error}"))
    }

    /// Execute a bounded sequential preclinical glioma campaign in the deterministic local
    /// sandbox. Each accepted aggregate batch updates the arm posterior before the next plan;
    /// a governed institution can replace the executor behind this route without granting MCP
    /// direct hardware or clinical authority.
    /// Re-estimate replicate requirements and interim boundaries for a local preclinical glioma
    /// assay. This is a planning artifact: it never dispatches a protocol or instrument.
    pub(super) fn glioma_power_reestimate(&self, arguments: &Value) -> Result<Value, String> {
        let request: PowerReestimationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_power_reestimate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma power-re-estimation request: {error}"))?;
        let arms: Vec<PowerArmObservation> = serde_json::from_value(
            arguments
                .get("arms")
                .cloned()
                .ok_or_else(|| "glioma_power_reestimate requires arms".to_string())?,
        )
        .map_err(|error| format!("invalid glioma power-re-estimation arms: {error}"))?;
        let plan = plan_glioma_power_reestimation(&request, &arms)
            .map_err(|error| format!("glioma power re-estimation refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "interim alpha spending, variance-aware replicate re-estimation, efficacy, futility, risk, and budget decisions use deterministic integer arithmetic",
                "underpowered, negative, and budget-blocked arms remain explicit and are never promoted to execution",
                "the selected order is a bounded next replicate plan for institution-local preclinical work",
                "the route performs no assay, instrument execution, federation export, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma power re-estimation plan: {error}"))
    }

    /// Stress-test a prospective glioma design over declared effect, variance, attrition, risk,
    /// and budget worlds. This is a planning/evaluation endpoint; it never executes an assay.
    pub(super) fn glioma_power_stress_surface(&self, arguments: &Value) -> Result<Value, String> {
        let request: PowerStressSurfaceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_power_stress_surface requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma power-stress request: {error}"))?;
        let surface = plan_glioma_power_stress_surface(&request)
            .map_err(|error| format!("glioma power stress surface refused: {error}"))?;
        serde_json::to_value(json!({
            "surface": surface,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_blocked_randomization_design",
                "glioma_robust_experiment_design",
                "glioma_power_reestimate"
            ],
            "guarantees": [
                "integer replicate requirements are evaluated across every declared effect, variance, and attrition scenario",
                "worst-case and weighted power proxies remain separate from validated observations",
                "risk, budget, arm-capacity, underpower, and negative evidence remain explicit",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma power stress surface: {error}"))
    }

    /// Choose a bounded assay order while penalizing declared transition carryover. The route
    /// compiles a local design artifact only; it never schedules a plate or controls hardware.
    pub(super) fn glioma_carryover_sequence_design(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: CarryoverSequenceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_carryover_sequence_design requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma carryover-sequence request: {error}"))?;
        let design = plan_glioma_carryover_sequence(&request)
            .map_err(|error| format!("glioma carryover sequence refused: {error}"))?;
        serde_json::to_value(json!({
            "design": design,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": [
                "glioma_power_stress_surface",
                "glioma_protocol_simulate",
                "glioma_instrument_preflight"
            ],
            "guarantees": [
                "transition carryover is penalized as an explicit directed graph rather than hidden in execution",
                "information, risk, feasibility, repeat, and budget gates remain visible at every sequence position",
                "partial sequence realization and blocked actions remain negative or unresolved evidence",
                "the route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma carryover sequence design: {error}"))
    }

    pub(super) fn glioma_sequential_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: SequentialCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_sequential_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma sequential-campaign request: {error}"))?;
        let mut executor = DryRunSequentialCampaignExecutor;
        let campaign = execute_glioma_sequential_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma sequential campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "the planner is rerun after every accepted aggregate batch",
                "exact replicate accounting, arm binding, local-only artifacts, retries, and budget debits are validated",
                "success, futility, risk, budget, no-progress, and executor-failure stops remain explicit",
                "MCP never contacts hardware, moves raw biology, or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma sequential campaign: {error}"))
    }

    /// Rank and batch mechanism-discriminating preclinical assays for a closed-loop campaign.
    /// The controller recomputes a deterministic posterior from local observations, then emits
    /// an information/cost/risk-aware next batch. It never dispatches hardware or asserts a
    /// biological result.
    pub(super) fn glioma_closed_loop_campaign(&self, arguments: &Value) -> Result<Value, String> {
        let request: ClosedLoopCampaignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_closed_loop_campaign requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma closed-loop campaign request: {error}"))?;
        let mechanisms: Vec<CampaignMechanism> = serde_json::from_value(
            arguments
                .get("mechanisms")
                .cloned()
                .ok_or_else(|| "glioma_closed_loop_campaign requires mechanisms".to_string())?,
        )
        .map_err(|error| format!("invalid glioma campaign mechanisms: {error}"))?;
        let actions: Vec<CampaignAction> = serde_json::from_value(
            arguments
                .get("actions")
                .cloned()
                .ok_or_else(|| "glioma_closed_loop_campaign requires actions".to_string())?,
        )
        .map_err(|error| format!("invalid glioma campaign actions: {error}"))?;
        let observations: Vec<CampaignObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid glioma campaign observations: {error}"))?;
        let output =
            plan_glioma_closed_loop_campaign(&request, &mechanisms, &actions, &observations)
                .map_err(|error| format!("glioma closed-loop campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": output,
            "dispatch": "not_started",
            "guarantees": [
                "posterior reweighting uses bounded deterministic residual arithmetic",
                "assay batches maximize mechanism separation while respecting cost, feasibility, risk, and replicate ceilings",
                "negative observations, no-information states, budget exhaustion, and posterior convergence remain explicit",
                "the route returns a caller-owned local execution plan and never dispatches an instrument or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma closed-loop campaign: {error}"))
    }

    /// Run the complete P06 experiment-design loop through the deterministic local sandbox.
    pub(super) fn glioma_experiment_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ExperimentOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_experiment_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma experiment operating-cycle request: {error}"))?;
        let mut executor = DryRunExperimentOperatingCycleExecutor;
        let cycle = execute_glioma_experiment_operating_cycle(&request, &mut executor)
            .map_err(|error| format!("glioma experiment operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the initial experiment plan is computed from typed mechanisms, action costs, feasibility, risk, and information value",
                "each returned local observation is incorporated before the next plan is generated",
                "posterior convergence, null observations, budget exhaustion, and unresolved states remain explicit",
                "the sandbox emits metadata-only observations and performs no instrument effect, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma experiment operating cycle: {error}"))
    }

    /// Analyze a two-agent preclinical glioma response surface with fixed-point Bliss synergy.
    /// This is analysis only: it does not choose a clinical dose or dispatch an assay.
    pub(super) fn glioma_combination_synergy(&self, arguments: &Value) -> Result<Value, String> {
        let request: CombinationSynergyRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_combination_synergy requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma combination-synergy request: {error}"))?;
        let observations: Vec<CombinationObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_combination_synergy requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma combination-synergy observations: {error}"))?;
        let output = analyze_glioma_combination_synergy(&request, &observations)
            .map_err(|error| format!("glioma combination synergy refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "single-agent controls are required for every combination cell",
                "Bliss expected response and synergy use bounded integer arithmetic",
                "under-replicated and noisy cells remain unresolved",
                "antagonistic and null responses remain negative evidence"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma combination synergy: {error}"))
    }

    /// Select the next diverse combination cells for a local preclinical glioma response surface.
    /// The planner interpolates only typed local observations and never recommends a clinical dose.
    pub(super) fn glioma_adaptive_dose_surface(&self, arguments: &Value) -> Result<Value, String> {
        let request: AdaptiveDoseSurfaceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_adaptive_dose_surface requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma adaptive dose-surface request: {error}"))?;
        let plan = plan_adaptive_glioma_dose_surface(&request)
            .map_err(|error| format!("glioma adaptive dose-surface planner refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "dose cells are planned for institution-local preclinical assays, never clinical treatment",
                "unmeasured cells are interpolated only from bounded neighboring observations and retain uncertainty",
                "replicate debt, residual noise, dose caps, budget, and diversity constraints remain explicit",
                "selected cells are validation plans, not observed biological effects"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive dose-surface plan: {error}"))
    }
}
