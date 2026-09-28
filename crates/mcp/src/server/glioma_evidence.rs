//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    pub(super) fn glioma_evidence_qualify(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_qualify requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence request: {error}"))?;
        let records: Vec<EvidenceRecord> = serde_json::from_value(
            arguments
                .get("records")
                .cloned()
                .ok_or_else(|| "glioma_evidence_qualify requires records".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence records: {error}"))?;
        let output = qualify_evidence(&request, &records)
            .map_err(|error| format!("glioma evidence qualification refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma evidence qualification: {error}"))
    }

    /// Group local preclinical evidence by claim scope and modality, collapse exact artifact
    /// copies, and preserve source-independence, negative, and contradictory states for P02/P10.
    pub(super) fn glioma_evidence_cluster_index(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceClusterRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_cluster_index requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence cluster request: {error}"))?;
        let output = cluster_glioma_evidence(&request)
            .map_err(|error| format!("glioma evidence clustering refused: {error}"))?;
        serde_json::to_value(json!({
            "clusters": output,
            "next_routes": [
                "glioma_knowledge_compile",
                "glioma_evidence_triangulate",
                "glioma_replication_closure_frontier"
            ],
            "guarantees": [
                "exact artifact copies never count as independent evidence",
                "support, negative, contradictory, stale, unknown, and unmeasured states remain explicit",
                "cluster identities and scores are deterministic and content-addressed",
                "the route performs no retrieval, raw-data movement, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence cluster index: {error}"))
    }

    /// Adjudicate whether candidate local glioma evidence is novel, an extension, a replication,
    /// an exact duplicate, an explicit contradiction, or unresolved against a baseline corpus.
    pub(super) fn glioma_evidence_novelty_adjudication(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: NoveltyAdjudicationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_novelty_adjudication requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma novelty adjudication request: {error}"))?;
        let output = adjudicate_glioma_evidence_novelty(&request)
            .map_err(|error| format!("glioma novelty adjudication refused: {error}"))?;
        serde_json::to_value(json!({
            "adjudication": output,
            "next_routes": [
                "glioma_knowledge_compile",
                "glioma_knowledge_belief_revision",
                "glioma_evidence_triangulate",
                "glioma_replication_closure_frontier"
            ],
            "guarantees": [
                "novelty is compared against an explicit baseline rather than inferred from citation count",
                "exact duplicates, replications, scope extensions, contradictions, and unresolved states remain distinct",
                "contradiction requires explicit typed state and is never inferred from wording alone",
                "the route performs no retrieval, raw-data movement, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma novelty adjudication: {error}"))
    }

    /// Compile a bounded prospective evidence event window into idempotent claim trends and a
    /// negative/contradiction frontier for P01 surveillance and P02 monitoring.
    pub(super) fn glioma_evidence_stream_snapshot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceStreamRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_stream_snapshot requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence stream request: {error}"))?;
        let output = snapshot_glioma_evidence_stream(&request)
            .map_err(|error| format!("glioma evidence stream snapshot refused: {error}"))?;
        serde_json::to_value(json!({
            "snapshot": output,
            "next_routes": [
                "glioma_evidence_surveillance",
                "glioma_prospective_knowledge_monitor",
                "glioma_evidence_cluster_index"
            ],
            "guarantees": [
                "duplicate events are idempotently identified and cannot inflate support",
                "late events, rejected quality, negative results, contradictions, and unresolved claims remain visible",
                "claim trends and coverage summaries are deterministic and content-addressed",
                "the route performs no retrieval, raw-data movement, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence stream snapshot: {error}"))
    }

    /// Turn a prospective evidence stream frontier into a researcher-capacity-aware review queue.
    /// This route assigns work and exposes omissions; it does not create evidence or dispatch
    /// retrieval on its own.
    pub(super) fn glioma_evidence_prospective_triage(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceProspectiveTriageRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_prospective_triage requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma prospective evidence triage request: {error}")
            })?;
        let plan = plan_glioma_prospective_evidence_triage(&request)
            .map_err(|error| format!("glioma prospective evidence triage refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_protocol_gateway",
                "glioma_evidence_acquisition_plan",
                "glioma_evidence_operating_cycle"
            ],
            "guarantees": [
                "contradiction, negative evidence, coverage debt, declining trends, and stale claims receive deterministic urgency",
                "researcher capacity, queue depth, review budget, suppression, and current-resolution states are explicit",
                "a review queue never promotes evidence or claims; source bytes remain local to the institution",
                "the route performs no retrieval, raw-data movement, federation side effect, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma prospective evidence triage: {error}"))
    }

    /// Rank caller-supplied local typed evidence for a researcher while preserving every
    /// negative, contradictory, stale, uncertain, and filtered record as an explicit omission.
    /// This is an interaction surface, not a conclusion engine: it never retrieves sources,
    /// moves raw data, runs an assay, or makes a clinical decision.
    pub(super) fn glioma_evidence_researcher_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceWorkbenchRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_researcher_workbench requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma researcher workbench request: {error}"))?;
        let plan = query_glioma_evidence_workbench(&request)
            .map_err(|error| format!("glioma researcher workbench refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "next_routes": [
                "glioma_evidence_prospective_triage",
                "glioma_evidence_acquisition_plan",
                "glioma_knowledge_protocol_gateway"
            ],
            "guarantees": [
                "the workbench ranks only caller-supplied local typed evidence and never creates a scientific conclusion",
                "negative, contradicted, uncertain, stale, and filtered records remain explicit with omission reasons",
                "raw source bytes remain local and the route performs no retrieval, federation side effect, assay, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma researcher evidence workbench: {error}"))
    }

    /// Build a bounded cross-study, multimodal evidence panel for researcher inspection. The
    /// route preserves disagreements and coverage gaps; it does not pool raw measurements or
    /// promote a panel into a causal or clinical conclusion.
    pub(super) fn glioma_multimodal_researcher_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalWorkbenchRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_researcher_workbench requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma multimodal researcher workbench request: {error}")
            })?;
        let plan = query_glioma_multimodal_researcher_workbench(&request)
            .map_err(|error| format!("glioma multimodal researcher workbench refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "next_routes": [
                "glioma_multimodal_knowledge_protocol_gateway",
                "glioma_multimodal_evidence_gap_router",
                "glioma_evidence_prospective_triage"
            ],
            "guarantees": [
                "panels contain only caller-supplied local preclinical evidence with explicit study and modality identities",
                "negative, contradicted, uncertain, stale, filtered, and under-covered records remain visible with omission reasons",
                "representative selection is deterministic and preserves coverage targets without pooling raw measurements or making a clinical decision"
            ]
        }))
        .map_err(|error| {
            format!("cannot encode glioma multimodal researcher workbench: {error}")
        })
    }

    /// Verify whether a local typed glioma evidence surface can enter knowledge/workflow
    /// planning. The gate exposes support debt, source independence, coverage debt,
    /// contradictions, negatives, and uncertainty without inferring a scientific conclusion.
    pub(super) fn glioma_evidence_verification_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceVerificationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_verification_gate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence verification request: {error}"))?;
        let report = verify_glioma_evidence(&request)
            .map_err(|error| format!("glioma evidence verification refused: {error}"))?;
        serde_json::to_value(json!({
            "report": report,
            "next_routes": [
                "glioma_knowledge_protocol_gateway",
                "glioma_multimodal_evidence_gap_router",
                "glioma_evidence_acquisition_plan",
                "plan_glioma_evidence_contradiction_cut"
            ],
            "guarantees": [
                "verification requires typed local preclinical evidence and never infers contradiction or causality from wording",
                "support, independent source diversity, modality/model coverage, freshness, negatives, uncertainty, and contradictions are explicit gate inputs",
                "a failed gate cannot promote a claim; remediation routes are bounded and raw source bytes remain local"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence verification report: {error}"))
    }

    /// Reconcile typed aggregate outcomes from independent preclinical glioma sites. Raw
    /// measurements remain local; this surface returns only bounded summaries and explicit
    /// support, negative, contradiction, heterogeneity, and coverage routes.
    pub(super) fn glioma_multisite_outcome_reconciliation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiSiteOutcomeReconciliationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multisite_outcome_reconciliation requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid multi-site outcome reconciliation request: {error}")
            })?;
        let reconciliation = reconcile_glioma_multisite_outcomes(&request)
            .map_err(|error| format!("multi-site outcome reconciliation refused: {error}"))?;
        serde_json::to_value(json!({
            "reconciliation": reconciliation,
            "next_routes": [
                "glioma_knowledge_protocol_gateway",
                "glioma_federated_evidence_acquisition_policy",
                "plan_glioma_evidence_contradiction_cut",
                "glioma_multimodal_evidence_gap_router"
            ],
            "guarantees": [
                "only typed aggregate preclinical site outcomes cross the boundary; raw measurements remain local",
                "site quorum, independent-site diversity, modality/model coverage, heterogeneity, and leave-one-site-out influence are explicit",
                "negative, contradictory, heterogeneous, and underpowered outcomes remain visible and cannot be promoted as confident conclusions"
            ]
        }))
        .map_err(|error| format!("cannot encode multi-site outcome reconciliation: {error}"))
    }

    /// Join the local evidence-verification gate to typed P02 knowledge without allowing an
    /// omitted, stale, contradictory, or unmapped record to be promoted by the workflow engine.
    pub(super) fn glioma_evidence_knowledge_bridge(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceKnowledgeBridgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_knowledge_bridge requires request".to_string())?,
        )
        .map_err(|error| format!("invalid evidence-knowledge bridge request: {error}"))?;
        let bridge = bridge_glioma_evidence_to_knowledge(&request)
            .map_err(|error| format!("evidence-knowledge bridge refused: {error}"))?;
        serde_json::to_value(json!({
            "bridge": bridge,
            "next_routes": [
                "glioma_knowledge_protocol_gateway",
                "glioma_multimodal_evidence_gap_router",
                "glioma_federated_evidence_acquisition_policy",
                "plan_glioma_evidence_contradiction_cut",
                "glioma_evidence_verification_gate"
            ],
            "guarantees": [
                "every typed-knowledge claim is aligned to the verification report digest and evidence identifiers",
                "stale, omitted, unmapped, negative, uncertain, and contradictory evidence remains visible with deterministic decisions",
                "only verified support can be admitted to the knowledge gateway; this surface never infers causality or makes a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode evidence-knowledge bridge: {error}"))
    }

    /// Calibrate source-family evidence over explicit retrospective windows and route temporal
    /// drift, volatility, or missing independent coverage into bounded surveillance work.
    pub(super) fn glioma_long_horizon_evidence_calibration(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LongHorizonCalibrationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_long_horizon_evidence_calibration requires request".to_string()
            })?)
            .map_err(|error| format!("invalid long-horizon calibration request: {error}"))?;
        let analysis = calibrate_glioma_evidence_long_horizon(&request)
            .map_err(|error| format!("long-horizon calibration refused: {error}"))?;
        serde_json::to_value(json!({
            "analysis": analysis,
            "next_routes": [
                "glioma_evidence_prospective_triage",
                "glioma_federated_evidence_acquisition_policy",
                "glioma_evidence_knowledge_bridge",
                "glioma_evidence_verification_gate"
            ],
            "guarantees": [
                "calibration uses only bounded local prediction/outcome summaries and explicit epochs",
                "unknown, stale, negative, contradictory, underpowered, and volatile windows remain visible",
                "source-family drift routes to recalibration or review and never becomes a biological or clinical conclusion"
            ]
        }))
        .map_err(|error| format!("cannot encode long-horizon calibration analysis: {error}"))
    }

    /// Compile the aggregate-only federation boundary for independent preclinical glioma
    /// outcomes. The result is a deterministic export plan; it does not copy raw measurements or
    /// make a biological, clinical, or causal decision.
    pub(super) fn glioma_federated_outcome_transport(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedOutcomeTransportRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_outcome_transport requires request".to_string()
            })?)
            .map_err(|error| format!("invalid federated outcome transport request: {error}"))?;
        let report = compile_glioma_federated_outcome_transport(&request)
            .map_err(|error| format!("federated outcome transport refused: {error}"))?;
        serde_json::to_value(json!({
            "transport": report,
            "next_routes": [
                "glioma_multisite_outcome_reconciliation",
                "glioma_federated_evidence_acquisition_policy",
                "glioma_evidence_prospective_triage",
                "glioma_evidence_knowledge_bridge"
            ],
            "guarantees": [
                "only aggregate, de-identified, content-addressed preclinical outcomes may cross the federation boundary",
                "raw measurements, specimens, human data, direct identifiers, clinical decisions, and instrument commands remain local or are denied",
                "site and independence quorum, claim/scope identity, freshness, quality, capability, revocation, and attestation checks are explicit",
                "negative, contradictory, unknown, stale, deferred, and under-quorum outcomes remain visible and never become confident biological conclusions"
            ]
        }))
        .map_err(|error| format!("cannot encode federated outcome transport report: {error}"))
    }

    /// Compile the ranked next-action portfolio for a federated preclinical glioma evidence
    /// cycle. This binds transport, calibration, and reconciliation reports by digest and remains
    /// advisory: it cannot execute assays, move raw data, or promote a biological conclusion.
    pub(super) fn glioma_federated_evidence_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedEvidenceOperatingCycleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_evidence_operating_cycle requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid federated evidence operating-cycle request: {error}")
            })?;
        let cycle = compile_glioma_federated_evidence_operating_cycle(&request)
            .map_err(|error| format!("federated evidence operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "next_routes": [
                "glioma_federated_evidence_acquisition_policy",
                "glioma_evidence_verification_gate",
                "glioma_long_horizon_evidence_calibration",
                "glioma_multisite_outcome_reconciliation",
                "glioma_evidence_knowledge_bridge"
            ],
            "guarantees": [
                "transport, calibration, and reconciliation inputs are validated and bound by content digest",
                "next actions are deterministically ranked, budgeted, prerequisite-aware, and omitted work remains visible",
                "negative, contradictory, unknown, stale, partial, and under-quorum evidence remains explicit",
                "the cycle is A0 advisory planning only and cannot execute assays, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode federated evidence operating cycle: {error}"))
    }

    /// Schedule multiple advisory federation cycles for prospective high-throughput operation.
    /// The scheduler validates every cycle, applies fairness and route quotas, and returns
    /// explicit deferred/rejected work without executing any action.
    pub(super) fn glioma_federated_batch_scheduler(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedBatchSchedulerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_batch_scheduler requires request".to_string())?,
        )
        .map_err(|error| format!("invalid federated batch scheduler request: {error}"))?;
        let schedule = schedule_glioma_federated_evidence_batch(&request)
            .map_err(|error| format!("federated batch scheduler refused: {error}"))?;
        serde_json::to_value(json!({
            "schedule": schedule,
            "next_routes": [
                "glioma_federated_evidence_operating_cycle",
                "glioma_federated_evidence_acquisition_policy",
                "glioma_evidence_verification_gate"
            ],
            "guarantees": [
                "each input cycle is digest-validated before scheduling",
                "eligible cycles receive deterministic fairness opportunities and route/cycle/global quotas are explicit",
                "blocked or held cycles are rejected, dependency-prefix violations are deferred, and every omitted candidate remains visible",
                "the schedule is advisory planning only and cannot execute assays, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode federated batch schedule: {error}"))
    }

    /// Evaluate later outcomes of a scheduled federation capability over explicit temporal
    /// windows. Promotion, continuation, rollback, and hold are recommendations only; this
    /// surface never changes a deployed capability or executes a physical action.
    pub(super) fn glioma_continual_promotion_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ContinualPromotionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_continual_promotion_control requires request".to_string()
            })?)
            .map_err(|error| format!("invalid continual promotion request: {error}"))?;
        let report = evaluate_glioma_continual_promotion(&request)
            .map_err(|error| format!("continual promotion control refused: {error}"))?;
        serde_json::to_value(json!({
            "promotion": report,
            "next_routes": [
                "glioma_evidence_knowledge_bridge",
                "glioma_federated_evidence_operating_cycle",
                "glioma_federated_batch_scheduler",
                "glioma_evidence_researcher_workbench"
            ],
            "guarantees": [
                "promotion is bound to a validated batch schedule and explicit replayed outcome windows",
                "quality, reproducibility, replay, independent-group, failure, contradiction, and regression thresholds are visible",
                "negative, contradictory, failed, unknown, omitted, and underpowered observations cannot be silently promoted",
                "promote/continue/rollback/hold is advisory and cannot execute assays, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode continual promotion report: {error}"))
    }

    /// Compile a consortium-aware, site-local acquisition policy for unresolved preclinical
    /// glioma evidence. The route emits bounded actions and never moves raw data or dispatches.
    pub(super) fn glioma_federated_evidence_acquisition_policy(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedAcquisitionPolicyRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_evidence_acquisition_policy requires request".to_string()
            })?)
            .map_err(|error| format!("invalid federated acquisition policy request: {error}"))?;
        let output = plan_federated_glioma_evidence_acquisition(&request)
            .map_err(|error| format!("federated acquisition policy refused: {error}"))?;
        serde_json::to_value(json!({
            "policy": output,
            "next_routes": [
                "glioma_evidence_acquisition_plan",
                "glioma_evidence_acquisition_campaign_execute",
                "glioma_federated_continual_knowledge"
            ],
            "guarantees": [
                "only eligible non-revoked sites with explicit modality/model support enter the policy",
                "independent-site quorum, budget, privacy, and action-capacity constraints remain explicit",
                "raw experimental data remains site-local and only bounded action metadata is returned",
                "the route plans but does not contact sites, move data, run assays, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode federated acquisition policy: {error}"))
    }

    /// Join local typed evidence into the next-action frontier for the autonomous glioma
    /// research director. Support, negatives, contradictions, and sparse claims stay distinct.
    pub(super) fn glioma_evidence_frontier_join(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceFrontierJoinRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_frontier_join requires request".to_string())?,
        )
        .map_err(|error| format!("invalid evidence frontier join request: {error}"))?;
        let output = join_glioma_evidence_frontier(&request)
            .map_err(|error| format!("evidence frontier join refused: {error}"))?;
        serde_json::to_value(json!({
            "frontier": output,
            "next_routes": [
                "glioma_knowledge_compile",
                "glioma_federated_evidence_acquisition_policy",
                "glioma_evidence_contradiction_cut",
                "glioma_evidence_acquisition_plan"
            ],
            "guarantees": [
                "exact artifact copies never increase independent support",
                "supported, negative, contradicted, unresolved, and sparse claims route to distinct actions",
                "every emitted action is traceable to local typed evidence and a deterministic frontier id",
                "the route performs no retrieval, causal inference, raw-data movement, assay execution, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode evidence frontier join: {error}"))
    }

    /// Compile the smallest bounded cross-modality/model action set for unresolved preclinical
    /// glioma claims. This is a plan-only route; adapters execute locally after policy approval.
    pub(super) fn glioma_multimodal_evidence_gap_router(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalGapRouterRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_evidence_gap_router requires request".to_string()
            })?)
            .map_err(|error| format!("invalid multimodal evidence-gap request: {error}"))?;
        let output = route_glioma_multimodal_evidence_gaps(&request)
            .map_err(|error| format!("multimodal evidence-gap routing refused: {error}"))?;
        serde_json::to_value(json!({
            "gap_plan": output,
            "next_routes": [
                "glioma_federated_evidence_acquisition_policy",
                "glioma_multimodal_quality_schedule",
                "glioma_knowledge_compile"
            ],
            "guarantees": [
                "the route selects only missing modality/model context and never invents observations",
                "contradiction and negative frontiers use distinct resolution/replication actions",
                "priority floors and action bounds prevent uncontrolled autonomous expansion",
                "the route performs no retrieval, raw-data movement, assay execution, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode multimodal evidence-gap plan: {error}"))
    }

    /// Assimilate explicit site-local acquisition outcomes back into typed P01 evidence. Failed,
    /// duplicate, unauthorized, and protected outcomes remain visible but never become support.
    pub(super) fn glioma_evidence_acquisition_feedback(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AcquisitionFeedbackRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_acquisition_feedback requires request".to_string()
            })?)
            .map_err(|error| format!("invalid acquisition feedback request: {error}"))?;
        let output = assimilate_glioma_acquisition_feedback(&request)
            .map_err(|error| format!("acquisition feedback refused: {error}"))?;
        serde_json::to_value(json!({
            "feedback": output,
            "next_routes": [
                "glioma_evidence_frontier_join",
                "glioma_evidence_stream_snapshot",
                "glioma_evidence_novelty_adjudication"
            ],
            "guarantees": [
                "only explicit completed or negative local outcomes with valid artifacts enter evidence",
                "failed, cancelled, expired, duplicate, unauthorized, and protected outcomes remain non-evidence",
                "outcome identity and plan binding are idempotent and deterministic",
                "the route performs no retrieval, raw-data movement, assay execution, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode acquisition feedback: {error}"))
    }

    /// Compile selected P01 actions into approval-gated site-local adapter handoffs. This route
    /// does not open a network session or execute an assay; it only emits bounded requests.
    pub(super) fn glioma_federated_execution_handoff(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedExecutionHandoffRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_execution_handoff requires request".to_string()
            })?)
            .map_err(|error| format!("invalid federated execution handoff request: {error}"))?;
        let output = compile_federated_glioma_execution_handoff(&request)
            .map_err(|error| format!("federated execution handoff refused: {error}"))?;
        serde_json::to_value(json!({
            "handoff": output,
            "next_routes": [
                "glioma_evidence_acquisition_feedback",
                "glioma_evidence_frontier_join"
            ],
            "guarantees": [
                "only selected policy actions with valid site-local constraints become handoffs",
                "exact action approvals, expiration, capacity, and idempotency remain explicit",
                "partial execution is compensated by an honest local failure outcome rather than inferred support",
                "the route does not contact sites, move raw data, run assays, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode federated execution handoff: {error}"))
    }

    /// Detect changes between local evidence snapshots and compile bounded review actions for a
    /// continuing glioma research campaign. No external retrieval or source movement occurs.
    pub(super) fn glioma_evidence_surveillance(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceSurveillanceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_surveillance requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence surveillance request: {error}"))?;
        let previous: Vec<EvidenceRecord> = serde_json::from_value(
            arguments
                .get("previous")
                .cloned()
                .ok_or_else(|| "glioma_evidence_surveillance requires previous".to_string())?,
        )
        .map_err(|error| format!("invalid glioma previous evidence snapshot: {error}"))?;
        let current: Vec<EvidenceRecord> = serde_json::from_value(
            arguments
                .get("current")
                .cloned()
                .ok_or_else(|| "glioma_evidence_surveillance requires current".to_string())?,
        )
        .map_err(|error| format!("invalid glioma current evidence snapshot: {error}"))?;
        let output = surveil_glioma_evidence(&request, &previous, &current)
            .map_err(|error| format!("glioma evidence surveillance refused: {error}"))?;
        serde_json::to_value(json!({
            "surveillance": output,
            "dispatch": "not_started",
            "guarantees": [
                "additions, removals, state transitions, score shifts, and scope changes are detected deterministically",
                "contradictory, negative, stale, unknown, and removed evidence produce explicit review actions",
                "required modality/model coverage and snapshot omissions remain visible",
                "the route does not fetch literature, move source bytes, infer causality, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence surveillance: {error}"))
    }

    /// Rank a bounded local snapshot of preclinical glioma evidence by novelty, freshness,
    /// quality, domain coverage, and near-duplicate suppression. This is a planning surface only;
    /// source acquisition remains behind institution-owned adapters.
    pub(super) fn glioma_evidence_novelty_radar(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceNoveltyRadarRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_novelty_radar requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence novelty radar request: {error}"))?;
        let radar = rank_glioma_evidence_novelty(&request)
            .map_err(|error| format!("glioma evidence novelty radar refused: {error}"))?;
        serde_json::to_value(json!({
            "radar": radar,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "novelty is scored separately from quality, freshness, citation signal, and domain gaps",
                "near-duplicate records cannot inflate the acquisition queue",
                "low-quality, stale, and no-novel-evidence states remain explicit",
                "only preclinical, local, de-identified source metadata is accepted",
                "the route performs no external retrieval, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence novelty radar: {error}"))
    }

    /// Detect weighted prospective shifts and reversals in aggregate preclinical glioma evidence.
    /// The result is a bounded re-review signal, not an autonomous biological conclusion.
    pub(super) fn glioma_evidence_temporal_shift(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceTemporalShiftRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_temporal_shift requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence temporal-shift request: {error}"))?;
        let output = detect_glioma_evidence_temporal_shifts(&request)
            .map_err(|error| format!("glioma evidence temporal-shift analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "temporal_shift": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_evidence_refresh_campaign",
                "glioma_knowledge_compile",
                "glioma_mechanism_autopilot_execute"
            ],
            "guarantees": [
                "baseline and recent windows use deterministic inverse-uncertainty and replicate weighting",
                "emergent, reversing, stable, contradictory, and under-observed trajectories remain distinct",
                "only aggregate, local, preclinical observations are accepted; source bytes never cross MCP",
                "the route proposes re-review or replanning and performs no external retrieval, instrument action, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence temporal shift: {error}"))
    }

    /// Compare institution-local temporal summaries for a claim without moving source data.
    /// This is a continual evidence-consensus gate, not a benchmark or clinical decision.
    pub(super) fn glioma_federated_evidence_shift(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedEvidenceShiftRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_evidence_shift requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated evidence-shift request: {error}"))?;
        let sites: Vec<FederatedEvidenceShiftSite> = serde_json::from_value(
            arguments
                .get("sites")
                .cloned()
                .ok_or_else(|| "glioma_federated_evidence_shift requires sites".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated evidence-shift sites: {error}"))?;
        let output = analyze_federated_evidence_shifts(&request, &sites).map_err(|error| {
            format!("glioma federated evidence-shift analysis refused: {error}")
        })?;
        serde_json::to_value(json!({
            "federated_shift": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_compile",
                "glioma_mechanism_autopilot_execute",
                "glioma_validation_replication_gate"
            ],
            "guarantees": [
                "only typed site summaries, uncertainty, replicate counts, quality, and artifact boundaries cross this route",
                "consortium-wide emergence, reversal, site-specific change, stability, heterogeneity, and influence remain distinct",
                "raw sources, specimen data, human data, instrument actions, and clinical decisions remain outside the route",
                "a qualified shift is a bounded downstream research action, never an asserted biological truth"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated evidence shift: {error}"))
    }

    /// Rank concrete refresh, resolution, measurement, revalidation, coverage, and replication
    /// actions for the next local glioma research cycle. This route only schedules work over a
    /// caller-supplied snapshot; it does not fetch evidence or execute biology.
    pub(super) fn glioma_evidence_priority(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidencePriorityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_priority requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence-priority request: {error}"))?;
        let records: Vec<EvidenceRecord> = serde_json::from_value(
            arguments
                .get("records")
                .cloned()
                .ok_or_else(|| "glioma_evidence_priority requires records".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence-priority records: {error}"))?;
        let priority = prioritize_glioma_evidence(&request, &records)
            .map_err(|error| format!("glioma evidence prioritization refused: {error}"))?;
        serde_json::to_value(json!({
            "priority": priority,
            "dispatch": "not_started",
            "guarantees": [
                "recency, evidence state pressure, quality, relevance, reproducibility, and coverage debt are scored deterministically",
                "stale, contradictory, unknown, negative, coverage-deficient, and supported records become explicit bounded actions",
                "negative evidence and unresolved uncertainty remain visible in the plan and are never promoted",
                "the route performs no external retrieval, source movement, biological execution, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence priority: {error}"))
    }

    /// Compile a dependency-closed, source-diverse evidence-acquisition portfolio for the next
    /// autonomous glioma research cycle. This route only plans over caller-supplied candidates;
    /// it never fetches sources, moves protected data, or claims an acquisition succeeded.
    pub(super) fn glioma_evidence_acquisition_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceAcquisitionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_acquisition_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence-acquisition request: {error}"))?;
        let candidates: Vec<EvidenceAcquisitionCandidate> =
            serde_json::from_value(arguments.get("candidates").cloned().ok_or_else(|| {
                "glioma_evidence_acquisition_plan requires candidates".to_string()
            })?)
            .map_err(|error| format!("invalid glioma evidence-acquisition candidates: {error}"))?;
        let plan = plan_glioma_evidence_acquisition(&request, &candidates)
            .map_err(|error| format!("glioma evidence-acquisition planning refused: {error}"))?;
        serde_json::to_value(json!({
            "acquisition": plan,
            "dispatch": "not_started",
            "preflight_required": true,
            "guarantees": [
                "dependency-closed source-diverse portfolios are selected under budget, privacy, and independence gates",
                "human-data, non-local payload, and privacy-ceiling violations remain blocked",
                "expected value, worst-case value, missing coverage, deferred candidates, and negative evidence remain explicit",
                "the route never fetches sources, moves protected bytes, or claims acquisition success"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence-acquisition plan: {error}"))
    }

    /// Execute a selected P01 acquisition portfolio through the synthetic local adapter exposed
    /// by MCP. Institution-local Rust callers can provide a production executor instead.
    pub(super) fn glioma_evidence_acquisition_campaign_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: EvidenceAcquisitionCampaignRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_evidence_acquisition_campaign_execute requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma evidence-acquisition campaign request: {error}")
            })?;
        let mut executor = DryRunEvidenceAcquisitionExecutor;
        let campaign = execute_glioma_evidence_acquisition_campaign(&request, &mut executor)
            .map_err(|error| format!("glioma evidence-acquisition campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": campaign,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "the content-addressed acquisition plan is executed only in dependency order",
                "retryable adapter failures, budget exhaustion, negative results, partial/unknown outcomes, and blocked dependants remain explicit",
                "dry-run artifacts are local metadata and are never promoted to biological evidence",
                "production retrieval, assay, simulation, and replication effects require a caller-owned institution-local executor"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence-acquisition campaign: {error}"))
    }

    /// Run the complete P01 evidence operating cycle in the deterministic local sandbox. The
    /// synthetic adapter returns unknown outcomes so portfolio execution cannot fabricate support.
    pub(super) fn glioma_evidence_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: GliomaEvidenceOperatingCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_operating_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence operating-cycle request: {error}"))?;
        if matches!(request.execution_mode, EvidenceExecutionMode::GovernedLocal) {
            return Err(
                "glioma_evidence_operating_cycle MCP route is simulation-only; governed_local requires an institution-owned evidence adapter"
                    .to_string(),
            );
        }
        let cycle = execute_glioma_evidence_operating_cycle_dry_run(&request)
            .map_err(|error| format!("glioma evidence operating cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "source-diverse evidence selection runs before acquisition",
                "dependency, privacy, budget, retry, unknown, negative, and blocked states remain explicit",
                "dry-run acquisition artifacts never count as biological support",
                "MCP fetches no sources, moves no protected bytes, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence operating cycle: {error}"))
    }

    /// Calibrate source-family support scores against resolved local preclinical outcomes. The
    /// route plans review work only and never fetches evidence or dispatches an assay.
    pub(super) fn glioma_evidence_calibrate(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceCalibrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_calibrate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence-calibration request: {error}"))?;
        let observations: Vec<EvidenceCalibrationObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_evidence_calibrate requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence-calibration observations: {error}"))?;
        let analysis = calibrate_glioma_evidence(&request, &observations)
            .map_err(|error| format!("glioma evidence calibration refused: {error}"))?;
        serde_json::to_value(json!({
            "calibration": analysis,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "quality-weighted isotonic regression is deterministic and monotonic across score bins",
                "unknown, stale, and unmeasured outcomes do not count as resolved support",
                "under-observed and miscalibrated source families become explicit review actions",
                "the route performs no retrieval, raw-data movement, biological execution, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence calibration: {error}"))
    }

    /// Triangulate scoped glioma claims across independent source families and local artifacts.
    /// This is a scientific synthesis route: it exposes contradiction, incomplete states, source
    /// dominance, and next evidence actions without fetching literature or making a clinical claim.
    pub(super) fn glioma_evidence_triangulate(&self, arguments: &Value) -> Result<Value, String> {
        let request: EvidenceTriangulationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_triangulate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence triangulation request: {error}"))?;
        let records: Vec<EvidenceRecord> = serde_json::from_value(
            arguments
                .get("records")
                .cloned()
                .ok_or_else(|| "glioma_evidence_triangulate requires records".to_string())?,
        )
        .map_err(|error| format!("invalid glioma evidence triangulation records: {error}"))?;
        let output = triangulate_glioma_evidence(&request, &records)
            .map_err(|error| format!("glioma evidence triangulation refused: {error}"))?;
        serde_json::to_value(json!({
            "triangulation": output,
            "dispatch": "not_started",
            "guarantees": [
                "claim support must survive independent source-family and artifact floors",
                "contradiction, negative, stale, unknown, and unmeasured records remain explicit",
                "leave-one-artifact sensitivity exposes source-dominant claims before promotion",
                "the route does not fetch literature, move raw data, infer causality, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence triangulation: {error}"))
    }

    /// Plan a weighted minimum evidence cut over contradictory local records. This identifies
    /// the smallest audit set that covers disagreement edges and routes uncovered claims to
    /// independent replication without changing any source record.
    pub(super) fn glioma_evidence_contradiction_cut(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ContradictionCutRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_evidence_contradiction_cut requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma contradiction-cut request: {error}"))?;
        let evidence: Vec<ContradictionEvidence> =
            serde_json::from_value(arguments.get("evidence").cloned().ok_or_else(|| {
                "glioma_evidence_contradiction_cut requires evidence".to_string()
            })?)
            .map_err(|error| format!("invalid glioma contradiction-cut evidence: {error}"))?;
        let cut = plan_glioma_evidence_contradiction_cut(&request, &evidence)
            .map_err(|error| format!("glioma evidence contradiction cut refused: {error}"))?;
        serde_json::to_value(json!({
            "cut": cut,
            "dispatch": "not_started",
            "next_route": "glioma_knowledge_compile",
            "guarantees": [
                "contradictory support/contradiction edges are covered by a deterministic weighted audit cut",
                "budget-blocked and uncovered claims remain explicit instead of being promoted",
                "independent replication is routed only when the caller requests it",
                "the route does not fetch literature, edit evidence, move raw data, or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma evidence contradiction cut: {error}"))
    }
}
