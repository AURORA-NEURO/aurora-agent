//! Autonomous operating-cycle compiler for federated glioma evidence.
//!
//! This is the P01 control surface that turns the preceding evidence products into a bounded
//! next-action portfolio. It consumes the aggregate transport gate, long-horizon calibration,
//! and multi-site reconciliation reports; it does not re-run their algorithms or pretend that a
//! successful gate is a biological conclusion. Every action is an explicit, replayable route for
//! local acquisition, review, reconciliation, negative-result preservation, or typed-knowledge
//! promotion. Physical execution remains outside this planner and must pass the downstream
//! instrument and governance gates.

use super::federated_outcome_transport::{
    FederatedOutcomeTransportDisposition, FederatedOutcomeTransportReport,
};
use super::long_horizon_calibration::{
    LongHorizonCalibrationAnalysis, LongHorizonCalibrationDisposition, LongHorizonCalibrationDrift,
};
use super::outcome_reconciliation::{
    MultiSiteOutcomeAction, MultiSiteOutcomeDisposition, MultiSiteOutcomeReconciliation,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedEvidenceOperatingCycle1@1";
pub const MAX_ACTIONS: usize = 32;
pub const MAX_ROUTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedCycleActionKind {
    ReconcileTransportedOutcomes,
    VerifyFederationOmissions,
    AcquireMissingCoverage,
    RefreshEvidenceCalibration,
    ReviewCalibrationDrift,
    PreserveNegativeResults,
    ResolveContradiction,
    RouteReplication,
    BridgeQualifiedEvidence,
    HoldForResearcherReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedCycleDisposition {
    Ready,
    Partial,
    Blocked,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCycleAction {
    pub action_id: String,
    pub rank: usize,
    pub kind: FederatedCycleActionKind,
    pub priority_milli: u16,
    pub rationale: String,
    pub prerequisite_order: Vec<String>,
    pub route: String,
    pub autonomy_tier: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceOperatingCycleRequest {
    pub objective: String,
    pub cycle_epoch: u32,
    pub max_actions: usize,
    pub require_reconciliation: bool,
    pub transport: FederatedOutcomeTransportReport,
    pub calibration: Option<LongHorizonCalibrationAnalysis>,
    pub reconciliation: Option<MultiSiteOutcomeReconciliation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub cycle_epoch: u32,
    pub next_cycle_epoch: u32,
    pub transport_digest: ContentHash,
    pub calibration_digest: Option<ContentHash>,
    pub reconciliation_digest: Option<ContentHash>,
    pub accepted_bundle_order: Vec<String>,
    pub deferred_bundle_order: Vec<String>,
    pub denied_bundle_order: Vec<String>,
    pub negative_bundle_order: Vec<String>,
    pub contradicted_bundle_order: Vec<String>,
    pub unknown_bundle_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<FederatedCycleAction>,
    pub omitted_action_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedCycleDisposition,
    pub next_routes: Vec<String>,
    pub operator_summary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedEvidenceOperatingCycleError {
    #[error("federated operating-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated operating-cycle input is invalid: {0}")]
    InvalidInput(String),
    #[error("federated operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedEvidenceOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "cycle_epoch": output.cycle_epoch,
        "next_cycle_epoch": output.next_cycle_epoch,
        "transport_digest": output.transport_digest,
        "calibration_digest": output.calibration_digest,
        "reconciliation_digest": output.reconciliation_digest,
        "accepted_bundle_order": output.accepted_bundle_order,
        "deferred_bundle_order": output.deferred_bundle_order,
        "denied_bundle_order": output.denied_bundle_order,
        "negative_bundle_order": output.negative_bundle_order,
        "contradicted_bundle_order": output.contradicted_bundle_order,
        "unknown_bundle_order": output.unknown_bundle_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "omitted_action_order": output.omitted_action_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_routes": output.next_routes,
        "operator_summary": output.operator_summary,
    })
}

impl FederatedEvidenceOperatingCycle {
    pub fn validate(&self) -> Result<(), FederatedEvidenceOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.cycle_epoch == 0
            || self.next_cycle_epoch != self.cycle_epoch.saturating_add(1)
            || !canonical(&self.accepted_bundle_order)
            || !canonical(&self.deferred_bundle_order)
            || !canonical(&self.denied_bundle_order)
            || !canonical(&self.negative_bundle_order)
            || !canonical(&self.contradicted_bundle_order)
            || !canonical(&self.unknown_bundle_order)
            || !canonical(&self.action_order)
            || !canonical(&self.omitted_action_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.next_routes)
            || !canonical(
                &self
                    .actions
                    .iter()
                    .map(|action| action.action_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.action_order.len() != self.actions.len()
            || self.operator_summary.trim().is_empty()
            || self.transport_digest.as_str().len() != 64
            || self
                .calibration_digest
                .as_ref()
                .is_some_and(|digest| digest.as_str().len() != 64)
            || self
                .reconciliation_digest
                .as_ref()
                .is_some_and(|digest| digest.as_str().len() != 64)
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.rank == 0
                    || action.priority_milli > 1_000
                    || action.rationale.trim().is_empty()
                    || !canonical(&action.prerequisite_order)
                    || action.route.trim().is_empty()
                    || action.autonomy_tier != "A0_advisory_local_planner"
            })
        {
            return Err(FederatedEvidenceOperatingCycleError::InvalidOutput(
                "identity, ordering, digest, action contract, or advisory autonomy fields are invalid".into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let action_order = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let omitted = self
            .omitted_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if action_ids.len() != self.actions.len()
            || action_order != action_ids
            || omitted.intersection(&action_ids).next().is_some()
            || self
                .actions
                .iter()
                .any(|action| action.rank > self.actions.len())
        {
            return Err(FederatedEvidenceOperatingCycleError::InvalidOutput(
                "action identity, ranking, or omission partition is inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedEvidenceOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedEvidenceOperatingCycleError::Digest(
                "operating-cycle digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ActionSeed {
    kind: FederatedCycleActionKind,
    priority_milli: u16,
    rationale: String,
    prerequisite_order: Vec<String>,
    route: String,
}

fn push_seed(seeds: &mut Vec<ActionSeed>, seed: ActionSeed) {
    if !seeds.iter().any(|existing| existing.kind == seed.kind) {
        seeds.push(seed);
    }
}

fn action_route(kind: FederatedCycleActionKind) -> &'static str {
    match kind {
        FederatedCycleActionKind::ReconcileTransportedOutcomes => {
            "glioma_multisite_outcome_reconciliation"
        }
        FederatedCycleActionKind::VerifyFederationOmissions => "glioma_evidence_verification_gate",
        FederatedCycleActionKind::AcquireMissingCoverage => {
            "glioma_federated_evidence_acquisition_policy"
        }
        FederatedCycleActionKind::RefreshEvidenceCalibration
        | FederatedCycleActionKind::ReviewCalibrationDrift => {
            "glioma_long_horizon_evidence_calibration"
        }
        FederatedCycleActionKind::PreserveNegativeResults => "glioma_evidence_knowledge_bridge",
        FederatedCycleActionKind::ResolveContradiction => "plan_glioma_evidence_contradiction_cut",
        FederatedCycleActionKind::RouteReplication => {
            "glioma_federated_evidence_acquisition_policy"
        }
        FederatedCycleActionKind::BridgeQualifiedEvidence => "glioma_evidence_knowledge_bridge",
        FederatedCycleActionKind::HoldForResearcherReview => "glioma_evidence_researcher_workbench",
    }
}

fn action_priority(kind: FederatedCycleActionKind) -> u16 {
    match kind {
        FederatedCycleActionKind::VerifyFederationOmissions => 980,
        FederatedCycleActionKind::ResolveContradiction => 960,
        FederatedCycleActionKind::RefreshEvidenceCalibration => 930,
        FederatedCycleActionKind::ReviewCalibrationDrift => 910,
        FederatedCycleActionKind::AcquireMissingCoverage => 880,
        FederatedCycleActionKind::ReconcileTransportedOutcomes => 850,
        FederatedCycleActionKind::PreserveNegativeResults => 800,
        FederatedCycleActionKind::RouteReplication => 780,
        FederatedCycleActionKind::HoldForResearcherReview => 760,
        FederatedCycleActionKind::BridgeQualifiedEvidence => 700,
    }
}

/// Compile the next bounded federation cycle from the preceding P01 reports. The returned plan
/// is advisory and deterministic: an institution-local executor must separately authorize every
/// acquisition, analysis, or knowledge promotion action.
pub fn compile_glioma_federated_evidence_operating_cycle(
    request: &FederatedEvidenceOperatingCycleRequest,
) -> Result<FederatedEvidenceOperatingCycle, FederatedEvidenceOperatingCycleError> {
    if request.objective.trim().is_empty()
        || request.cycle_epoch == 0
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.transport.objective != request.objective
    {
        return Err(FederatedEvidenceOperatingCycleError::InvalidRequest(
            "objective, cycle epoch, action bound, or transport binding is invalid".into(),
        ));
    }
    request
        .transport
        .validate()
        .map_err(|error| FederatedEvidenceOperatingCycleError::InvalidInput(error.to_string()))?;
    if let Some(calibration) = &request.calibration {
        if calibration.objective != request.objective {
            return Err(FederatedEvidenceOperatingCycleError::InvalidInput(
                "calibration objective does not match cycle objective".into(),
            ));
        }
        calibration.validate().map_err(|error| {
            FederatedEvidenceOperatingCycleError::InvalidInput(error.to_string())
        })?;
    }
    if let Some(reconciliation) = &request.reconciliation {
        if reconciliation.objective != request.objective {
            return Err(FederatedEvidenceOperatingCycleError::InvalidInput(
                "reconciliation objective does not match cycle objective".into(),
            ));
        }
        reconciliation.validate().map_err(|error| {
            FederatedEvidenceOperatingCycleError::InvalidInput(error.to_string())
        })?;
    } else if request.require_reconciliation {
        return Err(FederatedEvidenceOperatingCycleError::InvalidRequest(
            "cycle requires reconciliation but no reconciliation report was supplied".into(),
        ));
    }

    let mut seeds = Vec::new();
    if !request.transport.deferred_order.is_empty() || !request.transport.denied_order.is_empty() {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::VerifyFederationOmissions,
                priority_milli: action_priority(FederatedCycleActionKind::VerifyFederationOmissions),
                rationale: "deferred or denied bundles require explicit policy and evidence review before another cycle".into(),
                prerequisite_order: Vec::new(),
                route: action_route(FederatedCycleActionKind::VerifyFederationOmissions).into(),
            },
        );
    }
    if request.transport.disposition != FederatedOutcomeTransportDisposition::Ready
        || !request.transport.required_modalities_missing.is_empty()
        || !request.transport.required_model_systems_missing.is_empty()
    {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::AcquireMissingCoverage,
                priority_milli: action_priority(FederatedCycleActionKind::AcquireMissingCoverage),
                rationale: "transport is partial or lacks required modality/model coverage; acquire only the bounded missing context".into(),
                prerequisite_order: vec!["transport_report_validated".into()],
                route: action_route(FederatedCycleActionKind::AcquireMissingCoverage).into(),
            },
        );
    }
    if request.reconciliation.is_none() {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::ReconcileTransportedOutcomes,
                priority_milli: action_priority(FederatedCycleActionKind::ReconcileTransportedOutcomes),
                rationale: "accepted site summaries have not yet crossed the independent-site reconciliation gate".into(),
                prerequisite_order: request
                    .transport
                    .deferred_order
                    .iter()
                    .map(|bundle| format!("verify:{bundle}"))
                    .collect(),
                route: action_route(FederatedCycleActionKind::ReconcileTransportedOutcomes).into(),
            },
        );
    }
    if let Some(calibration) = &request.calibration {
        let drifted = calibration.families.iter().any(|family| {
            matches!(
                family.drift,
                LongHorizonCalibrationDrift::Degrading
                    | LongHorizonCalibrationDrift::Volatile
                    | LongHorizonCalibrationDrift::Insufficient
            )
        });
        if drifted {
            push_seed(
                &mut seeds,
                ActionSeed {
                    kind: FederatedCycleActionKind::RefreshEvidenceCalibration,
                    priority_milli: action_priority(FederatedCycleActionKind::RefreshEvidenceCalibration),
                    rationale: "source-family calibration drift or insufficient windows must be refreshed before promotion".into(),
                    prerequisite_order: calibration.review_order.clone(),
                    route: action_route(FederatedCycleActionKind::RefreshEvidenceCalibration).into(),
                },
            );
        } else if calibration.disposition != LongHorizonCalibrationDisposition::Qualified
            || !calibration.unknown_order.is_empty()
            || !calibration.omitted_order.is_empty()
        {
            push_seed(
                &mut seeds,
                ActionSeed {
                    kind: FederatedCycleActionKind::ReviewCalibrationDrift,
                    priority_milli: action_priority(FederatedCycleActionKind::ReviewCalibrationDrift),
                    rationale: "calibration has unresolved, unknown, or omitted windows that require a researcher-visible review".into(),
                    prerequisite_order: calibration.unknown_order.clone(),
                    route: action_route(FederatedCycleActionKind::ReviewCalibrationDrift).into(),
                },
            );
        }
    } else {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::ReviewCalibrationDrift,
                priority_milli: action_priority(FederatedCycleActionKind::ReviewCalibrationDrift),
                rationale: "no long-horizon calibration report is bound to this federation cycle"
                    .into(),
                prerequisite_order: Vec::new(),
                route: action_route(FederatedCycleActionKind::ReviewCalibrationDrift).into(),
            },
        );
    }

    let negative_bundle_order = request
        .transport
        .decisions
        .iter()
        .filter(|decision| {
            decision.outcome_state == crate::glioma::evidence::EvidenceState::Negative
        })
        .map(|decision| decision.bundle_id.clone())
        .collect::<Vec<_>>();
    let contradicted_bundle_order = request
        .transport
        .decisions
        .iter()
        .filter(|decision| {
            decision.outcome_state == crate::glioma::evidence::EvidenceState::Contradicted
        })
        .map(|decision| decision.bundle_id.clone())
        .collect::<Vec<_>>();
    let unknown_bundle_order = request
        .transport
        .decisions
        .iter()
        .filter(|decision| {
            matches!(
                decision.outcome_state,
                crate::glioma::evidence::EvidenceState::Unknown
                    | crate::glioma::evidence::EvidenceState::Unmeasured
                    | crate::glioma::evidence::EvidenceState::Stale
            )
        })
        .map(|decision| decision.bundle_id.clone())
        .collect::<Vec<_>>();
    if !negative_bundle_order.is_empty() {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::PreserveNegativeResults,
                priority_milli: action_priority(FederatedCycleActionKind::PreserveNegativeResults),
                rationale: "negative site outcomes are first-class evidence and must be retained in the next knowledge handoff".into(),
                prerequisite_order: negative_bundle_order.clone(),
                route: action_route(FederatedCycleActionKind::PreserveNegativeResults).into(),
            },
        );
    }
    if !contradicted_bundle_order.is_empty() {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::ResolveContradiction,
                priority_milli: action_priority(FederatedCycleActionKind::ResolveContradiction),
                rationale: "contradictory site outcomes cannot be averaged into support; route a bounded contradiction cut".into(),
                prerequisite_order: contradicted_bundle_order.clone(),
                route: action_route(FederatedCycleActionKind::ResolveContradiction).into(),
            },
        );
    }

    if let Some(reconciliation) = &request.reconciliation {
        for claim in &reconciliation.claims {
            let (kind, rationale) = match claim.action {
                MultiSiteOutcomeAction::PromoteToKnowledge
                    if reconciliation.disposition == MultiSiteOutcomeDisposition::Ready
                        && request.transport.disposition == FederatedOutcomeTransportDisposition::Ready => (
                    FederatedCycleActionKind::BridgeQualifiedEvidence,
                    "reconciled support passed the aggregate gates and can enter typed-knowledge review",
                ),
                MultiSiteOutcomeAction::PreserveNegative => (
                    FederatedCycleActionKind::PreserveNegativeResults,
                    "reconciler marked a null or negative result for explicit knowledge preservation",
                ),
                MultiSiteOutcomeAction::RouteContradiction => (
                    FederatedCycleActionKind::ResolveContradiction,
                    "reconciler found cross-site contradiction requiring explicit resolution",
                ),
                MultiSiteOutcomeAction::RouteReplication => (
                    FederatedCycleActionKind::RouteReplication,
                    "reconciler requires an independent replication route",
                ),
                MultiSiteOutcomeAction::AcquireCoverage => (
                    FederatedCycleActionKind::AcquireMissingCoverage,
                    "reconciler found a modality, model, or site coverage gap",
                ),
                MultiSiteOutcomeAction::Hold => (
                    FederatedCycleActionKind::HoldForResearcherReview,
                    "reconciler cannot safely classify the claim without researcher review",
                ),
                _ => continue,
            };
            push_seed(
                &mut seeds,
                ActionSeed {
                    kind,
                    priority_milli: action_priority(kind),
                    rationale: format!("{rationale}: {}", claim.claim_id),
                    prerequisite_order: claim.observation_order.clone(),
                    route: action_route(kind).into(),
                },
            );
        }
    }
    if seeds.is_empty() {
        push_seed(
            &mut seeds,
            ActionSeed {
                kind: FederatedCycleActionKind::BridgeQualifiedEvidence,
                priority_milli: action_priority(FederatedCycleActionKind::BridgeQualifiedEvidence),
                rationale: "all supplied federation signals are ready; route the bounded result into typed-knowledge review".into(),
                prerequisite_order: request.transport.accepted_order.clone(),
                route: action_route(FederatedCycleActionKind::BridgeQualifiedEvidence).into(),
            },
        );
    }
    seeds.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    let omitted_action_order = seeds
        .iter()
        .skip(request.max_actions)
        .enumerate()
        .map(|(index, seed)| format!("omitted-{:03}-{:?}", index + 1, seed.kind))
        .collect::<Vec<_>>();
    let actions = seeds
        .into_iter()
        .take(request.max_actions)
        .enumerate()
        .map(|(index, seed)| FederatedCycleAction {
            action_id: format!("action-{:03}", index + 1),
            rank: index + 1,
            kind: seed.kind,
            priority_milli: seed.priority_milli,
            rationale: seed.rationale,
            prerequisite_order: seed.prerequisite_order,
            route: seed.route,
            autonomy_tier: "A0_advisory_local_planner".into(),
        })
        .collect::<Vec<_>>();
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    if !unknown_bundle_order.is_empty() {
        uncertainty.push("unknown_or_unmeasured_bundle_states_remain_unresolved".into());
    }
    if request.transport.disposition != FederatedOutcomeTransportDisposition::Ready {
        uncertainty.push("federated_transport_is_not_ready".into());
    }
    if request.calibration.is_none() {
        uncertainty.push("long_horizon_calibration_report_missing".into());
    }
    if request.reconciliation.is_none() {
        uncertainty.push("multi_site_reconciliation_report_missing".into());
    }
    if !omitted_action_order.is_empty() {
        uncertainty.push("cycle_action_budget_truncated_the_candidate_portfolio".into());
    }
    uncertainty.sort();
    uncertainty.dedup();

    let disposition = if actions
        .iter()
        .any(|action| action.kind == FederatedCycleActionKind::HoldForResearcherReview)
    {
        FederatedCycleDisposition::Hold
    } else if request.transport.disposition == FederatedOutcomeTransportDisposition::Blocked
        && request.transport.accepted_order.is_empty()
    {
        FederatedCycleDisposition::Blocked
    } else if !omitted_action_order.is_empty()
        || request.transport.disposition != FederatedOutcomeTransportDisposition::Ready
        || request.reconciliation.is_none()
        || request.calibration.is_none()
    {
        FederatedCycleDisposition::Partial
    } else {
        FederatedCycleDisposition::Ready
    };
    let mut next_routes = actions
        .iter()
        .map(|action| action.route.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    next_routes.truncate(MAX_ROUTES);
    let operator_summary = match disposition {
        FederatedCycleDisposition::Ready => {
            "federated evidence is ready for the declared next routes; execute only after local policy admission".into()
        }
        FederatedCycleDisposition::Partial => {
            "federated evidence is actionable but incomplete; execute the ranked bounded actions and retain omissions".into()
        }
        FederatedCycleDisposition::Blocked => {
            "federated evidence is blocked; no downstream promotion or physical execution is authorized".into()
        }
        FederatedCycleDisposition::Hold => {
            "federated evidence requires researcher review before an autonomous continuation".into()
        }
    };
    let mut output = FederatedEvidenceOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        cycle_epoch: request.cycle_epoch,
        next_cycle_epoch: request.cycle_epoch.saturating_add(1),
        transport_digest: request.transport.digest.clone(),
        calibration_digest: request
            .calibration
            .as_ref()
            .map(|calibration| calibration.digest.clone()),
        reconciliation_digest: request
            .reconciliation
            .as_ref()
            .map(|reconciliation| reconciliation.digest.clone()),
        accepted_bundle_order: request.transport.accepted_order.clone(),
        deferred_bundle_order: request.transport.deferred_order.clone(),
        denied_bundle_order: request.transport.denied_order.clone(),
        negative_bundle_order,
        contradicted_bundle_order,
        unknown_bundle_order,
        action_order,
        actions,
        omitted_action_order,
        uncertainty,
        disposition,
        next_routes,
        operator_summary,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedEvidenceOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceState;
    use crate::glioma::programs::p01_evidence_surveillance::federated_outcome_transport::{
        FederatedOutcomeBundleDecision, FederatedOutcomeBundleDecisionRecord,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
    use bioprism_ids::ContentHash;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn transport() -> FederatedOutcomeTransportReport {
        let decisions = vec![
            FederatedOutcomeBundleDecisionRecord {
                bundle_id: "b-01".into(),
                site_id: "site-1".into(),
                independence_group: "group-1".into(),
                decision: FederatedOutcomeBundleDecision::Accepted,
                reason: "aggregate_summary_ready".into(),
                outcome_state: EvidenceState::Supported,
            },
            FederatedOutcomeBundleDecisionRecord {
                bundle_id: "b-02".into(),
                site_id: "site-2".into(),
                independence_group: "group-2".into(),
                decision: FederatedOutcomeBundleDecision::Accepted,
                reason: "aggregate_summary_ready".into(),
                outcome_state: EvidenceState::Negative,
            },
        ];
        let mut report = FederatedOutcomeTransportReport {
            feature_id: super::super::federated_outcome_transport::FEATURE_ID.into(),
            output_schema: super::super::federated_outcome_transport::OUTPUT_SCHEMA.into(),
            objective: "autonomous federation cycle".into(),
            consortium_id: "consortium-a".into(),
            claim_key: "claim".into(),
            scope_key: "scope".into(),
            bundle_order: vec!["b-01".into(), "b-02".into()],
            accepted_order: vec!["b-01".into(), "b-02".into()],
            deferred_order: Vec::new(),
            denied_order: Vec::new(),
            decisions,
            accepted_site_order: vec!["site-1".into(), "site-2".into()],
            accepted_independent_group_order: vec!["group-1".into(), "group-2".into()],
            accepted_modality_order: vec![GliomaModality::Transcriptomics],
            accepted_model_system_order: vec![GliomaModelSystem::Organoid],
            required_modalities_missing: Vec::new(),
            required_model_systems_missing: Vec::new(),
            accepted_count: 2,
            independent_group_count: 2,
            site_quorum_satisfied: true,
            independent_quorum_satisfied: true,
            raw_data_local_only: true,
            omission_order: Vec::new(),
            disposition: FederatedOutcomeTransportDisposition::Ready,
            next_routes: vec!["glioma_evidence_knowledge_bridge".into()],
            digest: hash("pending"),
        };
        report.digest = ContentHash::of_value(
            &super::super::federated_outcome_transport::digest_input(&report),
        )
        .unwrap();
        report
    }

    fn request() -> FederatedEvidenceOperatingCycleRequest {
        FederatedEvidenceOperatingCycleRequest {
            objective: "autonomous federation cycle".into(),
            cycle_epoch: 4,
            max_actions: 8,
            require_reconciliation: false,
            transport: transport(),
            calibration: None,
            reconciliation: None,
        }
    }

    #[test]
    fn cycle_ranks_missing_calibration_and_reconciliation_without_promoting() {
        let output = compile_glioma_federated_evidence_operating_cycle(&request()).unwrap();
        assert_eq!(output.disposition, FederatedCycleDisposition::Partial);
        assert!(output
            .actions
            .iter()
            .any(|action| action.kind == FederatedCycleActionKind::ReconcileTransportedOutcomes));
        assert!(output
            .uncertainty
            .contains(&"long_horizon_calibration_report_missing".into()));
        output.validate().unwrap();
    }

    #[test]
    fn negative_bundle_gets_explicit_preservation_action() {
        let output = compile_glioma_federated_evidence_operating_cycle(&request()).unwrap();
        assert!(output.negative_bundle_order.contains(&"b-02".to_string()));
        assert!(output
            .actions
            .iter()
            .any(|action| action.kind == FederatedCycleActionKind::PreserveNegativeResults));
    }

    #[test]
    fn action_budget_is_deterministic_and_reports_omissions() {
        let mut request = request();
        request.max_actions = 1;
        let first = compile_glioma_federated_evidence_operating_cycle(&request).unwrap();
        let second = compile_glioma_federated_evidence_operating_cycle(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.actions.len(), 1);
        assert!(!first.omitted_action_order.is_empty());
    }

    #[test]
    fn required_reconciliation_is_a_hard_input_gate() {
        let mut request = request();
        request.require_reconciliation = true;
        let error = compile_glioma_federated_evidence_operating_cycle(&request).unwrap_err();
        assert!(matches!(
            error,
            FederatedEvidenceOperatingCycleError::InvalidRequest(_)
        ));
    }
}
