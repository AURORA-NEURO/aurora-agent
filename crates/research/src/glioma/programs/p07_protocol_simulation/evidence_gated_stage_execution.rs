//! Evidence-admitted autonomous stage execution for preclinical glioma research.
//!
//! This is the workflow composition seam between P01 triangulation and the P07 autonomous
//! engine. It does not add another catalog slot: it extends the existing evidence-gated research
//! and stage-worker execution features into one usable product operation. A qualified evidence
//! packet admits routing; partial, negative, contradictory, unresolved, or fragile evidence holds
//! before any worker is invoked. A route hold is equally explicit when a ready stage has no local
//! capability. Only after both gates pass does the bounded engine run through the typed stage
//! registry.

use super::autonomous_engine::GliomaAutonomousResearchEngineRequest;
use super::stage_worker_registry::{
    compile_glioma_stage_worker_routes,
    execute_glioma_autonomous_research_engine_with_stage_workers,
    GliomaAutonomousResearchStageExecution, GliomaStageWorkerExecutionError,
    GliomaStageWorkerProfile, GliomaStageWorkerRouteDisposition, GliomaStageWorkerRouteError,
    GliomaStageWorkerRoutePlan, GliomaStageWorkerRouteRequest,
};
use crate::glioma::programs::p01_evidence_surveillance::{
    EvidenceTriangulation, EvidenceTriangulationDisposition,
};
use crate::glioma_engine::GliomaStageExecutor;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const PARENT_FEATURE_ID: &str = "GAF-GLIOMA-P07-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceGatedStageExecution1@1";
const MAX_QUALIFIED_CLAIMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaEvidenceGatedStageExecutionRequest {
    pub engine: GliomaAutonomousResearchEngineRequest,
    pub triangulation: EvidenceTriangulation,
    pub min_qualified_claims: usize,
    #[serde(default)]
    pub require_global_qualification: bool,
    pub workers: Vec<GliomaStageWorkerProfile>,
    #[serde(default = "default_require_deterministic")]
    pub require_deterministic: bool,
}

fn default_require_deterministic() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaEvidenceGatedStageExecutionDisposition {
    EvidenceHold,
    RouteHold,
    Executed,
    EngineBlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaEvidenceGatedStageExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub triangulation_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub qualified_claim_order: Vec<String>,
    pub blocked_claim_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub hold_reason_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub route_plan: Option<GliomaStageWorkerRoutePlan>,
    pub execution: Option<GliomaAutonomousResearchStageExecution>,
    pub execution_started: bool,
    pub disposition: GliomaEvidenceGatedStageExecutionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaEvidenceGatedStageExecutionError {
    #[error("evidence-gated stage request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-gated stage route failed: {0}")]
    Route(#[from] GliomaStageWorkerRouteError),
    #[error("evidence-gated stage engine failed: {0}")]
    Engine(#[from] GliomaStageWorkerExecutionError),
    #[error("evidence-gated stage output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-gated stage digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let values = values.into_iter().collect::<BTreeSet<_>>();
    values.iter().cloned().collect()
}

fn digest_input(output: &GliomaEvidenceGatedStageExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "mission_id": output.mission_id,
        "objective": output.objective,
        "triangulation_digest": output.triangulation_digest,
        "claim_order": output.claim_order,
        "qualified_claim_order": output.qualified_claim_order,
        "blocked_claim_order": output.blocked_claim_order,
        "next_action_order": output.next_action_order,
        "hold_reason_order": output.hold_reason_order,
        "uncertainty": output.uncertainty,
        "route_plan": output.route_plan,
        "execution": output.execution,
        "execution_started": output.execution_started,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &GliomaEvidenceGatedStageExecutionRequest,
) -> Result<(), GliomaEvidenceGatedStageExecutionError> {
    if request.engine.mission_id.trim().is_empty()
        || request.engine.intent.objective.trim().is_empty()
        || request.engine.budget_units == 0
        || request.engine.max_actions == 0
        || request.engine.max_cycles == 0
        || request.min_qualified_claims == 0
        || request.min_qualified_claims > MAX_QUALIFIED_CLAIMS
        || request.workers.is_empty()
    {
        return Err(GliomaEvidenceGatedStageExecutionError::InvalidRequest(
            "mission, objective, positive engine bounds, evidence floor, and local workers are required".into(),
        ));
    }
    request
        .triangulation
        .validate()
        .map_err(|error| GliomaEvidenceGatedStageExecutionError::InvalidRequest(error.to_string()))
}

impl GliomaEvidenceGatedStageExecution {
    pub fn validate(&self) -> Result<(), GliomaEvidenceGatedStageExecutionError> {
        if self.feature_id != PARENT_FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.qualified_claim_order)
            || !canonical(&self.blocked_claim_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.hold_reason_order)
            || !canonical(&self.uncertainty)
            || self.qualified_claim_order.iter().any(|id| {
                self.blocked_claim_order.binary_search(id).is_ok()
                    || self.claim_order.binary_search(id).is_err()
            })
            || self
                .blocked_claim_order
                .iter()
                .any(|id| self.claim_order.binary_search(id).is_err())
            || self.claim_order.len()
                != self.qualified_claim_order.len() + self.blocked_claim_order.len()
            || self.execution_started != self.execution.is_some()
            || matches!(
                self.disposition,
                GliomaEvidenceGatedStageExecutionDisposition::EvidenceHold
            ) && (self.route_plan.is_some() || self.execution.is_some())
            || matches!(
                self.disposition,
                GliomaEvidenceGatedStageExecutionDisposition::RouteHold
            ) && (self.route_plan.is_none() || self.execution.is_some())
            || matches!(
                self.disposition,
                GliomaEvidenceGatedStageExecutionDisposition::Executed
                    | GliomaEvidenceGatedStageExecutionDisposition::EngineBlocked
            ) && (self.route_plan.is_none() || self.execution.is_none())
        {
            return Err(GliomaEvidenceGatedStageExecutionError::InvalidOutput(
                "identity, evidence partition, execution binding, or disposition invariants are invalid".into(),
            ));
        }
        if let Some(route_plan) = &self.route_plan {
            route_plan.validate()?;
        }
        if let Some(execution) = &self.execution {
            execution.validate()?;
            if self
                .route_plan
                .as_ref()
                .is_some_and(|route| route.digest != execution.route_plan.digest)
            {
                return Err(GliomaEvidenceGatedStageExecutionError::InvalidOutput(
                    "embedded route plan differs from executed route plan".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaEvidenceGatedStageExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEvidenceGatedStageExecutionError::InvalidOutput(
                "evidence-gated stage digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Admit and execute a bounded autonomous glioma engine only after evidence and worker gates pass.
/// The worker map remains caller-owned: this function never opens instruments, exports data, or
/// makes a clinical decision. A hold is a successful, typed product outcome rather than an error.
pub fn execute_glioma_evidence_gated_stage_engine(
    request: &GliomaEvidenceGatedStageExecutionRequest,
    workers: std::collections::BTreeMap<String, Box<dyn GliomaStageExecutor>>,
) -> Result<GliomaEvidenceGatedStageExecution, GliomaEvidenceGatedStageExecutionError> {
    validate_request(request)?;
    let triangulation = &request.triangulation;
    let claim_order = triangulation.claim_order.clone();
    let qualified_claim_order = triangulation.qualified_order.clone();
    let blocked_claim_order = claim_order
        .iter()
        .filter(|claim_id| !qualified_claim_order.contains(claim_id))
        .cloned()
        .collect::<Vec<_>>();
    let enough_qualified = qualified_claim_order.len() >= request.min_qualified_claims;
    let evidence_admitted = enough_qualified
        && triangulation.disposition == EvidenceTriangulationDisposition::Qualified
        && (!request.require_global_qualification || blocked_claim_order.is_empty());

    let mut output = GliomaEvidenceGatedStageExecution {
        feature_id: PARENT_FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.engine.mission_id.clone(),
        objective: request.engine.intent.objective.clone(),
        triangulation_digest: triangulation.digest.clone(),
        claim_order,
        qualified_claim_order,
        blocked_claim_order,
        next_action_order: Vec::new(),
        hold_reason_order: Vec::new(),
        uncertainty: triangulation.uncertainty.clone(),
        route_plan: None,
        execution: None,
        execution_started: false,
        disposition: GliomaEvidenceGatedStageExecutionDisposition::EvidenceHold,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-gated-stage"),
    };

    if !evidence_admitted {
        let mut reasons = BTreeSet::new();
        reasons.insert(format!(
            "triangulation-disposition:{:?}",
            triangulation.disposition
        ));
        if !enough_qualified {
            reasons.insert(format!(
                "qualified-claim-floor:{}<{}",
                output.qualified_claim_order.len(),
                request.min_qualified_claims
            ));
        }
        if request.require_global_qualification && !output.blocked_claim_order.is_empty() {
            reasons.insert("global-qualification-required".into());
        }
        output.hold_reason_order = reasons.into_iter().collect();
        output.next_action_order = if triangulation.next_action_order.is_empty() {
            output
                .blocked_claim_order
                .iter()
                .map(|claim| format!("resolve-claim:{claim}"))
                .collect()
        } else {
            triangulation.next_action_order.clone()
        };
    } else {
        let route_request = GliomaStageWorkerRouteRequest {
            intent: request.engine.intent.clone(),
            workers: request.workers.clone(),
            require_deterministic: request.require_deterministic,
            require_all_ready: false,
        };
        let route_plan = compile_glioma_stage_worker_routes(&route_request)?;
        let blocked_ready = route_plan
            .routes
            .iter()
            .filter(|route| {
                route.disposition == GliomaStageWorkerRouteDisposition::MissingCapability
            })
            .map(|route| route.stage_id.clone())
            .collect::<Vec<_>>();
        if !blocked_ready.is_empty() {
            output.route_plan = Some(route_plan.clone());
            output.disposition = GliomaEvidenceGatedStageExecutionDisposition::RouteHold;
            output.next_action_order = sorted_unique(
                route_plan
                    .routes
                    .iter()
                    .filter(|route| route.worker_id.is_none())
                    .map(|route| format!("route-stage:{}", route.stage_id)),
            );
            output.hold_reason_order = sorted_unique(route_plan.routes.iter().flat_map(|route| {
                route
                    .reasons
                    .iter()
                    .map(move |reason| format!("{}:{reason}", route.stage_id))
            }));
            output.uncertainty = sorted_unique(
                output
                    .uncertainty
                    .iter()
                    .cloned()
                    .chain(std::iter::once("worker-capability-gate-not-cleared".into())),
            );
        } else {
            let execution = execute_glioma_autonomous_research_engine_with_stage_workers(
                &request.engine,
                &route_plan,
                workers,
            )?;
            output.route_plan = Some(route_plan);
            output.execution_started = true;
            output.disposition = if matches!(
                execution.engine.disposition,
                super::autonomous_engine::GliomaAutonomousResearchEngineDisposition::Blocked
                    | super::autonomous_engine::GliomaAutonomousResearchEngineDisposition::NoRunnableActions
            ) {
                GliomaEvidenceGatedStageExecutionDisposition::EngineBlocked
            } else {
                GliomaEvidenceGatedStageExecutionDisposition::Executed
            };
            output.next_action_order = sorted_unique(
                execution
                    .engine
                    .pending_action_order
                    .iter()
                    .cloned()
                    .chain(execution.engine.hold_order.iter().cloned())
                    .chain(execution.engine.blocked_order.iter().cloned()),
            );
            output.uncertainty = sorted_unique(
                output
                    .uncertainty
                    .iter()
                    .cloned()
                    .chain(execution.engine.uncertainty.iter().cloned()),
            );
            output.execution = Some(execution);
        }
    }
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaEvidenceGatedStageExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::autonomous_engine::GliomaAdaptiveReplanningPolicy;
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p01_evidence_surveillance::{
        triangulate_glioma_evidence, EvidenceTriangulationRequest,
    };
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaResearchIntent, GliomaSelectionWeights,
        LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_ids::ContentHash;
    use bioprism_onco::OutputUse;
    use std::collections::{BTreeMap, BTreeSet};

    fn intent() -> GliomaResearchIntent {
        let hash = ContentHash::of_bytes(b"evidence-gated-stage-input");
        GliomaResearchIntent {
            research_id: "evidence-gated-stage-research".into(),
            study_id: "evidence-gated-stage-study".into(),
            objective: "identify reproducible invasion mechanisms in glioma organoids".into(),
            output_uses: BTreeSet::from([OutputUse::CohortAnalysis, OutputUse::MethodDevelopment]),
            model_systems: BTreeSet::from([
                GliomaModelSystem::Organoid,
                GliomaModelSystem::InSilico,
            ]),
            modalities: BTreeSet::from([
                GliomaModality::Literature,
                GliomaModality::Transcriptomics,
                GliomaModality::Imaging,
                GliomaModality::Computational,
            ]),
            input_artifacts: vec![LocalArtifactRef {
                artifact_id: "input".into(),
                content_hash: hash.clone(),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 80,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn engine_request() -> GliomaAutonomousResearchEngineRequest {
        GliomaAutonomousResearchEngineRequest {
            mission_id: "evidence-gated-stage-mission".into(),
            intent: intent(),
            focus: super::super::director::GliomaDirectorFocus::MechanismFirst,
            completed_checkpoints: Vec::new(),
            budget_units: 80,
            max_actions: 2,
            max_cycles: 1,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
            outcome_summaries: BTreeMap::new(),
            adaptive_policy: GliomaAdaptiveReplanningPolicy::default(),
        }
    }

    fn triangulation(qualified: bool) -> EvidenceTriangulation {
        let records = (0..3)
            .map(|index| EvidenceRecord {
                evidence_id: format!("e{index}"),
                source_artifact: LocalArtifactRef {
                    artifact_id: format!("artifact-{index}"),
                    content_hash: ContentHash::of_bytes(format!("artifact-{index}").as_bytes()),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                source_kind: match index {
                    0 => EvidenceSourceKind::Literature,
                    1 => EvidenceSourceKind::Assay,
                    _ => EvidenceSourceKind::Replication,
                },
                claim: "EGFR signaling increases organoid invasion".into(),
                scope: "organoid:invasion".into(),
                modality: GliomaModality::Transcriptomics,
                model_system: Some(GliomaModelSystem::Organoid),
                state: if qualified {
                    EvidenceState::Supported
                } else {
                    EvidenceState::Unknown
                },
                relevance_milli: 900,
                quality_milli: 900,
                reproducibility_milli: 900,
                release_epoch: 1,
            })
            .collect::<Vec<_>>();
        triangulate_glioma_evidence(
            &EvidenceTriangulationRequest {
                objective: "triangulate glioma invasion evidence".into(),
                min_source_kinds: 3,
                min_independent_artifacts: 3,
                min_support_milli: 600,
                max_contradiction_milli: 200,
                min_diversity_milli: 1_000,
                max_leave_one_artifact_shift_milli: 100,
                max_claims: 8,
            },
            &records,
        )
        .unwrap()
    }

    fn profile() -> GliomaStageWorkerProfile {
        GliomaStageWorkerProfile {
            worker_id: "synthetic-all-stages".into(),
            capability_version: "1".into(),
            stage_kinds: crate::glioma_engine::GliomaStageKind::ALL.to_vec(),
            modalities: Vec::new(),
            model_systems: Vec::new(),
            output_schemas: crate::glioma_engine::GliomaStageKind::ALL
                .iter()
                .map(|kind| kind.output_schema().to_string())
                .collect(),
            max_autonomy: AutonomyTier::A3,
            local_only: true,
            available: true,
            deterministic: true,
            priority: 1,
        }
    }

    fn request(qualified: bool) -> GliomaEvidenceGatedStageExecutionRequest {
        GliomaEvidenceGatedStageExecutionRequest {
            engine: engine_request(),
            triangulation: triangulation(qualified),
            min_qualified_claims: 1,
            require_global_qualification: true,
            workers: vec![profile()],
            require_deterministic: true,
        }
    }

    #[test]
    fn unresolved_evidence_holds_before_routing_or_worker_invocation() {
        let output =
            execute_glioma_evidence_gated_stage_engine(&request(false), BTreeMap::new()).unwrap();
        assert_eq!(
            output.disposition,
            GliomaEvidenceGatedStageExecutionDisposition::EvidenceHold
        );
        assert!(output.route_plan.is_none());
        assert!(output.execution.is_none());
        assert!(!output.next_action_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn qualified_evidence_runs_through_the_stage_registry() {
        let request = request(true);
        let route = compile_glioma_stage_worker_routes(&GliomaStageWorkerRouteRequest {
            intent: request.engine.intent.clone(),
            workers: request.workers.clone(),
            require_deterministic: true,
            require_all_ready: true,
        })
        .unwrap();
        let mut workers = BTreeMap::new();
        workers.insert(
            "synthetic-all-stages".into(),
            Box::new(super::super::stage_worker_registry::DryRunGliomaStageWorker)
                as Box<dyn GliomaStageExecutor>,
        );
        let output = execute_glioma_evidence_gated_stage_engine(&request, workers).unwrap();
        assert!(matches!(
            output.disposition,
            GliomaEvidenceGatedStageExecutionDisposition::Executed
                | GliomaEvidenceGatedStageExecutionDisposition::EngineBlocked
        ));
        assert!(output.execution_started);
        assert_eq!(
            output.route_plan.as_ref().unwrap().plan_digest,
            route.plan_digest
        );
        output.validate().unwrap();
    }
}
