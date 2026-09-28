//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    /// Compare typed local glioma modality vectors for one sample lineage. This is a scientific
    /// concordance computation only; it never moves raw data or treats correlation as causation.
    pub(super) fn glioma_multimodal_concordance(&self, arguments: &Value) -> Result<Value, String> {
        let request: ConcordanceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_concordance requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma concordance request: {error}"))?;
        let vectors: Vec<ModalityVector> = serde_json::from_value(
            arguments
                .get("vectors")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_concordance requires vectors".to_string())?,
        )
        .map_err(|error| format!("invalid glioma concordance vectors: {error}"))?;
        let output = analyze_multimodal_concordance(&request, &vectors)
            .map_err(|error| format!("glioma multimodal concordance refused: {error}"))?;
        serde_json::to_value(json!({
            "concordance": output,
            "dispatch": "not_started",
            "guarantees": [
                "feature identifiers are aligned before correlation",
                "missing overlap and zero-variance pairs remain unresolved",
                "negative or contradictory modality pairs remain visible",
                "the output is local preclinical evidence, not a causal or clinical conclusion"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal concordance: {error}"))
    }

    /// Stress a local preclinical glioma endpoint under declared modality dropout. This is a
    /// no-imputation analysis: required missing modalities remain unresolved and every degraded
    /// or contradictory scenario is routed to an explicit acquisition or review action.
    pub(super) fn glioma_multimodal_dropout_stress(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DropoutStressRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_dropout_stress requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal dropout-stress request: {error}"))?;
        let analysis = analyze_glioma_multimodal_dropout_stress(&request)
            .map_err(|error| format!("glioma multimodal dropout stress refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_qc", "glioma_robust_experiment_design"],
            "guarantees": [
                "missing modalities are never imputed",
                "quality-gated dropout, stability, sign reversal, negative evidence, and acquisition actions remain explicit",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal dropout stress: {error}"))
    }

    /// Audit the local preclinical glioma sample-by-modality missingness topology. This route
    /// distinguishes absent, partial, and quality-failed assays, then returns reacquisition
    /// priorities without imputing values or dispatching an instrument.
    pub(super) fn glioma_multimodal_missingness(&self, arguments: &Value) -> Result<Value, String> {
        let request: MissingnessAuditRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_missingness requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal missingness request: {error}"))?;
        let audit = analyze_glioma_multimodal_missingness(&request)
            .map_err(|error| format!("glioma multimodal missingness audit refused: {error}"))?;
        serde_json::to_value(json!({
            "audit": audit,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_qc", "glioma_multimodal_dropout_stress", "glioma_robust_experiment_design"],
            "guarantees": [
                "absent, partial, and quality-failed cells remain distinct",
                "correlated dropout and rare patterns are explicit uncertainty or blocking evidence",
                "reacquisition order is deterministic and no assay, instrument, federation, or clinical action is performed"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal missingness audit: {error}"))
    }

    /// Calibrate replicate-level reliability for local preclinical glioma modalities. This is a
    /// deterministic QC gate only: it does not infer an unmeasured value or dispatch an assay.
    pub(super) fn glioma_multimodal_reliability(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReliabilityCalibrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_reliability requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal reliability request: {error}"))?;
        let calibration = calibrate_glioma_multimodal_reliability(&request).map_err(|error| {
            format!("glioma multimodal reliability calibration refused: {error}")
        })?;
        serde_json::to_value(json!({
            "calibration": calibration,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_missingness", "glioma_multimodal_dropout_stress", "glioma_robust_experiment_design"],
            "guarantees": [
                "within-sample spread, leave-one-replicate-out instability, quality attrition, and replicate debt remain explicit",
                "reliability gates never convert low-quality or missing observations into biological evidence",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal reliability calibration: {error}"))
    }

    /// Select the smallest reliability- and budget-aware local preclinical glioma modality
    /// portfolio that covers the endpoint's declared evidence dimensions.
    pub(super) fn glioma_multimodal_portfolio(&self, arguments: &Value) -> Result<Value, String> {
        let request: ModalityPortfolioRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_portfolio requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal portfolio request: {error}"))?;
        let plan = plan_glioma_multimodal_portfolio(&request)
            .map_err(|error| format!("glioma multimodal portfolio planner refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_missingness", "glioma_multimodal_reliability", "glioma_robust_experiment_design"],
            "guarantees": [
                "portfolio selection is bounded by declared endpoint dimensions, reliability, redundancy, throughput, and budget gates",
                "uncovered dimensions and infeasible alternatives remain explicit instead of being silently dropped",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal portfolio plan: {error}"))
    }

    /// Monitor ordered local preclinical glioma QC epochs for sustained modality/metric drift.
    /// Drift alerts are recalibration gates, never biological or clinical conclusions.
    pub(super) fn glioma_multimodal_drift(&self, arguments: &Value) -> Result<Value, String> {
        let request: DriftSurveillanceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_drift requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal drift request: {error}"))?;
        let surveillance = surveil_glioma_multimodal_drift(&request)
            .map_err(|error| format!("glioma multimodal drift surveillance refused: {error}"))?;
        serde_json::to_value(json!({
            "surveillance": surveillance,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_reliability", "glioma_multimodal_missingness", "glioma_multimodal_portfolio"],
            "guarantees": [
                "baseline, terminal, excursion, signed slope, quality attrition, and missing epochs remain explicit",
                "drift gates route to recalibration and never become biological evidence",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal drift surveillance: {error}"))
    }

    /// Fuse eligible local preclinical glioma endpoint estimates while preserving uncertainty,
    /// contradiction, and missing-modality gates for downstream research planning.
    pub(super) fn glioma_multimodal_evidence_fusion(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceFusionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_evidence_fusion requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal evidence-fusion request: {error}"))?;
        let analysis = analyze_glioma_multimodal_evidence_fusion(&request)
            .map_err(|error| format!("glioma multimodal evidence fusion refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_reliability", "glioma_multimodal_missingness", "glioma_mechanism_explore"],
            "guarantees": [
                "eligible modality estimates are fused with explicit reliability, quality, replicate, and uncertainty weights",
                "sign reversals, high dispersion, and absent required modalities remain blocking or conditional evidence",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal evidence fusion: {error}"))
    }

    /// Quantify endpoint fragility to modality removal and bounded measurement perturbations.
    /// The result is a planning signal for autonomous preclinical workflows, never an assay
    /// dispatch or a clinical conclusion.
    pub(super) fn glioma_multimodal_sensitivity(&self, arguments: &Value) -> Result<Value, String> {
        let request: GliomaMultimodalSensitivityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_sensitivity requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal sensitivity request: {error}"))?;
        let analysis = analyze_glioma_multimodal_sensitivity(&request)
            .map_err(|error| format!("glioma multimodal sensitivity analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_evidence_fusion", "glioma_multimodal_reliability", "glioma_mechanism_explore"],
            "guarantees": [
                "leave-one-modality-out influence and bounded perturbation ranges remain explicit",
                "sign-changing or fragile endpoint evidence is routed to reacquisition or orthogonal replication",
                "missing and ineligible modalities are never imputed into a scientific conclusion",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal sensitivity analysis: {error}"))
    }

    /// Evaluate a multimodal preclinical endpoint against a declared research threshold while
    /// preserving interval, confidence, contradiction, and missingness gates.
    pub(super) fn glioma_multimodal_decision_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalDecisionGateRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_decision_gate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal decision-gate request: {error}"))?;
        let analysis = analyze_glioma_multimodal_decision_gate(&request)
            .map_err(|error| format!("glioma multimodal decision gate refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_sensitivity", "glioma_mechanism_explore", "glioma_experiment_design"],
            "guarantees": [
                "threshold decisions are computed only from eligible local preclinical evidence and bounded intervals",
                "negative, contradictory, low-confidence, wide-interval, and missing states remain explicit",
                "a supported endpoint is a research-planning handoff and never a clinical conclusion",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal decision gate: {error}"))
    }

    /// Classify pairwise multimodal disagreement and return a deterministic preclinical
    /// resolution queue without deleting rival measurements.
    pub(super) fn glioma_multimodal_contradiction_adjudication(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ContradictionAdjudicationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_contradiction_adjudication requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multimodal contradiction request: {error}"))?;
        let adjudication =
            adjudicate_glioma_multimodal_contradictions(&request).map_err(|error| {
                format!("glioma multimodal contradiction adjudication refused: {error}")
            })?;
        serde_json::to_value(json!({
            "adjudication": adjudication,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_sensitivity", "glioma_multimodal_decision_gate", "glioma_experiment_design"],
            "guarantees": [
                "sign reversals, magnitude conflicts, quality asymmetries, and unresolved gates remain explicit",
                "rival modality evidence is retained while trust-ranked resolution actions are emitted",
                "resolution recommendations are preclinical research planning signals, never clinical decisions",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal contradiction adjudication: {error}"))
    }

    /// Forecast the next local preclinical modality-quality state before autonomous acquisition.
    pub(super) fn glioma_multimodal_quality_forecast(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProspectiveQualityRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_forecast requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma prospective quality request: {error}"))?;
        let forecast = forecast_glioma_multimodal_quality(&request)
            .map_err(|error| format!("glioma prospective quality forecast refused: {error}"))?;
        serde_json::to_value(json!({
            "forecast": forecast,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_reliability", "glioma_multimodal_drift", "glioma_multimodal_ingestion_campaign"],
            "guarantees": [
                "future quality risk is forecast only from ordered local QC metadata",
                "missing history and quality failure remain explicit blocked or at-risk states",
                "preflight and reacquisition actions never invent measurements or predict biology",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma prospective quality forecast: {error}"))
    }

    /// Turn prospective modality-quality risk into a bounded local acquisition schedule.
    pub(super) fn glioma_multimodal_quality_scheduler(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: QualityScheduleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_scheduler requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma quality schedule request: {error}"))?;
        let plan = plan_glioma_multimodal_quality_schedule(&request)
            .map_err(|error| format!("glioma quality schedule refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_forecast", "glioma_multimodal_ingestion_campaign", "glioma_multimodal_readiness"],
            "guarantees": [
                "schedule is deterministic and bounded by local budget, duration, deadline, and required-modality constraints",
                "uncovered required modalities and forecast-risk conditions remain explicit",
                "alternatives are returned for researcher approval without silently substituting assays",
                "route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma quality schedule: {error}"))
    }

    /// Execute an approved quality schedule through the deterministic metadata-only MCP seam.
    pub(super) fn glioma_multimodal_quality_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let mut request: QualityExecutionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_quality_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma quality execution request: {error}"))?;
        request.execution_mode = QualityExecutionMode::DryRun;
        let mut executor = DryRunQualityScheduleExecutor;
        let result = execute_glioma_multimodal_quality_schedule(&request, &mut executor)
            .map_err(|error| format!("glioma quality schedule execution refused: {error}"))?;
        serde_json::to_value(json!({
            "execution": result,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_forecast", "glioma_multimodal_quality_scheduler", "glioma_multimodal_readiness"],
            "guarantees": [
                "execution is bound to the supplied schedule and active study approval",
                "retryable failures, below-floor QC, required failures, and blocked continuation remain explicit",
                "only typed local metadata observations cross the executor seam",
                "MCP dry-run performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma quality schedule execution: {error}"))
    }

    /// Run bounded adaptive quality rounds using only returned local QC metadata.
    pub(super) fn glioma_multimodal_quality_adaptive_campaign(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let mut request: QualityAdaptiveCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_adaptive_campaign requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma adaptive quality campaign request: {error}")
            })?;
        request.execution_mode = QualityExecutionMode::DryRun;
        let mut executor = DryRunQualityScheduleExecutor;
        let campaign = execute_glioma_multimodal_quality_adaptive_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma adaptive quality campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_forecast", "glioma_multimodal_quality_scheduler", "glioma_multimodal_quality_execute", "glioma_multimodal_readiness"],
            "guarantees": [
                "each round is budgeted, approval-bound, and planned from typed local QC observations only",
                "satisfied modalities leave the pending frontier while below-floor, failed, and unresolved modalities remain explicit",
                "required quality failure and resource exhaustion stop the campaign rather than creating an unconstrained loop",
                "MCP dry-run performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive quality campaign: {error}"))
    }

    /// Calibrate quality-policy transport between two local preclinical glioma studies.
    pub(super) fn glioma_multimodal_quality_transport(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: QualityTransportRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_transport requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma quality transport request: {error}"))?;
        let calibration = calibrate_glioma_multimodal_quality_transport(&request)
            .map_err(|error| format!("glioma quality transport refused: {error}"))?;
        serde_json::to_value(json!({
            "calibration": calibration,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_forecast", "glioma_multimodal_quality_scheduler", "glioma_multimodal_quality_adaptive_campaign"],
            "guarantees": [
                "source and target QC summaries remain bound to their declared study and model systems",
                "missing target cells, weak alignment, quality gaps, and drift block or condition policy transport",
                "qualified output is a target-local confirmation candidate and never biological or clinical validity",
                "route performs no raw-data movement, assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma quality transport: {error}"))
    }

    /// Attribute a local QC incident to measurement-process causes and route remediation.
    pub(super) fn glioma_multimodal_quality_root_cause(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: QualityRootCauseRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_root_cause requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma quality root-cause request: {error}"))?;
        let attribution = attribute_glioma_multimodal_quality_root_cause(&request)
            .map_err(|error| format!("glioma quality root-cause attribution refused: {error}"))?;
        serde_json::to_value(json!({
            "attribution": attribution,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_scheduler", "glioma_multimodal_quality_adaptive_campaign", "glioma_multimodal_quality_transport"],
            "guarantees": [
                "attribution is limited to declared measurement-process QC signals",
                "supporting, contradicting, low-reliability, and missing modality evidence remain explicit",
                "qualified cause is a remediation handoff, never a biological or clinical conclusion",
                "route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma quality root-cause attribution: {error}"))
    }

    /// Plan a bounded local remediation sequence from QC process-cause attributions.
    pub(super) fn glioma_multimodal_quality_remediation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: QualityRemediationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_remediation requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma quality remediation request: {error}"))?;
        let plan = plan_glioma_multimodal_quality_remediation(&request)
            .map_err(|error| format!("glioma quality remediation planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_execute", "glioma_multimodal_quality_adaptive_campaign", "glioma_multimodal_readiness"],
            "guarantees": [
                "selected steps are bounded by declared budget, duration, risk, evidence, and approval gates",
                "rejected, unavailable, unresolved, and approval-blocked actions remain explicit",
                "planning is limited to measurement-process remediation and never infers biology or clinical action",
                "route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma quality remediation plan: {error}"))
    }

    /// Verify paired baseline and post-remediation local QC recovery.
    pub(super) fn glioma_multimodal_quality_recovery(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: QualityRecoveryRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_quality_recovery requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma quality recovery request: {error}"))?;
        let recovery = verify_glioma_multimodal_quality_recovery(&request)
            .map_err(|error| format!("glioma quality recovery verification refused: {error}"))?;
        serde_json::to_value(json!({
            "recovery": recovery,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_multimodal_quality_adaptive_campaign", "glioma_multimodal_readiness", "glioma_multimodal_quality_scheduler"],
            "guarantees": [
                "paired baseline and post-remediation evidence is conservatively aggregated without imputation",
                "quality floors, reliability, sample coverage, improvement, and drift gates remain explicit",
                "failed or blocked modalities cannot be admitted to downstream analysis by this route",
                "route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma quality recovery verification: {error}"))
    }

    /// Cluster de-identified preclinical glioma sample lineages from multiple modality vectors.
    /// This is a deterministic consensus computation only: it never imputes missing assays,
    /// moves raw data, dispatches an instrument, or makes a clinical decision.
    pub(super) fn glioma_multimodal_consensus(&self, arguments: &Value) -> Result<Value, String> {
        let request: ConsensusRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_consensus requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma consensus request: {error}"))?;
        let vectors: Vec<ModalityVector> = serde_json::from_value(
            arguments
                .get("vectors")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_consensus requires vectors".to_string())?,
        )
        .map_err(|error| format!("invalid glioma consensus vectors: {error}"))?;
        let output = analyze_multimodal_consensus(&request, &vectors)
            .map_err(|error| format!("glioma multimodal consensus refused: {error}"))?;
        serde_json::to_value(json!({
            "consensus": output,
            "dispatch": "not_started",
            "guarantees": [
                "per-sample feature values use deterministic modality medians without imputation",
                "sample assignments use bounded deterministic k-medoids with stable tie-breaking",
                "missing modality coverage, disconnected pairs, and distance-bound failures remain explicit",
                "the output is local preclinical evidence, not a causal or clinical conclusion"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal consensus: {error}"))
    }

    /// Harmonize local preclinical glioma modality vectors with deterministic robust
    /// median-centering. Missing features remain missing and no raw artifact leaves the caller.
    pub(super) fn glioma_multimodal_harmonize(&self, arguments: &Value) -> Result<Value, String> {
        let request: HarmonizationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_harmonize requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma harmonization request: {error}"))?;
        let vectors: Vec<HarmonizationVector> = serde_json::from_value(
            arguments
                .get("vectors")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_harmonize requires vectors".to_string())?,
        )
        .map_err(|error| format!("invalid glioma harmonization vectors: {error}"))?;
        let output = harmonize_glioma_multimodal_batches(&request, &vectors)
            .map_err(|error| format!("glioma multimodal harmonization refused: {error}"))?;
        serde_json::to_value(json!({
            "harmonization": output,
            "dispatch": "not_started",
            "guarantees": [
                "batch corrections use robust per-modality medians with deterministic integer arithmetic",
                "reference batches, correction magnitudes, residual spread, and excluded batches remain visible",
                "missing features and modality overlap are never imputed or silently treated as comparable",
                "the route produces local preclinical vectors only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal harmonization: {error}"))
    }

    /// Extract deterministic latent states from complete-case multimodal glioma vectors. This is
    /// a local analysis only: it does not impute missing values, export raw artifacts, or dispatch
    /// an assay.
    pub(super) fn glioma_multimodal_latent_factors(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LatentFactorRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_latent_factors requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma latent-factor request: {error}"))?;
        let vectors: Vec<LatentFactorVector> = serde_json::from_value(
            arguments
                .get("vectors")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_latent_factors requires vectors".to_string())?,
        )
        .map_err(|error| format!("invalid glioma latent-factor vectors: {error}"))?;
        let output = analyze_glioma_latent_factors(&request, &vectors)
            .map_err(|error| format!("glioma latent-factor analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "complete-case rows and robust median/MAD scaling are deterministic",
                "latent components use bounded fixed-point power iteration with explicit convergence",
                "explained variance, reconstruction error, omitted features, and modality gaps remain visible",
                "the route produces local preclinical analysis only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma latent-factor analysis: {error}"))
    }

    /// Fuse modality-specific sample neighbourhoods into a bounded local graph. Dropout,
    /// sparse overlap, reliability weighting, and cross-modal disagreement remain explicit.
    pub(super) fn glioma_multimodal_graph_fusion(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GraphFusionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_graph_fusion requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma graph-fusion request: {error}"))?;
        let vectors: Vec<GraphFusionVector> = serde_json::from_value(
            arguments
                .get("vectors")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_graph_fusion requires vectors".to_string())?,
        )
        .map_err(|error| format!("invalid glioma graph-fusion vectors: {error}"))?;
        let output = analyze_glioma_multimodal_graph_fusion(&request, &vectors)
            .map_err(|error| format!("glioma multimodal graph fusion refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "modality-specific neighbours are fused only from observed local vectors",
                "reliability weighting, missing modalities, sparse shared features, cross-modal disagreement, and diffusion limits remain explicit",
                "the route does not move raw artifacts, dispatch instruments, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal graph fusion: {error}"))
    }

    /// Score signed pathway activity from local preclinical glioma observations. This emits
    /// ranked mechanism priorities and bottlenecks; it does not infer a clinical state.
    pub(super) fn glioma_pathway_activity(&self, arguments: &Value) -> Result<Value, String> {
        let request: PathwayActivityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_pathway_activity requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma pathway-activity request: {error}"))?;
        let definitions: Vec<PathwayActivityDefinition> = serde_json::from_value(
            arguments
                .get("definitions")
                .cloned()
                .ok_or_else(|| "glioma_pathway_activity requires definitions".to_string())?,
        )
        .map_err(|error| format!("invalid glioma pathway definitions: {error}"))?;
        let observations: Vec<PathwayActivityObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_pathway_activity requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma pathway observations: {error}"))?;
        let output = analyze_glioma_pathway_activity(&request, &definitions, &observations)
            .map_err(|error| format!("glioma pathway activity refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "signed node activity is reliability- and weight-adjusted without imputing missing features",
                "coverage, cross-modal agreement, confidence, and bottleneck nodes remain explicit",
                "the route produces mechanism research priorities only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma pathway activity: {error}"))
    }

    /// Run the multimodal graph, pathway activity, and bounded action-selection vertical as one
    /// autonomous research cycle. Actions remain plans until existing execution gates admit them.
    pub(super) fn glioma_multimodal_mechanism_campaign(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalMechanismCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_mechanism_campaign requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism campaign request: {error}"))?;
        let graph_vectors: Vec<GraphFusionVector> =
            serde_json::from_value(arguments.get("graph_vectors").cloned().ok_or_else(|| {
                "glioma_multimodal_mechanism_campaign requires graph_vectors".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism campaign graph vectors: {error}"))?;
        let pathway_definitions: Vec<PathwayActivityDefinition> =
            serde_json::from_value(arguments.get("pathway_definitions").cloned().ok_or_else(
                || "glioma_multimodal_mechanism_campaign requires pathway_definitions".to_string(),
            )?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign pathway definitions: {error}")
            })?;
        let pathway_observations: Vec<PathwayActivityObservation> =
            serde_json::from_value(arguments.get("pathway_observations").cloned().ok_or_else(
                || "glioma_multimodal_mechanism_campaign requires pathway_observations".to_string(),
            )?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign pathway observations: {error}")
            })?;
        let candidates: Vec<GliomaActionCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_multimodal_mechanism_campaign requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism campaign candidates: {error}"))?;
        let output = execute_glioma_multimodal_mechanism_campaign(
            &request,
            &graph_vectors,
            &pathway_definitions,
            &pathway_observations,
            &candidates,
        )
        .map_err(|error| format!("glioma multimodal mechanism campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": output,
            "dispatch": "not_started",
            "guarantees": [
                "graph and pathway evidence are evaluated before action selection",
                "selected actions are dependency-safe plans and still require existing policy, approval, and execution gates",
                "partial, unresolved, contradictory, and missing evidence remains visible; no clinical decision is made"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal mechanism campaign: {error}"))
    }

    /// Execute the selected mechanism portfolio through the deterministic dry-run worker.
    /// Institution-local deployments provide the production executor through the Rust API.
    pub(super) fn glioma_multimodal_mechanism_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalMechanismCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_mechanism_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign execution request: {error}")
            })?;
        let graph_vectors: Vec<GraphFusionVector> =
            serde_json::from_value(arguments.get("graph_vectors").cloned().ok_or_else(|| {
                "glioma_multimodal_mechanism_campaign_execute requires graph_vectors".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign execution graph vectors: {error}")
            })?;
        let pathway_definitions: Vec<PathwayActivityDefinition> =
            serde_json::from_value(arguments.get("pathway_definitions").cloned().ok_or_else(
                || {
                    "glioma_multimodal_mechanism_campaign_execute requires pathway_definitions"
                        .to_string()
                },
            )?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign execution pathway definitions: {error}")
            })?;
        let pathway_observations: Vec<PathwayActivityObservation> =
            serde_json::from_value(arguments.get("pathway_observations").cloned().ok_or_else(
                || {
                    "glioma_multimodal_mechanism_campaign_execute requires pathway_observations"
                        .to_string()
                },
            )?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign execution pathway observations: {error}")
            })?;
        let candidates: Vec<GliomaActionCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_multimodal_mechanism_campaign_execute requires candidates".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma mechanism campaign execution candidates: {error}")
            })?;
        let max_retries = arguments
            .get("max_retries")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .min(8) as u8;
        let require_artifacts = arguments
            .get("require_artifacts")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_multimodal_mechanism_campaign_with_executor(
            &request,
            &graph_vectors,
            &pathway_definitions,
            &pathway_observations,
            &candidates,
            max_retries,
            require_artifacts,
            &mut executor,
        )
        .map_err(|error| {
            format!("glioma multimodal mechanism campaign execution refused: {error}")
        })?;
        serde_json::to_value(json!({
            "execution": output,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "unresolved graph or pathway evidence blocks dispatch",
                "selected actions execute only through dependency, retry, local-artifact, and policy gates",
                "this MCP worker creates synthetic local artifacts and never touches instruments, biology, or clinical decisions"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal mechanism campaign execution: {error}"))
    }

    /// Run the multimodal mechanism vertical as a bounded autonomous loop in the deterministic
    /// MCP sandbox. Each round gates evidence, executes one local batch, retires all outcomes,
    /// and replans the remaining frontier.
    pub(super) fn glioma_mechanism_autopilot_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaMechanismAutopilotRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_mechanism_autopilot_execute requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma mechanism autopilot request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let output = execute_glioma_mechanism_autopilot(&request, &mut executor)
            .map_err(|error| format!("glioma mechanism autopilot refused: {error}"))?;
        serde_json::to_value(json!({
            "autopilot": output,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "graph and pathway evidence are recompiled before every execution round",
                "only dependency-safe local batches are dispatched and every completion, negative, partial, failed, skipped, and blocked outcome is retired",
                "budget, retry, artifact, evidence-readiness, and no-progress stops remain explicit",
                "the MCP worker creates synthetic local artifacts and never touches instruments, raw data, biology, or clinical decisions"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma mechanism autopilot: {error}"))
    }

    /// Build spatially connected same-lineage niches and retain cross-lineage interaction
    /// enrichment for local preclinical glioma observations. This is analysis only: it does not
    /// move raw spatial data, dispatch an assay, or make a clinical decision.
    pub(super) fn glioma_spatial_niches(&self, arguments: &Value) -> Result<Value, String> {
        let request: SpatialNicheRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_spatial_niches requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-niche request: {error}"))?;
        let cells: Vec<SpatialCell> = serde_json::from_value(
            arguments
                .get("cells")
                .cloned()
                .ok_or_else(|| "glioma_spatial_niches requires cells".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-niche cells: {error}"))?;
        let output = analyze_glioma_spatial_niches(&request, &cells)
            .map_err(|error| format!("glioma spatial-niche analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "same-lineage spatial components are deterministic candidate niches",
                "cross-lineage edges are compared with a random-mixing expected-edge null model",
                "isolated cells, undersized niches, sparse graphs, and absent interactions remain explicit",
                "the route produces local preclinical spatial analysis only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma spatial-niche analysis: {error}"))
    }

    /// Infer local ligand-receptor communication enrichments in preclinical glioma spatial data.
    /// The analyzer compares observed sender/receiver signal with a lineage-marginal null; it
    /// never claims causal signalling or dispatches an assay.
    pub(super) fn glioma_spatial_communication(&self, arguments: &Value) -> Result<Value, String> {
        let request: SpatialCommunicationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_spatial_communication requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-communication request: {error}"))?;
        let cells: Vec<SpatialCommunicationCell> = serde_json::from_value(
            arguments
                .get("cells")
                .cloned()
                .ok_or_else(|| "glioma_spatial_communication requires cells".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-communication cells: {error}"))?;
        let pairs: Vec<LigandReceptorPair> = serde_json::from_value(
            arguments
                .get("pairs")
                .cloned()
                .ok_or_else(|| "glioma_spatial_communication requires pairs".to_string())?,
        )
        .map_err(|error| format!("invalid glioma ligand-receptor pairs: {error}"))?;
        let output = analyze_glioma_spatial_communication(&request, &cells, &pairs)
            .map_err(|error| format!("glioma spatial-communication analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "sender-receiver neighborhoods are deterministic from the declared spatial radius",
                "observed communication is compared with a lineage-marginal random-mixing null",
                "missing ligand/receptor coverage, sparse neighborhoods, zero-null signal, and non-enrichment remain explicit",
                "the route produces local preclinical association analysis only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma spatial-communication analysis: {error}"))
    }

    /// Propagate a declared state signal over local same-sample spatial neighborhoods. The
    /// integer diffusion is a bounded simulation for sampling and mechanism prioritisation; it
    /// never treats geometry as biological proof or dispatches an assay.
    pub(super) fn glioma_spatial_state_propagation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: SpatialPropagationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_spatial_state_propagation requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-propagation request: {error}"))?;
        let cells: Vec<SpatialCell> = serde_json::from_value(
            arguments
                .get("cells")
                .cloned()
                .ok_or_else(|| "glioma_spatial_state_propagation requires cells".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-propagation cells: {error}"))?;
        let output = analyze_glioma_spatial_state_propagation(&request, &cells)
            .map_err(|error| format!("glioma spatial-state propagation refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "same-sample neighborhood edges and integer diffusion are deterministic and replayable",
                "self-retention, neighbor coupling, cross-lineage attenuation, radius, and step bounds are explicit",
                "isolated cells, non-convergence, and no-neighborhood null results remain visible",
                "the route is a local preclinical spatial simulation and never executes biology or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma spatial-state propagation: {error}"))
    }

    /// Register institution-local glioma spatial samples into a declared reference frame using
    /// lineage landmarks; no raw image payload or clinical interpretation leaves the process.
    pub(super) fn glioma_spatial_registration(&self, arguments: &Value) -> Result<Value, String> {
        let request: SpatialRegistrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_spatial_registration requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-registration request: {error}"))?;
        let cells: Vec<SpatialRegistrationCell> = serde_json::from_value(
            arguments
                .get("cells")
                .cloned()
                .ok_or_else(|| "glioma_spatial_registration requires cells".to_string())?,
        )
        .map_err(|error| format!("invalid glioma spatial-registration cells: {error}"))?;
        let output = register_glioma_spatial_samples(&request, &cells)
            .map_err(|error| format!("glioma spatial registration refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "transforms are estimated only from repeated typed lineage landmarks in local preclinical samples",
                "shared-lineage coverage, residuals, missing landmarks, and unregistered cells remain explicit",
                "rotation and shear are not silently inferred; downstream consumers receive the declared uncertainty",
                "the route aligns coordinates for research analysis only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma spatial-registration analysis: {error}"))
    }

    /// Quantify the declared hidden-confounding budget at which a local preclinical effect tips.
    pub(super) fn glioma_causal_sensitivity(&self, arguments: &Value) -> Result<Value, String> {
        let request: SensitivityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_causal_sensitivity requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma causal-sensitivity request: {error}"))?;
        let observations: Vec<SensitivityObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_causal_sensitivity requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma causal-sensitivity observations: {error}"))?;
        let output = analyze_causal_sensitivity(&request, &observations)
            .map_err(|error| format!("glioma causal-sensitivity analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": output,
            "dispatch": "not_started",
            "guarantees": [
                "declared confounder strength is swept with deterministic worst-case effect intervals",
                "the exact threshold tipping point and maximum robust strength remain visible",
                "leave-one-unit-out instability is reported rather than hidden",
                "the route produces local preclinical interpretation only and never makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma causal-sensitivity analysis: {error}"))
    }

    /// Choose a bounded next batch of local glioma actions. This only ranks typed candidates; it
    /// does not execute assays, contact instruments, move data, or claim scientific results.
    pub(super) fn glioma_research_select_actions(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let candidates_value = arguments
            .get("candidates")
            .cloned()
            .ok_or_else(|| "glioma_research_select_actions requires candidates".to_string())?;
        let candidates: Vec<GliomaActionCandidate> = serde_json::from_value(candidates_value)
            .map_err(|error| format!("invalid glioma action candidates: {error}"))?;
        let completed = arguments
            .get("completed_actions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let completed: BTreeSet<String> = serde_json::from_value(completed)
            .map_err(|error| format!("invalid completed_actions: {error}"))?;
        let config = arguments
            .get("config")
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("invalid glioma selection config: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let selection = select_glioma_actions(&candidates, &completed, &config)
            .map_err(|error| format!("glioma action selection refused: {error}"))?;
        serde_json::to_value(selection)
            .map_err(|error| format!("cannot encode glioma action selection: {error}"))
    }

    /// Combine P02 typed knowledge, P03 modality readiness, and the bounded selector into one
    /// scientific next-batch decision. This is planning only; no action or instrument effect is
    /// dispatched by the MCP surface.
    pub(super) fn glioma_scientific_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let request: ScientificFrontierRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_scientific_frontier requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma scientific-frontier request: {error}"))?;
        let plan = plan_glioma_scientific_frontier(&request)
            .map_err(|error| format!("glioma scientific frontier refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_autonomous_program_cycle",
            "guarantees": [
                "mechanism, experiment, computation, replication, publication, and federation actions are held until their admitted P03 surfaces and P02 knowledge state allow them",
                "the existing dependency-aware selector remains the authority for budget, autonomy, instrument, and federation policy",
                "every held or blocked candidate retains a deterministic reason and no candidate is silently dropped",
                "the route performs no assay, instrument, network, protected-data, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma scientific frontier: {error}"))
    }

    /// Execute only the admitted scientific-frontier batch through MCP's deterministic local
    /// adapter. Held and blocked P02/P03 candidates never reach the executor.
    pub(super) fn glioma_scientific_frontier_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ScientificFrontierExecutionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_scientific_frontier_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma scientific-frontier execution request: {error}")
            })?;
        let plan: bioprism_research::ScientificFrontierPlan = serde_json::from_value(
            arguments
                .get("plan")
                .cloned()
                .ok_or_else(|| "glioma_scientific_frontier_execute requires plan".to_string())?,
        )
        .map_err(|error| format!("invalid glioma scientific-frontier plan: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_scientific_frontier(&request, &plan, &mut executor)
            .map_err(|error| format!("glioma scientific-frontier execution refused: {error}"))?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "completed_local_dry_run",
            "simulation_only": true,
            "next_route": "glioma_scientific_frontier",
            "guarantees": [
                "only the immutable frontier plan's admitted and selected actions are dispatched",
                "held and blocked candidates are filtered before the local executor is called",
                "the executor's returned selection must exactly reconcile with the frontier plan",
                "MCP performs no instrument, federation, raw-data, network, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma scientific-frontier execution: {error}"))
    }

    /// Return the folder-owned glioma catalog with an explicit implementation status per slot.
    pub(super) fn glioma_program_catalog(&self, _arguments: &Value) -> Result<Value, String> {
        let programs = glioma_program_catalog();
        let features = generate_feature_catalog();
        validate_feature_catalog(&features)
            .map_err(|error| format!("glioma program catalog is invalid: {error}"))?;
        let registered_ids = bioprism_research::implemented_feature_ids();
        let implemented_ids = registered_ids
            .iter()
            .map(|feature_id| (*feature_id).to_string())
            .collect::<std::collections::BTreeSet<_>>();
        if implemented_ids.len() != registered_ids.len() {
            return Err("implemented glioma feature registry contains duplicate ids".into());
        }
        let catalog_ids = features
            .iter()
            .map(|feature| feature.feature_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(unlisted) = implemented_ids.difference(&catalog_ids).next() {
            return Err(format!(
                "implemented glioma feature {unlisted} is absent from the generated catalog"
            ));
        }
        let planned_ids = catalog_ids
            .difference(&implemented_ids)
            .cloned()
            .collect::<Vec<_>>();
        let feature_values = features
            .into_iter()
            .map(|feature| {
                let implementation_status = if implemented_ids.contains(&feature.feature_id) {
                    "implemented"
                } else {
                    "planned"
                };
                let mut value = serde_json::to_value(feature)
                    .map_err(|error| format!("cannot encode glioma feature: {error}"))?;
                value["implementation_status"] = Value::String(implementation_status.to_string());
                Ok(value)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let implemented_ids = implemented_ids.into_iter().collect::<Vec<_>>();
        let implemented_count = implemented_ids.len();
        let planned_count = planned_ids.len();
        let program_count = programs.len();
        let feature_count = feature_values.len();
        if program_count == 0 || feature_count % program_count != 0 {
            return Err("glioma program and feature counts do not form complete slots".into());
        }
        let features_per_program = feature_count / program_count;
        serde_json::to_value(json!({
            "programs": programs,
            "features": feature_values,
            "program_count": program_count,
            "features_per_program": features_per_program,
            "feature_count": feature_count,
            "implementation_status_source": "research_feature_registry",
            "implementation_status_semantics": "implemented means a concrete feature module is registered; planned means its slot exists but no module is registered",
            "implemented_feature_count": implemented_count,
            "implemented_feature_ids": implemented_ids,
            "planned_feature_count": planned_count,
            "planned_feature_ids": planned_ids,
            "organization_root": "crates/research/src/glioma/programs",
        }))
        .map_err(|error| format!("cannot encode glioma program catalog: {error}"))
    }
}
