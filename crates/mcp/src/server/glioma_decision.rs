//! Glioma research handlers grouped by workflow domain.

use super::*;

impl Server {
    /// Reconcile quorum-bounded, digest-validated site deltas after a partition.
    pub(super) fn glioma_partition_resilient_context_checkpoint(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: PartitionResilientContextCheckpointRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_partition_resilient_context_checkpoint requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma partition checkpoint request: {error}"))?;
        let checkpoint =
            reconcile_partition_resilient_context_checkpoint(&request).map_err(|error| {
                format!("glioma partition checkpoint reconciliation refused: {error}")
            })?;
        Ok(json!({
            "checkpoint": checkpoint,
            "dispatch": "not_started",
            "guarantees": [
                "site quorum, epoch, parent checkpoint, local-data assertions, staleness, signer identity, and delta digests are validated",
                "conflicting values are preserved according to the declared policy and never silently overwritten",
                "reconciliation returns a checkpoint only; it does not write to a remote site"
            ]
        }))
    }

    /// Turn typed glioma knowledge gaps into executable candidates for the bounded action
    /// selector. The returned candidates are still caller-dispatched and cannot touch instruments
    /// or export data without the selector's explicit policy gates.
    pub(super) fn glioma_decision_context(&self, arguments: &Value) -> Result<Value, String> {
        let request: DecisionContextRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_context requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-context request: {error}"))?;
        let knowledge: TypedKnowledge = serde_json::from_value(
            arguments
                .get("knowledge")
                .cloned()
                .ok_or_else(|| "glioma_decision_context requires knowledge".to_string())?,
        )
        .map_err(|error| format!("invalid glioma typed knowledge: {error}"))?;
        let output = compile_decision_context(&request, &knowledge)
            .map_err(|error| format!("glioma decision-context compilation refused: {error}"))?;
        serde_json::to_value(json!({
            "context": output,
            "dispatch": "not_started",
            "guarantees": [
                "each action is a typed candidate consumable by glioma_research_select_actions",
                "missing coverage, contradictions, negative results, and unknown evidence receive distinct actions",
                "all candidates are local A1 computation and remain subject to downstream budget and policy gates",
                "no action is executed and no clinical decision is produced"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision context: {error}"))
    }

    /// Query a bounded, capability-authorized page from a typed decision-context record set.
    pub(super) fn glioma_decision_context_query(&self, arguments: &Value) -> Result<Value, String> {
        let request: DecisionContextQueryRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_context_query requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision context query: {error}"))?;
        let result = query_glioma_decision_context(&request)
            .map_err(|error| format!("glioma decision context query refused: {error}"))?;
        Ok(json!({
            "result": result,
            "dispatch": "not_started",
            "guarantees": [
                "scope, capability digest, revocation, expiry, field access, page size, and result budget are checked before records are returned",
                "omissions and uncertainty follow separate capability grants",
                "query results are typed, bounded, and read-only"
            ]
        }))
    }

    /// Apply a bounded, replay-checked event page to a decision-context snapshot.
    pub(super) fn glioma_decision_context_update(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionContextUpdateRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_context_update requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision context update: {error}"))?;
        let result = update_glioma_decision_context(&request)
            .map_err(|error| format!("glioma decision context update refused: {error}"))?;
        Ok(json!({
            "result": result,
            "dispatch": "not_started",
            "guarantees": [
                "event ordering, digest binding, duplicate sequence, anchor context, and update bounds are validated",
                "negative, contradicted, expired, and invalidated actions remain distinct and update the final context deterministically",
                "updates return a proposed new context and do not dispatch its actions"
            ]
        }))
    }

    /// Store and prune a bounded study-local decision-context snapshot index.
    pub(super) fn glioma_decision_context_snapshot_store(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionContextSnapshotStoreRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_decision_context_snapshot_store requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma decision context snapshot store request: {error}")
            })?;
        let index = store_glioma_decision_context_snapshots(&request)
            .map_err(|error| format!("glioma decision context snapshot store refused: {error}"))?;
        Ok(json!({
            "index": index,
            "dispatch": "not_started",
            "guarantees": [
                "snapshot study identity, parent lineage, epoch order, event references, pinning, and retention bounds are checked",
                "pinned, referenced, and recovery snapshots are protected from pruning",
                "the route compiles a snapshot index and performs no external persistence"
            ]
        }))
    }

    /// Materialize a local P04 context into the portable typed artifact consumed by agents,
    /// workbenches, SDKs, and MCP clients. The artifact contains metadata and action contracts
    /// only; raw evidence remains in the institution-local store.
    pub(super) fn glioma_decision_context_artifact(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionContextArtifactRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_context_artifact requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-context artifact request: {error}"))?;
        let artifact = materialize_glioma_decision_context_artifact(&request).map_err(|error| {
            format!("glioma decision-context artifact materialization refused: {error}")
        })?;
        serde_json::to_value(json!({
            "artifact": artifact,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_decision_admission_gate",
                "glioma_decision_action_graph",
                "glioma_decision_operating_cycle"
            ],
            "guarantees": [
                "the artifact preserves typed actions, dependencies, autonomy tiers, effects, omissions, negatives, and uncertainty",
                "consumer compatibility and semantic-loss declarations are explicit and preclinical-only",
                "source context and artifact content are content-addressed for byte-stable replay",
                "the route performs no retrieval, assay, instrument execution, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-context artifact: {error}"))
    }

    /// Align compatible local context artifacts from independent preclinical studies into a
    /// typed support/conflict frontier. Only action contracts and digest-addressed outcome
    /// summaries are combined; raw evidence and execution authority remain local.
    pub(super) fn glioma_multi_study_context_artifact(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiStudyContextRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multi_study_context_artifact requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma multi-study context request: {error}"))?;
        let artifact = align_glioma_multi_study_context_artifacts(&request)
            .map_err(|error| format!("glioma multi-study context alignment refused: {error}"))?;
        serde_json::to_value(json!({
            "artifact": artifact,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_decision_admission_gate",
                "glioma_decision_action_graph",
                "glioma_federated_decision_context"
            ],
            "guarantees": [
                "independent-study support, group quorum, quality, and typed action conflicts are explicit",
                "denied or low-quality studies are omitted with reasons and cannot silently contribute support",
                "negative and unknown partitions are namespaced by study and retained in the output",
                "the route exchanges no raw evidence, executes no assay or instrument, and makes no clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study context artifact: {error}"))
    }

    /// Compile a validated multi-study frontier into dependency-closed local research tasks.
    /// The planner reserves declared resources and routes material, external, instrument, and
    /// higher-autonomy effects through explicit approval/preflight gates; it never executes them.
    pub(super) fn glioma_multi_study_workflow_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiStudyWorkflowRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multi_study_workflow_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multi-study workflow request: {error}"))?;
        let plan = plan_glioma_multi_study_workflow(&request)
            .map_err(|error| format!("glioma multi-study workflow planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "next_routes": [
                "researcher_approval_gate",
                "glioma_instrument_preflight",
                "glioma_decision_operating_cycle"
            ],
            "guarantees": [
                "only the digest-bound qualified multi-study frontier can enter dependency closure",
                "replication uses distinct independent groups and incomplete quorums are never scheduled as complete",
                "study-local resource budgets, data locality, autonomy ceilings, dependencies, and deterministic waves are explicit",
                "material consumption, external/federated effects, instrument execution, and elevated autonomy require approval/preflight routing",
                "negative, unknown, blocked, and deferred research states remain explicit; no task executes and no clinical decision is produced"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study workflow plan: {error}"))
    }

    /// Bind validated cross-program engine receipts to exact P04 task, study, and stage identities.
    /// The returned value-free ledger can be copied into the next F06 input for outcome replay.
    pub(super) fn glioma_multi_study_execution_receipt(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiStudyExecutionReceiptRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multi_study_execution_receipt requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma multi-study execution receipt request: {error}")
            })?;
        let report = reconcile_glioma_multi_study_execution_receipts(&request)
            .map_err(|error| format!("glioma multi-study execution receipt refused: {error}"))?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "not_started",
            "next_routes": [
                "execution_receipt_collection",
                "researcher_review",
                "glioma_multi_study_context_artifact"
            ],
            "guarantees": [
                "every accepted stage is bound to a digest-validated source receipt, exact P04 task, study, and stage kind",
                "approval-held tasks cannot accept execution receipts before the workflow plan is recompiled",
                "negative or partial engine stage labels map to unknown because generic receipts cannot prove a biological negative result",
                "raw run events and error text are not copied into the report; only bounded digest-addressed stage snapshots are retained",
                "the report can be converted into the per-study F06 outcome_order input and does not dispatch work or make a clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study execution receipt: {error}"))
    }

    /// Aggregate site-local branch plans into a robust continual decision frontier. MCP receives
    /// only digest-bound aggregate summaries; local raw data and execution authority stay at each
    /// institution.
    pub(super) fn glioma_federated_decision_context(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedDecisionContextRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_federated_decision_context requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma federated decision-context request: {error}"))?;
        let report = aggregate_glioma_federated_decision_context(&request).map_err(|error| {
            format!("glioma federated decision-context aggregation refused: {error}")
        })?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_decision_operating_cycle",
                "glioma_federated_decision_context",
                "glioma_decision_branch_plan",
                "glioma_researcher_workbench"
            ],
            "guarantees": [
                "only aggregate-only site summaries and content digests cross the federation boundary",
                "independent-site quorum, quality, support, heterogeneity, and leave-one-site-out influence gates are explicit",
                "negative, contradicted, failed, unknown, denied, and underpowered branches remain visible",
                "the route produces a research-plan recommendation only and performs no retrieval, assay, instrument, raw-data, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated decision context: {error}"))
    }

    /// Require a robust branch to persist across independent federation epochs before advancing.
    pub(super) fn glioma_federated_continual_context_promotion(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FederatedContinualPromotionRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_federated_continual_context_promotion requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma federated continual-promotion request: {error}")
            })?;
        let report = promote_glioma_federated_continual_context(&request).map_err(|error| {
            format!("glioma federated continual-context promotion refused: {error}")
        })?;
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_decision_operating_cycle",
                "glioma_federated_continual_context_promotion",
                "glioma_researcher_workbench",
                "glioma_decision_branch_plan"
            ],
            "guarantees": [
                "each retained source report is independently digest-validated and replayed",
                "promotion requires one qualified top-ranked branch across consecutive epochs",
                "negative, contradicted, failed, unknown, and omitted evidence remains in the epoch ledger",
                "observation identities cannot be reused across epochs to manufacture temporal support",
                "the route performs no assay, instrument, raw-data, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma federated continual promotion: {error}"))
    }

    /// Replay successive decision contexts against explicit local action outcomes so stale plans
    /// are retired before autonomous routing and negative results remain visible.
    pub(super) fn glioma_decision_context_replay(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionContextReplayRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_context_replay requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-context replay request: {error}"))?;
        let replay = replay_glioma_decision_context(&request)
            .map_err(|error| format!("glioma decision-context replay refused: {error}"))?;
        serde_json::to_value(json!({
            "replay": replay,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_decision_admission_gate",
                "glioma_decision_action_graph",
                "glioma_decision_operating_cycle"
            ],
            "guarantees": [
                "successive context snapshots are digest-bound and ordered by epoch",
                "promoted, retired, stable, negative, and unresolved actions remain explicit",
                "unknown, failed, and blocked outcomes prevent an automatic ready disposition",
                "the route performs no retrieval, assay, instrument execution, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-context replay: {error}"))
    }

    /// Require an aligned multi-study action frontier to remain qualified across study epochs.
    pub(super) fn glioma_multi_study_context_epoch_replay(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MultiStudyContextEpochReplayRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_multi_study_context_epoch_replay requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma multi-study context epoch-replay request: {error}")
            })?;
        let report = replay_glioma_multi_study_context_epochs(&request)
            .map_err(|error| format!("glioma multi-study context epoch replay refused: {error}"))?;
        let next_route = report.next_route.clone();
        serde_json::to_value(json!({
            "report": report,
            "dispatch": "not_started",
            "next_route": next_route,
            "guarantees": [
                "each source artifact is digest-validated and retained in the replay request",
                "study epochs are consecutive and independent-group identities cannot drift",
                "stable actions must clear the same quorum and disagreement thresholds for consecutive epochs",
                "negative, unknown, omitted, and conflicted evidence remains explicit",
                "the route performs no assay, instrument, raw-data, publication, or clinical effect"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma multi-study context epoch replay: {error}"))
    }

    /// Assimilate explicit local outcomes into a planned decision-branch frontier, preserving
    /// contradicted, blocked, unobserved, and inconclusive branches for the next research cycle.
    pub(super) fn glioma_decision_branch_evidence(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionBranchEvidenceRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_branch_evidence requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-branch evidence request: {error}"))?;
        let evidence = assimilate_glioma_decision_branch_evidence(&request)
            .map_err(|error| format!("glioma decision-branch evidence refused: {error}"))?;
        serde_json::to_value(json!({
            "evidence": evidence,
            "dispatch": "not_started",
            "next_routes": [
                "glioma_decision_admission_gate",
                "glioma_decision_branch_plan",
                "glioma_decision_operating_cycle"
            ],
            "guarantees": [
                "every planned branch remains explicit after local outcome assimilation",
                "contradicted, failed, blocked, inconclusive, and unobserved evidence is preserved",
                "frontier ordering is deterministic and confirmed selection requires the configured confidence gate",
                "the route performs no retrieval, assay, instrument execution, raw-data movement, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-branch evidence: {error}"))
    }

    /// Gate generated research actions before autonomous execution.
    pub(super) fn glioma_decision_admission_gate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionAdmissionRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_admission_gate requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-admission request: {error}"))?;
        let admission = admit_glioma_decision_actions(&request)
            .map_err(|error| format!("glioma decision admission refused: {error}"))?;
        serde_json::to_value(json!({
            "admission": admission,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_decision_action_graph", "glioma_decision_action_plan", "glioma_decision_context_campaign_execute"],
            "guarantees": [
                "evidence, freshness, coverage, contradiction, reproducibility, dependency, effect, approval, preflight, and budget gates remain explicit",
                "admitted actions are only local typed candidates and are not executed by this route",
                "blocked, approval-required, and denied actions cannot be silently promoted",
                "route performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision admission: {error}"))
    }

    /// Optimize a bounded decision portfolio by expected information value.
    pub(super) fn glioma_decision_value_optimizer(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionValueRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_value_optimizer requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision value request: {error}"))?;
        let optimization = optimize_glioma_decision_value(&request)
            .map_err(|error| format!("glioma decision value optimization refused: {error}"))?;
        serde_json::to_value(json!({
            "optimization": optimization,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_decision_admission_gate", "glioma_decision_action_graph", "glioma_decision_action_plan"],
            "guarantees": [
                "portfolio search is bounded by declared beam, action, alternative, and budget limits",
                "information, uncertainty, contradiction, reproducibility, failure, and diversity terms remain explicit",
                "alternatives, blocked candidates, and deferred candidates remain visible for researcher review",
                "route produces forecast utility only and performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision value optimization: {error}"))
    }

    /// Calibrate bounded decision value forecasts from typed local outcomes without mutating
    /// historical observations or dispatching a new research action.
    pub(super) fn glioma_decision_value_calibrator(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionValueCalibrationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_value_calibrator requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision value calibration request: {error}"))?;
        let calibration = calibrate_glioma_decision_value(&request)
            .map_err(|error| format!("glioma decision value calibration refused: {error}"))?;
        serde_json::to_value(json!({
            "calibration": calibration,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_decision_value_optimizer", "glioma_decision_admission_gate", "glioma_decision_action_graph"],
            "guarantees": [
                "historical observations remain immutable and every failed outcome remains visible",
                "prior utility is shrunk toward typed local outcomes with explicit forecast error and confidence",
                "prior-only, conflicted, and unreliable candidates remain marked before learned scores reach portfolio selection",
                "route produces planning calibration only and performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision value calibration: {error}"))
    }

    /// Select an adaptive exploration/exploitation portfolio from calibrated local outcomes.
    pub(super) fn glioma_adaptive_decision_controller(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AdaptiveDecisionControllerRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_adaptive_decision_controller requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma adaptive controller request: {error}"))?;
        let control = execute_glioma_adaptive_decision_controller(&request)
            .map_err(|error| format!("glioma adaptive decision controller refused: {error}"))?;
        serde_json::to_value(json!({
            "control": control,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_decision_admission_gate", "glioma_decision_action_graph", "glioma_decision_action_plan"],
            "guarantees": [
                "learned utility, bounded exploration, conflict penalties, dependency closure, and budget remain explicit",
                "prior-only candidates can be explored only within the declared exploration bound",
                "blocked, deferred, negative, conflicted, and uncertain actions remain visible for researcher review",
                "route selects a planning portfolio only and performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma adaptive decision controller: {error}"))
    }

    /// Govern whether a bounded autonomous decision loop may continue after local rounds.
    pub(super) fn glioma_decision_loop_governor(&self, arguments: &Value) -> Result<Value, String> {
        let request: DecisionLoopGovernorRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_loop_governor requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision loop request: {error}"))?;
        let governance = govern_glioma_decision_loop(&request)
            .map_err(|error| format!("glioma decision loop governance refused: {error}"))?;
        serde_json::to_value(json!({
            "governance": governance,
            "dispatch": "not_started",
            "simulation_only": true,
            "next_routes": ["glioma_adaptive_decision_controller", "glioma_decision_admission_gate", "glioma_decision_action_graph"],
            "guarantees": [
                "information gain and uncertainty reduction are scored separately from failures, contradictions, and negative outcomes",
                "budget, repeated no-progress, failure limits, incomplete closure, and review requirements can stop continuation",
                "negative results and uncertainty remain visible even when a round is productive",
                "route governs continuation only and performs no assay, instrument, federation, raw-data, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision loop governance: {error}"))
    }

    /// Compile composed knowledge paths into dependency-closed, parallelizable decision actions.
    /// This remains a planning surface; the downstream selector and local executor retain policy
    /// authority for every effect.
    pub(super) fn glioma_decision_action_graph(&self, arguments: &Value) -> Result<Value, String> {
        let request: DecisionActionGraphRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_action_graph requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-action graph request: {error}"))?;
        let context: DecisionContext = serde_json::from_value(
            arguments
                .get("context")
                .cloned()
                .ok_or_else(|| "glioma_decision_action_graph requires context".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision context: {error}"))?;
        let composition: bioprism_research::KnowledgeComposition = serde_json::from_value(
            arguments
                .get("composition")
                .cloned()
                .ok_or_else(|| "glioma_decision_action_graph requires composition".to_string())?,
        )
        .map_err(|error| format!("invalid glioma knowledge composition: {error}"))?;
        let graph = compile_decision_action_graph(&request, &context, &composition)
            .map_err(|error| format!("glioma decision-action graph refused: {error}"))?;
        serde_json::to_value(json!({
            "graph": graph,
            "dispatch": "not_started",
            "guarantees": [
                "explicit claim paths become dependency-closed action candidates with deterministic topological order",
                "independent branches are grouped into bounded parallel waves and critical-path cost is reported",
                "missing claims, unresolved paths, negative evidence, and budget blocks remain visible",
                "the route performs no assay, instrument effect, federation export, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-action graph: {error}"))
    }

    /// Execute a validated P04 decision graph through the P07 autonomous mission controller.
    /// MCP uses the deterministic local worker; institution deployments provide the executor
    /// that owns assay, computation, or instrument effects.
    pub(super) fn glioma_decision_mission_execute(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionMissionBridgeRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_mission_execute requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision mission request: {error}"))?;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_decision_mission(&request, &mut executor)
            .map_err(|error| format!("glioma decision mission refused: {error}"))?;
        serde_json::to_value(json!({
            "run": run,
            "dispatch": "dry_run",
            "simulation_only": true,
            "guarantees": [
                "P04 context and dependency-closed graph digests remain bound into the P07 mission",
                "autonomous rounds replan only from typed local action outcomes",
                "partial graphs require explicit caller opt-in and unresolved action debt remains visible",
                "negative, failed, blocked, budget, and no-progress outcomes stop honestly",
                "the MCP route performs no instrument execution, clinical decision, or raw-data movement"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision mission: {error}"))
    }

    /// Certify claim, modality, model-system, and dependency closure without treating a planned
    /// action as an executed scientific result. Omitted, blocked, and unmeasured coverage remain
    /// separately addressable by the next-action route.
    pub(super) fn glioma_decision_omission_certificate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: DecisionOmissionCertificateRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_decision_omission_certificate requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma omission-certificate request: {error}"))?;
        let context: DecisionContext =
            serde_json::from_value(arguments.get("context").cloned().ok_or_else(|| {
                "glioma_decision_omission_certificate requires context".to_string()
            })?)
            .map_err(|error| format!("invalid glioma decision context: {error}"))?;
        let graph: bioprism_research::DecisionActionGraph =
            serde_json::from_value(arguments.get("graph").cloned().ok_or_else(|| {
                "glioma_decision_omission_certificate requires graph".to_string()
            })?)
            .map_err(|error| format!("invalid glioma decision-action graph: {error}"))?;
        let certificate = certify_decision_omissions(&request, &context, &graph)
            .map_err(|error| format!("glioma omission certification refused: {error}"))?;
        serde_json::to_value(json!({
            "certificate": certificate,
            "dispatch": "not_started",
            "next_route": "glioma_decision_action_plan",
            "guarantees": [
                "claim, modality, model-system, and dependency requirements are classified as closed, omitted, blocked, or unmeasured",
                "budget blocks and unresolved dependency paths remain explicit rather than being treated as complete",
                "the bounded next-action order is deterministic and derived only from typed context and graph state",
                "the route performs no assay, instrument execution, federation export, or clinical decision"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma omission certificate: {error}"))
    }

    /// Plan robust, dependency-closed alternatives across explicit preclinical scenarios. The
    /// branch planner compares portfolios only; the caller-owned selector or executor retains
    /// authority for every assay, instrument, computation, and federation effect.
    pub(super) fn glioma_decision_branch_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: DecisionBranchPlannerRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_branch_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision branch request: {error}"))?;
        let context: DecisionContext = serde_json::from_value(
            arguments
                .get("context")
                .cloned()
                .ok_or_else(|| "glioma_decision_branch_plan requires context".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision context: {error}"))?;
        let plan = plan_glioma_decision_branches(&request, &context)
            .map_err(|error| format!("glioma decision branch planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "candidate portfolios are dependency-closed and bounded by action, cost, beam, and branch limits",
                "expected value, worst-case value, uncertainty, failure risk, and Pareto frontier remain explicit across declared scenarios",
                "missing or negative scenario outcomes never become confident evidence and remain visible as unresolved or negative findings",
                "the route performs no assay, instrument, computation, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision branch plan: {error}"))
    }

    /// Select the next executable portfolio from a compiled glioma decision context. This keeps
    /// evidence compilation and action selection composable while preserving a planning-only MCP
    /// boundary; the returned ids can be submitted to a caller-owned local executor.
    pub(super) fn glioma_decision_action_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request: DecisionActionPlanRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_decision_action_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision-action request: {error}"))?;
        let context: DecisionContext = serde_json::from_value(
            arguments
                .get("context")
                .cloned()
                .ok_or_else(|| "glioma_decision_action_plan requires context".to_string())?,
        )
        .map_err(|error| format!("invalid glioma decision context: {error}"))?;
        let plan = plan_decision_actions(&request, &context)
            .map_err(|error| format!("glioma decision-action planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "guarantees": [
                "compiled evidence gaps are selected with the dependency-aware glioma portfolio policy",
                "completed actions are never selected again and budget or policy holds remain explicit",
                "the selected_order can be handed to glioma_action_portfolio_execute or a caller-owned worker",
                "this route performs no assay, instrument, federation, or clinical action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma decision-action plan: {error}"))
    }

    pub(super) fn glioma_multimodal_qc(&self, arguments: &Value) -> Result<Value, String> {
        let request: MultimodalRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_qc requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal request: {error}"))?;
        let observations: Vec<MultimodalObservation> = serde_json::from_value(
            arguments
                .get("observations")
                .cloned()
                .ok_or_else(|| "glioma_multimodal_qc requires observations".to_string())?,
        )
        .map_err(|error| format!("invalid glioma multimodal observations: {error}"))?;
        let output = harmonize_multimodal_inputs(&request, &observations)
            .map_err(|error| format!("glioma multimodal QC refused: {error}"))?;
        serde_json::to_value(output)
            .map_err(|error| format!("cannot encode glioma multimodal QC: {error}"))
    }
}
