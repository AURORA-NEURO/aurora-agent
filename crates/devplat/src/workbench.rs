//! Authoring, notebook, dashboard, and CI contracts for an agent-facing workbench.
//!
//! This is the executable core of the developer-platform surface: it makes an authoring session
//! a digestable object, makes notebook cells dependency-aware, makes stale evidence visible, and
//! turns a capability inventory into a bounded dashboard query. It also generates a deterministic
//! GitHub Actions plan from explicit checks. The plan is an artifact, not an executor: this crate
//! never contacts GitHub, runs a command, opens a notebook kernel, or writes a file.
//!
//! The model is deliberately domain-neutral. An `ArtifactCard` can describe an oncology result,
//! a BioIR world, a runtime trace, a benchmark pack, or an operational release record. Domain and
//! capability strings remain caller-owned, while state, evidence posture, digest binding, cell
//! dependencies, and release readiness are enforced here. That gives every domain the same useful
//! authoring and review ergonomics without inventing a second domain ontology.
//!
//! The workbench has four layers:
//!
//! 1. [`StudioSession`] — immutable-ish authoring state with artifacts, notebook cells, and a
//!    logical change ledger.
//! 2. [`audit_session`] — structural validation, deterministic cell ordering, stale-input
//!    detection, and a conservative release posture.
//! 3. [`query_dashboard`] — bounded filtering that keeps evidence holes and stale cells visible.
//! 4. [`plan_ci`] — safe YAML generation from explicit checks, with a digest and an explicit
//!    not-executed posture.
//!
//! The generated CI plan can be handed to a consumer-repository adapter, committed by a human, or
//! reviewed in an authoring UI. It is never mistaken for a green run merely because it rendered.
//!
//! Every type inside a [`WorkbenchReport`] — the audit and its findings, the dashboard projection
//! and its rows, the CI plan — refuses a field it does not declare, because [`verify_workbench`]
//! recomputes the report's digest by re-serialising the *parsed* report: a key the reader dropped
//! would be outside the seal by construction, the claimed digest would still agree, and a report
//! carrying content nobody hashed would read as verified. The request types stay open, because a
//! caller sending a field a newer schema added is forward compatibility rather than tampering, and
//! no digest covers them.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;

/// Schema version for all workbench objects.
pub const WORKBENCH_SCHEMA_VERSION: &str = "bioprism-devplat-workbench/0.1";
/// Schema version for retained workbench verification reports.
pub const WORKBENCH_VERIFY_SCHEMA_VERSION: &str = "bioprism-devplat-workbench-verify/0.1";
const MAX_ARTIFACTS: usize = 2_048;
const MAX_CELLS: usize = 4_096;
const MAX_CHANGES: usize = 4_096;
const MAX_TAGS: usize = 64;

fn default_true() -> bool {
    true
}

fn default_dashboard_limit() -> usize {
    100
}

fn default_max_cells() -> usize {
    MAX_CELLS
}

/// The evidence posture of a workbench artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePosture {
    Missing,
    Declared,
    Observed,
    Reproduced,
    Blocked,
}

impl EvidencePosture {
    pub fn is_measured(self) -> bool {
        matches!(
            self,
            EvidencePosture::Observed | EvidencePosture::Reproduced
        )
    }

    pub fn is_hole(self) -> bool {
        matches!(self, EvidencePosture::Missing | EvidencePosture::Blocked)
    }
}

/// Lifecycle state of an authored artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactState {
    Draft,
    Validated,
    Released,
    Withdrawn,
}

/// Kind of notebook cell. The kind changes review expectations, not the source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellKind {
    Markdown,
    Code,
    Query,
    Decision,
    Review,
}

/// Operation recorded in the session's logical change ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Create,
    Update,
    Review,
    Release,
    Withdraw,
}

/// One artifact that can appear in a dashboard or be consumed by a notebook cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactCard {
    pub id: String,
    pub title: String,
    pub path: String,
    pub domain: String,
    pub capability: String,
    pub state: ArtifactState,
    pub evidence: EvidencePosture,
    pub digest: Option<String>,
    pub score: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ArtifactCard {
    fn validate(&self) -> Result<(), WorkbenchError> {
        for (field, value) in [
            ("artifact.id", self.id.as_str()),
            ("artifact.title", self.title.as_str()),
            ("artifact.path", self.path.as_str()),
            ("artifact.domain", self.domain.as_str()),
            ("artifact.capability", self.capability.as_str()),
        ] {
            require_text(field, value)?;
            check_single_line(field, value)?;
        }
        if let Some(digest) = &self.digest {
            validate_digest("artifact.digest", digest)?;
        }
        if let Some(score) = self.score {
            finite("artifact.score", score)?;
        }
        if self.tags.len() > MAX_TAGS {
            return Err(WorkbenchError::TooMany {
                kind: "artifact tags",
                count: self.tags.len(),
                maximum: MAX_TAGS,
            });
        }
        let mut tags = BTreeSet::new();
        for tag in &self.tags {
            require_text("artifact.tag", tag)?;
            check_single_line("artifact.tag", tag)?;
            if !tags.insert(tag) {
                return Err(WorkbenchError::Duplicate {
                    kind: "artifact tag",
                    id: tag.clone(),
                });
            }
        }
        if self.state == ArtifactState::Released
            && (self.digest.is_none() || !self.evidence.is_measured())
        {
            return Err(WorkbenchError::ReleaseWithoutEvidence {
                artifact: self.id.clone(),
            });
        }
        Ok(())
    }
}

/// A digest binding between a notebook cell and the artifact version it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellInput {
    pub artifact_id: String,
    pub digest: String,
}

impl CellInput {
    fn validate(&self) -> Result<(), WorkbenchError> {
        require_text("cell input artifact_id", &self.artifact_id)?;
        check_single_line("cell input artifact_id", &self.artifact_id)?;
        validate_digest("cell input digest", &self.digest)
    }
}

/// One notebook cell with explicit dependencies and output binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudioCell {
    pub id: String,
    pub kind: CellKind,
    pub source: String,
    #[serde(default)]
    pub inputs: Vec<CellInput>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub executed: bool,
    pub output_digest: Option<String>,
}

impl StudioCell {
    fn validate(&self) -> Result<(), WorkbenchError> {
        require_text("cell.id", &self.id)?;
        check_single_line("cell.id", &self.id)?;
        require_text("cell.source", &self.source)?;
        if self.source.contains('\0') {
            return Err(WorkbenchError::ControlCharacter {
                field: "cell.source",
            });
        }
        for input in &self.inputs {
            input.validate()?;
        }
        if let Some(digest) = &self.output_digest {
            validate_digest("cell output_digest", digest)?;
        }
        if self.executed && !matches!(self.kind, CellKind::Markdown) && self.output_digest.is_none()
        {
            return Err(WorkbenchError::ExecutedCellWithoutOutput {
                cell: self.id.clone(),
            });
        }
        Ok(())
    }
}

/// A logical, monotonic change record. It is not a replacement for Git history; it is the
/// authoring-level record needed to explain how a notebook or studio session arrived at a digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudioChange {
    pub id: String,
    pub artifact_id: String,
    pub kind: ChangeKind,
    pub actor: String,
    pub logical_time: u64,
    pub input_digest: Option<String>,
    pub output_digest: Option<String>,
    pub reason: String,
}

impl StudioChange {
    fn validate(&self) -> Result<(), WorkbenchError> {
        require_text("change.id", &self.id)?;
        check_single_line("change.id", &self.id)?;
        require_text("change.artifact_id", &self.artifact_id)?;
        check_single_line("change.artifact_id", &self.artifact_id)?;
        require_text("change.actor", &self.actor)?;
        check_single_line("change.actor", &self.actor)?;
        require_text("change.reason", &self.reason)?;
        if let Some(digest) = &self.input_digest {
            validate_digest("change input_digest", digest)?;
        }
        if let Some(digest) = &self.output_digest {
            validate_digest("change output_digest", digest)?;
        }
        if matches!(self.kind, ChangeKind::Release | ChangeKind::Update)
            && self.output_digest.is_none()
        {
            return Err(WorkbenchError::ChangeWithoutOutput {
                change: self.id.clone(),
            });
        }
        Ok(())
    }
}

/// Session-level policy. Defaults are conservative and explicit in serialized output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookPolicy {
    #[serde(default = "default_true")]
    pub require_executed_for_release: bool,
    #[serde(default = "default_max_cells")]
    pub max_cells: usize,
    #[serde(default)]
    pub allow_network: bool,
}

impl Default for NotebookPolicy {
    fn default() -> Self {
        NotebookPolicy {
            require_executed_for_release: true,
            max_cells: MAX_CELLS,
            allow_network: false,
        }
    }
}

/// Complete authoring session consumed by the workbench.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudioSession {
    pub session_id: String,
    pub owner: String,
    pub goal: String,
    pub environment_digest: Option<String>,
    pub artifacts: Vec<ArtifactCard>,
    pub cells: Vec<StudioCell>,
    pub changes: Vec<StudioChange>,
    #[serde(default)]
    pub policy: NotebookPolicy,
}

impl StudioSession {
    /// Validate references, digest syntax, and dependency topology.
    pub fn validate(&self) -> Result<(), WorkbenchError> {
        require_text("session_id", &self.session_id)?;
        check_single_line("session_id", &self.session_id)?;
        require_text("owner", &self.owner)?;
        check_single_line("owner", &self.owner)?;
        require_text("goal", &self.goal)?;
        if let Some(digest) = &self.environment_digest {
            validate_digest("environment_digest", digest)?;
        }
        if self.artifacts.len() > MAX_ARTIFACTS {
            return Err(WorkbenchError::TooMany {
                kind: "artifacts",
                count: self.artifacts.len(),
                maximum: MAX_ARTIFACTS,
            });
        }
        if self.cells.len() > MAX_CELLS || self.cells.len() > self.policy.max_cells {
            return Err(WorkbenchError::TooMany {
                kind: "notebook cells",
                count: self.cells.len(),
                maximum: self.policy.max_cells.min(MAX_CELLS),
            });
        }
        if self.changes.len() > MAX_CHANGES {
            return Err(WorkbenchError::TooMany {
                kind: "changes",
                count: self.changes.len(),
                maximum: MAX_CHANGES,
            });
        }
        if !(1..=MAX_CELLS).contains(&self.policy.max_cells) {
            return Err(WorkbenchError::InvalidLimit {
                field: "policy.max_cells",
                value: self.policy.max_cells,
            });
        }

        let mut artifact_ids = BTreeSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !artifact_ids.insert(identity_key(&artifact.id)) {
                return Err(WorkbenchError::Duplicate {
                    kind: "artifact",
                    id: artifact.id.clone(),
                });
            }
        }

        let mut cell_ids = BTreeSet::new();
        for cell in &self.cells {
            cell.validate()?;
            if !cell_ids.insert(identity_key(&cell.id)) {
                return Err(WorkbenchError::Duplicate {
                    kind: "cell",
                    id: cell.id.clone(),
                });
            }
            let mut inputs = BTreeSet::new();
            for input in &cell.inputs {
                if !self
                    .artifacts
                    .iter()
                    .any(|artifact| artifact.id == input.artifact_id)
                {
                    return Err(WorkbenchError::UnknownReference {
                        kind: "artifact",
                        id: input.artifact_id.clone(),
                        subject: cell.id.clone(),
                    });
                }
                if !inputs.insert(input.artifact_id.clone()) {
                    return Err(WorkbenchError::Duplicate {
                        kind: "cell input",
                        id: input.artifact_id.clone(),
                    });
                }
            }
        }
        for cell in &self.cells {
            let mut dependencies = BTreeSet::new();
            for dependency in &cell.depends_on {
                require_text("cell dependency", dependency)?;
                check_single_line("cell dependency", dependency)?;
                if dependency == &cell.id {
                    return Err(WorkbenchError::SelfDependency {
                        cell: cell.id.clone(),
                    });
                }
                if !self.cells.iter().any(|cell| cell.id == *dependency) {
                    return Err(WorkbenchError::UnknownReference {
                        kind: "cell",
                        id: dependency.clone(),
                        subject: cell.id.clone(),
                    });
                }
                if !dependencies.insert(dependency.clone()) {
                    return Err(WorkbenchError::Duplicate {
                        kind: "cell dependency",
                        id: dependency.clone(),
                    });
                }
            }
        }

        let mut change_ids = BTreeSet::new();
        let mut last_time = None;
        for change in &self.changes {
            change.validate()?;
            if !self
                .artifacts
                .iter()
                .any(|artifact| artifact.id == change.artifact_id)
            {
                return Err(WorkbenchError::UnknownReference {
                    kind: "artifact",
                    id: change.artifact_id.clone(),
                    subject: change.id.clone(),
                });
            }
            if !change_ids.insert(identity_key(&change.id)) {
                return Err(WorkbenchError::Duplicate {
                    kind: "change",
                    id: change.id.clone(),
                });
            }
            if let Some(previous) = last_time {
                if change.logical_time < previous {
                    return Err(WorkbenchError::NonMonotonicChange {
                        previous,
                        current: change.logical_time,
                    });
                }
            }
            last_time = Some(change.logical_time);
        }
        let _ = self.ordered_cells()?;
        Ok(())
    }

    /// Return a deterministic topological order, breaking ties by cell id.
    pub fn ordered_cells(&self) -> Result<Vec<String>, WorkbenchError> {
        let ids = self
            .cells
            .iter()
            .map(|cell| cell.id.clone())
            .collect::<BTreeSet<_>>();
        let mut remaining = self
            .cells
            .iter()
            .map(|cell| {
                (
                    cell.id.clone(),
                    cell.depends_on.iter().cloned().collect::<BTreeSet<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let mut order = Vec::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let Some(index) = remaining
                .iter()
                .enumerate()
                .filter(|(_, (_, dependencies))| dependencies.is_empty())
                .map(|(index, _)| index)
                .min_by_key(|index| remaining[*index].0.clone())
            else {
                return Err(WorkbenchError::DependencyCycle {
                    cells: remaining.iter().map(|(id, _)| id.clone()).collect(),
                });
            };
            let (id, _) = remaining.remove(index);
            order.push(id.clone());
            for (_, dependencies) in &mut remaining {
                dependencies.remove(&id);
            }
        }
        if order.iter().any(|id| !ids.contains(id)) {
            return Err(WorkbenchError::DependencyCycle { cells: order });
        }
        Ok(order)
    }

    fn artifact(&self, id: &str) -> Option<&ArtifactCard> {
        self.artifacts.iter().find(|artifact| artifact.id == id)
    }

    fn cell_is_stale(&self, cell: &StudioCell) -> bool {
        cell.inputs.iter().any(|input| {
            let Some(artifact) = self.artifact(&input.artifact_id) else {
                return true;
            };
            artifact.digest.as_deref() != Some(input.digest.as_str())
        })
    }
}

/// One conservative finding produced by a structural audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchFinding {
    pub code: String,
    pub severity: String,
    pub subject: String,
    pub detail: String,
}

/// Structural session audit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionAudit {
    pub valid: bool,
    pub session_digest: String,
    pub artifact_count: usize,
    pub cell_count: usize,
    pub change_count: usize,
    pub executed_cell_count: usize,
    pub stale_cells: Vec<String>,
    pub ordered_cells: Vec<String>,
    pub release_ready: bool,
    pub release_blockers: Vec<String>,
    pub findings: Vec<WorkbenchFinding>,
}

/// Query over domain-neutral capability cards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardQuery {
    pub search: Option<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub states: Vec<ArtifactState>,
    #[serde(default)]
    pub evidence: Vec<EvidencePosture>,
    pub minimum_score: Option<f64>,
    #[serde(default = "default_true")]
    pub include_holes: bool,
    #[serde(default = "default_dashboard_limit")]
    pub limit: usize,
}

impl Default for DashboardQuery {
    fn default() -> Self {
        DashboardQuery {
            search: None,
            domains: Vec::new(),
            states: Vec::new(),
            evidence: Vec::new(),
            minimum_score: None,
            include_holes: true,
            limit: 100,
        }
    }
}

impl DashboardQuery {
    fn validate(&self) -> Result<(), WorkbenchError> {
        if self.limit == 0 || self.limit > 1_000 {
            return Err(WorkbenchError::InvalidLimit {
                field: "dashboard.limit",
                value: self.limit,
            });
        }
        if let Some(search) = &self.search {
            if search.contains('\0') {
                return Err(WorkbenchError::ControlCharacter {
                    field: "dashboard.search",
                });
            }
        }
        for domain in &self.domains {
            require_text("dashboard domain", domain)?;
        }
        if let Some(score) = self.minimum_score {
            finite("dashboard.minimum_score", score)?;
        }
        Ok(())
    }
}

/// A row returned by a dashboard query. Holes have no manufactured score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardRow {
    pub artifact: String,
    pub title: String,
    pub domain: String,
    pub capability: String,
    pub state: ArtifactState,
    pub evidence: EvidencePosture,
    pub score: Option<f64>,
    pub stale: bool,
    pub posture: String,
    pub tags: Vec<String>,
}

/// Bounded dashboard result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardReport {
    pub query: DashboardQuery,
    pub matched: usize,
    pub returned: usize,
    pub omitted: usize,
    pub holes: usize,
    pub stale: usize,
    pub rows: Vec<DashboardRow>,
}

/// One explicit CI check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiCheck {
    pub name: String,
    pub run: String,
    pub working_directory: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
}

impl CiCheck {
    fn validate(&self) -> Result<(), WorkbenchError> {
        require_text("ci check name", &self.name)?;
        require_text("ci check run", &self.run)?;
        check_single_line("ci check name", &self.name)?;
        check_single_line("ci check run", &self.run)?;
        if let Some(directory) = &self.working_directory {
            require_text("ci working_directory", directory)?;
            check_single_line("ci working_directory", directory)?;
            let windows_drive = directory.as_bytes().get(1) == Some(&b':');
            if directory.starts_with('/')
                || directory.starts_with('\\')
                || windows_drive
                || directory.split(['/', '\\']).any(|part| part == "..")
            {
                return Err(WorkbenchError::UnsafePath {
                    path: directory.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Inputs for deterministic GitHub Actions YAML generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiRequest {
    pub workflow: String,
    pub triggers: Vec<String>,
    pub rust_toolchain: String,
    pub checks: Vec<CiCheck>,
    #[serde(default)]
    pub offline: bool,
}

impl CiRequest {
    fn validate(&self) -> Result<(), WorkbenchError> {
        require_text("ci workflow", &self.workflow)?;
        require_text("ci rust_toolchain", &self.rust_toolchain)?;
        check_single_line("ci workflow", &self.workflow)?;
        check_single_line("ci rust_toolchain", &self.rust_toolchain)?;
        if self.triggers.is_empty() {
            return Err(WorkbenchError::NoCiTriggers);
        }
        let mut triggers = BTreeSet::new();
        for trigger in &self.triggers {
            require_text("ci trigger", trigger)?;
            if !matches!(
                trigger.as_str(),
                "push" | "pull_request" | "workflow_dispatch"
            ) {
                return Err(WorkbenchError::UnknownCiTrigger {
                    trigger: trigger.clone(),
                });
            }
            if !triggers.insert(trigger) {
                return Err(WorkbenchError::Duplicate {
                    kind: "ci trigger",
                    id: trigger.clone(),
                });
            }
        }
        if self.checks.is_empty() || self.checks.len() > 64 {
            return Err(WorkbenchError::InvalidCiCheckCount(self.checks.len()));
        }
        let mut names = BTreeSet::new();
        for check in &self.checks {
            check.validate()?;
            if !names.insert(identity_key(&check.name)) {
                return Err(WorkbenchError::Duplicate {
                    kind: "ci check",
                    id: check.name.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Generated CI artifact and its non-execution posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiPlan {
    pub workflow: String,
    pub workflow_yaml: String,
    pub digest: String,
    pub check_count: usize,
    pub required_check_count: usize,
    pub execution: String,
    pub network_access: String,
    pub limitations: Vec<String>,
}

/// One request may ask for a session audit, a dashboard projection, and a CI plan together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchRequest {
    pub session: StudioSession,
    pub dashboard: Option<DashboardQuery>,
    pub ci: Option<CiRequest>,
}

/// Full composed workbench response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchReport {
    pub schema_version: String,
    pub audit: SessionAudit,
    pub dashboard: Option<DashboardReport>,
    pub ci: Option<CiPlan>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

/// Policy controlling how much of a retained workbench report must be replayed.
///
/// Session audits and dashboard projections can always be recomputed from the retained session.
/// CI plans need the original caller-owned [`CiRequest`] to be replayed, so the policy keeps that
/// distinction explicit instead of treating a retained YAML string as executable evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkbenchVerificationPolicy {
    #[serde(default)]
    pub require_dashboard: bool,
    #[serde(default)]
    pub require_ci: bool,
    #[serde(default)]
    pub require_ci_replay: bool,
}

/// Input for verifying a retained authoring/notebook workbench report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchVerificationRequest {
    pub session: StudioSession,
    pub report: WorkbenchReport,
    #[serde(default)]
    pub expected_report_digest: Option<String>,
    #[serde(default)]
    pub ci_replay: Option<CiRequest>,
    #[serde(default)]
    pub policy: WorkbenchVerificationPolicy,
}

/// One digest or structural mismatch retained for repair and review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchMismatch {
    pub code: String,
    pub path: String,
    pub expected: Option<Value>,
    pub observed: Option<Value>,
    pub detail: String,
}

/// Digest-bound verification of a retained workbench report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkbenchVerificationReport {
    pub schema_version: String,
    pub workflow: String,
    pub valid: bool,
    pub status: String,
    pub retained_report_digest: String,
    pub expected_report_digest: Option<String>,
    pub report_digest_matched: Option<bool>,
    pub retained_audit_digest: String,
    pub observed_audit_digest: String,
    pub dashboard_present: bool,
    pub dashboard_verified: bool,
    pub ci_present: bool,
    pub ci_replay_supplied: bool,
    pub ci_verified: bool,
    pub mismatches: Vec<WorkbenchMismatch>,
    pub execution: String,
    pub network_access: String,
    pub verification_digest: String,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

/// Errors returned by workbench validation and projections.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkbenchError {
    #[error("{field} must be a non-empty string")]
    EmptyField { field: &'static str },
    #[error("{field} contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} is not a finite number: {value}")]
    NonFinite { field: &'static str, value: String },
    #[error("{field} is not a valid content digest: {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("{kind} contains too many entries: {count}; maximum is {maximum}")]
    TooMany {
        kind: &'static str,
        count: usize,
        maximum: usize,
    },
    #[error("duplicate {kind} `{id}`")]
    Duplicate { kind: &'static str, id: String },
    #[error("unknown {kind} reference `{id}` from `{subject}`")]
    UnknownReference {
        kind: &'static str,
        id: String,
        subject: String,
    },
    #[error("cell `{cell}` depends on itself")]
    SelfDependency { cell: String },
    #[error("notebook dependency cycle contains {cells:?}")]
    DependencyCycle { cells: Vec<String> },
    #[error("logical change time moves from {previous} back to {current}")]
    NonMonotonicChange { previous: u64, current: u64 },
    #[error("released artifact `{artifact}` must carry a digest and measured/reproduced evidence")]
    ReleaseWithoutEvidence { artifact: String },
    #[error("executed cell `{cell}` has no output digest")]
    ExecutedCellWithoutOutput { cell: String },
    #[error("change `{change}` requires an output digest")]
    ChangeWithoutOutput { change: String },
    #[error("{field} must be between 1 and its safety ceiling, got {value}")]
    InvalidLimit { field: &'static str, value: usize },
    #[error("CI requires at least one trigger")]
    NoCiTriggers,
    #[error("unsupported CI trigger `{trigger}`")]
    UnknownCiTrigger { trigger: String },
    #[error("CI check count {0} is outside 1..=64")]
    InvalidCiCheckCount(usize),
    #[error("CI path `{path}` may not traverse a parent directory")]
    UnsafePath { path: String },
    #[error("cannot canonicalise workbench object: {0}")]
    Canonicalisation(String),
}

fn require_text(field: &'static str, value: &str) -> Result<(), WorkbenchError> {
    if value.trim().is_empty() {
        return Err(WorkbenchError::EmptyField { field });
    }
    Ok(())
}

fn finite(field: &'static str, value: f64) -> Result<(), WorkbenchError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(WorkbenchError::NonFinite {
            field,
            value: value.to_string(),
        })
    }
}

fn validate_digest(field: &'static str, value: &str) -> Result<(), WorkbenchError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WorkbenchError::InvalidDigest {
            field,
            value: value.to_string(),
        });
    }
    ContentHash::parse(value.to_string())
        .map(|_| ())
        .map_err(|_| WorkbenchError::InvalidDigest {
            field,
            value: value.to_string(),
        })
}

fn check_single_line(field: &'static str, value: &str) -> Result<(), WorkbenchError> {
    if value.chars().any(char::is_control) {
        return Err(WorkbenchError::ControlCharacter { field });
    }
    if value != value.trim() {
        return Err(WorkbenchError::EmptyField { field });
    }
    Ok(())
}

fn identity_key(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn session_digest(session: &StudioSession) -> Result<String, WorkbenchError> {
    let value = serde_json::to_value(session)
        .map_err(|error| WorkbenchError::Canonicalisation(error.to_string()))?;
    ContentHash::of_value(&value)
        .map(|hash| hash.to_string())
        .map_err(|error| WorkbenchError::Canonicalisation(error.to_string()))
}

fn value_digest<T: Serialize>(value: &T) -> Result<String, WorkbenchError> {
    let value = serde_json::to_value(value)
        .map_err(|error| WorkbenchError::Canonicalisation(error.to_string()))?;
    ContentHash::of_value(&value)
        .map(|hash| hash.to_string())
        .map_err(|error| WorkbenchError::Canonicalisation(error.to_string()))
}

/// Audit one authoring session and derive its release posture.
pub fn audit_session(session: &StudioSession) -> Result<SessionAudit, WorkbenchError> {
    session.validate()?;
    let ordered_cells = session.ordered_cells()?;
    let stale_cells = session
        .cells
        .iter()
        .filter(|cell| session.cell_is_stale(cell))
        .map(|cell| cell.id.clone())
        .collect::<Vec<_>>();
    let executed_cell_count = session.cells.iter().filter(|cell| cell.executed).count();
    let mut findings = Vec::new();
    if session.artifacts.is_empty() {
        findings.push(WorkbenchFinding {
            code: "no_artifacts".into(),
            severity: "warning".into(),
            subject: session.session_id.clone(),
            detail: "the session has no authored artifacts yet".into(),
        });
    }
    for cell in &stale_cells {
        findings.push(WorkbenchFinding {
            code: "stale_cell".into(),
            severity: "blocking".into(),
            subject: cell.clone(),
            detail: "a notebook input digest no longer matches its artifact".into(),
        });
    }
    if session.policy.require_executed_for_release {
        for artifact in session
            .artifacts
            .iter()
            .filter(|artifact| artifact.state == ArtifactState::Released)
        {
            let referenced = session.cells.iter().any(|cell| {
                cell.executed
                    && cell
                        .inputs
                        .iter()
                        .any(|input| input.artifact_id == artifact.id)
            });
            if !referenced {
                findings.push(WorkbenchFinding {
                    code: "released_artifact_without_notebook_witness".into(),
                    severity: "blocking".into(),
                    subject: artifact.id.clone(),
                    detail: "release policy requires an executed notebook witness for the released artifact".into(),
                });
            }
        }
    }
    let release_blockers = findings
        .iter()
        .filter(|finding| finding.severity == "blocking")
        .map(|finding| format!("{}: {}", finding.subject, finding.detail))
        .collect::<Vec<_>>();
    let has_release = session
        .artifacts
        .iter()
        .any(|artifact| artifact.state == ArtifactState::Released);
    Ok(SessionAudit {
        valid: true,
        session_digest: session_digest(session)?,
        artifact_count: session.artifacts.len(),
        cell_count: session.cells.len(),
        change_count: session.changes.len(),
        executed_cell_count,
        stale_cells,
        ordered_cells,
        release_ready: has_release && release_blockers.is_empty(),
        release_blockers,
        findings,
    })
}

fn lower(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn dashboard_posture(artifact: &ArtifactCard, stale: bool) -> &'static str {
    if stale || artifact.evidence.is_hole() {
        "blocked"
    } else if artifact.state == ArtifactState::Released && artifact.evidence.is_measured() {
        "ready"
    } else if artifact.evidence == EvidencePosture::Declared {
        "declared"
    } else {
        "draft"
    }
}

/// Query a session's artifact cards without converting holes to scores or hiding stale inputs.
pub fn query_dashboard(
    session: &StudioSession,
    query: &DashboardQuery,
) -> Result<DashboardReport, WorkbenchError> {
    session.validate()?;
    query.validate()?;
    let search = query.search.as_ref().map(|value| lower(value));
    let mut matched = Vec::new();
    for artifact in &session.artifacts {
        if !query.domains.is_empty()
            && !query
                .domains
                .iter()
                .any(|domain| domain == &artifact.domain)
        {
            continue;
        }
        if !query.states.is_empty() && !query.states.contains(&artifact.state) {
            continue;
        }
        if !query.evidence.is_empty() && !query.evidence.contains(&artifact.evidence) {
            continue;
        }
        if !query.include_holes && artifact.evidence.is_hole() {
            continue;
        }
        if let Some(minimum) = query.minimum_score {
            if !artifact.evidence.is_hole() && artifact.score.is_none_or(|score| score < minimum) {
                continue;
            }
        }
        if let Some(search) = &search {
            let haystack = lower(&format!(
                "{} {} {} {} {}",
                artifact.id,
                artifact.title,
                artifact.domain,
                artifact.capability,
                artifact.tags.join(" ")
            ));
            if !haystack.contains(search) {
                continue;
            }
        }
        let stale = session.cells.iter().any(|cell| {
            cell.inputs
                .iter()
                .any(|input| input.artifact_id == artifact.id)
                && session.cell_is_stale(cell)
        });
        matched.push(DashboardRow {
            artifact: artifact.id.clone(),
            title: artifact.title.clone(),
            domain: artifact.domain.clone(),
            capability: artifact.capability.clone(),
            state: artifact.state,
            evidence: artifact.evidence,
            score: if artifact.evidence.is_hole() {
                None
            } else {
                artifact.score
            },
            stale,
            posture: dashboard_posture(artifact, stale).into(),
            tags: artifact.tags.clone(),
        });
    }
    matched.sort_by(|left, right| {
        left.domain
            .cmp(&right.domain)
            .then_with(|| left.artifact.cmp(&right.artifact))
    });
    let matched_count = matched.len();
    let holes = matched.iter().filter(|row| row.evidence.is_hole()).count();
    let stale = matched.iter().filter(|row| row.stale).count();
    let rows = matched.into_iter().take(query.limit).collect::<Vec<_>>();
    Ok(DashboardReport {
        query: query.clone(),
        matched: matched_count,
        returned: rows.len(),
        omitted: matched_count.saturating_sub(rows.len()),
        holes,
        stale,
        rows,
    })
}

fn yaml_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Generate a deterministic, reviewable GitHub Actions workflow plan.
pub fn plan_ci(request: &CiRequest) -> Result<CiPlan, WorkbenchError> {
    request.validate()?;
    let mut triggers = request.triggers.clone();
    triggers.sort();
    let mut yaml = String::new();
    yaml.push_str(&format!("name: {}\n\n", yaml_quote(&request.workflow)));
    yaml.push_str("on:\n");
    for trigger in &triggers {
        yaml.push_str(&format!("  {trigger}:\n"));
    }
    yaml.push_str("\njobs:\n  workbench-contracts:\n    runs-on: ubuntu-latest\n    steps:\n");
    yaml.push_str("      - uses: actions/checkout@v4\n");
    yaml.push_str("      - uses: dtolnay/rust-toolchain@stable\n");
    yaml.push_str(&format!(
        "        with:\n          toolchain: {}\n",
        yaml_quote(&request.rust_toolchain)
    ));
    for check in &request.checks {
        yaml.push_str(&format!("      - name: {}\n", yaml_quote(&check.name)));
        if let Some(directory) = &check.working_directory {
            yaml.push_str(&format!(
                "        working-directory: {}\n",
                yaml_quote(directory)
            ));
        }
        if request.offline {
            yaml.push_str("        env:\n          CARGO_NET_OFFLINE: 'true'\n");
        }
        if !check.required {
            yaml.push_str("        continue-on-error: true\n");
        }
        yaml.push_str("        run: |\n");
        yaml.push_str(&format!("          {}\n", check.run));
        yaml.push_str(&format!(
            "          # posture: {}\n",
            if check.required {
                "required"
            } else {
                "advisory"
            }
        ));
    }
    let digest = ContentHash::of_bytes(yaml.as_bytes()).to_string();
    Ok(CiPlan {
        workflow: request.workflow.clone(),
        workflow_yaml: yaml,
        digest,
        check_count: request.checks.len(),
        required_check_count: request.checks.iter().filter(|check| check.required).count(),
        execution: "not_executed".into(),
        network_access: if request.offline {
            "denied_by_plan".into()
        } else {
            "available_to_runner".into()
        },
        limitations: vec![
            "the YAML is generated for review and is not written to a repository or submitted to GitHub".into(),
            "check commands and their scientific meaning remain caller-supplied".into(),
            "a rendered workflow is not a passing run, release approval, or evidence of biological validity".into(),
        ],
    })
}

/// Compose session audit, optional dashboard projection, and optional CI planning.
pub fn run_workbench(request: &WorkbenchRequest) -> Result<WorkbenchReport, WorkbenchError> {
    let audit = audit_session(&request.session)?;
    let dashboard = request
        .dashboard
        .as_ref()
        .map(|query| query_dashboard(&request.session, query))
        .transpose()?;
    let ci = request.ci.as_ref().map(plan_ci).transpose()?;
    Ok(WorkbenchReport {
        schema_version: WORKBENCH_SCHEMA_VERSION.into(),
        audit,
        dashboard,
        ci,
        guarantees: vec![
            "session, notebook, dashboard, and CI objects are validated before projection".into(),
            "notebook dependencies are topologically ordered and cycles refuse".into(),
            "artifact digests remain explicit; stale cell inputs become blocking findings".into(),
            "dashboard holes remain visible and never receive manufactured scores".into(),
            "generated CI YAML is content-addressed and explicitly marked not executed".into(),
        ],
        limitations: vec![
            "the workbench does not execute notebook cells, run CI, call GitHub, or write files".into(),
            "domain semantics, evidence quality, and command correctness remain delegated to their authoritative contracts".into(),
            "a release-ready posture is a structural gate, not scientific, clinical, security, or production approval".into(),
        ],
    })
}

/// Verify a retained workbench report against the current session and optional CI request.
///
/// This is intentionally a replay/audit operation. It re-runs only deterministic in-process
/// validation and projection functions; it never executes notebook cells, writes generated YAML,
/// contacts GitHub, or treats a matching report as release, scientific, clinical, safety, or
/// production authority.
pub fn verify_workbench(
    request: &WorkbenchVerificationRequest,
) -> Result<WorkbenchVerificationReport, WorkbenchError> {
    request.session.validate()?;
    if let Some(digest) = &request.expected_report_digest {
        validate_digest("expected_report_digest", digest)?;
    }

    let retained_report_digest = value_digest(&request.report)?;
    let retained_audit_digest = value_digest(&request.report.audit)?;
    let dashboard_query = request
        .report
        .dashboard
        .as_ref()
        .map(|dashboard| dashboard.query.clone());
    let observed = run_workbench(&WorkbenchRequest {
        session: request.session.clone(),
        dashboard: dashboard_query,
        ci: request.ci_replay.clone(),
    })?;
    let observed_audit_digest = value_digest(&observed.audit)?;
    let mut mismatches = Vec::new();

    if request.report.schema_version != WORKBENCH_SCHEMA_VERSION {
        mismatches.push(WorkbenchMismatch {
            code: "schema_mismatch".into(),
            path: "/report/schema_version".into(),
            expected: Some(Value::String(WORKBENCH_SCHEMA_VERSION.into())),
            observed: Some(Value::String(request.report.schema_version.clone())),
            detail: "retained report uses a different workbench schema version".into(),
        });
    }

    let report_digest_matched = request.expected_report_digest.as_ref().map(|expected| {
        let matched = expected == &retained_report_digest;
        if !matched {
            mismatches.push(WorkbenchMismatch {
                code: "report_digest_mismatch".into(),
                path: "/report".into(),
                expected: Some(Value::String(expected.clone())),
                observed: Some(Value::String(retained_report_digest.clone())),
                detail: "retained report bytes do not match the caller-supplied report digest"
                    .into(),
            });
        }
        matched
    });

    if retained_audit_digest != observed_audit_digest {
        mismatches.push(WorkbenchMismatch {
            code: "audit_mismatch".into(),
            path: "/report/audit".into(),
            expected: Some(Value::String(observed_audit_digest.clone())),
            observed: Some(Value::String(retained_audit_digest.clone())),
            detail: "the retained session audit differs from the current deterministic audit"
                .into(),
        });
    }

    let dashboard_present = request.report.dashboard.is_some();
    let dashboard_verified = if let Some(retained_dashboard) = &request.report.dashboard {
        match &observed.dashboard {
            None => {
                mismatches.push(WorkbenchMismatch {
                    code: "dashboard_missing_from_observed_projection".into(),
                    path: "/report/dashboard".into(),
                    expected: Some(Value::String("present".into())),
                    observed: Some(Value::String("missing".into())),
                    detail: "the retained dashboard could not be regenerated".into(),
                });
                false
            }
            Some(observed_dashboard) => {
                let retained_digest = value_digest(retained_dashboard)?;
                let observed_digest = value_digest(observed_dashboard)?;
                if retained_digest != observed_digest {
                    mismatches.push(WorkbenchMismatch {
                        code: "dashboard_mismatch".into(),
                        path: "/report/dashboard".into(),
                        expected: Some(Value::String(observed_digest)),
                        observed: Some(Value::String(retained_digest)),
                        detail:
                            "the retained dashboard differs from the current session projection"
                                .into(),
                    });
                    false
                } else {
                    true
                }
            }
        }
    } else {
        if request.policy.require_dashboard {
            mismatches.push(WorkbenchMismatch {
                code: "dashboard_required".into(),
                path: "/report/dashboard".into(),
                expected: Some(Value::String("present".into())),
                observed: Some(Value::String("missing".into())),
                detail: "verification policy requires a retained dashboard projection".into(),
            });
        }
        false
    };

    let ci_present = request.report.ci.is_some();
    let ci_replay_supplied = request.ci_replay.is_some();
    let ci_verified = if let Some(retained_ci) = &request.report.ci {
        if let Some(observed_ci) = &observed.ci {
            let retained_digest = value_digest(retained_ci)?;
            let observed_digest = value_digest(observed_ci)?;
            if retained_digest != observed_digest {
                mismatches.push(WorkbenchMismatch {
                    code: "ci_plan_mismatch".into(),
                    path: "/report/ci".into(),
                    expected: Some(Value::String(observed_digest)),
                    observed: Some(Value::String(retained_digest)),
                    detail: "the retained CI plan differs from the regenerated plan".into(),
                });
                false
            } else {
                true
            }
        } else if ci_replay_supplied {
            mismatches.push(WorkbenchMismatch {
                code: "ci_replay_missing_plan".into(),
                path: "/report/ci".into(),
                expected: Some(Value::String("present".into())),
                observed: Some(Value::String("missing".into())),
                detail: "a CI replay request was supplied but the regenerated plan is absent"
                    .into(),
            });
            false
        } else {
            false
        }
    } else if request.policy.require_ci {
        mismatches.push(WorkbenchMismatch {
            code: "ci_required".into(),
            path: "/report/ci".into(),
            expected: Some(Value::String("present".into())),
            observed: Some(Value::String("missing".into())),
            detail: "verification policy requires a retained CI plan".into(),
        });
        false
    } else {
        false
    };

    if ci_replay_supplied && !ci_present {
        mismatches.push(WorkbenchMismatch {
            code: "ci_replay_without_retained_plan".into(),
            path: "/report/ci".into(),
            expected: Some(Value::String("present".into())),
            observed: Some(Value::String("missing".into())),
            detail: "a CI replay request cannot verify a report that retained no CI plan".into(),
        });
    }

    if request.policy.require_ci_replay && ci_present && !ci_replay_supplied {
        mismatches.push(WorkbenchMismatch {
            code: "ci_replay_required".into(),
            path: "/ci_replay".into(),
            expected: Some(Value::String("present".into())),
            observed: Some(Value::String("missing".into())),
            detail: "verification policy requires the original CiRequest for plan replay".into(),
        });
    }

    finish_workbench_verification(
        request,
        WorkbenchVerificationFacts {
            retained_report_digest,
            report_digest_matched,
            retained_audit_digest,
            observed_audit_digest,
            dashboard_present,
            dashboard_verified,
            ci_present,
            ci_replay_supplied,
            ci_verified,
            mismatches,
        },
    )
}

struct WorkbenchVerificationFacts {
    retained_report_digest: String,
    report_digest_matched: Option<bool>,
    retained_audit_digest: String,
    observed_audit_digest: String,
    dashboard_present: bool,
    dashboard_verified: bool,
    ci_present: bool,
    ci_replay_supplied: bool,
    ci_verified: bool,
    mismatches: Vec<WorkbenchMismatch>,
}

fn finish_workbench_verification(
    request: &WorkbenchVerificationRequest,
    facts: WorkbenchVerificationFacts,
) -> Result<WorkbenchVerificationReport, WorkbenchError> {
    let WorkbenchVerificationFacts {
        retained_report_digest,
        report_digest_matched,
        retained_audit_digest,
        observed_audit_digest,
        dashboard_present,
        dashboard_verified,
        ci_present,
        ci_replay_supplied,
        ci_verified,
        mismatches,
    } = facts;
    let valid = mismatches.is_empty();
    let status = if !valid {
        "mismatch"
    } else if ci_present && !ci_replay_supplied {
        "verified_without_replay"
    } else {
        "verified"
    };
    let mut report = WorkbenchVerificationReport {
        schema_version: WORKBENCH_VERIFY_SCHEMA_VERSION.into(),
        workflow: "developer_workbench_verify".into(),
        valid,
        status: status.into(),
        retained_report_digest,
        expected_report_digest: request.expected_report_digest.clone(),
        report_digest_matched,
        retained_audit_digest,
        observed_audit_digest,
        dashboard_present,
        dashboard_verified,
        ci_present,
        ci_replay_supplied,
        ci_verified,
        mismatches,
        execution: "not_started".into(),
        network_access: "not_started".into(),
        verification_digest: String::new(),
        guarantees: vec![
            "retained session, dashboard, and optional CI plan are recomputed through deterministic in-process contracts".into(),
            "digest and structural mismatches remain visible with expected/observed witnesses".into(),
            "verification never executes notebook cells, writes YAML, contacts GitHub, or grants authority".into(),
        ],
        limitations: vec![
            "a CI plan cannot be replayed without the original caller-owned CiRequest".into(),
            "matching structure is not provider authentication, execution evidence, scientific validity, clinical safety, or release approval".into(),
        ],
    };
    report.verification_digest = value_digest(&report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn hash(label: &str) -> String {
        ContentHash::of_bytes(label.as_bytes()).to_string()
    }

    fn session() -> StudioSession {
        StudioSession {
            session_id: "session-1".into(),
            owner: "agent-a".into(),
            goal: "author a verified oncology capability card".into(),
            environment_digest: Some(hash("env")),
            artifacts: vec![ArtifactCard {
                id: "artifact-1".into(),
                title: "verification card".into(),
                path: "artifacts/verification.json".into(),
                domain: "oncology".into(),
                capability: "verification".into(),
                state: ArtifactState::Validated,
                evidence: EvidencePosture::Reproduced,
                digest: Some(hash("artifact")),
                score: Some(0.8),
                tags: vec!["public-card".into()],
            }],
            cells: vec![StudioCell {
                id: "cell-1".into(),
                kind: CellKind::Query,
                source: "workspace.metrics_analytics_audit(...)".into(),
                inputs: vec![CellInput {
                    artifact_id: "artifact-1".into(),
                    digest: hash("artifact"),
                }],
                depends_on: Vec::new(),
                executed: true,
                output_digest: Some(hash("output")),
            }],
            changes: vec![StudioChange {
                id: "change-1".into(),
                artifact_id: "artifact-1".into(),
                kind: ChangeKind::Create,
                actor: "agent-a".into(),
                logical_time: 1,
                input_digest: None,
                output_digest: Some(hash("artifact")),
                reason: "initial authored artifact".into(),
            }],
            policy: NotebookPolicy::default(),
        }
    }

    #[test]
    fn audit_orders_cells_and_accepts_digest_bound_reproduction() {
        let report = audit_session(&session()).unwrap();
        assert!(report.valid);
        assert_eq!(report.ordered_cells, vec!["cell-1"]);
        assert!(report.stale_cells.is_empty());
        assert_eq!(report.executed_cell_count, 1);
        assert!(!report.release_ready);
    }

    #[test]
    fn changed_artifact_digest_blocks_release_and_marks_cell_stale() {
        let mut value = session();
        value.artifacts[0].digest = Some(hash("changed"));
        let report = audit_session(&value).unwrap();
        assert_eq!(report.stale_cells, vec!["cell-1"]);
        assert!(!report.release_blockers.is_empty());
    }

    #[test]
    fn release_requires_an_executed_notebook_witness() {
        let mut value = session();
        value.artifacts[0].state = ArtifactState::Released;
        value.artifacts[0].evidence = EvidencePosture::Reproduced;
        value.cells[0].executed = false;
        let report = audit_session(&value).unwrap();
        assert!(!report.release_ready);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "released_artifact_without_notebook_witness"));
    }

    #[test]
    fn dashboard_keeps_holes_scoreless_and_filters_by_domain() {
        let mut value = session();
        value.artifacts.push(ArtifactCard {
            id: "artifact-2".into(),
            title: "missing cross-modal card".into(),
            path: "artifacts/modal.json".into(),
            domain: "oncology".into(),
            capability: "cross_modal".into(),
            state: ArtifactState::Draft,
            evidence: EvidencePosture::Missing,
            digest: None,
            score: Some(0.1),
            tags: Vec::new(),
        });
        let report = query_dashboard(
            &value,
            &DashboardQuery {
                domains: vec!["oncology".into()],
                minimum_score: Some(0.9),
                ..DashboardQuery::default()
            },
        )
        .unwrap();
        let hole = report
            .rows
            .iter()
            .find(|row| row.artifact == "artifact-2")
            .unwrap();
        assert_eq!(hole.score, None);
        assert_eq!(hole.posture, "blocked");
        assert_eq!(report.holes, 1);
    }

    #[test]
    fn notebook_cycles_and_invalid_releases_refuse() {
        let mut value = session();
        value.cells.push(StudioCell {
            id: "cell-2".into(),
            kind: CellKind::Review,
            source: "review".into(),
            inputs: Vec::new(),
            depends_on: vec!["cell-1".into()],
            executed: true,
            output_digest: Some(hash("review")),
        });
        value.cells[0].depends_on = vec!["cell-2".into()];
        assert!(matches!(
            value.validate(),
            Err(WorkbenchError::DependencyCycle { .. })
        ));
        value = session();
        value.artifacts[0].state = ArtifactState::Released;
        value.artifacts[0].evidence = EvidencePosture::Declared;
        assert!(matches!(
            value.validate(),
            Err(WorkbenchError::ReleaseWithoutEvidence { .. })
        ));
    }

    #[test]
    fn metadata_identifiers_reject_controls_but_code_cells_remain_multiline() {
        let mut value = session();
        value.artifacts[0].id = "artifact\tunsafe".into();
        assert!(matches!(
            value.validate(),
            Err(WorkbenchError::ControlCharacter {
                field: "artifact.id"
            })
        ));

        let mut value = session();
        value.cells[0].source = "line one\nline two".into();
        assert!(value.validate().is_ok());
    }

    #[test]
    fn workbench_rejects_padded_metadata_case_aliases_and_uppercase_digests() {
        let mut value = session();
        value.artifacts[0].title = " artifact".into();
        assert!(matches!(
            value.validate(),
            Err(WorkbenchError::EmptyField {
                field: "artifact.title"
            })
        ));

        let mut value = session();
        value.artifacts.push(ArtifactCard {
            id: "ARTIFACT-1".into(),
            ..value.artifacts[0].clone()
        });
        assert!(matches!(
            value.validate(),
            Err(WorkbenchError::Duplicate {
                kind: "artifact",
                ..
            })
        ));

        let mut value = session();
        value.artifacts[0].digest = Some(hash("artifact").to_uppercase());
        assert!(matches!(
            value.validate(),
            Err(WorkbenchError::InvalidDigest {
                field: "artifact.digest",
                ..
            })
        ));
    }

    #[test]
    fn ci_plan_is_deterministic_and_explicitly_not_executed() {
        let mut request = CiRequest {
            workflow: "consumer contracts".into(),
            triggers: vec!["push".into(), "pull_request".into()],
            rust_toolchain: "1.85.0".into(),
            checks: vec![
                CiCheck {
                    name: "workspace tests".into(),
                    run: "cargo test --workspace --offline".into(),
                    working_directory: Some("crates/devplat".into()),
                    required: true,
                },
                CiCheck {
                    name: "advisory lint".into(),
                    run: "cargo clippy -p bioprism-devplat".into(),
                    working_directory: None,
                    required: false,
                },
            ],
            offline: true,
        };
        let plan = plan_ci(&request).unwrap();
        request.triggers.reverse();
        let reordered_triggers = plan_ci(&request).unwrap();
        assert_eq!(plan.digest, reordered_triggers.digest);
        assert_eq!(plan.workflow_yaml, reordered_triggers.workflow_yaml);
        assert_eq!(plan.execution, "not_executed");
        assert_eq!(plan.network_access, "denied_by_plan");
        assert!(plan
            .workflow_yaml
            .contains("cargo test --workspace --offline"));
        assert!(plan
            .workflow_yaml
            .contains("working-directory: 'crates/devplat'"));
        assert!(plan.workflow_yaml.contains("continue-on-error: true"));
        assert_eq!(plan.check_count, 2);
        assert_eq!(plan.required_check_count, 1);
        assert_eq!(plan.digest.len(), 64);
    }

    #[test]
    fn ci_rejects_absolute_paths_and_duplicate_triggers() {
        let mut request = CiRequest {
            workflow: "contracts".into(),
            triggers: vec!["push".into(), "push".into()],
            rust_toolchain: "stable".into(),
            checks: vec![CiCheck {
                name: "tests".into(),
                run: "cargo test".into(),
                working_directory: Some("/outside".into()),
                required: true,
            }],
            offline: false,
        };
        assert!(matches!(
            request.validate(),
            Err(WorkbenchError::Duplicate {
                kind: "ci trigger",
                ..
            })
        ));
        request.triggers = vec!["push".into()];
        assert!(matches!(
            request.validate(),
            Err(WorkbenchError::UnsafePath { .. })
        ));

        request.checks.push(CiCheck {
            name: "TESTS".into(),
            run: "cargo test".into(),
            working_directory: None,
            required: true,
        });
        request.triggers = vec!["push".into()];
        request.checks[0].name = "tests".into();
        request.checks[0].working_directory = None;
        assert!(matches!(
            request.validate(),
            Err(WorkbenchError::Duplicate {
                kind: "ci check",
                ..
            })
        ));
    }

    #[test]
    fn composed_report_serializes_as_a_single_agent_facing_contract() {
        let report = run_workbench(&WorkbenchRequest {
            session: session(),
            dashboard: Some(DashboardQuery::default()),
            ci: None,
        })
        .unwrap();
        let wire: Value = serde_json::to_value(report).unwrap();
        assert_eq!(wire["schema_version"], WORKBENCH_SCHEMA_VERSION);
        assert_eq!(wire["audit"]["artifact_count"], 1);
        assert!(wire["dashboard"]["rows"].is_array());
    }

    #[test]
    fn workbench_verification_replays_dashboard_and_ci_without_execution() {
        let ci = CiRequest {
            workflow: "consumer contracts".into(),
            triggers: vec!["pull_request".into()],
            rust_toolchain: "stable".into(),
            checks: vec![CiCheck {
                name: "unit".into(),
                run: "cargo test -p bioprism-devplat".into(),
                working_directory: None,
                required: true,
            }],
            offline: true,
        };
        let retained = run_workbench(&WorkbenchRequest {
            session: session(),
            dashboard: Some(DashboardQuery::default()),
            ci: Some(ci.clone()),
        })
        .unwrap();
        let verified = verify_workbench(&WorkbenchVerificationRequest {
            session: session(),
            expected_report_digest: Some(value_digest(&retained).unwrap()),
            report: retained.clone(),
            ci_replay: Some(ci.clone()),
            policy: WorkbenchVerificationPolicy {
                require_dashboard: true,
                require_ci: true,
                require_ci_replay: true,
            },
        })
        .unwrap();
        assert!(verified.valid);
        assert_eq!(verified.status, "verified");
        assert!(verified.dashboard_verified);
        assert!(verified.ci_verified);
        assert_eq!(verified.execution, "not_started");
        assert_eq!(verified.network_access, "not_started");
        assert_eq!(verified.report_digest_matched, Some(true));

        let without_replay = verify_workbench(&WorkbenchVerificationRequest {
            session: session(),
            report: retained.clone(),
            expected_report_digest: None,
            ci_replay: None,
            policy: WorkbenchVerificationPolicy::default(),
        })
        .unwrap();
        assert!(without_replay.valid);
        assert_eq!(without_replay.status, "verified_without_replay");
        assert!(!without_replay.ci_verified);
    }

    #[test]
    fn workbench_verification_retains_audit_and_replay_mismatches() {
        let ci = CiRequest {
            workflow: "consumer contracts".into(),
            triggers: vec!["pull_request".into()],
            rust_toolchain: "stable".into(),
            checks: vec![CiCheck {
                name: "unit".into(),
                run: "cargo test -p bioprism-devplat".into(),
                working_directory: None,
                required: true,
            }],
            offline: true,
        };
        let retained = run_workbench(&WorkbenchRequest {
            session: session(),
            dashboard: None,
            ci: Some(ci.clone()),
        })
        .unwrap();
        let mut tampered = retained;
        tampered.audit.ordered_cells.clear();
        let result = verify_workbench(&WorkbenchVerificationRequest {
            session: session(),
            report: tampered,
            expected_report_digest: None,
            ci_replay: Some(CiRequest {
                workflow: "changed workflow".into(),
                ..ci
            }),
            policy: WorkbenchVerificationPolicy::default(),
        })
        .unwrap();
        assert!(!result.valid);
        assert_eq!(result.status, "mismatch");
        assert!(result
            .mismatches
            .iter()
            .any(|mismatch| mismatch.code == "audit_mismatch"));
        assert!(result
            .mismatches
            .iter()
            .any(|mismatch| mismatch.code == "ci_plan_mismatch"));
    }
}
