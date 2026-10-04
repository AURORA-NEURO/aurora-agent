//! Capability-aware routing for institution-local glioma stage workers.
//!
//! The autonomous engine can only be useful in production when it knows which local workers can
//! actually execute the stages selected by the research plan. This module compiles a deterministic
//! route plan from the typed 14-stage workflow and declared worker capabilities, then provides a
//! small executor registry that dispatches each admitted stage to the selected worker. It never
//! opens a connection, moves a payload, or turns a capability declaration into scientific
//! evidence; physical and external effects remain with the caller-owned workers and their gates.

use super::autonomous_engine::{
    execute_glioma_autonomous_research_engine, GliomaAutonomousResearchEngineError,
    GliomaAutonomousResearchEngineRequest, GliomaAutonomousResearchEngineRun,
};
use super::stage_executor_adapter::GliomaStageActionExecutor;
use crate::glioma_engine::{
    compile_glioma_research, GliomaModality, GliomaModelSystem, GliomaResearchIntent, GliomaStage,
    GliomaStageExecutor, GliomaStageFailure, GliomaStageInput, GliomaStageKind, GliomaStageOutput,
    StageReadiness,
};
use bioprism_foundation::{AutonomyTier, TypedResearchArtifact};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// This routing seam is an implementation extension of the P07 research-director feature; it
/// intentionally does not consume a second catalog slot.
pub const ROUTE_FEATURE_ID: &str = "GAF-GLIOMA-P07-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaStageWorkerRoute1@1";
pub const EXECUTION_OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchStageExecution1@1";
const MAX_WORKERS: usize = 128;
const MAX_ROUTES: usize = 32;

/// Institution-local worker capability declaration. Empty modality/model lists mean that the
/// worker is stage-scoped but not restricted to one declared modality or model system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageWorkerProfile {
    pub worker_id: String,
    pub capability_version: String,
    pub stage_kinds: Vec<GliomaStageKind>,
    pub modalities: Vec<GliomaModality>,
    pub model_systems: Vec<GliomaModelSystem>,
    pub output_schemas: Vec<String>,
    pub max_autonomy: AutonomyTier,
    pub local_only: bool,
    pub available: bool,
    pub deterministic: bool,
    pub priority: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaStageWorkerRouteDisposition {
    Selected,
    NotReady,
    MissingCapability,
    Unavailable,
    LocalityBlocked,
    DeterminismBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageWorkerRoute {
    pub stage_id: String,
    pub stage_kind: GliomaStageKind,
    pub worker_id: Option<String>,
    pub disposition: GliomaStageWorkerRouteDisposition,
    pub candidate_worker_order: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageWorkerRouteRequest {
    pub intent: GliomaResearchIntent,
    pub workers: Vec<GliomaStageWorkerProfile>,
    #[serde(default)]
    pub require_deterministic: bool,
    #[serde(default = "default_require_all_ready")]
    pub require_all_ready: bool,
}

fn default_require_all_ready() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStageWorkerRoutePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub plan_digest: ContentHash,
    pub worker_order: Vec<String>,
    pub routes: Vec<GliomaStageWorkerRoute>,
    pub selected_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaStageWorkerRouteError {
    #[error("stage worker route request is invalid: {0}")]
    InvalidRequest(String),
    #[error("stage worker route compilation failed: {0}")]
    Compilation(String),
    #[error("stage worker route output is invalid: {0}")]
    InvalidOutput(String),
    #[error("stage worker route digest failed: {0}")]
    Digest(String),
    #[error("stage worker is unavailable: {0}")]
    MissingWorker(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaStageWorkerExecutionError {
    #[error("stage worker route is invalid: {0}")]
    Route(#[from] GliomaStageWorkerRouteError),
    #[error("autonomous glioma engine failed: {0}")]
    Engine(#[from] GliomaAutonomousResearchEngineError),
    #[error("stage worker execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("stage worker execution digest failed: {0}")]
    Digest(String),
    #[error("stage worker execution output is invalid: {0}")]
    InvalidOutput(String),
}

/// The combined result of route admission and an actual bounded autonomous engine run. Keeping
/// the route plan beside the engine run makes the worker decision auditable and lets a caller
/// resume with the exact same capability snapshot rather than silently re-routing later cycles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchStageExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub route_plan: GliomaStageWorkerRoutePlan,
    pub engine: GliomaAutonomousResearchEngineRun,
    pub digest: ContentHash,
}

fn execution_digest_input(output: &GliomaAutonomousResearchStageExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "route_plan": output.route_plan,
        "engine": output.engine,
    })
}

impl GliomaAutonomousResearchStageExecution {
    pub fn validate(&self) -> Result<(), GliomaStageWorkerExecutionError> {
        if self.feature_id != ROUTE_FEATURE_ID || self.output_schema != EXECUTION_OUTPUT_SCHEMA {
            return Err(GliomaStageWorkerExecutionError::InvalidOutput(
                "execution identity or schema is invalid".into(),
            ));
        }
        self.route_plan.validate()?;
        self.engine.validate()?;
        if self.route_plan.plan_digest != self.engine_digest_plan()? {
            return Err(GliomaStageWorkerExecutionError::InvalidOutput(
                "route plan and engine do not share the same compiled plan digest".into(),
            ));
        }
        let expected = ContentHash::of_value(&execution_digest_input(self))
            .map_err(|error| GliomaStageWorkerExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaStageWorkerExecutionError::InvalidOutput(
                "execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }

    fn engine_digest_plan(&self) -> Result<ContentHash, GliomaStageWorkerExecutionError> {
        self.engine
            .cycles
            .first()
            .map(|cycle| cycle.director.workflow_plan.plan_digest.clone())
            .ok_or_else(|| {
                GliomaStageWorkerExecutionError::InvalidOutput(
                    "engine produced no cycle from which to bind the route plan".into(),
                )
            })
    }
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_sorted<T: Ord + Clone>(values: &[T]) -> Option<Vec<T>> {
    let mut sorted = values.to_vec();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        None
    } else {
        Some(sorted)
    }
}

fn route_digest_input(plan: &GliomaStageWorkerRoutePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "plan_digest": plan.plan_digest,
        "worker_order": plan.worker_order,
        "routes": plan.routes,
        "selected_order": plan.selected_order,
        "blocked_order": plan.blocked_order,
        "uncertainty": plan.uncertainty,
    })
}

impl GliomaStageWorkerProfile {
    fn validate(&self) -> Result<(), GliomaStageWorkerRouteError> {
        if self.worker_id.trim().is_empty()
            || self.capability_version.trim().is_empty()
            || !self.local_only
            || self.stage_kinds.is_empty()
            || unique_sorted(&self.stage_kinds).is_none()
            || unique_sorted(&self.modalities).is_none()
            || unique_sorted(&self.model_systems).is_none()
            || self
                .output_schemas
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(GliomaStageWorkerRouteError::InvalidRequest(format!(
                "worker {} has invalid identity, locality, capability, or canonical fields",
                self.worker_id
            )));
        }
        Ok(())
    }

    fn supports(
        &self,
        stage: &GliomaStage,
        intent: &GliomaResearchIntent,
        deterministic: bool,
    ) -> bool {
        self.available
            && self.local_only
            && self.stage_kinds.contains(&stage.kind)
            && self.output_schemas.contains(&stage.output_schema)
            && self.max_autonomy >= stage.autonomy_tier
            && (!deterministic || self.deterministic)
            && (self.modalities.is_empty()
                || self
                    .modalities
                    .iter()
                    .any(|modality| intent.modalities.contains(modality)))
            && (self.model_systems.is_empty()
                || self
                    .model_systems
                    .iter()
                    .any(|model_system| intent.model_systems.contains(model_system)))
    }
}

impl GliomaStageWorkerRoutePlan {
    pub fn validate(&self) -> Result<(), GliomaStageWorkerRouteError> {
        if self.feature_id != ROUTE_FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.plan_digest.as_str().len() != 64
            || self.routes.is_empty()
            || self.routes.len() > MAX_ROUTES
            || !canonical(&self.worker_order)
            || self.worker_order.windows(2).any(|pair| pair[0] == pair[1])
            || !canonical(&self.selected_order)
            || !canonical(&self.blocked_order)
            || self
                .selected_order
                .iter()
                .any(|stage| self.blocked_order.contains(stage))
            || self
                .routes
                .windows(2)
                .any(|pair| pair[0].stage_id >= pair[1].stage_id)
            || self.routes.iter().any(|route| {
                route.stage_id.trim().is_empty()
                    || route
                        .candidate_worker_order
                        .windows(2)
                        .any(|pair| pair[0] == pair[1])
                    || route.reasons.iter().any(|reason| reason.trim().is_empty())
                    || (route.disposition == GliomaStageWorkerRouteDisposition::Selected
                        && route.worker_id.is_none())
                    || (route.disposition != GliomaStageWorkerRouteDisposition::Selected
                        && route.worker_id.is_some())
            })
            || !canonical(&self.uncertainty)
        {
            return Err(GliomaStageWorkerRouteError::InvalidOutput(
                "route identity, ordering, selected/blocked partition, or capability fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&route_digest_input(self))
            .map_err(|error| GliomaStageWorkerRouteError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaStageWorkerRouteError::InvalidOutput(
                "route plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn route_reason(stage: &GliomaStage) -> (GliomaStageWorkerRouteDisposition, String) {
    match stage.readiness {
        StageReadiness::MissingInput => (
            GliomaStageWorkerRouteDisposition::NotReady,
            "stage-missing-typed-input".into(),
        ),
        StageReadiness::ApprovalRequired => (
            GliomaStageWorkerRouteDisposition::NotReady,
            "stage-requires-approval-before-worker-routing".into(),
        ),
        StageReadiness::Disabled => (
            GliomaStageWorkerRouteDisposition::NotReady,
            "stage-disabled-by-research-intent".into(),
        ),
        StageReadiness::Ready => (
            GliomaStageWorkerRouteDisposition::MissingCapability,
            "no-available-worker-matches-stage-contract".into(),
        ),
    }
}

/// Compile a deterministic worker route for every stage in the typed glioma plan. Ready stages
/// without a matching local worker remain blocked; they are never silently assigned a generic
/// executor or removed from the workflow.
pub fn compile_glioma_stage_worker_routes(
    request: &GliomaStageWorkerRouteRequest,
) -> Result<GliomaStageWorkerRoutePlan, GliomaStageWorkerRouteError> {
    if request.workers.is_empty() || request.workers.len() > MAX_WORKERS {
        return Err(GliomaStageWorkerRouteError::InvalidRequest(
            "a bounded non-empty worker profile set is required".into(),
        ));
    }
    for worker in &request.workers {
        worker.validate()?;
    }
    let mut workers = request.workers.clone();
    workers.sort_by(|left, right| left.worker_id.cmp(&right.worker_id));
    if workers
        .windows(2)
        .any(|pair| pair[0].worker_id == pair[1].worker_id)
    {
        return Err(GliomaStageWorkerRouteError::InvalidRequest(
            "worker identifiers must be unique".into(),
        ));
    }
    let plan = compile_glioma_research(&request.intent)
        .map_err(|error| GliomaStageWorkerRouteError::Compilation(error.to_string()))?;
    let mut routes = Vec::with_capacity(plan.stages.len());
    for stage in &plan.stages {
        let (default_disposition, default_reason) = route_reason(stage);
        if stage.readiness != StageReadiness::Ready {
            routes.push(GliomaStageWorkerRoute {
                stage_id: stage.stage_id.clone(),
                stage_kind: stage.kind,
                worker_id: None,
                disposition: default_disposition,
                candidate_worker_order: Vec::new(),
                reasons: vec![default_reason],
            });
            continue;
        }
        let mut eligible = workers
            .iter()
            .filter(|worker| worker.supports(stage, &request.intent, request.require_deterministic))
            .collect::<Vec<_>>();
        eligible.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| right.deterministic.cmp(&left.deterministic))
                .then_with(|| left.worker_id.cmp(&right.worker_id))
        });
        let candidate_worker_order = eligible
            .iter()
            .map(|worker| worker.worker_id.clone())
            .collect::<Vec<_>>();
        if let Some(worker) = eligible.first() {
            routes.push(GliomaStageWorkerRoute {
                stage_id: stage.stage_id.clone(),
                stage_kind: stage.kind,
                worker_id: Some(worker.worker_id.clone()),
                disposition: GliomaStageWorkerRouteDisposition::Selected,
                candidate_worker_order,
                reasons: vec!["selected-by-priority-deterministic-tie-break".into()],
            });
        } else {
            let reason = if workers.iter().any(|worker| {
                worker.stage_kinds.contains(&stage.kind)
                    && worker.output_schemas.contains(&stage.output_schema)
            }) {
                "declared-workers-fail-availability-locality-autonomy-or-modality-gates"
            } else {
                "no-worker-declares-the-stage-and-output-schema"
            };
            routes.push(GliomaStageWorkerRoute {
                stage_id: stage.stage_id.clone(),
                stage_kind: stage.kind,
                worker_id: None,
                disposition: GliomaStageWorkerRouteDisposition::MissingCapability,
                candidate_worker_order,
                reasons: vec![reason.into()],
            });
        }
    }
    routes.sort_by(|left, right| left.stage_id.cmp(&right.stage_id));
    let selected_order = routes
        .iter()
        .filter(|route| route.disposition == GliomaStageWorkerRouteDisposition::Selected)
        .map(|route| route.stage_id.clone())
        .collect::<Vec<_>>();
    let blocked_order = routes
        .iter()
        .filter(|route| route.disposition != GliomaStageWorkerRouteDisposition::Selected)
        .map(|route| route.stage_id.clone())
        .collect::<Vec<_>>();
    if request.require_all_ready
        && routes.iter().any(|route| {
            route.disposition != GliomaStageWorkerRouteDisposition::Selected
                && plan
                    .stages
                    .iter()
                    .find(|stage| stage.stage_id == route.stage_id)
                    .is_some_and(|stage| stage.readiness == StageReadiness::Ready)
        })
    {
        return Err(GliomaStageWorkerRouteError::Compilation(
            "one or more ready stages have no eligible local worker".into(),
        ));
    }
    let mut output = GliomaStageWorkerRoutePlan {
        feature_id: ROUTE_FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        plan_digest: plan.plan_digest,
        worker_order: workers
            .iter()
            .map(|worker| worker.worker_id.clone())
            .collect(),
        routes,
        selected_order,
        blocked_order,
        uncertainty: vec![
            "route-plan-does-not-authorize-physical-or-external-effects".into(),
            "unavailable-or-missing-workers-remain-blocked".into(),
            "worker-capabilities-are-declarations-not-scientific-evidence".into(),
        ],
        digest: ContentHash::of_bytes(b"unsealed-glioma-stage-worker-route"),
    };
    output.digest = ContentHash::of_value(&route_digest_input(&output))
        .map_err(|error| GliomaStageWorkerRouteError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// A runtime router that implements the existing typed stage-worker seam. The route plan is
/// compiled and validated before construction; a missing worker still fails closed at dispatch.
pub struct GliomaStageExecutorRegistry {
    routes: BTreeMap<GliomaStageKind, String>,
    workers: BTreeMap<String, Box<dyn GliomaStageExecutor>>,
}

impl GliomaStageExecutorRegistry {
    pub fn new(
        plan: &GliomaStageWorkerRoutePlan,
        workers: BTreeMap<String, Box<dyn GliomaStageExecutor>>,
    ) -> Result<Self, GliomaStageWorkerRouteError> {
        plan.validate()?;
        let mut routes = BTreeMap::new();
        for route in &plan.routes {
            if let Some(worker_id) = &route.worker_id {
                if !workers.contains_key(worker_id) {
                    return Err(GliomaStageWorkerRouteError::MissingWorker(
                        worker_id.clone(),
                    ));
                }
                routes.insert(route.stage_kind, worker_id.clone());
            }
        }
        Ok(Self { routes, workers })
    }
}

impl GliomaStageExecutor for GliomaStageExecutorRegistry {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        let Some(worker_id) = self.routes.get(&stage.kind) else {
            return Err(GliomaStageFailure {
                reason: format!("no admitted local worker route for {}", stage.stage_id),
                retryable: false,
            });
        };
        let Some(worker) = self.workers.get_mut(worker_id) else {
            return Err(GliomaStageFailure {
                reason: format!("routed worker {} is unavailable", worker_id),
                retryable: false,
            });
        };
        worker.execute(stage, input)
    }
}

/// Deterministic local rehearsal worker. It emits a schema-correct synthetic artifact for the
/// selected stage and marks the result as simulation-only; it never represents an observation,
/// opens an instrument connection, or moves raw data. Production hosts replace it with their
/// own worker implementations behind the same trait.
#[derive(Debug, Default)]
pub struct DryRunGliomaStageWorker;

impl GliomaStageExecutor for DryRunGliomaStageWorker {
    fn execute(
        &mut self,
        stage: &GliomaStage,
        input: &GliomaStageInput,
    ) -> Result<GliomaStageOutput, GliomaStageFailure> {
        let artifact = TypedResearchArtifact::from_payload(
            format!("dry-run-stage:{}", stage.stage_id),
            stage.output_schema.clone(),
            &serde_json::json!({
                "simulation_only": true,
                "stage_id": stage.stage_id,
                "research_id": input.research_id,
                "study_id": input.study_id,
                "upstream_artifacts": input.upstream_artifacts,
                "replay_identity": input.replay_identity,
                "attempt": input.attempt,
            }),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| GliomaStageFailure {
            reason: format!("dry-run stage artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(GliomaStageOutput {
            artifact,
            disposition: crate::glioma_engine::GliomaStageDisposition::Completed,
            uncertainty: vec!["simulation-only-result".into()],
            negative_evidence: vec!["synthetic-dry-run-not-biological-evidence".into()],
        })
    }
}

/// Execute the real autonomous engine through the admitted stage-worker registry. This is the
/// production host seam: the route plan is bound to the same compiled intent, the existing typed
/// stage/action adapter carries prerequisite artifacts, and every engine cycle remains resumable.
pub fn execute_glioma_autonomous_research_engine_with_stage_workers(
    request: &GliomaAutonomousResearchEngineRequest,
    route_plan: &GliomaStageWorkerRoutePlan,
    workers: BTreeMap<String, Box<dyn GliomaStageExecutor>>,
) -> Result<GliomaAutonomousResearchStageExecution, GliomaStageWorkerExecutionError> {
    route_plan.validate()?;
    let compiled = compile_glioma_research(&request.intent).map_err(|error| {
        GliomaStageWorkerExecutionError::InvalidRequest(format!(
            "cannot compile request intent for route binding: {error}"
        ))
    })?;
    if compiled.plan_digest != route_plan.plan_digest {
        return Err(GliomaStageWorkerExecutionError::InvalidRequest(
            "route plan was compiled from a different research intent".into(),
        ));
    }
    let mut registry = GliomaStageExecutorRegistry::new(route_plan, workers)?;
    let mut adapter = GliomaStageActionExecutor::new(&request.intent, &mut registry)
        .map_err(|error| GliomaStageWorkerExecutionError::InvalidRequest(error.to_string()))?;
    let engine = execute_glioma_autonomous_research_engine(request, &mut adapter)?;
    let mut output = GliomaAutonomousResearchStageExecution {
        feature_id: ROUTE_FEATURE_ID.into(),
        output_schema: EXECUTION_OUTPUT_SCHEMA.into(),
        route_plan: route_plan.clone(),
        engine,
        digest: ContentHash::of_bytes(b"unsealed-glioma-stage-worker-execution"),
    };
    output.digest = ContentHash::of_value(&execution_digest_input(&output))
        .map_err(|error| GliomaStageWorkerExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{AutonomyTier, TypedResearchArtifact, PRECLINICAL_BOUNDARY};
    use bioprism_ids::ContentHash;
    use bioprism_onco::OutputUse;
    use serde_json::json;
    use std::collections::BTreeSet;

    use crate::glioma_engine::GliomaStageDisposition;

    fn intent() -> GliomaResearchIntent {
        let hash = ContentHash::of_bytes(b"stage-worker-registry-input");
        GliomaResearchIntent {
            research_id: "worker-registry-research".into(),
            study_id: "worker-registry-study".into(),
            objective: "map reproducible invasion mechanisms in a preclinical glioma organoid"
                .into(),
            output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
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
            input_artifacts: vec![crate::glioma_engine::LocalArtifactRef {
                artifact_id: "input".into(),
                content_hash: hash.clone(),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 512,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn profile(worker_id: &str, priority: u16) -> GliomaStageWorkerProfile {
        GliomaStageWorkerProfile {
            worker_id: worker_id.into(),
            capability_version: "1".into(),
            stage_kinds: GliomaStageKind::ALL.to_vec(),
            modalities: Vec::new(),
            model_systems: Vec::new(),
            output_schemas: GliomaStageKind::ALL
                .iter()
                .map(|kind| kind.output_schema().to_string())
                .collect(),
            max_autonomy: AutonomyTier::A3,
            local_only: true,
            available: true,
            deterministic: true,
            priority,
        }
    }

    #[test]
    fn routing_is_deterministic_and_priority_ordered() {
        let request = GliomaStageWorkerRouteRequest {
            intent: intent(),
            workers: vec![profile("worker-b", 10), profile("worker-a", 10)],
            require_deterministic: true,
            require_all_ready: false,
        };
        let first = compile_glioma_stage_worker_routes(&request).unwrap();
        let second = compile_glioma_stage_worker_routes(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.worker_order, vec!["worker-a", "worker-b"]);
        assert!(first
            .routes
            .iter()
            .filter(|route| route.disposition == GliomaStageWorkerRouteDisposition::Selected)
            .all(|route| route.worker_id.as_deref() == Some("worker-a")));
        first.validate().unwrap();
    }

    #[test]
    fn ready_stage_without_capability_is_explicitly_blocked() {
        let mut worker = profile("evidence-only", 1);
        worker.stage_kinds = vec![GliomaStageKind::EvidenceCompilation];
        worker.output_schemas = vec![GliomaStageKind::EvidenceCompilation.output_schema().into()];
        let request = GliomaStageWorkerRouteRequest {
            intent: intent(),
            workers: vec![worker],
            require_deterministic: true,
            require_all_ready: false,
        };
        let plan = compile_glioma_stage_worker_routes(&request).unwrap();
        assert!(plan.routes.iter().any(|route| {
            route.stage_kind == GliomaStageKind::MechanismExploration
                && route.disposition == GliomaStageWorkerRouteDisposition::MissingCapability
        }));
        assert!(plan.blocked_order.contains(&"mechanism-exploration".into()));
    }

    #[derive(Default)]
    struct RecordingWorker {
        calls: Vec<String>,
    }

    impl GliomaStageExecutor for RecordingWorker {
        fn execute(
            &mut self,
            stage: &GliomaStage,
            _input: &GliomaStageInput,
        ) -> Result<GliomaStageOutput, GliomaStageFailure> {
            self.calls.push(stage.stage_id.clone());
            let artifact = TypedResearchArtifact::from_payload(
                format!("route-artifact:{}", stage.stage_id),
                stage.output_schema.clone(),
                &json!({"stage": stage.stage_id}),
                Vec::new(),
                Vec::new(),
            )
            .map_err(|error| GliomaStageFailure {
                reason: error.to_string(),
                retryable: false,
            })?;
            Ok(GliomaStageOutput {
                artifact,
                disposition: GliomaStageDisposition::Negative,
                uncertainty: vec!["registry-test-output".into()],
                negative_evidence: vec!["registry-test-null".into()],
            })
        }
    }

    #[test]
    fn registry_dispatches_through_the_selected_worker() {
        let request = GliomaStageWorkerRouteRequest {
            intent: intent(),
            workers: vec![profile("worker-a", 10)],
            require_deterministic: true,
            require_all_ready: false,
        };
        let plan = compile_glioma_stage_worker_routes(&request).unwrap();
        let mut workers: BTreeMap<String, Box<dyn GliomaStageExecutor>> = BTreeMap::new();
        workers.insert("worker-a".into(), Box::new(RecordingWorker::default()));
        let mut registry = GliomaStageExecutorRegistry::new(&plan, workers).unwrap();
        let stage = compile_glioma_research(&request.intent)
            .unwrap()
            .stages
            .into_iter()
            .find(|stage| stage.readiness == StageReadiness::Ready)
            .unwrap();
        let result = registry
            .execute(
                &stage,
                &GliomaStageInput {
                    research_id: request.intent.research_id.clone(),
                    study_id: request.intent.study_id.clone(),
                    stage_id: stage.stage_id.clone(),
                    kind: stage.kind,
                    upstream_artifacts: Vec::new(),
                    source_artifacts: request.intent.input_artifacts.clone(),
                    replay_identity: request.intent.replay_identity.clone(),
                    attempt: 1,
                },
            )
            .unwrap();
        assert_eq!(result.disposition, GliomaStageDisposition::Negative);
    }

    #[test]
    fn route_registry_drives_the_autonomous_engine_and_preserves_checkpoints() {
        let route_request = GliomaStageWorkerRouteRequest {
            intent: intent(),
            workers: vec![profile("dry-run-worker", 10)],
            require_deterministic: true,
            require_all_ready: false,
        };
        let route_plan = compile_glioma_stage_worker_routes(&route_request).unwrap();
        let engine_request = GliomaAutonomousResearchEngineRequest {
            mission_id: "stage-worker-engine-integration".into(),
            intent: route_request.intent.clone(),
            focus: super::super::director::GliomaDirectorFocus::Adaptive,
            completed_checkpoints: Vec::new(),
            budget_units: 64,
            max_actions: 2,
            max_cycles: 2,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: crate::glioma_engine::GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
            outcome_summaries: BTreeMap::new(),
            adaptive_policy:
                super::super::autonomous_engine::GliomaAdaptiveReplanningPolicy::default(),
        };
        let mut workers: BTreeMap<String, Box<dyn GliomaStageExecutor>> = BTreeMap::new();
        workers.insert("dry-run-worker".into(), Box::new(DryRunGliomaStageWorker));
        let execution = execute_glioma_autonomous_research_engine_with_stage_workers(
            &engine_request,
            &route_plan,
            workers,
        )
        .unwrap();
        assert_eq!(execution.route_plan.plan_digest, route_plan.plan_digest);
        assert!(!execution.engine.cycles.is_empty());
        assert!(!execution.engine.completed_checkpoints.is_empty());
        assert!(execution
            .engine
            .negative_evidence
            .iter()
            .any(|item| item.contains("synthetic-dry-run-not-biological-evidence")));
        execution.validate().unwrap();
    }
}
