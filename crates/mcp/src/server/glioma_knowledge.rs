//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    /// Compile caller-supplied local evidence into scoped, ranked preclinical glioma knowledge.
    /// This is an analysis capability only: it does not infer causality or execute an assay.
    pub(super) fn glioma_knowledge_compile(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_compile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge request: {error}"))?;
        let records: Vec<EvidenceRecord> = serde_json::from_value(
            arguments
                .get("records")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_compile requires records".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge records: {error}"))?;
        let output = compile_typed_knowledge(&request, &records)
            .map_err(|error| format!("glioma typed-knowledge compilation refused: {error}"))?;
        serde_json::to_value(json!({
            "knowledge": output,
            "dispatch": "not_started",
            "guarantees": [
                "claims are coalesced only within their declared preclinical scope",
                "support, negative, contradictory, stale, and unknown evidence remain addressable",
                "required modality and model coverage is evaluated on supporting records",
                "the result ranks research claims and never infers causality or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma typed knowledge: {error}"))
    }

    /// Compose explicit typed-knowledge relations into auditable preclinical evidence paths.
    /// Relation direction is caller-declared; MCP never infers causality or promotes a path into
    /// a clinical or therapeutic conclusion.
    pub(super) fn glioma_knowledge_compose(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeCompositionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_compose requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-composition request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_compose requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let relations: Vec<KnowledgeRelation> = serde_json::from_value(
            arguments
                .get("relations")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_compose requires relations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge relations: {error}"))?;
        let composition = compose_knowledge_graph(&request, &knowledge, &relations)
            .map_err(|error| format!("glioma knowledge composition refused: {error}"))?;
        serde_json::to_value(json!({
            "composition": composition,
            "dispatch": "not_started",
            "guarantees": [
                "only caller-declared supports, prerequisites, and contradictions are traversed",
                "path strength is bounded by claim confidence and the weakest relation",
                "contradiction, unresolved dependencies, negative evidence, and bottleneck claims remain explicit",
                "the result is an evidence-network planning artifact, not causal identification, treatment advice, or a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge composition: {error}"))
    }

    /// Compute a bounded support/contradiction closure over typed knowledge before downstream
    /// action compilation. Lower-scoring rivals remain visible as contested rather than deleted.
    pub(super) fn glioma_knowledge_consistency(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeConsistencyRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_consistency requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-consistency request: {error}"))?;
        let closure = compile_glioma_knowledge_consistency(&request)
            .map_err(|error| format!("glioma knowledge consistency refused: {error}"))?;
        serde_json::to_value(json!({
            "closure": closure,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "support and contradiction edges are explicit and bounded",
                "selected claims retain score provenance while contested rivals remain visible",
                "negative claims are excluded from the action-ready closure without deletion",
                "unresolved claims emit evidence-acquisition actions for the next P01/P02 cycle",
                "the route infers no causality, moves no raw data, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge consistency closure: {error}"))
    }

    /// Compare two validated typed-knowledge snapshots and route material claim drift for bounded
    /// revalidation or replanning. This is a knowledge-state transition, not a causal conclusion.
    pub(super) fn glioma_knowledge_drift(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeDriftRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_drift requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-drift request: {error}"))?;
        let output = detect_glioma_knowledge_drift(&request)
            .map_err(|error| format!("glioma knowledge-drift analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "drift": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_consistency",
                "glioma_knowledge_gap_compile",
                "glioma_mechanism_autopilot_execute"
            ],
            "guarantees": [
                "only validated typed-knowledge claim snapshots are compared",
                "claim additions, removals, strengthening, weakening, contradiction, resolution, and stability remain distinct",
                "priority-gated transitions and negative evidence remain explicit",
                "the route performs no retrieval, raw-data movement, causal inference, instrument action, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge drift: {error}"))
    }

    /// Reconcile every compiled typed-knowledge claim to caller-supplied local evidence before
    /// allowing bounded downstream planning. Missing references, coverage debt, negative records,
    /// and orphan artifacts stay visible; this route never retrieves, moves, or executes data.
    pub(super) fn glioma_knowledge_closure(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeClosureRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_closure requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-closure request: {error}"))?;
        let output = compile_glioma_knowledge_closure(&request)
            .map_err(|error| format!("glioma knowledge-closure analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "closure": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_consistency",
                "glioma_knowledge_gap_compile",
                "glioma_mechanism_autopilot_execute"
            ],
            "guarantees": [
                "every claim is reconciled to caller-supplied evidence ids before closure can qualify",
                "independent artifact, modality, and model coverage are explicit",
                "missing references, negative evidence, and orphan evidence remain visible",
                "the route performs no retrieval, raw-data movement, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge closure: {error}"))
    }

    /// Align caller-declared equivalent typed claims across local studies. The route computes a
    /// multi-study table and influence diagnostics; it never guesses equivalence from prose or
    /// exports raw evidence.
    pub(super) fn glioma_multi_study_knowledge(&self, arguments: &Value) -> Result<Value, String> {
        let request: MultiStudyKnowledgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multi_study_knowledge requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multi-study knowledge request: {error}"))?;
        let output = compile_multi_study_knowledge(&request)
            .map_err(|error| format!("glioma multi-study knowledge alignment refused: {error}"))?;
        serde_json::to_value(json!({
            "knowledge": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_consistency",
                "glioma_knowledge_closure",
                "glioma_mechanism_autopilot_execute"
            ],
            "guarantees": [
                "equivalence is caller-declared by canonical key and claim id, never inferred from text",
                "study-level support, negative, contested, unresolved, modality, and model coverage remain addressable",
                "pooled scores and leave-one-study-out influence expose dominant-study risk",
                "the route performs no retrieval, raw-data movement, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study knowledge: {error}"))
    }

    /// Monitor an ordered stream of typed claim observations for bounded prospective drift.
    /// Change points route to review or acquisition; contradictory states are quarantined.
    pub(super) fn glioma_prospective_knowledge_monitor(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProspectiveKnowledgeRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_prospective_knowledge_monitor requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma prospective knowledge request: {error}"))?;
        let output = monitor_prospective_knowledge(&request)
            .map_err(|error| format!("glioma prospective knowledge monitoring refused: {error}"))?;
        serde_json::to_value(json!({
            "monitor": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_closure",
                "glioma_knowledge_consistency",
                "glioma_knowledge_gap_compile"
            ],
            "guarantees": [
                "change detection is sequential, bounded, and claim-key scoped",
                "multiplicity-adjusted signals and persistence gates prevent a single noisy event from promotion",
                "negative, contradictory, unresolved, and missing-coverage states remain explicit",
                "the route performs no retrieval, raw-data movement, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma prospective knowledge monitor: {error}"))
    }

    /// Fuse aggregate-only typed observations across sites and release epochs using robust
    /// consensus, quorum, and influence gates before continual downstream planning.
    pub(super) fn glioma_federated_continual_knowledge(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedContinualKnowledgeRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_continual_knowledge requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma federated-continual request: {error}"))?;
        let output = analyze_federated_continual_knowledge(&request)
            .map_err(|error| format!("glioma federated-continual analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "knowledge": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_closure",
                "glioma_knowledge_consistency",
                "glioma_knowledge_gap_compile"
            ],
            "guarantees": [
                "only aggregate-only preclinical observations and semantic digests cross the federation boundary",
                "robust medians, quorum, outlier, and leave-one-site-out influence gates prevent silent site dominance",
                "epoch drift, negative, contradictory, unresolved, and coverage states remain explicit",
                "the route performs no raw-data movement, retrieval, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated-continual knowledge: {error}"))
    }

    /// Rank bounded research actions from federated continual knowledge. This creates a plan only;
    /// execution still requires the local dispatcher, policy, and any physical authorization.
    pub(super) fn glioma_federated_continual_agent(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedContinualAgentRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_continual_agent requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated-continual agent request: {error}"))?;
        let output = plan_federated_continual_agent(&request)
            .map_err(|error| format!("glioma federated-continual agent refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_action_dispatch",
                "glioma_federated_continual_knowledge",
                "glioma_knowledge_closure"
            ],
            "guarantees": [
                "priority combines information value, coverage debt, contradiction pressure, trend, confidence debt, and site influence",
                "budget, dependency, autonomy, and physical-effect authorization gates are explicit",
                "quarantine and approval-required actions are never silently selected for execution",
                "the route performs no retrieval, raw-data movement, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated-continual agent plan: {error}"))
    }

    /// Compile the selected autonomous frontier into local dependency waves with checkpoint and
    /// compensation metadata. The local dispatcher remains the only effect boundary.
    pub(super) fn glioma_local_research_workflow(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: LocalWorkflowRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_local_research_workflow requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma local workflow request: {error}"))?;
        let output = compile_local_research_workflow(&request)
            .map_err(|error| format!("glioma local workflow compilation refused: {error}"))?;
        serde_json::to_value(json!({
            "workflow": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_action_dispatch",
                "glioma_federated_continual_agent",
                "glioma_knowledge_closure"
            ],
            "guarantees": [
                "selected actions are dependency-closed before wave assignment",
                "checkpoints, bounded retries, expected artifacts, and compensations are explicit",
                "approval-required and omitted actions cannot silently enter a local workflow",
                "the route performs no retrieval, raw-data movement, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma local research workflow: {error}"))
    }

    /// Reconcile a local workflow with checkpoint and failure observations and emit only safe
    /// resume/replay actions. No step is executed by this route.
    pub(super) fn glioma_workflow_recovery(&self, arguments: &Value) -> Result<Value, String> {
        let request: WorkflowRecoveryRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_workflow_recovery requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma workflow recovery request: {error}"))?;
        let output = plan_glioma_workflow_recovery(&request)
            .map_err(|error| format!("glioma workflow recovery refused: {error}"))?;
        serde_json::to_value(json!({
            "recovery": output,
            "next_routes": [
                "glioma_local_research_workflow",
                "glioma_knowledge_action_dispatch",
                "glioma_knowledge_synthesis_operating_cycle"
            ],
            "guarantees": [
                "completed steps with valid checkpoints are not replayed",
                "retryable failures use bounded compensation and retry budgets",
                "running, terminal, unknown, and dependency-blocked steps remain held for an operator",
                "the route performs no retrieval, raw-data movement, execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma workflow recovery: {error}"))
    }

    /// Assimilate retry, duplicate, failed, and conflicting knowledge-action results without
    /// silently replacing prior evidence. This route only returns a reconciled state.
    pub(super) fn glioma_knowledge_action_outcome_assimilation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeActionOutcomeAssimilationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_knowledge_action_outcome_assimilation requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma knowledge action outcome assimilation request: {error}")
            })?;
        let output = assimilate_glioma_knowledge_action_outcomes(&request).map_err(|error| {
            format!("glioma knowledge action outcome assimilation refused: {error}")
        })?;
        serde_json::to_value(json!({
            "assimilation": output,
            "next_routes": [
                "glioma_knowledge_compile",
                "glioma_knowledge_frontier",
                "glioma_knowledge_action_dispatch"
            ],
            "guarantees": [
                "identical retries are idempotent and prior non-failed results are not erased by a failed callback",
                "conflicting action and evidence identities remain explicit for adjudication",
                "records are canonicalized and digest-checked before downstream compilation",
                "the route performs no retrieval, raw-data movement, execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| {
            format!("cannot encode glioma knowledge action outcome assimilation: {error}")
        })
    }

    /// Bind action outcomes and local evidence back to typed claims and report whether the
    /// research question closed, remained partial, contradicted, negative, or blocked.
    pub(super) fn glioma_claim_experiment_closure(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ClaimExperimentClosureRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_claim_experiment_closure requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma claim-experiment closure request: {error}"))?;
        let output = close_glioma_claims_to_experiments(&request)
            .map_err(|error| format!("glioma claim-experiment closure refused: {error}"))?;
        serde_json::to_value(json!({
            "closure": output,
            "next_routes": [
                "glioma_knowledge_frontier",
                "glioma_knowledge_gap_compile",
                "glioma_knowledge_action_compile",
                "glioma_knowledge_synthesis_operating_cycle"
            ],
            "guarantees": [
                "completed, failed, uncertain, negative, and contradictory outcomes remain distinguishable",
                "modality, model-system, and independent-artifact coverage are quantified before closure",
                "missing evidence and orphan actions are explicit next-work signals",
                "the route performs no retrieval, raw-data movement, instrument execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma claim-experiment closure: {error}"))
    }

    /// Reconcile quantified closure support against the prior typed claim state. This route
    /// promotes only stronger evidence and keeps downgrades, nulls, and contradictions reviewable.
    pub(super) fn glioma_claim_evidence_reconciliation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ClaimEvidenceReconciliationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_claim_evidence_reconciliation requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma claim-evidence reconciliation request: {error}")
            })?;
        let output = reconcile_glioma_claim_evidence(&request)
            .map_err(|error| format!("glioma claim-evidence reconciliation refused: {error}"))?;
        serde_json::to_value(json!({
            "reconciliation": output,
            "next_routes": [
                "glioma_belief_revision",
                "glioma_knowledge_frontier",
                "glioma_knowledge_action_compile",
                "glioma_knowledge_synthesis_operating_cycle"
            ],
            "guarantees": [
                "promotions require quantified closure support and never overwrite a claim silently",
                "partial, negative, contradictory, and unresolved results remain explicit review work",
                "the supplied prior knowledge and closure digests must match before reconciliation",
                "the route performs no retrieval, raw-data movement, instrument execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma claim-evidence reconciliation: {error}"))
    }

    /// Promote reconciled claims into ranked next research actions without executing them.
    pub(super) fn glioma_closed_loop_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let request: ClosedLoopFrontierRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_closed_loop_frontier requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma closed-loop frontier request: {error}"))?;
        let output = promote_glioma_closed_loop_frontier(&request)
            .map_err(|error| format!("glioma closed-loop frontier refused: {error}"))?;
        serde_json::to_value(json!({
            "frontier": output,
            "next_routes": [
                "glioma_knowledge_action_compile",
                "glioma_local_research_workflow",
                "glioma_knowledge_action_dispatch"
            ],
            "guarantees": [
                "negative, contradictory, downgraded, and unresolved claims become explicit research actions",
                "candidate ordering is deterministic and budget bounded",
                "this route only plans research work; it does not execute instruments or move raw data",
                "the route performs no causal inference or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma closed-loop frontier: {error}"))
    }

    /// Score prospective belief forecasts against observed preclinical outcomes.
    pub(super) fn glioma_prospective_belief_calibration(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ProspectiveBeliefCalibrationRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_prospective_belief_calibration requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma prospective calibration request: {error}"))?;
        let output = calibrate_glioma_beliefs_prospectively(&request)
            .map_err(|error| format!("glioma prospective belief calibration refused: {error}"))?;
        serde_json::to_value(json!({
            "calibration": output,
            "next_routes": [
                "glioma_claim_evidence_reconciliation",
                "glioma_closed_loop_frontier",
                "glioma_knowledge_frontier"
            ],
            "guarantees": [
                "calibration uses only eligible observed outcomes and reports omitted observations",
                "Brier loss and calibration error use deterministic fixed-point arithmetic",
                "negative, contradicted, stale, and unmeasured outcomes cannot be silently converted into positive support",
                "the route performs no retrieval, raw-data movement, instrument execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma prospective belief calibration: {error}"))
    }

    /// Batch the selected frontier into review-gated, budgeted rounds for local workflow admission.
    pub(super) fn glioma_frontier_campaign(&self, arguments: &Value) -> Result<Value, String> {
        let request: FrontierCampaignRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_frontier_campaign requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma frontier campaign request: {error}"))?;
        let output = schedule_glioma_frontier_campaign(&request)
            .map_err(|error| format!("glioma frontier campaign refused: {error}"))?;
        serde_json::to_value(json!({
            "campaign": output,
            "next_routes": [
                "glioma_local_research_workflow",
                "glioma_workflow_recovery",
                "glioma_knowledge_action_dispatch"
            ],
            "guarantees": [
                "round budgets and action capacities are enforced before workflow admission",
                "review-gated claims block campaign rounds when policy requires it",
                "deferred work remains explicit and can be resumed in a later campaign",
                "the route performs no retrieval, raw-data movement, instrument execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma frontier campaign: {error}"))
    }

    /// Negotiate typed-knowledge capabilities and export boundaries before exchange.
    pub(super) fn glioma_knowledge_protocol_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeProtocolRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_protocol_gateway requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge protocol request: {error}"))?;
        let output = negotiate_glioma_knowledge_protocol(&request)
            .map_err(|error| format!("glioma knowledge protocol negotiation refused: {error}"))?;
        serde_json::to_value(json!({
            "negotiation": output,
            "next_routes": [
                "glioma_knowledge_compile",
                "glioma_federated_knowledge",
                "glioma_knowledge_action_dispatch"
            ],
            "guarantees": [
                "only typed claims, aggregate metrics, digests, and provenance metadata can be exported",
                "raw data, identifiers, clinical decisions, and instrument commands are denied",
                "federation remains policy bounded and raw data remains local"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge protocol negotiation: {error}"))
    }

    /// Negotiate multimodal typed-knowledge exchange with explicit degraded modality coverage.
    pub(super) fn glioma_multimodal_knowledge_protocol_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalKnowledgeProtocolRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_knowledge_protocol_gateway requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multimodal protocol request: {error}"))?;
        let output = negotiate_glioma_multimodal_knowledge_protocol(&request).map_err(|error| {
            format!("glioma multimodal knowledge protocol negotiation refused: {error}")
        })?;
        serde_json::to_value(json!({
            "negotiation": output,
            "next_routes": [
                "glioma_multimodal_knowledge_workflow",
                "glioma_knowledge_compile",
                "glioma_local_research_workflow"
            ],
            "guarantees": [
                "missing modalities and capabilities select an explicit degraded branch",
                "study identifiers are unique and raw data remains local",
                "the route performs no execution, raw-data movement, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| {
            format!("cannot encode glioma multimodal knowledge protocol negotiation: {error}")
        })
    }

    /// Admit local, schema-versioned multimodal artifacts before harmonization or analysis.
    pub(super) fn glioma_multimodal_ingestion_manifest(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalIngestionManifestRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_ingestion_manifest requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multimodal ingestion request: {error}"))?;
        let output = build_glioma_multimodal_ingestion_manifest(&request)
            .map_err(|error| format!("glioma multimodal ingestion admission refused: {error}"))?;
        serde_json::to_value(json!({
            "manifest": output,
            "next_routes": [
                "glioma_multimodal_qc",
                "glioma_multimodal_knowledge_workflow",
                "glioma_multimodal_operating_cycle"
            ],
            "guarantees": [
                "local, de-identified, content-addressed artifacts are required",
                "duplicate and schema-incompatible artifacts are quarantined",
                "missing modality and model-system coverage is explicit before harmonization",
                "the route performs no raw-data movement, execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal ingestion manifest: {error}"))
    }

    /// Synchronize the local action DAG against study-level multimodal readiness and choose a
    /// full, degraded, or acquire-coverage branch before computation or experiment execution.
    pub(super) fn glioma_multimodal_knowledge_workflow(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultimodalWorkflowRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multimodal_knowledge_workflow requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multimodal workflow request: {error}"))?;
        let output = compile_multimodal_knowledge_workflow(&request).map_err(|error| {
            format!("glioma multimodal workflow synchronization refused: {error}")
        })?;
        serde_json::to_value(json!({
            "workflow": output,
            "execution": "not_started",
            "next_routes": [
                "glioma_multimodal_qc",
                "glioma_local_research_workflow",
                "glioma_knowledge_action_dispatch"
            ],
            "guarantees": [
                "study and action readiness are evaluated before downstream computation or experiment execution",
                "full, degraded, and acquire-coverage branches are explicit and deterministically selected",
                "missingness, low quality, non-exportable observations, and workflow approval are preserved as blocking conditions",
                "the route performs no raw-data movement, instrument execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multimodal knowledge workflow: {error}"))
    }

    /// Turn the selected multimodal branch into bounded, typed downstream route admissions.
    /// This remains an admission decision: the local dispatcher and instrument gateways own all
    /// effects, and every action carries explicit stop conditions.
    pub(super) fn glioma_research_workflow_admission(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: WorkflowAdmissionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_workflow_admission requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma workflow admission request: {error}"))?;
        let output = admit_glioma_research_workflow(&request)
            .map_err(|error| format!("glioma workflow admission refused: {error}"))?;
        serde_json::to_value(json!({
            "admission": output,
            "execution": "not_started",
            "next_routes": [
                "glioma_knowledge_action_dispatch",
                "glioma_multimodal_qc",
                "glioma_computation_workflow",
                "glioma_instrument_preflight"
            ],
            "guarantees": [
                "only synchronized actions with explicit dependency and barrier state are admitted",
                "degraded branches carry a coverage stop condition into downstream interpretation",
                "approval-required, non-exportable, and incomplete workflow actions remain blocked",
                "the route performs no retrieval, raw-data movement, instrument execution, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma workflow admission: {error}"))
    }

    /// Compare aggregate typed-knowledge summaries across institutions while preserving local
    /// sources and contested claim states.
    pub(super) fn glioma_federated_knowledge(&self, arguments: &Value) -> Result<Value, String> {
        let request: FederatedKnowledgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_knowledge requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated-knowledge request: {error}"))?;
        let sites: Vec<FederatedKnowledgeSiteClaim> = serde_json::from_value(
            arguments
                .get("sites")
                .cloned()
                .ok_or_else(|| "glioma_federated_knowledge requires sites".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated-knowledge site claims: {error}"))?;
        let output = analyze_federated_knowledge(&request, &sites)
            .map_err(|error| format!("glioma federated-knowledge analysis refused: {error}"))?;
        serde_json::to_value(json!({
            "knowledge": output,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_knowledge_consistency",
                "glioma_knowledge_gap_compile",
                "glioma_mechanism_autopilot_execute"
            ],
            "guarantees": [
                "only aggregate typed claims, source counts, provenance boundaries, and privacy declarations cross this route",
                "consensus-supported, consensus-contested, consensus-negative, site-specific, and unresolved states remain distinct",
                "leave-one-site-out influence and disagreement gates prevent one institution from silently defining the consortium claim",
                "the route performs no raw-data movement, retrieval, instrument action, causal inference, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated knowledge: {error}"))
    }

    /// Revise an explicit preclinical claim-conflict graph into a bounded consistent portfolio.
    /// The route never infers conflict from text and never promotes a retained claim to a
    /// clinical or therapeutic conclusion.
    pub(super) fn glioma_belief_revision(&self, arguments: &Value) -> Result<Value, String> {
        let request: BeliefRevisionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_belief_revision requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma belief-revision request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_belief_revision requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let conflicts: Vec<BeliefConflict> = serde_json::from_value(
            arguments
                .get("conflicts")
                .cloned()
                .ok_or_else(|| "glioma_belief_revision requires conflicts".to_string())?,
        )
        .map_err(|error| format!("invalid glioma belief conflicts: {error}"))?;
        let revision = revise_glioma_beliefs(&request, &knowledge, &conflicts)
            .map_err(|error| format!("glioma belief revision refused: {error}"))?;
        serde_json::to_value(json!({
            "revision": revision,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "only explicit typed conflict edges can exclude a claim from the retained portfolio",
                "rival, negative, unresolved, and conflict evidence remain visible for the next autonomous cycle",
                "bounded maximal-consistency search is deterministic and replay-stable",
                "the route performs no retrieval, assay, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma belief revision: {error}"))
    }

    /// Rank the claims that should drive the next autonomous glioma research cycle. This is a
    /// local prioritizer over typed knowledge; it does not fetch evidence or promote a claim.
    pub(super) fn glioma_knowledge_frontier(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeFrontierRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_frontier requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-frontier request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_frontier requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let frontier = prioritize_knowledge_frontier(&request, &knowledge)
            .map_err(|error| format!("glioma knowledge frontier refused: {error}"))?;
        serde_json::to_value(json!({
            "frontier": frontier,
            "dispatch": "not_started",
            "guarantees": [
                "coverage debt, contradiction, uncertainty, support, and workflow leverage are scored deterministically",
                "selected claim ids are suitable for the next P04 decision-context cycle",
                "negative and unresolved evidence remain visible and no claim evidence state is upgraded",
                "the route performs no external retrieval, biological execution, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge frontier: {error}"))
    }

    /// Compile P02 typed-knowledge frontier debt into P01 acquisition candidates. This handoff
    /// is deterministic and local; it does not select, fetch, execute, or promote evidence.
    pub(super) fn glioma_knowledge_gap_compile(&self, arguments: &Value) -> Result<Value, String> {
        let request: KnowledgeGapCompilerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_gap_compile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-gap compiler request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_gap_compile requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let frontier: bioprism_research::KnowledgeFrontier = serde_json::from_value(
            arguments
                .get("frontier")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_gap_compile requires frontier".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge frontier: {error}"))?;
        let portfolio = compile_glioma_knowledge_gaps(&request, &knowledge, &frontier)
            .map_err(|error| format!("glioma knowledge-gap compilation refused: {error}"))?;
        serde_json::to_value(json!({
            "portfolio": portfolio,
            "dispatch": "not_started",
            "next_route": "glioma_evidence_acquisition_plan",
            "guarantees": [
                "typed P02 coverage, contradiction, uncertainty, negative, and replication debt becomes concrete P01 candidate metadata",
                "candidate generation is deterministic, local-only, source-template bounded, and digest-bound to knowledge and frontier inputs",
                "human-data and non-local source templates are blocked rather than emitted as executable candidates",
                "the route performs no retrieval, assay, simulation, data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge-gap portfolio: {error}"))
    }

    /// Compile typed knowledge-frontier claims into dependency-closed research actions. This
    /// route plans validation, replication, coverage, contradiction, and negative-result work;
    /// it does not execute biology or promote evidence.
    pub(super) fn glioma_knowledge_action_compile(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeActionCompilerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_compile requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_compile requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let frontier: KnowledgeFrontier = serde_json::from_value(
            arguments
                .get("frontier")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_compile requires frontier".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge frontier: {error}"))?;
        let templates: Vec<KnowledgeActionTemplate> = serde_json::from_value(
            arguments
                .get("templates")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_compile requires templates".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action templates: {error}"))?;
        let plan = compile_glioma_knowledge_actions(&request, &knowledge, &frontier, &templates)
            .map_err(|error| format!("glioma knowledge-action compilation refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_research_select_actions",
            "guarantees": [
                "every selected action remains bound to a validated TypedKnowledge claim and frontier score",
                "dependencies, budget, risk, information floors, missing dependencies, and cycles remain explicit",
                "deferred and blocked work is never counted as executed evidence",
                "the route plans preclinical research only and moves no raw data or makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge-action plan: {error}"))
    }

    /// Bridge a validated P02 action plan into local candidates for the dependency-aware glioma
    /// selector. This never dispatches work or widens the plan's preclinical authority.
    pub(super) fn glioma_knowledge_action_bridge(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeActionBridgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_bridge requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action bridge request: {error}"))?;
        let plan: KnowledgeActionPlan = serde_json::from_value(
            arguments
                .get("plan")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_bridge requires plan".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action plan: {error}"))?;
        let bridge = bridge_glioma_knowledge_actions(&request, &plan)
            .map_err(|error| format!("glioma knowledge-action bridge refused: {error}"))?;
        serde_json::to_value(json!({
            "bridge": bridge,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_research_select_actions",
            "guarantees": [
                "every emitted candidate remains bound to a validated knowledge-action plan digest",
                "candidate dependencies are preserved for the existing selector and shared prerequisites are not duplicated",
                "only local read/compute/write effects at autonomy A1 are emitted",
                "instrument, federation, raw-data movement, and clinical effects remain unavailable"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge-action bridge: {error}"))
    }

    /// Run the P02 knowledge-action bridge and the bounded glioma portfolio selector as one
    /// planning cycle. The returned selected ids are ready for a caller-owned local executor;
    /// MCP itself performs no assay, instrument, federation, or clinical effect.
    pub(super) fn glioma_knowledge_selection_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeActionSelectionCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_selection_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge selection-cycle request: {error}"))?;
        let plan: KnowledgeActionPlan = serde_json::from_value(
            arguments
                .get("plan")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_selection_cycle requires plan".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action plan: {error}"))?;
        let cycle = execute_glioma_knowledge_selection_cycle(&request, &plan)
            .map_err(|error| format!("glioma knowledge selection-cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_action_portfolio_execute",
            "guarantees": [
                "knowledge action dependencies are preserved through bridge and selector namespaces",
                "completed source actions are excluded before beam selection",
                "the selector's budget, autonomy, diversity, and dependency gates remain authoritative",
                "selected work is a local preclinical plan and is not counted as executed evidence"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge selection-cycle: {error}"))
    }

    /// Execute exactly the selected P02 knowledge-action batch through MCP's deterministic local
    /// adapter. Returned evidence is synthetic, explicitly simulation-only, and recompiled into
    /// a next frontier; institution-local callers provide the production executor through Rust.
    pub(super) fn glioma_knowledge_action_dispatch(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeActionDispatchRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_dispatch requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action dispatch request: {error}"))?;
        let plan: KnowledgeActionPlan = serde_json::from_value(
            arguments
                .get("plan")
                .cloned()
                .ok_or_else(|| "glioma_knowledge_action_dispatch requires plan".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge-action plan: {error}"))?;
        let selection: KnowledgeActionSelectionCycle =
            serde_json::from_value(arguments.get("selection").cloned().ok_or_else(|| {
                "glioma_knowledge_action_dispatch requires selection".to_string()
            })?)
            .map_err(|error| format!("invalid glioma knowledge selection-cycle: {error}"))?;
        let mut executor = DryRunKnowledgeActionExecutor;
        let run =
            execute_glioma_knowledge_action_dispatch(&request, &plan, &selection, &mut executor)
                .map_err(|error| format!("glioma knowledge-action dispatch refused: {error}"))?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "completed_local_dry_run",
            "simulation_only": true,
            "next_route": "glioma_knowledge_action_compile",
            "guarantees": [
                "only actions selected by the immutable P02 selection-cycle digest are executed",
                "returned records are bound to the selected claim and scope before recompilation",
                "negative, contradictory, unknown, and budget-blocked outcomes remain explicit",
                "MCP performs no network, instrument, federation, raw-data, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge-action dispatch: {error}"))
    }

    /// Run the bounded P02-to-P01 autonomous research cycle with MCP's deterministic local
    /// executor. This compiles knowledge debt, plans acquisition, and executes only a dry run;
    /// institution-local callers must provide the production executor for real effects.
    pub(super) fn glioma_autonomous_gap_cycle(&self, arguments: &Value) -> Result<Value, String> {
        let request: AutonomousGapCycleRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_autonomous_gap_cycle requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma autonomous gap-cycle request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_autonomous_gap_cycle requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let frontier: KnowledgeFrontier = serde_json::from_value(
            arguments
                .get("frontier")
                .cloned()
                .ok_or_else(|| "glioma_autonomous_gap_cycle requires frontier".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge frontier: {error}"))?;
        let mut executor = DryRunEvidenceAcquisitionExecutor;
        let cycle =
            execute_glioma_autonomous_gap_cycle(&request, &knowledge, &frontier, &mut executor)
                .map_err(|error| format!("glioma autonomous gap-cycle refused: {error}"))?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "dry_run",
            "simulation_only": true,
            "next_route": "glioma_knowledge_compile",
            "guarantees": [
                "P02 typed coverage, contradiction, uncertainty, negative, and replication debt is compiled into P01 acquisition candidates",
                "source-diverse acquisition planning is budgeted and policy-bounded before any executor call",
                "only a deterministic local dry-run executor is used by MCP; production effects require an institution-local executor",
                "partial, negative, unknown, blocked, retry, and budget outcomes remain explicit and no dry-run artifact becomes biological evidence",
                "the route performs no network retrieval, protected-data movement, clinical decision, or treatment recommendation"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma autonomous gap-cycle: {error}"))
    }

    /// Execute the complete P02 knowledge synthesis path and return the next P01 handoff.
    /// MCP keeps this operation local and deterministic; it compiles no external source and
    /// cannot promote an unresolved or dry-run result into biological evidence.
    pub(super) fn glioma_knowledge_synthesis_operating_cycle(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: KnowledgeSynthesisOperatingCycleRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_knowledge_synthesis_operating_cycle requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma knowledge synthesis operating-cycle request: {error}")
            })?;
        let cycle =
            execute_glioma_knowledge_synthesis_operating_cycle(&request).map_err(|error| {
                format!("glioma knowledge synthesis operating cycle refused: {error}")
            })?;
        serde_json::to_value(json!({
            "cycle": cycle,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_route": "glioma_evidence_acquisition_plan",
            "guarantees": [
                "P02 compiles typed evidence, explicit claim paths, conflict-aware belief portfolios, and a ranked knowledge frontier",
                "the emitted P01 candidates are digest-bound to the local knowledge and frontier outputs",
                "unknown, negative, contradictory, and missing coverage remain explicit and route to a bounded next action",
                "the route performs no retrieval, assay, instrument execution, protected-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma knowledge synthesis operating cycle: {error}"))
    }
}
